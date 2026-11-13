//! Mojang/Fabric metadata types, cached fetching, and profile merging.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::download::fetch_verified;

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    pub main_class: String,
    #[serde(default)]
    pub arguments: Arguments,
    #[serde(default)]
    pub libraries: Vec<Library>,
    pub asset_index: Option<AssetIndexRef>,
    pub downloads: Option<VersionDownloads>,
    pub java_version: Option<JavaVersion>,
    pub logging: Option<Logging>,
    #[serde(rename = "type", default)]
    pub kind: String,
}

#[derive(Deserialize, Clone, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
    #[serde(default, rename = "default-user-jvm")]
    pub default_user_jvm: Vec<Argument>,
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Ruled {
        #[serde(default)]
        rules: Vec<Rule>,
        value: ArgValue,
    },
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize, Clone)]
pub struct Rule {
    pub action: RuleAction,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version_range: Option<VersionRange>,
}

#[derive(Deserialize, Clone)]
pub struct VersionRange {
    pub min: Option<String>,
    pub max: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    /// Maven repository base (Fabric style), used when `downloads` is absent.
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Deserialize, Clone)]
pub struct LibraryDownloads {
    pub artifact: Option<Download>,
}

#[derive(Deserialize, Clone)]
pub struct Download {
    pub path: Option<String>,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Deserialize, Clone)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub url: String,
}

#[derive(Deserialize, Clone)]
pub struct VersionDownloads {
    pub client: Download,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
}

#[derive(Deserialize, Clone)]
pub struct Logging {
    pub client: Option<LoggingClient>,
}

#[derive(Deserialize, Clone)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LogFile,
}

#[derive(Deserialize, Clone)]
pub struct LogFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

#[derive(Deserialize)]
pub struct RuntimeManifest {
    pub files: HashMap<String, RuntimeFile>,
}

#[derive(Deserialize)]
pub struct RuntimeFile {
    #[serde(rename = "type")]
    pub kind: String,
    pub downloads: Option<RuntimeDownloads>,
}

#[derive(Deserialize)]
pub struct RuntimeDownloads {
    pub raw: Download,
}

#[derive(Deserialize)]
struct VersionManifest {
    versions: Vec<ManifestEntry>,
}

#[derive(Deserialize)]
struct ManifestEntry {
    id: String,
    url: String,
    sha1: String,
}

#[derive(Deserialize)]
struct FabricLoaderEntry {
    loader: FabricLoader,
}

#[derive(Deserialize)]
struct FabricLoader {
    version: String,
    stable: bool,
}

type RuntimeIndex = HashMap<String, HashMap<String, Vec<RuntimeEntry>>>;

#[derive(Deserialize)]
struct RuntimeEntry {
    manifest: RuntimeManifestRef,
}

#[derive(Deserialize)]
struct RuntimeManifestRef {
    sha1: String,
    url: String,
}

const VERSION_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const FABRIC_META: &str = "https://meta.fabricmc.net/v2/versions/loader";
const RUNTIME_INDEX: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const RUNTIME_PLATFORM: &str = "windows-x64";

/// Reads `path` if present; otherwise downloads `url`, verifies it, and caches it.
/// Only content addressed by sha1, or pinned by a cached parent, goes through here,
/// so a warm start never touches the network.
async fn cached<T: DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    sha1: Option<&str>,
    path: &Path,
) -> Result<T> {
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(_) => {
            let bytes = fetch_verified(client, url, sha1, None).await?;
            write_atomic(path, &bytes).await?;
            bytes
        }
    };
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
}

async fn fetch_json<T: DeserializeOwned>(client: &reqwest::Client, url: &str) -> Result<T> {
    let bytes = fetch_verified(client, url, None, None).await?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {url}"))
}

pub async fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let tmp = path.with_extension("part");
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, path)
        .await
        .with_context(|| format!("writing {}", path.display()))
}

pub async fn vanilla_version(client: &reqwest::Client, versions_dir: &Path, id: &str) -> Result<VersionJson> {
    let path = versions_dir.join(id).join(format!("{id}.json"));
    if let Ok(bytes) = tokio::fs::read(&path).await {
        return serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()));
    }
    let manifest: VersionManifest = fetch_json(client, VERSION_MANIFEST).await?;
    let entry = manifest
        .versions
        .into_iter()
        .find(|v| v.id == id)
        .ok_or_else(|| anyhow!("Minecraft {id} is not in Mojang's version manifest"))?;
    cached(client, &entry.url, Some(&entry.sha1), &path).await
}

/// Resolves the newest stable Fabric loader once, then reuses the cached profile.
pub async fn fabric_profile(client: &reqwest::Client, versions_dir: &Path, mc: &str) -> Result<VersionJson> {
    let path = versions_dir.join(format!("fabric-{mc}")).join("profile.json");
    if let Ok(bytes) = tokio::fs::read(&path).await {
        return serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()));
    }
    let loaders: Vec<FabricLoaderEntry> = fetch_json(client, &format!("{FABRIC_META}/{mc}")).await?;
    let loader = loaders
        .iter()
        .find(|l| l.loader.stable)
        .or(loaders.first())
        .ok_or_else(|| anyhow!("Fabric has no loader for Minecraft {mc}"))?;
    let url = format!("{FABRIC_META}/{mc}/{}/profile/json", loader.loader.version);
    cached(client, &url, None, &path).await
}

pub async fn asset_index(client: &reqwest::Client, assets_dir: &Path, r: &AssetIndexRef) -> Result<AssetIndex> {
    let path = assets_dir.join("indexes").join(format!("{}.json", r.id));
    cached(client, &r.url, Some(&r.sha1), &path).await
}

pub async fn runtime_manifest(client: &reqwest::Client, runtime_dir: &Path, component: &str) -> Result<RuntimeManifest> {
    let path = runtime_dir.join(format!("{component}.manifest.json"));
    if let Ok(bytes) = tokio::fs::read(&path).await {
        return serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()));
    }
    let index: RuntimeIndex = fetch_json(client, RUNTIME_INDEX).await?;
    let entry = index
        .get(RUNTIME_PLATFORM)
        .and_then(|p| p.get(component))
        .and_then(|v| v.first())
        .ok_or_else(|| anyhow!("Mojang has no Java runtime '{component}' for {RUNTIME_PLATFORM}"))?;
    cached(client, &entry.manifest.url, Some(&entry.manifest.sha1), &path).await
}

/// `group:artifact[:classifier]`, i.e. the library identity without its version.
fn library_key(name: &str) -> String {
    let name = name.split('@').next().unwrap_or(name);
    let parts: Vec<&str> = name.split(':').collect();
    match parts.as_slice() {
        [g, a, _v, c, ..] => format!("{g}:{a}:{c}"),
        [g, a, ..] => format!("{g}:{a}"),
        _ => name.to_string(),
    }
}

/// Converts `group:artifact:version[:classifier][@ext]` to a Maven repository path.
pub fn maven_path(name: &str) -> Result<String> {
    let (coords, ext) = name.split_once('@').unwrap_or((name, "jar"));
    let parts: Vec<&str> = coords.split(':').collect();
    let (group, artifact, version, classifier) = match parts.as_slice() {
        [g, a, v] => (*g, *a, *v, None),
        [g, a, v, c] => (*g, *a, *v, Some(*c)),
        _ => bail!("bad Maven coordinate '{name}'"),
    };
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Ok(format!("{}/{artifact}/{version}/{file}", group.replace('.', "/")))
}

/// Applies a Fabric profile on top of the vanilla version it inherits from.
/// Child libraries win over parent libraries with the same identity.
pub fn merge(child: VersionJson, parent: VersionJson) -> VersionJson {
    let child_keys: HashSet<String> = child.libraries.iter().map(|l| library_key(&l.name)).collect();
    let mut libraries = child.libraries;
    libraries.extend(parent.libraries.into_iter().filter(|l| !child_keys.contains(&library_key(&l.name))));

    let mut arguments = parent.arguments;
    arguments.jvm.extend(child.arguments.jvm);
    arguments.game.extend(child.arguments.game);

    VersionJson {
        id: child.id,
        main_class: child.main_class,
        arguments,
        libraries,
        asset_index: child.asset_index.or(parent.asset_index),
        downloads: child.downloads.or(parent.downloads),
        java_version: child.java_version.or(parent.java_version),
        logging: child.logging.or(parent.logging),
        kind: parent.kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(json: &str) -> VersionJson {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.20.1").unwrap(),
            "net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar"
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.4.1:natives-windows").unwrap(),
            "org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar"
        );
        assert_eq!(maven_path("a.b:c:1@zip").unwrap(), "a/b/c/1/c-1.zip");
        assert!(maven_path("broken").is_err());
    }

    #[test]
    fn merge_prefers_child_libraries_and_main_class() {
        let parent = version(
            r#"{"id":"26.1.2","mainClass":"net.minecraft.client.main.Main","type":"release",
                "arguments":{"game":["--a"],"jvm":["-Dp"]},
                "libraries":[{"name":"org.ow2.asm:asm:9.0"},{"name":"org.lwjgl:lwjgl:3.4.1"},
                             {"name":"org.lwjgl:lwjgl:3.4.1:natives-windows"}]}"#,
        );
        let child = version(
            r#"{"id":"fabric-loader-0.20.1-26.1.2","mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
                "arguments":{"game":[],"jvm":["-Dc"]},
                "libraries":[{"name":"org.ow2.asm:asm:9.10.1","url":"https://maven.fabricmc.net/"}]}"#,
        );
        let merged = merge(child, parent);
        assert_eq!(merged.main_class, "net.fabricmc.loader.impl.launch.knot.KnotClient");
        assert_eq!(merged.kind, "release");
        let names: Vec<&str> = merged.libraries.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["org.ow2.asm:asm:9.10.1", "org.lwjgl:lwjgl:3.4.1", "org.lwjgl:lwjgl:3.4.1:natives-windows"]);
        assert_eq!(merged.arguments.jvm.len(), 2);
    }

    #[test]
    fn parses_real_vanilla_argument_shapes() {
        let v = version(
            r#"{"id":"x","mainClass":"M","arguments":{
                "default-user-jvm":[{"value":["-Xmx4G"]}],
                "game":["--username","${auth_player_name}",
                        {"rules":[{"action":"allow","features":{"is_demo_user":true}}],"value":"--demo"}],
                "jvm":[{"rules":[{"action":"allow","os":{"name":"windows","versionRange":{"min":"10.0.17134"}}}],
                        "value":["-XX:+UseZGC"]}]}}"#,
        );
        assert_eq!(v.arguments.default_user_jvm.len(), 1);
        assert_eq!(v.arguments.game.len(), 3);
    }
}
