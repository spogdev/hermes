//! Installs Fabric Minecraft into the Hermes data folder and starts it.
//!
//! Warm-start path: all metadata is read from disk (no network), every file is
//! checked by size on all cores, then the JVM is spawned using a base CDS
//! archive of the JDK's own classes.

use std::collections::HashMap;
use std::fs::File;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::download::{FileTask, ensure_all};
use crate::meta::{self, AssetIndex, Library, RuntimeManifest, VersionJson};
use crate::rules::{self, Env};

pub const MC_VERSION: &str = "26.1.2";
const PLAYER_NAME: &str = "Hermes";
/// Java's `UUID.nameUUIDFromBytes("OfflinePlayer:Hermes")`, the id an offline server assigns.
const PLAYER_UUID: &str = "b48f8beb-14f2-3d85-8bb8-f41038d06b08";
const ASSET_BASE: &str = "https://resources.download.minecraft.net";
const MOJANG_LIBRARIES: &str = "https://libraries.minecraft.net/";

/// Everything lives under `%APPDATA%\.hermes`, separate from the official launcher.
pub struct Paths {
    pub root: PathBuf,
    libraries: PathBuf,
    assets: PathBuf,
    versions: PathBuf,
    runtime: PathBuf,
    instance: PathBuf,
    natives: PathBuf,
    logs: PathBuf,
}

impl Paths {
    pub fn new() -> Result<Self> {
        let appdata = std::env::var_os("APPDATA").context("APPDATA is not set")?;
        let root = PathBuf::from(appdata).join(".hermes");
        std::fs::create_dir_all(&root).with_context(|| format!("creating {}", root.display()))?;
        let root = real_path(&root)?;
        Ok(Paths {
            libraries: root.join("libraries"),
            assets: root.join("assets"),
            versions: root.join("versions"),
            runtime: root.join("runtime"),
            instance: root.join("instance"),
            natives: root.join("natives").join(MC_VERSION),
            logs: root.join("logs"),
            root,
        })
    }

    pub fn game_output_log(&self) -> PathBuf {
        self.logs.join("game-output.log")
    }
}

/// Resolves junctions, symlinks and per-app folder redirection (e.g. MSIX
/// packages redirect `%APPDATA%`). Fabric compares classpath entries with the
/// paths Java reports for loaded jars; if they differ it loads itself twice and
/// crashes, so the classpath must use the same real paths Java sees.
fn real_path(path: &Path) -> Result<PathBuf> {
    let real = std::fs::canonicalize(path).with_context(|| format!("resolving {}", path.display()))?;
    // canonicalize returns `\\?\C:\...`; Java's classpath expects a plain `C:\...`.
    Ok(match real.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(plain) if !plain.starts_with("UNC\\") => PathBuf::from(plain),
        _ => real,
    })
}

pub struct Launched {
    pub child: Child,
    pub elapsed: Duration,
    pub downloaded: usize,
}

pub fn launch(status: &(dyn Fn(String) + Sync)) -> Result<Launched> {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on(launch_async(status))
}

async fn launch_async(status: &(dyn Fn(String) + Sync)) -> Result<Launched> {
    let start = Instant::now();
    let paths = Paths::new()?;
    let client = reqwest::Client::builder()
        .user_agent(concat!("hermes-launcher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .build()?;

    status("Reading game metadata".into());
    let (vanilla, fabric) = futures::try_join!(
        meta::vanilla_version(&client, &paths.versions, MC_VERSION),
        meta::fabric_profile(&client, &paths.versions, MC_VERSION),
    )?;
    let version = meta::merge(fabric, vanilla);
    let asset_ref = version.asset_index.clone().context("version has no asset index")?;
    let component = version.java_version.as_ref().context("version has no Java runtime")?.component.clone();
    let (assets, runtime) = futures::try_join!(
        meta::asset_index(&client, &paths.assets, &asset_ref),
        meta::runtime_manifest(&client, &paths.runtime, &component),
    )?;

    status("Checking game files".into());
    let env = Env::current();
    let tasks = file_tasks(&paths, &version, &assets, &runtime, &component, &env)?;
    let downloaded = ensure_all(&client, tasks, status).await?;

    let bin = paths.runtime.join(&component).join("bin");
    let args = game_command(&paths, &version, &asset_ref.id, &env)?;
    ensure_base_cds(&paths, &bin, &args, status)?;

    status("Starting Minecraft".into());
    let java = bin.join("javaw.exe");
    let child = spawn(&paths, &java, &args)?;
    Ok(Launched { child, elapsed: start.elapsed(), downloaded })
}

fn client_jar(paths: &Paths) -> PathBuf {
    paths.versions.join(MC_VERSION).join(format!("{MC_VERSION}.jar"))
}

struct LibraryFile {
    path: String,
    url: String,
    sha1: Option<String>,
    size: Option<u64>,
}

fn library_file(lib: &Library) -> Result<LibraryFile> {
    if let Some(a) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
        let path = match &a.path {
            Some(p) => p.clone(),
            None => meta::maven_path(&lib.name)?,
        };
        return Ok(LibraryFile { path, url: a.url.clone(), sha1: Some(a.sha1.clone()), size: Some(a.size) });
    }
    let path = meta::maven_path(&lib.name)?;
    let base = lib.url.as_deref().unwrap_or(MOJANG_LIBRARIES);
    let url = format!("{}/{path}", base.trim_end_matches('/'));
    Ok(LibraryFile { path, url, sha1: lib.sha1.clone(), size: lib.size })
}

fn file_tasks(
    paths: &Paths,
    version: &VersionJson,
    assets: &AssetIndex,
    runtime: &RuntimeManifest,
    component: &str,
    env: &Env,
) -> Result<Vec<FileTask>> {
    let mut tasks = Vec::new();

    let client = &version.downloads.as_ref().context("version has no client download")?.client;
    tasks.push(FileTask {
        url: client.url.clone(),
        path: client_jar(paths),
        sha1: Some(client.sha1.clone()),
        size: Some(client.size),
    });

    for lib in version.libraries.iter().filter(|l| rules::library_allowed(l, env)) {
        let f = library_file(lib)?;
        tasks.push(FileTask { url: f.url, path: paths.libraries.join(&f.path), sha1: f.sha1, size: f.size });
    }

    if let Some(log) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        tasks.push(FileTask {
            url: log.file.url.clone(),
            path: paths.assets.join("log_configs").join(&log.file.id),
            sha1: Some(log.file.sha1.clone()),
            size: Some(log.file.size),
        });
    }

    for object in assets.objects.values() {
        let prefix = &object.hash[..2];
        tasks.push(FileTask {
            url: format!("{ASSET_BASE}/{prefix}/{}", object.hash),
            path: paths.assets.join("objects").join(prefix).join(&object.hash),
            sha1: Some(object.hash.clone()),
            size: Some(object.size),
        });
    }

    let runtime_root = paths.runtime.join(component);
    for (rel, file) in &runtime.files {
        if let (Some(d), "file") = (&file.downloads, file.kind.as_str()) {
            tasks.push(FileTask {
                url: d.raw.url.clone(),
                path: runtime_root.join(rel),
                sha1: Some(d.raw.sha1.clone()),
                size: Some(d.raw.size),
            });
        }
    }
    Ok(tasks)
}

/// Builds the arguments passed to `javaw.exe`: JVM options, main class, game options.
fn game_command(paths: &Paths, version: &VersionJson, asset_index_id: &str, env: &Env) -> Result<Vec<String>> {
    let mut classpath = Vec::new();
    for lib in version.libraries.iter().filter(|l| rules::library_allowed(l, env)) {
        classpath.push(path_str(&paths.libraries.join(library_file(lib)?.path)));
    }
    classpath.push(path_str(&client_jar(paths)));

    let log_config = version.logging.as_ref().and_then(|l| l.client.as_ref());
    let mut vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", PLAYER_NAME.into()),
        ("auth_uuid", PLAYER_UUID.into()),
        ("auth_access_token", "0".into()),
        ("auth_xuid", "0".into()),
        ("clientid", "0".into()),
        ("user_type", "msa".into()),
        ("version_name", version.id.clone()),
        ("version_type", version.kind.clone()),
        ("game_directory", path_str(&paths.instance)),
        ("assets_root", path_str(&paths.assets)),
        ("assets_index_name", asset_index_id.into()),
        ("natives_directory", path_str(&paths.natives)),
        ("library_directory", path_str(&paths.libraries)),
        ("launcher_name", "hermes".into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()),
        ("classpath_separator", ";".into()),
        ("classpath", classpath.join(";")),
    ]);
    if let Some(log) = log_config {
        vars.insert("path", path_str(&paths.assets.join("log_configs").join(&log.file.id)));
    }

    // No per-game AppCDS archive (AutoCreateSharedArchive): measured on 26.1.2 it
    // saved ~0.5 s per launch (Minecraft's classes load through Fabric's Knot
    // loader, which CDS can't archive) but recording it made one launch ~66 s slower.
    let mut args = rules::expand(&version.arguments.default_user_jvm, env, &vars);
    args.extend(rules::expand(&version.arguments.jvm, env, &vars));
    if let Some(log) = log_config {
        args.push(rules::substitute(&log.argument, &vars));
    }
    args.push(version.main_class.clone());
    args.extend(rules::expand(&version.arguments.game, env, &vars));
    Ok(args)
}

/// The JVM flags the base CDS archive must match: GC, compressed oops and
/// object header layout pick which archive file the JVM opens, and module
/// options must be identical or the JVM disables optimized module handling.
fn cds_layout_flags(args: &[String]) -> Vec<&str> {
    args.iter()
        .map(String::as_str)
        .filter(|a| a.starts_with("-XX:+Use") || a.starts_with("-XX:-Use") || a.starts_with("--enable-native-access"))
        .collect()
}

/// Mojang's Java runtime ships without a base CDS archive, so every JDK class
/// would be parsed and verified on each launch. Generate the archive once with
/// the game's own layout flags; a marker records which flags it matches.
fn ensure_base_cds(paths: &Paths, java_bin: &Path, game_args: &[String], status: &(dyn Fn(String) + Sync)) -> Result<()> {
    let flags = cds_layout_flags(game_args).join(" ");
    let marker = java_bin.join("server").join("hermes-base-cds.flags");
    if std::fs::read_to_string(&marker).is_ok_and(|m| m == flags) {
        return Ok(());
    }
    status("Preparing Java class cache (first launch only)".into());
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = Command::new(java_bin.join("java.exe"))
        .args(cds_layout_flags(game_args))
        .arg("-Xshare:dump")
        .current_dir(&paths.root)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .context("running java -Xshare:dump")?;
    // A failed dump only costs speed: the game still starts without the archive.
    if out.status.success() {
        std::fs::write(&marker, flags)?;
    } else {
        status(format!("Java class cache skipped: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn spawn(paths: &Paths, java: &Path, args: &[String]) -> Result<Child> {
    for dir in [&paths.instance, &paths.natives, &paths.logs] {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    // javaw has no console; keep its output so startup crashes can be diagnosed.
    let log = File::create(paths.game_output_log())?;
    Command::new(java)
        .args(args)
        .current_dir(&paths.instance)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .with_context(|| format!("starting {}", java.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cds_flags_keep_only_layout_options() {
        let args: Vec<String> = [
            "-Xmx4G",
            "-XX:+UseZGC",
            "-XX:+AlwaysPreTouch",
            "-XX:+UseCompactObjectHeaders",
            "--enable-native-access=ALL-UNNAMED",
            "-cp",
            "x",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(
            cds_layout_flags(&args),
            ["-XX:+UseZGC", "-XX:+UseCompactObjectHeaders", "--enable-native-access=ALL-UNNAMED"]
        );
    }

    #[test]
    fn real_path_strips_verbatim_prefix() {
        let p = real_path(&std::env::temp_dir()).unwrap();
        assert!(!p.to_string_lossy().starts_with(r"\?\"));
        assert!(p.is_absolute());
    }
}
