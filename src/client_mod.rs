//! The Hermes in-game client: a Fabric mod built from `client/` (Gradle) and
//! bundled inside the launcher. Every game the launcher starts loads it through
//! Fabric's `fabric.addMods` option, so it never touches a profile's own `mods`
//! folder.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Built by `client\gradlew build`; `build.rs` stops with a hint if it's missing.
const JAR: &[u8] = include_bytes!("../client/build/libs/hermes-client.jar");

/// Writes the bundled mod to `<root>/client/hermes-client.jar` when it's missing
/// or differs from the bundled one, and returns its path.
pub fn install(root: &Path) -> Result<PathBuf> {
    let path = root.join("client").join("hermes-client.jar");
    if std::fs::read(&path).ok().as_deref() != Some(JAR) {
        let dir = path.parent().expect("jar path has a folder");
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        std::fs::write(&path, JAR).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(path)
}

/// The JVM option that makes Fabric load the mod at `jar`.
pub fn jvm_arg(jar: &Path) -> String {
    format!("-Dfabric.addMods={}", jar.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_once_and_repairs_a_changed_copy() {
        let root = std::env::temp_dir().join(format!("hermes-client-mod-test-{}", std::process::id()));
        let path = install(&root).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), JAR);
        std::fs::write(&path, b"stale").unwrap();
        install(&root).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), JAR);
        assert_eq!(jvm_arg(&path), format!("-Dfabric.addMods={}", path.display()));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
