use std::env;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// A detected RimWorld 1.6 installation.
#[derive(Debug, Clone)]
pub struct GameInstall {
    pub root: PathBuf,
    pub managed: PathBuf,
    pub references: Vec<PathBuf>,
    /// Contents of `Version.txt` when present, e.g. `1.6.4871 rev573`.
    pub version: Option<String>,
}

impl GameInstall {
    /// Locates a 1.6 installation.
    ///
    /// Search order: explicit path, `RIMWORLD_DIR`, common GOG/Steam locations.
    pub fn locate(explicit: Option<&Path>) -> Result<Self> {
        let root = explicit
            .map(Path::to_path_buf)
            .or_else(|| env::var_os("RIMWORLD_DIR").map(PathBuf::from))
            .or_else(|| {
                candidates()
                    .into_iter()
                    .find(|path| looks_like_rimworld(path))
            });

        let root = root.ok_or(Error::GameDirNotFound)?;
        Self::from_root(root)
    }

    pub fn from_root(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();

        let managed = [
            "RimWorldWin64_Data",
            "RimWorldLinux_Data",
            "RimWorldMac_Data",
        ]
        .iter()
        .map(|name| root.join(name).join("Managed"))
        .find(|path| path.join("Assembly-CSharp.dll").is_file())
        .ok_or_else(|| Error::NotRimWorld(root.display().to_string()))?;

        let references = [
            "Assembly-CSharp.dll",
            "Assembly-CSharp-firstpass.dll",
            "UnityEngine.dll",
            "UnityEngine.CoreModule.dll",
            "0Harmony.dll",
            "HarmonyLib.dll",
        ]
        .iter()
        .map(|name| managed.join(name))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();

        if !references.iter().any(|path| {
            path.file_name()
                .is_some_and(|name| name == "Assembly-CSharp.dll")
        }) {
            return Err(Error::MissingAssembly(managed.join("Assembly-CSharp.dll")));
        }

        let version = std::fs::read_to_string(root.join("Version.txt"))
            .ok()
            .map(|version| version.trim().to_string())
            .filter(|version| !version.is_empty());

        Ok(Self {
            root,
            managed,
            references,
            version,
        })
    }

    pub fn mods_dir(&self) -> PathBuf {
        self.root.join("Mods")
    }

    /// Fails when `Version.txt` exists and does not report a 1.6 build.
    pub fn require_16(&self) -> Result<()> {
        match &self.version {
            Some(version) if !version.starts_with("1.6") => {
                Err(Error::UnsupportedVersion(version.clone()))
            }
            _ => Ok(()),
        }
    }
}

fn looks_like_rimworld(path: &Path) -> bool {
    path.join("Data/Core/About/About.xml").is_file()
        || path
            .join("RimWorldWin64_Data/Managed/Assembly-CSharp.dll")
            .is_file()
        || path
            .join("RimWorldLinux_Data/Managed/Assembly-CSharp.dll")
            .is_file()
}

fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        paths.push(home.join("Games/RimWorld"));
        paths.push(home.join(".steam/steam/steamapps/common/RimWorld"));
        paths.push(home.join(".local/share/Steam/steamapps/common/RimWorld"));
        paths.push(home.join("GOG Games/RimWorld"));
    }

    if cfg!(windows) {
        paths.push(PathBuf::from(
            r"C:\Program Files (x86)\Steam\steamapps\common\RimWorld",
        ));
        paths.push(PathBuf::from(r"C:\Program Files\GOG Galaxy\Games\RimWorld"));
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_non_16_version_file() {
        let dir = tempfile::tempdir().unwrap();
        let managed = dir.path().join("RimWorldWin64_Data/Managed");
        std::fs::create_dir_all(&managed).unwrap();
        std::fs::write(managed.join("Assembly-CSharp.dll"), b"").unwrap();
        std::fs::write(dir.path().join("Version.txt"), "1.5.4104 rev435").unwrap();

        let install = GameInstall::from_root(dir.path()).unwrap();
        assert!(matches!(
            install.require_16(),
            Err(Error::UnsupportedVersion(version)) if version.starts_with("1.5")
        ));
    }

    #[test]
    fn detects_the_local_gog_install() {
        let path = PathBuf::from("/home/miquella/Games/RimWorld");
        if !path.is_dir() {
            return;
        }

        let install = GameInstall::from_root(&path).unwrap();
        assert!(install.managed.join("Assembly-CSharp.dll").is_file());
        assert!(
            install
                .references
                .iter()
                .any(|path| path.ends_with("Assembly-CSharp.dll"))
        );
        assert!(
            install
                .version
                .as_deref()
                .is_some_and(|v| v.starts_with("1.6"))
        );
        assert!(install.require_16().is_ok());
    }
}
