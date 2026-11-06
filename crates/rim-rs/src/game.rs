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
    ///
    /// Missing `Version.txt` is treated as unknown and allowed, so portable
    /// installs without the file still work. Unparseable versions are rejected.
    pub fn require_16(&self) -> Result<()> {
        match &self.version {
            Some(version) if !is_rimworld_16(version) => {
                Err(Error::UnsupportedVersion(version.clone()))
            }
            _ => Ok(()),
        }
    }
}

fn is_rimworld_16(version: &str) -> bool {
    let Some(core) = version.split_whitespace().next() else {
        return false;
    };
    let mut parts = core.split('.');
    let major = parts.next().and_then(|part| part.parse::<u32>().ok());
    let minor = parts.next().and_then(|part| part.parse::<u32>().ok());
    matches!((major, minor), (Some(1), Some(6)))
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

    fn fake_install(version: Option<&str>, harmony: bool) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let managed = dir.path().join("RimWorldWin64_Data/Managed");
        std::fs::create_dir_all(&managed).unwrap();
        std::fs::write(managed.join("Assembly-CSharp.dll"), b"").unwrap();
        if harmony {
            std::fs::write(managed.join("0Harmony.dll"), b"").unwrap();
        }
        if let Some(version) = version {
            std::fs::write(dir.path().join("Version.txt"), version).unwrap();
        }
        dir
    }

    #[test]
    fn detects_a_fake_install() {
        let dir = fake_install(Some("1.6.4871 rev573"), true);

        let install = GameInstall::from_root(dir.path()).unwrap();
        assert_eq!(
            install.managed,
            dir.path().join("RimWorldWin64_Data/Managed")
        );
        assert!(
            install
                .references
                .iter()
                .any(|path| path.ends_with("Assembly-CSharp.dll"))
        );
        assert!(
            install
                .references
                .iter()
                .any(|path| path.ends_with("0Harmony.dll"))
        );
        assert_eq!(install.version.as_deref(), Some("1.6.4871 rev573"));
        assert!(install.require_16().is_ok());
    }

    #[test]
    fn allows_a_missing_version_file() {
        let dir = fake_install(None, false);

        let install = GameInstall::from_root(dir.path()).unwrap();
        assert!(install.version.is_none());
        assert!(install.require_16().is_ok());
    }

    #[test]
    fn parses_the_major_and_minor_version() {
        assert!(is_rimworld_16("1.6.4871 rev573"));
        assert!(is_rimworld_16("1.6"));
        assert!(is_rimworld_16("1.6.0.0"));

        assert!(!is_rimworld_16("1.5.4104 rev435"));
        assert!(!is_rimworld_16("1.60.4871"));
        assert!(!is_rimworld_16("1.6x"));
        assert!(!is_rimworld_16("2.6.1"));
        assert!(!is_rimworld_16(""));
        assert!(!is_rimworld_16("unknown"));
    }

    #[test]
    fn rejects_a_non_16_version_file() {
        for version in ["1.5.4104 rev435", "1.60.4871", "1.6x"] {
            let dir = fake_install(Some(version), false);
            let install = GameInstall::from_root(dir.path()).unwrap();
            assert!(
                matches!(
                    install.require_16(),
                    Err(Error::UnsupportedVersion(reported)) if reported == version
                ),
                "{version}"
            );
        }
    }

    #[test]
    #[ignore = "requires a local RimWorld 1.6 install; set RIMWORLD_DIR and run with --ignored"]
    fn detects_an_env_install() {
        let root = env::var_os("RIMWORLD_DIR").expect("set RIMWORLD_DIR to a RimWorld 1.6 install");
        let install = GameInstall::from_root(root).unwrap();

        assert!(install.managed.join("Assembly-CSharp.dll").is_file());
        assert!(
            install
                .references
                .iter()
                .any(|path| path.ends_with("Assembly-CSharp.dll"))
        );
        assert!(install.version.is_some(), "missing Version.txt");
        assert!(install.require_16().is_ok());
    }
}
