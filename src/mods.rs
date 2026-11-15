//! A profile's mods: the jars in its `mods` folder (read from each jar's
//! `fabric.mod.json`), what Modrinth knows about them (found by file hash), and
//! searching and installing mods from Modrinth (with their required
//! dependencies). Network work runs on one shared background runtime.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::download::{fetch_verified, sha1_hex};
use crate::meta::write_atomic;

const API: &str = "https://api.modrinth.com/v2";
/// Mods shown per page of search results.
pub const PAGE_SIZE: usize = 20;
/// Fabric API: every profile gets it (owner request). It can't be removed or
/// turned off, and browsing doesn't offer it.
pub const FABRIC_API_ID: &str = "fabric-api";
pub const FABRIC_API_PROJECT: &str = "P7dR8mSH";
/// The most results Modrinth returns for one search.
const SEARCH_MAX: usize = 100;
/// Icons are kept this size (pixels), enough for the largest place they're drawn.
const ICON_PIXELS: u32 = 96;
/// Larger icon downloads are refused.
const ICON_MAX_BYTES: usize = 4 * 1024 * 1024;
/// Suffix Fabric ignores: how a mod is turned off without removing it.
const DISABLED: &str = ".disabled";

/// The background runtime all mod network work runs on.
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("starting the mod download runtime")
    })
}

pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("hermes-launcher/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("building the HTTP client")
    })
}

/// A jar in a profile's `mods` folder.
pub struct InstalledMod {
    /// File name in the `mods` folder (with `.disabled` while turned off).
    pub file: String,
    pub enabled: bool,
    pub sha1: String,
    /// The Fabric mod id, from `fabric.mod.json`.
    pub id: Option<String>,
    /// From `fabric.mod.json`; the file name if the jar doesn't say.
    pub name: String,
    pub authors: Vec<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    /// The jar's own icon, if it has one.
    pub icon: Option<image::RgbaImage>,
}

impl InstalledMod {
    /// A copy without the icon image (icons become textures once, separately).
    pub fn clone_without_icon(&self) -> Self {
        InstalledMod {
            file: self.file.clone(),
            enabled: self.enabled,
            sha1: self.sha1.clone(),
            id: self.id.clone(),
            name: self.name.clone(),
            authors: self.authors.clone(),
            version: self.version.clone(),
            description: self.description.clone(),
            icon: None,
        }
    }
}

/// A mod on Modrinth.
#[derive(Clone)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub author: Option<String>,
    pub downloads: u64,
    pub description: String,
    pub icon_url: Option<String>,
}

impl Project {
    pub fn page_url(&self) -> String {
        format!("https://modrinth.com/mod/{}", self.slug)
    }
}

pub fn mods_dir(game_dir: &Path) -> PathBuf {
    game_dir.join("mods")
}

/// Reads every mod jar in `<game_dir>/mods`, sorted by name. A missing folder
/// means no mods.
pub fn scan(game_dir: &Path) -> Result<Vec<InstalledMod>> {
    let dir = mods_dir(game_dir);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", dir.display())),
    };
    let mut mods = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Some(file) = entry.file_name().to_str().map(str::to_string) else { continue };
        let enabled = file.to_ascii_lowercase().ends_with(".jar");
        if !enabled && !file.to_ascii_lowercase().ends_with(&format!(".jar{DISABLED}")) {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else { continue };
        mods.push(read_jar(file, enabled, &bytes));
    }
    mods.sort_by_key(|m| m.name.to_lowercase());
    Ok(mods)
}

#[derive(Deserialize)]
struct FabricModJson {
    #[serde(default)]
    depends: Depends,
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    #[serde(default)]
    authors: Vec<Person>,
    icon: Option<IconPath>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Person {
    Name(String),
    Object { name: String },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum IconPath {
    One(String),
    /// Paths by pixel size.
    Sizes(BTreeMap<String, String>),
}

impl IconPath {
    /// The smallest icon at least `ICON_PIXELS` wide, else the largest.
    fn best(self) -> Option<String> {
        match self {
            IconPath::One(path) => Some(path),
            IconPath::Sizes(sizes) => {
                let mut sized: Vec<(u32, String)> = sizes.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect();
                sized.sort_by_key(|(size, _)| *size);
                let pick = sized.iter().position(|(size, _)| *size >= ICON_PIXELS).unwrap_or(sized.len().checked_sub(1)?);
                Some(sized.swap_remove(pick).1)
            }
        }
    }
}

fn read_jar(file: String, enabled: bool, bytes: &[u8]) -> InstalledMod {
    let base = file.trim_end_matches(DISABLED);
    let fallback = base.strip_suffix(".jar").or_else(|| base.strip_suffix(".JAR")).unwrap_or(base).to_string();
    let mut found = InstalledMod {
        sha1: sha1_hex(bytes),
        id: None,
        file,
        enabled,
        name: fallback,
        authors: Vec::new(),
        version: None,
        description: None,
        icon: None,
    };
    let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else { return found };
    let Some(info) = mod_json(&mut zip) else { return found };
    if let Some(name) = info.name.filter(|n| !n.trim().is_empty()) {
        found.name = name;
    }
    found.id = info.id;
    found.version = info.version;
    found.description = info.description.filter(|d| !d.trim().is_empty());
    found.authors = info.authors.into_iter().map(|p| match p {
        Person::Name(name) | Person::Object { name } => name,
    }).collect();
    found.icon = info
        .icon
        .and_then(IconPath::best)
        .and_then(|path| read_entry(&mut zip, path.trim_start_matches('/'), ICON_MAX_BYTES))
        .and_then(|png| image::load_from_memory(&png).ok())
        .map(thumbnail);
    found
}

/// The jar's `fabric.mod.json`, if it has a readable one.
fn mod_json(zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>) -> Option<FabricModJson> {
    let json = read_entry(zip, "fabric.mod.json", 1024 * 1024)?;
    // Some mods write raw line breaks inside strings, which strict JSON forbids.
    let json = String::from_utf8_lossy(&json).replace(['\n', '\r', '\t'], " ");
    serde_json::from_str(&json).ok()
}

fn read_entry(zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>, name: &str, limit: usize) -> Option<Vec<u8>> {
    let entry = zip.by_name(name).ok()?;
    let mut bytes = Vec::new();
    entry.take(limit as u64 + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= limit).then_some(bytes)
}

fn thumbnail(image: image::DynamicImage) -> image::RgbaImage {
    image.resize(ICON_PIXELS, ICON_PIXELS, image::imageops::FilterType::Triangle).to_rgba8()
}

/// Turns a mod on or off (Fabric skips `*.jar.disabled`). Returns its new file name.
pub fn set_enabled(game_dir: &Path, file: &str, enabled: bool) -> Result<String> {
    let dir = mods_dir(game_dir);
    let base = file.strip_suffix(DISABLED).unwrap_or(file);
    let new = if enabled { base.to_string() } else { format!("{base}{DISABLED}") };
    if new != file {
        std::fs::rename(dir.join(file), dir.join(&new)).with_context(|| format!("renaming {file}"))?;
    }
    Ok(new)
}

/// Removes a mod's jar from the profile.
pub fn remove(game_dir: &Path, file: &str) -> Result<()> {
    let path = mods_dir(game_dir).join(file);
    std::fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))
}

#[derive(Deserialize)]
struct Version {
    project_id: String,
    files: Vec<VersionFile>,
    #[serde(default)]
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct VersionFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    size: u64,
    hashes: Hashes,
}

#[derive(Deserialize)]
struct Hashes {
    sha1: String,
}

#[derive(Deserialize)]
struct Dependency {
    project_id: Option<String>,
    dependency_type: String,
}

#[derive(Deserialize)]
struct SearchResults {
    hits: Vec<SearchHit>,
    #[serde(default)]
    total_hits: usize,
}

/// One page of search results.
#[derive(Clone)]
pub struct SearchPage {
    pub projects: Vec<Project>,
    /// Matches across all pages.
    pub total: usize,
}

#[derive(Deserialize)]
struct SearchHit {
    project_id: String,
    slug: String,
    title: String,
    author: Option<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    downloads: u64,
    icon_url: Option<String>,
}

impl From<SearchHit> for Project {
    fn from(h: SearchHit) -> Self {
        Project {
            id: h.project_id,
            slug: h.slug,
            title: h.title,
            author: h.author,
            downloads: h.downloads,
            description: h.description,
            icon_url: h.icon_url,
        }
    }
}

async fn get_json<T: serde::de::DeserializeOwned>(request: reqwest::RequestBuilder) -> Result<T> {
    let response = request.send().await?.error_for_status()?;
    Ok(response.json().await?)
}

/// Finds which Modrinth projects the jars with these sha1 hashes belong to.
/// Jars Modrinth doesn't know (or that aren't listed in its search) are left out.
pub async fn identify(sha1s: Vec<String>) -> Result<HashMap<String, Project>> {
    if sha1s.is_empty() {
        return Ok(HashMap::new());
    }
    let body = serde_json::json!({ "hashes": sha1s, "algorithm": "sha1" });
    let versions: HashMap<String, Version> = get_json(client().post(format!("{API}/version_files")).json(&body)).await?;
    let ids: Vec<String> = versions.values().map(|v| v.project_id.clone()).collect::<HashSet<_>>().into_iter().collect();
    // Looked up through search, so each mod shows the same details (its owner
    // as author) as when browsing.
    let mut projects = HashMap::new();
    for chunk in ids.chunks(SEARCH_MAX) {
        let any_of: Vec<String> = chunk.iter().map(|id| format!("project_id:{id}")).collect();
        let facets = serde_json::json!([any_of]).to_string();
        let limit = SEARCH_MAX.to_string();
        let request = client().get(format!("{API}/search")).query(&[("facets", &facets), ("limit", &limit)]);
        let results: SearchResults = get_json(request).await?;
        projects.extend(results.hits.into_iter().map(|h| (h.project_id.clone(), Project::from(h))));
    }
    Ok(versions.into_iter().filter_map(|(sha1, v)| Some((sha1, projects.get(&v.project_id)?.clone()))).collect())
}

/// Page `page` (from 0) of the Fabric mods for Minecraft `mc` matching `query`
/// (the most downloaded when empty), without Fabric API (always installed).
pub async fn search(query: &str, mc: &str, page: usize) -> Result<SearchPage> {
    // Pages are kept for a while, so every profile page (built in the
    // background) shares the same results instead of asking again.
    let key = (query.trim().to_string(), mc.to_string(), page);
    if let Some((at, found)) = search_cache().lock().unwrap().get(&key) {
        if at.elapsed() < CACHE_TIME {
            return Ok(found.clone());
        }
    }
    let found = search_uncached(query, mc, page).await?;
    search_cache().lock().unwrap().insert(key, (std::time::Instant::now(), found.clone()));
    Ok(found)
}

/// How long search results are reused.
const CACHE_TIME: std::time::Duration = std::time::Duration::from_secs(10 * 60);

type SearchCache = std::sync::Mutex<HashMap<(String, String, usize), (std::time::Instant, SearchPage)>>;

fn search_cache() -> &'static SearchCache {
    static CACHE: OnceLock<SearchCache> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

async fn search_uncached(query: &str, mc: &str, page: usize) -> Result<SearchPage> {
    let facets = serde_json::json!([
        ["project_type:mod"],
        ["categories:fabric"],
        [format!("versions:{mc}")],
        [format!("project_id!={FABRIC_API_PROJECT}")]
    ])
    .to_string();
    let index = if query.trim().is_empty() { "downloads" } else { "relevance" };
    let (limit, offset) = (PAGE_SIZE.to_string(), (page * PAGE_SIZE).to_string());
    let request = client()
        .get(format!("{API}/search"))
        .query(&[("query", query.trim()), ("facets", &facets), ("index", index), ("limit", &limit), ("offset", &offset)]);
    let results: SearchResults = get_json(request).await?;
    // Search matches loader and Minecraft version per project, not per version:
    // a mod with Fabric builds for older Minecraft and a 26.1.2 build for another
    // loader would match. Each result is checked for a Fabric build for `mc`.
    let checks = results.hits.iter().map(|h| compatible_versions(&h.project_id, mc));
    let compatible = futures::future::join_all(checks).await;
    let projects = results
        .hits
        .into_iter()
        .zip(compatible)
        // A failed check keeps the result; installing checks again.
        .filter(|(_, versions)| versions.as_ref().map_or(true, |v| !v.is_empty()))
        .map(|(hit, _)| Project::from(hit))
        .collect();
    Ok(SearchPage { projects, total: results.total_hits })
}

/// Makes sure the profile has Fabric API, turned on: installs it if it's
/// missing. Returns whether anything changed.
pub async fn ensure_fabric_api(game_dir: &Path, target: &Target) -> Result<bool> {
    let dir = game_dir.to_path_buf();
    let found = tokio::task::spawn_blocking(move || find_mod(&dir, FABRIC_API_ID)).await?;
    match found {
        Some((_, true)) => Ok(false),
        Some((file, false)) => set_enabled(game_dir, &file, true).map(|_| true),
        None => install(game_dir, FABRIC_API_PROJECT, target, &HashSet::new()).await.map(|()| true),
    }
}

/// The jar (file name, turned on) whose `fabric.mod.json` has mod id `id`.
fn find_mod(game_dir: &Path, id: &str) -> Option<(String, bool)> {
    std::fs::read_dir(mods_dir(game_dir)).ok()?.flatten().find_map(|entry| {
        let file = entry.file_name().to_str()?.to_string();
        let lower = file.to_ascii_lowercase();
        let enabled = lower.ends_with(".jar");
        if !enabled && !lower.ends_with(&format!(".jar{DISABLED}")) {
            return None;
        }
        let bytes = std::fs::read(entry.path()).ok()?;
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.as_slice())).ok()?;
        let info = mod_json(&mut zip)?;
        (info.id.as_deref() == Some(id)).then_some((file, enabled))
    })
}

/// What a mod has to work with: the profile's Minecraft version and the Fabric
/// Loader the launcher starts it with.
#[derive(Clone, Debug)]
pub struct Target {
    pub minecraft: String,
    pub loader: String,
}

impl Target {
    /// From a Fabric launch profile id (`fabric-loader-<loader>-<minecraft>`).
    pub fn from_profile_id(id: &str, minecraft: &str) -> Option<Target> {
        let loader = id.strip_prefix("fabric-loader-")?.strip_suffix(&format!("-{minecraft}"))?;
        Some(Target { minecraft: minecraft.to_string(), loader: loader.to_string() })
    }
}

/// The target for Minecraft `mc`: the Fabric Loader launches use (resolved
/// once and cached with the game files).
pub async fn target(mc: &str) -> Result<Target> {
    let paths = crate::launch::Paths::new()?;
    let profile = crate::meta::fabric_profile(client(), &paths.versions_dir(), mc).await?;
    Target::from_profile_id(&profile.id, mc).with_context(|| format!("unexpected Fabric profile {}", profile.id))
}

/// How many of a project's newest versions are tried before giving up.
const VERSIONS_TRIED: usize = 6;

/// Installs the newest Fabric version of `project` that works with `target`
/// into `<game_dir>/mods`, plus every required dependency not in `installed`
/// (project ids already in the folder). Modrinth lists versions by Minecraft
/// version and loader; the jar itself says which Minecraft and Fabric Loader
/// versions it needs (`depends` in `fabric.mod.json`, which Fabric enforces), so
/// a version whose jar asks for more than `target` gives way to an older one.
pub async fn install(game_dir: &Path, project: &str, target: &Target, installed: &HashSet<String>) -> Result<()> {
    let dir = mods_dir(game_dir);
    tokio::fs::create_dir_all(&dir).await.with_context(|| format!("creating {}", dir.display()))?;
    let mut seen = installed.clone();
    let mut queue = vec![project.to_string()];
    while let Some(id) = queue.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let found = compatible_versions(&id, &target.minecraft).await.with_context(|| format!("looking up {id}"))?;
        if found.is_empty() {
            bail!("{id} has no Fabric version for Minecraft {}", target.minecraft);
        }
        let mut problem = None;
        let mut chosen = None;
        // Newest first.
        for version in found.into_iter().take(VERSIONS_TRIED) {
            let Some(file) = version.files.iter().find(|f| f.primary).or(version.files.first()) else { continue };
            // The name comes from the server: only a plain jar name may be written.
            if !valid_jar_name(&file.filename) {
                bail!("{id}: unexpected file name {:?}", file.filename);
            }
            let path = dir.join(&file.filename);
            if path.exists() {
                chosen = Some(version);
                break;
            }
            let bytes = fetch_verified(client(), &file.url, Some(&file.hashes.sha1), Some(file.size)).await?;
            match unmet_requirement(&bytes, target) {
                Some(why) => problem = Some(why),
                None => {
                    write_atomic(&path, &bytes).await?;
                    chosen = Some(version);
                    break;
                }
            }
        }
        let Some(version) = chosen else {
            bail!(
                "{id} has no version that works with Minecraft {} and Fabric Loader {}{}",
                target.minecraft,
                target.loader,
                problem.map(|p| format!(" ({p})")).unwrap_or_default()
            );
        };
        queue.extend(version.dependencies.into_iter().filter(|d| d.dependency_type == "required").filter_map(|d| d.project_id));
    }
    Ok(())
}

/// A project's Fabric versions for Minecraft `mc`, newest first.
async fn compatible_versions(project: &str, mc: &str) -> Result<Vec<Version>> {
    let loaders = serde_json::json!(["fabric"]).to_string();
    let versions = serde_json::json!([mc]).to_string();
    let request = client()
        .get(format!("{API}/project/{project}/version"))
        .query(&[("loaders", loaders.as_str()), ("game_versions", versions.as_str()), ("include_changelog", "false")]);
    get_json(request).await
}

/// Why a jar won't load with `target`, going by its own `depends`; `None` when
/// it will (or doesn't say).
fn unmet_requirement(jar: &[u8], target: &Target) -> Option<String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(jar)).ok()?;
    let depends = mod_json(&mut zip)?.depends;
    for (what, need, have) in [("Minecraft", &depends.minecraft, &target.minecraft), ("Fabric Loader", &depends.fabricloader, &target.loader)] {
        // An unknown loader version (empty) isn't checked.
        if let Some(need) = need.as_ref().filter(|n| !have.is_empty() && !n.allows(have)) {
            return Some(format!("needs {what} {need}"));
        }
    }
    None
}

/// A version requirement from `fabric.mod.json`: one spec, or several of which
/// any may match.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum Requirement {
    One(String),
    AnyOf(Vec<String>),
}

impl Requirement {
    fn allows(&self, version: &str) -> bool {
        match self {
            Requirement::One(spec) => spec_allows(spec, version),
            Requirement::AnyOf(specs) => specs.iter().any(|spec| spec_allows(spec, version)),
        }
    }
}

impl std::fmt::Display for Requirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Requirement::One(spec) => f.write_str(spec),
            Requirement::AnyOf(specs) => f.write_str(&specs.join(" or ")),
        }
    }
}

/// The `depends` entries the launcher checks.
#[derive(Deserialize, Default)]
struct Depends {
    minecraft: Option<Requirement>,
    fabricloader: Option<Requirement>,
}

/// Fabric's version predicates: space-separated constraints that must all
/// hold, each `*`, `=x`, `>=x`, `>x`, `<=x`, `<x`, `~x` (same minor), `^x`
/// (same major) or a plain version where `x` / `*` parts match anything.
/// Anything unreadable is allowed (Fabric itself decides at launch).
fn spec_allows(spec: &str, version: &str) -> bool {
    spec.split_whitespace().all(|c| constraint_allows(c, version))
}

fn constraint_allows(c: &str, version: &str) -> bool {
    use std::cmp::Ordering::*;
    let (op, want) = [">=", "<=", ">", "<", "=", "~", "^"]
        .iter()
        .find_map(|op| c.strip_prefix(op).map(|rest| (*op, rest)))
        .unwrap_or(("", c));
    if want == "*" || want.is_empty() {
        return true;
    }
    let order = compare_versions(version, want);
    match op {
        ">=" => order != Less,
        ">" => order == Greater,
        "<=" => order != Greater,
        "<" => order == Less,
        "~" | "^" => {
            let parts: Vec<u64> = core(want).split('.').map_while(|p| p.parse().ok()).collect();
            let upper = match (op, parts.as_slice()) {
                ("~", [major, minor, ..]) => format!("{major}.{}", minor + 1),
                ("^", [0, minor, ..]) => format!("0.{}", minor + 1),
                (_, [major, ..]) => format!("{}", major + 1),
                _ => return true,
            };
            order != Less && compare_versions(version, &upper) == Less
        }
        // "=" or a plain version: equal, with x / * parts matching anything.
        _ => {
            let have: Vec<&str> = core(version).split('.').collect();
            let want_parts: Vec<&str> = core(want).split('.').collect();
            if want_parts.iter().any(|p| matches!(*p, "x" | "X" | "*")) {
                want_parts.iter().enumerate().all(|(i, w)| matches!(*w, "x" | "X" | "*") || have.get(i).is_some_and(|h| h == w))
            } else {
                order == Equal
            }
        }
    }
}

/// A version without its build metadata (`+...`).
fn core(v: &str) -> &str {
    v.split('+').next().unwrap_or(v)
}

/// Compares versions part by part (numbers as numbers, missing parts as 0);
/// a pre-release (`-...`) comes before its release.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let split = |v: &str| {
        let v = core(v);
        match v.split_once('-') {
            Some((main, pre)) => (main.to_string(), Some(pre.to_string())),
            None => (v.to_string(), None),
        }
    };
    let ((a_main, a_pre), (b_main, b_pre)) = (split(a), split(b));
    let a_parts: Vec<&str> = a_main.split('.').collect();
    let b_parts: Vec<&str> = b_main.split('.').collect();
    for i in 0..a_parts.len().max(b_parts.len()) {
        let (x, y) = (a_parts.get(i).copied().unwrap_or("0"), b_parts.get(i).copied().unwrap_or("0"));
        let order = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            _ => x.cmp(y),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    match (a_pre, b_pre) {
        (None, None) => Ordering::Equal,
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(x), Some(y)) => x.cmp(&y),
    }
}

fn valid_jar_name(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".jar")
        && !name.contains(['/', '\\', ':'])
        && !name.starts_with('.')
        && name.chars().all(|c| !c.is_control())
}

/// Downloads an icon and shrinks it to `ICON_PIXELS`; `None` if it can't be
/// read. Icons are kept for the session, shared by all profile pages.
pub async fn icon(url: String) -> Option<image::RgbaImage> {
    type Icons = std::sync::Mutex<HashMap<String, Option<image::RgbaImage>>>;
    static ICONS: OnceLock<Icons> = OnceLock::new();
    let icons = ICONS.get_or_init(Default::default);
    if let Some(found) = icons.lock().unwrap().get(&url) {
        return found.clone();
    }
    let found = fetch_icon(&url).await;
    icons.lock().unwrap().insert(url, found.clone());
    found
}

async fn fetch_icon(url: &str) -> Option<image::RgbaImage> {
    let response = client().get(url).send().await.ok()?.error_for_status().ok()?;
    if response.content_length().is_some_and(|n| n as usize > ICON_MAX_BYTES) {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.len() > ICON_MAX_BYTES {
        return None;
    }
    image::load_from_memory(&bytes).ok().map(thumbnail)
}

/// A download count as a short number: 950, 12.4K, 1.6M.
pub fn short_count(n: u64) -> String {
    let (value, suffix) = match n {
        0..1_000 => return n.to_string(),
        1_000..1_000_000 => (n as f64 / 1e3, "K"),
        1_000_000..1_000_000_000 => (n as f64 / 1e6, "M"),
        _ => (n as f64 / 1e9, "B"),
    };
    if value < 100.0 { format!("{:.1}{suffix}", (value * 10.0).floor() / 10.0) } else { format!("{}{suffix}", value.floor()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn jar(json: &str) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut out);
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(json.as_bytes()).unwrap();
        zip.finish().unwrap();
        out.into_inner()
    }

    #[test]
    fn reads_name_authors_and_version_from_the_jar() {
        let bytes = jar(r#"{"id":"x","name":"Sodium","version":"1.0","description":"Fast
rendering","authors":["JellySquid",{"name":"IMS"}]}"#);
        let m = read_jar("sodium.jar".into(), true, &bytes);
        assert_eq!(m.name, "Sodium");
        assert_eq!(m.version.as_deref(), Some("1.0"));
        assert_eq!(m.authors, ["JellySquid", "IMS"]);
        assert_eq!(m.description.as_deref(), Some("Fast rendering"));
    }

    #[test]
    fn jars_without_metadata_use_the_file_name() {
        let m = read_jar("thing-2.jar.disabled".into(), false, b"not a zip");
        assert_eq!(m.name, "thing-2");
        assert!(!m.enabled);
    }

    #[test]
    fn scan_lists_jars_and_turned_off_jars_and_toggling_renames() {
        let root = std::env::temp_dir().join(format!("hermes-mods-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(mods_dir(&root)).unwrap();
        std::fs::write(mods_dir(&root).join("b.jar"), jar(r#"{"name":"Beta"}"#)).unwrap();
        std::fs::write(mods_dir(&root).join("a.jar.disabled"), jar(r#"{"name":"Alpha"}"#)).unwrap();
        std::fs::write(mods_dir(&root).join("notes.txt"), b"x").unwrap();
        let mods = scan(&root).unwrap();
        assert_eq!(mods.iter().map(|m| (m.name.as_str(), m.enabled)).collect::<Vec<_>>(), [("Alpha", false), ("Beta", true)]);
        assert_eq!(set_enabled(&root, "a.jar.disabled", true).unwrap(), "a.jar");
        assert!(mods_dir(&root).join("a.jar").exists());
        assert_eq!(set_enabled(&root, "b.jar", false).unwrap(), "b.jar.disabled");
        remove(&root, "b.jar.disabled").unwrap();
        assert_eq!(scan(&root).unwrap().len(), 1);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn server_file_names_must_be_plain_jars() {
        assert!(valid_jar_name("fabric-api-0.155.3+26.1.2.jar"));
        assert!(!valid_jar_name("../evil.jar"));
        assert!(!valid_jar_name("C:evil.jar"));
        assert!(!valid_jar_name("readme.txt"));
    }

    /// Talks to Modrinth: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn installs_a_mod_with_its_required_dependencies() {
        let root = std::env::temp_dir().join(format!("hermes-install-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // Mod Menu requires Fabric API (and more).
        runtime().block_on(install(&root, "modmenu", &target(), &HashSet::new())).unwrap();
        let mods = scan(&root).unwrap();
        let names: Vec<&str> = mods.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"Mod Menu") && names.contains(&"Fabric API"), "{names:?}");
        let found = runtime().block_on(identify(mods.iter().map(|m| m.sha1.clone()).collect())).unwrap();
        assert_eq!(found.len(), mods.len());
        // Already there: nothing to do; turned off: turned back on.
        assert!(!runtime().block_on(ensure_fabric_api(&root, &target())).unwrap());
        let api = mods.iter().find(|m| m.id.as_deref() == Some(FABRIC_API_ID)).unwrap();
        set_enabled(&root, &api.file, false).unwrap();
        assert!(runtime().block_on(ensure_fabric_api(&root, &target())).unwrap());
        assert!(scan(&root).unwrap().iter().all(|m| m.enabled));
        let page = runtime().block_on(search("", crate::launch::MC_VERSION, 1)).unwrap();
        assert_eq!(page.projects.len(), PAGE_SIZE);
        assert!(page.total > PAGE_SIZE && page.projects.iter().all(|p| p.id != FABRIC_API_PROJECT));
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn target() -> Target {
        Target { minecraft: crate::launch::MC_VERSION.into(), loader: "0.20.1".into() }
    }

    #[test]
    fn fabric_version_predicates() {
        let ok = |spec: &str, v: &str| spec_allows(spec, v);
        assert!(ok(">=0.16.0", "0.20.1"));
        assert!(!ok(">=0.21.0", "0.20.1"));
        assert!(ok("~26.1", "26.1.2"));
        assert!(!ok("~26.1", "26.2"));
        assert!(ok("26.1.x", "26.1.2"));
        assert!(!ok("26.1.1", "26.1.2"));
        assert!(ok(">=26.1 <26.2-", "26.1.2"));
        assert!(!ok(">=26.1 <26.2-", "26.2"));
        assert!(ok("^0.20.0", "0.20.1") && !ok("^0.20.0", "0.21.0"));
        assert!(ok("*", "1.0") && ok("26.1.2+build.3", "26.1.2"));
        assert!(Requirement::AnyOf(vec!["26.1.1".into(), "26.1.2".into()]).allows("26.1.2"));
    }

    #[test]
    fn jars_asking_for_a_newer_loader_or_other_minecraft_are_refused() {
        let t = target();
        assert_eq!(unmet_requirement(&jar(r#"{"depends":{"fabricloader":">=0.16","minecraft":"~26.1"}}"#), &t), None);
        assert!(unmet_requirement(&jar(r#"{"depends":{"fabricloader":">=0.99.0"}}"#), &t).unwrap().contains("Fabric Loader"));
        assert!(unmet_requirement(&jar(r#"{"depends":{"minecraft":"1.21.4"}}"#), &t).unwrap().contains("Minecraft"));
        assert_eq!(Target::from_profile_id("fabric-loader-0.20.1-26.1.2", "26.1.2").unwrap().loader, "0.20.1");
    }

    #[test]
    fn counts_are_shortened() {
        assert_eq!(short_count(950), "950");
        assert_eq!(short_count(12_480), "12.4K");
        assert_eq!(short_count(1_587_207), "1.5M");
        assert_eq!(short_count(250_000), "250K");
    }
}
