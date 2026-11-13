//! Launch profiles. Each profile is a folder `<data>/profiles/<uuid>/` that is
//! also its Minecraft game folder (mods, config, saves, options). Inside it,
//! `profile.json` holds the profile's details, and its icon image sits next to it.
//! The selected profile is remembered in `<data>/launcher.json`.
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

/// Contents of `profile.json`. Add new details here; unknown fields in older
/// or newer files are ignored and missing ones take their defaults.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ProfileInfo {
    /// Owner-chosen name. `None` shows the `launcher.profile.unnamed` key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// File name of the icon image inside the profile folder (PNG).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Creation time (Unix milliseconds); orders the profile list.
    #[serde(default)]
    pub created: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// UUID; also the folder name.
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

impl Profiles {
    pub fn dir(root: &Path, id: &str) -> PathBuf {
        root.join(PROFILES_DIR).join(id)
    }

    /// Reads every `profiles/<uuid>/profile.json`, creating a default profile on
    /// first run and converting the older layouts (see `migrate_legacy`).
    pub fn load(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root.join(PROFILES_DIR))?;
        migrate_legacy(root)?;
        let mut profiles = scan(root)?;
        if profiles.is_empty() {
            profiles.push(create(root, ProfileInfo::default(), None)?);
        }
        profiles.sort_by(|a, b| (a.info.created, &a.id).cmp(&(b.info.created, &b.id)));
        let settings: Settings = std::fs::read(root.join(SETTINGS_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let selected = settings
            .selected_profile
            .filter(|id| profiles.iter().any(|p| &p.id == id))
            .unwrap_or_else(|| profiles[0].id.clone());
        Ok(Profiles { selected, profiles })
    }

    /// Selects a profile and remembers the choice in `launcher.json`.
    pub fn select(&mut self, root: &Path, id: &str) -> Result<()> {
        self.selected = id.to_string();
        write_json(&root.join(SETTINGS_FILE), &Settings { selected_profile: Some(self.selected.clone()) })
    }

    pub fn selected_index(&self) -> usize {
        self.profiles.iter().position(|p| p.id == self.selected).unwrap_or(0)
    }

    /// Path of the profile's icon, if it names a plain file inside its folder.
    pub fn icon_path(root: &Path, profile: &Profile) -> Option<PathBuf> {
        let icon = profile.info.icon.as_deref().filter(|n| valid_file_name(n))?;
        Some(Self::dir(root, &profile.id).join(icon))
    }
}

/// Profiles are folders whose name is a UUID and that contain a readable `profile.json`.
fn scan(root: &Path) -> Result<Vec<Profile>> {
    let mut profiles = Vec::new();
    for entry in std::fs::read_dir(root.join(PROFILES_DIR))? {
        let entry = entry?;
        let Some(id) = entry.file_name().to_str().and_then(|n| Uuid::parse_str(n).ok()).map(|u| u.to_string()) else {
            continue;
        };
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path().join(PROFILE_FILE)) else { continue };
        let Ok(info) = serde_json::from_slice::<ProfileInfo>(&bytes) else { continue };
        profiles.push(Profile { id, info });
    }
    Ok(profiles)
}

/// Creates a profile folder with a new UUID. `existing` is a game folder to move in.
fn create(root: &Path, mut info: ProfileInfo, existing: Option<&Path>) -> Result<Profile> {
    let id = Uuid::new_v4().to_string();
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

/// The previous layout: one `profiles.json` listing numbered folders.
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

/// Converts older layouts in place by renaming folders (nothing is deleted):
/// `profiles.json` + `profiles/<n>/`, or the original single `instance/` folder.
fn migrate_legacy(root: &Path) -> Result<()> {
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
            let info = ProfileInfo { name: old.name.clone(), icon: None, created: start + i as u64 };
            let profile = create(root, info, old_dir.is_dir().then_some(old_dir.as_path()))?;
            if old.id == legacy.selected {
                selected = Some(profile.id);
            }
        }
        write_json(&root.join(SETTINGS_FILE), &Settings { selected_profile: selected })?;
        std::fs::remove_file(&list)?;
        return Ok(());
    }
    let instance = root.join("instance");
    if instance.is_dir() && scan(root)?.is_empty() {
        create(root, ProfileInfo::default(), Some(&instance))?;
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

    #[test]
    fn first_run_creates_one_uuid_profile_with_its_json() {
        let root = temp_root("fresh");
        let p = Profiles::load(&root).unwrap();
        assert_eq!(p.profiles.len(), 1);
        let id = &p.profiles[0].id;
        assert!(Uuid::parse_str(id).is_ok());
        assert!(Profiles::dir(&root, id).join(PROFILE_FILE).is_file());
        assert_eq!(p.selected, *id);
        assert_eq!(Profiles::load(&root).unwrap(), p);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn selection_is_remembered() {
        let root = temp_root("select");
        let first = Profiles::load(&root).unwrap();
        let second = create(&root, ProfileInfo { name: Some("PvP".into()), created: u64::MAX, ..Default::default() }, None).unwrap();
        let mut p = Profiles::load(&root).unwrap();
        assert_eq!(p.profiles.len(), 2);
        assert_eq!(p.selected, first.selected);
        p.select(&root, &second.id).unwrap();
        assert_eq!(Profiles::load(&root).unwrap().selected, second.id);
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
        assert!(Profiles::dir(&root, &p.profiles[0].id).join("saves").is_dir());
        let pvp = &p.profiles[1];
        assert_eq!(pvp.info.name.as_deref(), Some("PvP"));
        assert!(Profiles::dir(&root, &pvp.id).join("mods").is_dir());
        assert_eq!(p.selected, pvp.id);
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
        std::fs::create_dir_all(root.join("profiles/not-a-uuid")).unwrap();
        std::fs::create_dir_all(root.join(PROFILES_DIR).join(Uuid::new_v4().to_string())).unwrap(); // no profile.json
        assert_eq!(Profiles::load(&root).unwrap().profiles, p.profiles);
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
