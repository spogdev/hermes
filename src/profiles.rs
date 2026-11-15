//! Launch profiles. Each profile is a folder `<data>/profiles/<name>/` that is
//! also its Minecraft game folder (mods, config, saves, options). The folder is
//! named after the profile's name when it is created (`Unnamed` without one, and
//! `Name (1)`, `Name (2)`, ... when taken). Inside it, `profile.json` holds the
//! profile's details (name, icon, creation time, pinned), and its icon image sits next to it. The selected profile is
//! remembered in `<data>/launcher.json`.
//!
//! Downloaded game files (libraries, assets, Java) are identical for every
//! profile and stay shared in the data folder.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const PROFILES_DIR: &str = "profiles";
const PROFILE_FILE: &str = "profile.json";
const SETTINGS_FILE: &str = "launcher.json";
/// A build briefly saved the settings under this name by mistake; moved back once.
const MISNAMED_SETTINGS_FILE: &str = "json";
/// Folder name for a profile without a name.
const UNNAMED_FOLDER: &str = "Unnamed";
/// File name of an icon chosen when the profile is created.
const ICON_FILE: &str = "icon.png";
/// Chosen icons are cropped to a square and stored at this size (pixels).
const ICON_PIXELS: u32 = 128;
/// Larger image files are refused rather than decoded.
const ICON_MAX_BYTES: u64 = 20 * 1024 * 1024;

/// Contents of `profile.json`. Add new details here; unknown fields in older
/// or newer files are ignored and missing ones take their defaults.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ProfileInfo {
    /// Owner-chosen name. `None` shows the `profile.unnamed` key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// File name of the icon image inside the profile folder (PNG).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Creation time (Unix milliseconds); orders the profile list.
    #[serde(default)]
    pub created: u64,
    /// Pinned profiles are listed on the home screen.
    #[serde(default, skip_serializing_if = "is_false")]
    pub pinned: bool,
    /// Minecraft version chosen when the profile was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minecraft: Option<String>,
    /// Last launch (Unix milliseconds); 0 = never. Orders the home screen.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub last_played: u64,
    /// Most RAM the game may use (MB, `-Xmx`); `None` = `launch::DEFAULT_MEMORY_MB`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_memory_mb: Option<u32>,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// The profile's folder name; fixed when it is created.
    pub id: String,
    pub info: ProfileInfo,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Profiles {
    pub selected: String,
    pub profiles: Vec<Profile>,
}

/// Contents of `launcher.json`: launcher-wide choices.
#[derive(Serialize, Deserialize, Default)]
struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_profile: Option<String>,
}

fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".part");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp, path).with_context(|| format!("writing {}", path.display()))
}

/// The icon field comes from a user-editable file: only a plain file name inside
/// the profile folder is accepted, never a path.
fn valid_file_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':'])
}

/// A Windows-safe folder name from a profile name: characters Windows forbids
/// become `_`, trailing dots and spaces are dropped (Windows strips them), and
/// reserved device names (`CON`, `COM1.txt`, ...) get a `_` in front.
/// Empty or missing names become `Unnamed`.
fn folder_base(name: Option<&str>) -> String {
    let cleaned: String = name
        .unwrap_or("")
        .chars()
        .map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .collect();
    let mut base = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    if base.is_empty() {
        return UNNAMED_FOLDER.to_string();
    }
    let stem = base.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit());
    if reserved {
        base.insert(0, '_');
    }
    base
}

/// `base`, or `base (n)` with the lowest n ≥ 1 that isn't taken.
/// (Windows folder names are case-insensitive, and so is `taken`.)
fn unique_name(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    (1..).map(|n| format!("{base} ({n})")).find(|name| !taken(name)).unwrap()
}

/// Folder name for a new profile called `name` under `root`.
fn new_folder_name(root: &Path, name: Option<&str>) -> String {
    unique_name(&folder_base(name), |candidate| Profiles::dir(root, candidate).exists())
}

impl Profiles {
    pub fn dir(root: &Path, id: &str) -> PathBuf {
        root.join(PROFILES_DIR).join(id)
    }

    /// Reads every `profiles/<name>/profile.json`, creating a default profile on
    /// first run and converting the older layouts (see `migrate_legacy`).
    pub fn load(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root.join(PROFILES_DIR))?;
        migrate_legacy(root)?;
        let mut profiles = scan(root)?;
        if profiles.is_empty() {
            profiles.push(create(root, ProfileInfo::default(), None)?);
        }
        profiles.sort_by(|a, b| (a.info.created, &a.id).cmp(&(b.info.created, &b.id)));
        let selected = read_settings(root)
            .selected_profile
            .filter(|id| profiles.iter().any(|p| &p.id == id))
            .unwrap_or_else(|| profiles[0].id.clone());
        Ok(Profiles { selected, profiles })
    }

    /// Selects a profile and remembers the choice in `launcher.json`.
    pub fn select(&mut self, root: &Path, id: &str) -> Result<()> {
        self.selected = id.to_string();
        write_settings(root, &Settings { selected_profile: Some(self.selected.clone()) })
    }


    /// Creates a profile (folder named after `name`, see the module docs) with
    /// an optional icon from `icon_from_file`, adds it to the list and returns its id.
    pub fn add(&mut self, root: &Path, name: Option<String>, minecraft: &str, max_memory_mb: u32, icon: Option<&image::RgbaImage>) -> Result<String> {
        let info = ProfileInfo {
            name,
            minecraft: Some(minecraft.to_string()),
            max_memory_mb: Some(max_memory_mb),
            icon: icon.map(|_| ICON_FILE.to_string()),
            ..Default::default()
        };
        let profile = create(root, ProfileInfo { icon: None, ..info }, None)?;
        let id = profile.id.clone();
        self.profiles.push(profile);
        self.profiles.sort_by(|a, b| (a.info.created, &a.id).cmp(&(b.info.created, &b.id)));
        if let Some(icon) = icon {
            self.save_icon(root, &id, icon)?;
        }
        Ok(id)
    }

    /// Changes a profile's details and saves its `profile.json`.
    pub fn update(&mut self, root: &Path, id: &str, change: impl FnOnce(&mut ProfileInfo)) -> Result<()> {
        let profile = self.profiles.iter_mut().find(|p| p.id == id).with_context(|| format!("profile {id} not found"))?;
        let mut info = profile.info.clone();
        change(&mut info);
        write_json(&Self::dir(root, id).join(PROFILE_FILE), &info)?;
        profile.info = info;
        Ok(())
    }

    /// Saves `icon` (from `icon_from_file`) as the profile's icon, `icon.png`.
    pub fn save_icon(&mut self, root: &Path, id: &str, icon: &image::RgbaImage) -> Result<()> {
        let path = Self::dir(root, id).join(ICON_FILE);
        icon.save_with_format(&path, image::ImageFormat::Png).with_context(|| format!("saving {}", path.display()))?;
        self.update(root, id, |info| info.icon = Some(ICON_FILE.to_string()))
    }

    /// Deletes a profile: `discard` disposes of its folder (the app moves it to
    /// the Recycle Bin), then it leaves the list. If it was the remembered
    /// profile, the first one left is remembered instead.
    pub fn remove(&mut self, root: &Path, id: &str, discard: impl FnOnce(&Path) -> Result<()>) -> Result<()> {
        anyhow::ensure!(valid_file_name(id), "profile {id} not found");
        discard(&Self::dir(root, id))?;
        self.profiles.retain(|p| p.id != id);
        if self.selected == id {
            self.selected = self.profiles.first().map(|p| p.id.clone()).unwrap_or_default();
            let selected_profile = Some(self.selected.clone()).filter(|s| !s.is_empty());
            write_settings(root, &Settings { selected_profile })?;
        }
        Ok(())
    }

    /// The home screen's list: pinned profiles first (always), then the other
    /// profiles that have been launched; each group most recently played first.
    pub fn home_order(&self) -> Vec<&Profile> {
        let mut shown: Vec<&Profile> = self.profiles.iter().filter(|p| p.info.pinned || p.info.last_played > 0).collect();
        shown.sort_by(|a, b| (b.info.pinned, b.info.last_played).cmp(&(a.info.pinned, a.info.last_played)));
        shown
    }

    /// Path of the profile's icon, if it names a plain file inside its folder.
    pub fn icon_path(root: &Path, profile: &Profile) -> Option<PathBuf> {
        let icon = profile.info.icon.as_deref().filter(|n| valid_file_name(n))?;
        Some(Self::dir(root, &profile.id).join(icon))
    }
}

/// Reads an image file (PNG or JPEG) for a profile icon: the centre square,
/// scaled to `ICON_PIXELS`.
pub fn icon_from_file(path: &Path) -> Result<image::RgbaImage> {
    let size = std::fs::metadata(path).with_context(|| format!("reading {}", path.display()))?.len();
    anyhow::ensure!(size <= ICON_MAX_BYTES, "{} is too large for an icon", path.display());
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let image = image::load_from_memory(&bytes).with_context(|| format!("{} isn't a PNG or JPEG image", path.display()))?;
    Ok(square_icon(image))
}

fn square_icon(image: image::DynamicImage) -> image::RgbaImage {
    let side = image.width().min(image.height());
    let square = image.crop_imm((image.width() - side) / 2, (image.height() - side) / 2, side, side);
    square.resize_exact(ICON_PIXELS, ICON_PIXELS, image::imageops::FilterType::Lanczos3).to_rgba8()
}

fn read_settings(root: &Path) -> Settings {
    std::fs::read(root.join(SETTINGS_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn write_settings(root: &Path, settings: &Settings) -> Result<()> {
    write_json(&root.join(SETTINGS_FILE), settings)
}

/// Profiles are the folders in `profiles/` that contain a readable `profile.json`.
fn scan(root: &Path) -> Result<Vec<Profile>> {
    let mut profiles = Vec::new();
    for entry in std::fs::read_dir(root.join(PROFILES_DIR))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Some(id) = entry.file_name().to_str().map(str::to_string) else { continue };
        let Ok(bytes) = std::fs::read(entry.path().join(PROFILE_FILE)) else { continue };
        let Ok(info) = serde_json::from_slice::<ProfileInfo>(&bytes) else { continue };
        profiles.push(Profile { id, info });
    }
    Ok(profiles)
}

/// Creates a profile folder named after the profile. `existing` is a game folder to move in.
fn create(root: &Path, mut info: ProfileInfo, existing: Option<&Path>) -> Result<Profile> {
    let id = new_folder_name(root, info.name.as_deref());
    let dir = Profiles::dir(root, &id);
    match existing {
        Some(from) => std::fs::rename(from, &dir).with_context(|| format!("moving {} to {}", from.display(), dir.display()))?,
        None => std::fs::create_dir_all(&dir)?,
    }
    if info.created == 0 {
        info.created = now_millis();
    }
    write_json(&dir.join(PROFILE_FILE), &info)?;
    Ok(Profile { id, info })
}

/// The layout before folders had names: one `profiles.json` listing numbered folders.
#[derive(Deserialize)]
struct LegacyProfiles {
    selected: String,
    profiles: Vec<LegacyProfile>,
}

#[derive(Deserialize)]
struct LegacyProfile {
    id: String,
    #[serde(default)]
    name: Option<String>,
}

/// Converts older layouts in place by renaming (nothing is deleted):
/// - settings saved as `json` by mistake go back to `launcher.json`;
/// - `profiles.json` + `profiles/<n>/` become named profile folders;
/// - the original single `instance/` folder becomes a profile;
/// - profile folders named by UUID are renamed after their profile name.
fn migrate_legacy(root: &Path) -> Result<()> {
    let misnamed = root.join(MISNAMED_SETTINGS_FILE);
    if misnamed.is_file() {
        // Best effort, like the folder renames below: retried next start if it fails.
        let _ = std::fs::rename(&misnamed, root.join(SETTINGS_FILE));
    }

    let list = root.join("profiles.json");
    if let Ok(bytes) = std::fs::read(&list) {
        let legacy: LegacyProfiles = serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", list.display()))?;
        let start = now_millis();
        let mut selected = None;
        for (i, old) in legacy.profiles.iter().enumerate() {
            // Numbered ids were plain names; anything else is left where it is.
            if old.id.is_empty() || !old.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                continue;
            }
            let old_dir = Profiles::dir(root, &old.id);
            let info = ProfileInfo { name: old.name.clone(), created: start + i as u64, ..Default::default() };
            let profile = create(root, info, old_dir.is_dir().then_some(old_dir.as_path()))?;
            if old.id == legacy.selected {
                selected = Some(profile.id);
            }
        }
        write_settings(root, &Settings { selected_profile: selected })?;
        std::fs::remove_file(&list)?;
    }

    let instance = root.join("instance");
    if instance.is_dir() && scan(root)?.is_empty() {
        create(root, ProfileInfo::default(), Some(&instance))?;
    }

    let mut by_uuid: Vec<Profile> = scan(root)?.into_iter().filter(|p| Uuid::parse_str(&p.id).is_ok()).collect();
    by_uuid.sort_by(|a, b| (a.info.created, &a.id).cmp(&(b.info.created, &b.id)));
    let mut settings = read_settings(root);
    for profile in by_uuid {
        let name = new_folder_name(root, profile.info.name.as_deref());
        let (from, to) = (Profiles::dir(root, &profile.id), Profiles::dir(root, &name));
        // A running game holds files open in its profile folder, which blocks the
        // rename; that profile keeps its old folder name and is retried next start.
        if std::fs::rename(&from, &to).is_err() {
            continue;
        }
        if settings.selected_profile.as_deref() == Some(profile.id.as_str()) {
            settings.selected_profile = Some(name);
            write_settings(root, &settings)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("hermes-profiles-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn named(name: &str) -> ProfileInfo {
        ProfileInfo { name: Some(name.into()), ..Default::default() }
    }

    #[test]
    fn first_run_creates_an_unnamed_profile_folder() {
        let root = temp_root("fresh");
        let p = Profiles::load(&root).unwrap();
        assert_eq!(p.profiles.len(), 1);
        assert_eq!(p.profiles[0].id, "Unnamed");
        assert!(Profiles::dir(&root, "Unnamed").join(PROFILE_FILE).is_file());
        assert_eq!(p.selected, "Unnamed");
        assert_eq!(Profiles::load(&root).unwrap(), p);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn folders_take_the_lowest_free_number() {
        let root = temp_root("numbering");
        std::fs::create_dir_all(root.join(PROFILES_DIR)).unwrap();
        let ids: Vec<String> = (0..3).map(|_| create(&root, ProfileInfo::default(), None).unwrap().id).collect();
        assert_eq!(ids, ["Unnamed", "Unnamed (1)", "Unnamed (2)"]);
        // A gap is filled first.
        std::fs::remove_dir_all(Profiles::dir(&root, "Unnamed (1)")).unwrap();
        assert_eq!(create(&root, ProfileInfo::default(), None).unwrap().id, "Unnamed (1)");
        // Names work the same way, case-insensitively like Windows.
        assert_eq!(create(&root, named("PvP"), None).unwrap().id, "PvP");
        assert_eq!(create(&root, named("pvp"), None).unwrap().id, "pvp (1)");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn names_become_safe_folder_names() {
        assert_eq!(folder_base(None), "Unnamed");
        assert_eq!(folder_base(Some("  ")), "Unnamed");
        assert_eq!(folder_base(Some("PvP: 1.21/test?")), "PvP_ 1.21_test_");
        assert_eq!(folder_base(Some("trailing dots...")), "trailing dots");
        assert_eq!(folder_base(Some("con")), "_con");
        assert_eq!(folder_base(Some("COM1.txt")), "_COM1.txt");
        assert_eq!(folder_base(Some("Console")), "Console");
        assert_eq!(unique_name("A", |n| n == "A" || n == "A (1)"), "A (2)");
    }

    #[test]
    fn selection_is_remembered() {
        let root = temp_root("select");
        let first = Profiles::load(&root).unwrap();
        let second = create(&root, ProfileInfo { created: u64::MAX, ..named("PvP") }, None).unwrap();
        let mut p = Profiles::load(&root).unwrap();
        assert_eq!(p.profiles.len(), 2);
        assert_eq!(p.selected, first.selected);
        p.select(&root, &second.id).unwrap();
        assert!(root.join(SETTINGS_FILE).is_file());
        assert_eq!(Profiles::load(&root).unwrap().selected, "PvP");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn misnamed_settings_file_is_moved_back() {
        let root = temp_root("misnamed");
        let first = Profiles::load(&root).unwrap();
        create(&root, ProfileInfo { created: u64::MAX, ..named("PvP") }, None).unwrap();
        std::fs::write(root.join(MISNAMED_SETTINGS_FILE), r#"{"selected_profile":"PvP"}"#).unwrap();
        let p = Profiles::load(&root).unwrap();
        assert_ne!(p.selected, first.selected);
        assert_eq!(p.selected, "PvP");
        assert!(!root.join(MISNAMED_SETTINGS_FILE).exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn uuid_folders_are_renamed_after_their_profile() {
        let root = temp_root("uuid");
        let profiles = root.join(PROFILES_DIR);
        let (a, b, c) = (
            "0b1f6f0e-6a45-4c2e-9d3b-2f6d1b7a9c01".to_string(),
            "6f2a9d4c-1e3b-4a8f-b7c5-0d9e8f7a6b02".to_string(),
            "c3d4e5f6-a7b8-4c9d-8e0f-1a2b3c4d5e03".to_string(),
        );
        for (id, json, created) in [(&a, "", 1), (&b, r#""name":"PvP","#, 2), (&c, "", 3)] {
            std::fs::create_dir_all(profiles.join(id).join("saves")).unwrap();
            std::fs::write(profiles.join(id).join(PROFILE_FILE), format!("{{{json}\"created\":{created}}}")).unwrap();
        }
        std::fs::write(root.join(SETTINGS_FILE), format!(r#"{{"selected_profile":"{c}"}}"#)).unwrap();
        let p = Profiles::load(&root).unwrap();
        let ids: Vec<&str> = p.profiles.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["Unnamed", "PvP", "Unnamed (1)"]);
        assert_eq!(p.selected, "Unnamed (1)");
        assert!(Profiles::dir(&root, "PvP").join("saves").is_dir());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn profile_in_use_keeps_its_folder_until_next_start() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = temp_root("uuid-in-use");
        let id = "0b1f6f0e-6a45-4c2e-9d3b-2f6d1b7a9c01";
        let dir = root.join(PROFILES_DIR).join(id);
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        std::fs::write(dir.join(PROFILE_FILE), r#"{"created":1}"#).unwrap();
        // Like a running game writing its log.
        let held = std::fs::OpenOptions::new().write(true).create(true).share_mode(0).open(dir.join("logs/latest.log")).unwrap();
        let p = Profiles::load(&root).unwrap();
        assert_eq!(p.profiles.len(), 1);
        assert_eq!(p.profiles[0].id, id);
        drop(held);
        assert_eq!(Profiles::load(&root).unwrap().profiles[0].id, "Unnamed");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn migrates_numbered_profiles_and_keeps_their_files() {
        let root = temp_root("legacy-json");
        std::fs::create_dir_all(root.join("profiles/1/saves")).unwrap();
        std::fs::create_dir_all(root.join("profiles/2/mods")).unwrap();
        std::fs::write(
            root.join("profiles.json"),
            r#"{"selected":"2","profiles":[{"id":"1"},{"id":"2","name":"PvP"},{"id":"../x"}]}"#,
        )
        .unwrap();
        let p = Profiles::load(&root).unwrap();
        assert_eq!(p.profiles.len(), 2);
        assert!(!root.join("profiles.json").exists());
        assert!(Profiles::dir(&root, "Unnamed").join("saves").is_dir());
        assert!(Profiles::dir(&root, "PvP").join("mods").is_dir());
        assert_eq!(p.selected, "PvP");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn migrates_original_instance_folder() {
        let root = temp_root("legacy-instance");
        std::fs::create_dir_all(root.join("instance/saves")).unwrap();
        let p = Profiles::load(&root).unwrap();
        assert!(!root.join("instance").exists());
        assert!(Profiles::dir(&root, &p.selected).join("saves").is_dir());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn ignores_folders_that_are_not_profiles() {
        let root = temp_root("junk");
        let p = Profiles::load(&root).unwrap();
        std::fs::create_dir_all(root.join("profiles/no-profile-json")).unwrap();
        std::fs::write(root.join("profiles/a-file"), "x").unwrap();
        assert_eq!(Profiles::load(&root).unwrap().profiles, p.profiles);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn pinned_is_read_and_only_written_when_set() {
        let pinned: ProfileInfo = serde_json::from_str(r#"{"name":"PvP","pinned":true,"created":1}"#).unwrap();
        assert!(pinned.pinned);
        let old: ProfileInfo = serde_json::from_str(r#"{"created":1}"#).unwrap();
        assert!(!old.pinned);
        assert!(!serde_json::to_string(&old).unwrap().contains("pinned"));
        assert!(serde_json::to_string(&pinned).unwrap().contains(r#""pinned":true"#));
    }

    #[test]
    fn added_profiles_save_name_version_and_icon() {
        let root = temp_root("add");
        let mut p = Profiles::load(&root).unwrap();
        let icon = square_icon(image::DynamicImage::ImageRgba8(image::RgbaImage::new(300, 200)));
        assert_eq!(icon.dimensions(), (ICON_PIXELS, ICON_PIXELS));
        let id = p.add(&root, Some("PvP".into()), "26.1.2", 4096, Some(&icon)).unwrap();
        assert_eq!(id, "PvP");
        let unnamed = p.add(&root, None, "26.1.2", 4096, None).unwrap();
        assert_eq!(unnamed, "Unnamed (1)");
        let loaded = Profiles::load(&root).unwrap();
        let pvp = loaded.profiles.iter().find(|x| x.id == "PvP").unwrap();
        assert_eq!(pvp.info.minecraft.as_deref(), Some("26.1.2"));
        let icon_path = Profiles::icon_path(&root, pvp).unwrap();
        assert_eq!(image::open(icon_path).unwrap().to_rgba8().dimensions(), (ICON_PIXELS, ICON_PIXELS));
        assert_eq!(loaded.profiles.len(), p.profiles.len());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn home_lists_pinned_then_recently_played() {
        let p = |id: &str, pinned, last_played| Profile { id: id.into(), info: ProfileInfo { pinned, last_played, ..Default::default() } };
        let profiles = Profiles {
            selected: String::new(),
            profiles: vec![p("never", false, 0), p("old", false, 10), p("pin-never", true, 0), p("new", false, 30), p("pin-old", true, 5), p("pin-new", true, 20)],
        };
        let ids: Vec<&str> = profiles.home_order().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["pin-new", "pin-old", "pin-never", "new", "old"]);
    }

    #[test]
    fn removing_a_profile_drops_its_folder_and_moves_the_selection() {
        let root = temp_root("remove");
        let mut profiles = Profiles::load(&root).unwrap();
        let first = profiles.profiles[0].id.clone();
        let second = profiles.add(&root, Some("PvP".into()), "26.1.2", 4096, None).unwrap();
        profiles.select(&root, &second).unwrap();
        profiles.remove(&root, &second, |dir| Ok(std::fs::remove_dir_all(dir)?)).unwrap();
        assert!(!Profiles::dir(&root, &second).exists());
        assert_eq!(profiles.selected, first);
        assert_eq!(Profiles::load(&root).unwrap().selected, first);
        assert!(profiles.remove(&root, "..", |_| Ok(())).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn updates_are_saved() {
        let root = temp_root("update");
        let mut p = Profiles::load(&root).unwrap();
        let id = p.profiles[0].id.clone();
        p.update(&root, &id, |info| {
            info.pinned = true;
            info.last_played = 42;
            info.name = Some("Renamed".into());
        })
        .unwrap();
        let loaded = Profiles::load(&root).unwrap();
        assert_eq!(loaded.profiles[0].info, p.profiles[0].info);
        assert_eq!(loaded.profiles[0].id, id, "the folder keeps its name");
        assert!(p.update(&root, "missing", |_| {}).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn icon_files_must_be_images() {
        let root = temp_root("bad-icon");
        let file = root.join("not-an-image.png");
        std::fs::write(&file, b"hello").unwrap();
        assert!(icon_from_file(&file).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn icon_must_be_a_plain_file_name() {
        let root = Path::new("data");
        let mut profile = Profile { id: "id".into(), info: ProfileInfo { icon: Some("icon.png".into()), ..Default::default() } };
        assert_eq!(Profiles::icon_path(root, &profile), Some(root.join("profiles/id/icon.png")));
        for bad in ["../x.png", "a/b.png", r"a\b.png", "C:x.png", ".."] {
            profile.info.icon = Some(bad.into());
            assert_eq!(Profiles::icon_path(root, &profile), None, "{bad}");
        }
    }
}
