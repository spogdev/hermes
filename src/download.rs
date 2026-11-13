//! Verified downloads and the "make sure every file is present" pass.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, bail};
use futures::{StreamExt, TryStreamExt};
use sha1::{Digest, Sha1};

use crate::meta::write_atomic;

const PARALLEL_DOWNLOADS: usize = 48;
const ATTEMPTS: usize = 3;

pub struct FileTask {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Downloads `url`, checking size and sha1 when known. Retries transient failures.
pub async fn fetch_verified(
    client: &reqwest::Client,
    url: &str,
    sha1: Option<&str>,
    size: Option<u64>,
) -> Result<Vec<u8>> {
    let mut last_err = None;
    for _ in 0..ATTEMPTS {
        match fetch_once(client, url, sha1, size).await {
            Ok(bytes) => return Ok(bytes),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap())
}

async fn fetch_once(client: &reqwest::Client, url: &str, sha1: Option<&str>, size: Option<u64>) -> Result<Vec<u8>> {
    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = response.bytes().await.with_context(|| format!("reading {url}"))?;
    if let Some(size) = size {
        if bytes.len() as u64 != size {
            bail!("{url}: expected {size} bytes, got {}", bytes.len());
        }
    }
    if let Some(sha1) = sha1 {
        let actual = sha1_hex(&bytes);
        if !actual.eq_ignore_ascii_case(sha1) {
            bail!("{url}: sha1 mismatch (expected {sha1}, got {actual})");
        }
    }
    Ok(bytes.to_vec())
}

/// A file counts as present when it exists with the expected size. Content was
/// hash-checked when it was downloaded, so warm starts skip re-hashing gigabytes.
fn is_present(task: &FileTask) -> bool {
    match std::fs::metadata(&task.path) {
        Ok(meta) => meta.is_file() && task.size.is_none_or(|s| s == meta.len()),
        Err(_) => false,
    }
}

/// Stats every file across all cores and returns the ones that must be downloaded.
fn missing(tasks: Vec<FileTask>) -> Vec<FileTask> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = tasks.len().div_ceil(threads).max(1);
    let mut chunks: Vec<Vec<FileTask>> = Vec::new();
    let mut iter = tasks.into_iter().peekable();
    while iter.peek().is_some() {
        chunks.push(iter.by_ref().take(chunk).collect());
    }
    std::thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|c| scope.spawn(move || c.into_iter().filter(|t| !is_present(t)).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

/// Ensures every task's file exists, downloading the missing ones in parallel.
/// Returns how many files were downloaded.
pub async fn ensure_all(
    client: &reqwest::Client,
    tasks: Vec<FileTask>,
    status: &(dyn Fn(String) + Sync),
) -> Result<usize> {
    // Distinct asset names can share one content hash, i.e. one path.
    let mut seen = HashSet::new();
    let tasks: Vec<FileTask> = tasks.into_iter().filter(|t| seen.insert(t.path.clone())).collect();

    let todo = tokio::task::spawn_blocking(move || missing(tasks)).await?;
    let total = todo.len();
    if total == 0 {
        return Ok(0);
    }
    let done = AtomicUsize::new(0);
    status(format!("Downloading 0/{total} files"));
    futures::stream::iter(todo)
        .map(|task| {
            let done = &done;
            async move {
                let bytes = fetch_verified(client, &task.url, task.sha1.as_deref(), task.size).await?;
                write_atomic(&task.path, &bytes).await?;
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                if n % 25 == 0 || n == total {
                    status(format!("Downloading {n}/{total} files"));
                }
                anyhow::Ok(())
            }
        })
        .buffer_unordered(PARALLEL_DOWNLOADS)
        .try_collect::<()>()
        .await?;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_matches_known_value() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn presence_checks_size() {
        let dir = std::env::temp_dir().join(format!("hermes-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("f.bin");
        std::fs::write(&path, b"1234").unwrap();
        let task = |size| FileTask { url: String::new(), path: path.clone(), sha1: None, size };
        assert!(is_present(&task(Some(4))));
        assert!(is_present(&task(None)));
        assert!(!is_present(&task(Some(5))));
        let gone = FileTask { url: String::new(), path: dir.join("nope"), sha1: None, size: None };
        assert_eq!(missing(vec![task(Some(4)), gone]).len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
