use std::path::PathBuf;

use walkdir::WalkDir;

use crate::error::{Error, Result};
use crate::package::validate_relative;

/// Maps source assets into the mod's staging directory.
#[derive(Debug, Default, Clone)]
pub struct AssetSet {
    mappings: Vec<AssetMapping>,
}

#[derive(Debug, Clone)]
struct AssetMapping {
    source: PathBuf,
    dest: PathBuf,
    kind: MappingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MappingKind {
    File,
    Dir,
}

impl AssetSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Maps one file to `dest_file`, a complete path relative to the mod root.
    pub fn map_file(
        &mut self,
        source: impl Into<PathBuf>,
        dest_file: impl Into<PathBuf>,
    ) -> &mut Self {
        self.mappings.push(AssetMapping {
            source: source.into(),
            dest: dest_file.into(),
            kind: MappingKind::File,
        });
        self
    }

    /// Maps a directory tree to `dest_dir`, preserving relative paths.
    pub fn map_dir(
        &mut self,
        source: impl Into<PathBuf>,
        dest_dir: impl Into<PathBuf>,
    ) -> &mut Self {
        self.mappings.push(AssetMapping {
            source: source.into(),
            dest: dest_dir.into(),
            kind: MappingKind::Dir,
        });
        self
    }

    pub fn textures(&mut self, source: impl Into<PathBuf>) -> &mut Self {
        self.map_dir(source, "Textures")
    }

    pub fn sounds(&mut self, source: impl Into<PathBuf>) -> &mut Self {
        self.map_dir(source, "Sounds")
    }

    pub fn languages(&mut self, source: impl Into<PathBuf>) -> &mut Self {
        self.map_dir(source, "Languages")
    }

    /// Maps a file to `About/Preview.png`.
    pub fn preview(&mut self, source: impl Into<PathBuf>) -> &mut Self {
        self.map_file(source, "About/Preview.png")
    }

    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// Expands all mappings into `(source_file, relative_destination)` pairs.
    pub fn collect(&self) -> Result<Vec<(PathBuf, PathBuf)>> {
        let mut output = Vec::new();

        for mapping in &self.mappings {
            if !mapping.source.exists() {
                return Err(Error::AssetSourceMissing(mapping.source.clone()));
            }
            validate_relative(&mapping.dest)?;

            match mapping.kind {
                MappingKind::File => {
                    if !mapping.source.is_file() {
                        return Err(Error::AssetFileExpected(mapping.source.clone()));
                    }
                    output.push((mapping.source.clone(), mapping.dest.clone()));
                    continue;
                }
                MappingKind::Dir => {
                    if !mapping.source.is_dir() {
                        return Err(Error::AssetDirExpected(mapping.source.clone()));
                    }
                }
            }

            for entry in WalkDir::new(&mapping.source).follow_links(false) {
                let entry = entry.map_err(|err| Error::Message(err.to_string()))?;
                if entry.file_type().is_symlink() {
                    return Err(Error::Message(format!(
                        "symlink in asset directory is not allowed: {}",
                        entry.path().display()
                    )));
                }
                if !entry.file_type().is_file() {
                    continue;
                }

                let relative = entry
                    .path()
                    .strip_prefix(&mapping.source)
                    .expect("walkdir entry is inside its root");
                output.push((entry.path().to_path_buf(), mapping.dest.join(relative)));
            }
        }

        sort_output(&mut output)?;
        Ok(output)
    }
}

/// Sorts mappings by destination so staging writes are independent of
/// filesystem iteration order, and rejects two mappings with the same
/// destination instead of letting one silently win.
fn sort_output(output: &mut [(PathBuf, PathBuf)]) -> Result<()> {
    output.sort_by(|left, right| left.1.cmp(&right.1));
    for pair in output.windows(2) {
        if pair[0].1 == pair[1].1 {
            return Err(Error::DuplicateOutputPath(pair[0].1.display().to_string()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_directory_contents() {
        let dir = tempfile::tempdir().unwrap();
        let textures = dir.path().join("textures/Things");
        std::fs::create_dir_all(&textures).unwrap();
        std::fs::write(textures.join("A.png"), b"png").unwrap();

        let mut assets = AssetSet::new();
        assets.textures(dir.path().join("textures"));

        let files = assets.collect().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].1, PathBuf::from("Textures/Things/A.png"));
    }

    #[test]
    fn maps_a_file_to_an_explicit_path() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("preview.png");
        std::fs::write(&file, b"png").unwrap();

        let mut assets = AssetSet::new();
        assets.map_file(&file, "About/Preview.png");

        let files = assets.collect().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].1, PathBuf::from("About/Preview.png"));
    }

    #[test]
    fn rejects_a_directory_for_file_mapping() {
        let dir = tempfile::tempdir().unwrap();
        let textures = dir.path().join("textures");
        std::fs::create_dir_all(&textures).unwrap();

        let mut assets = AssetSet::new();
        assets.map_file(&textures, "About/Preview.png");

        assert!(matches!(
            assets.collect(),
            Err(Error::AssetFileExpected(path)) if path == textures
        ));
    }

    #[test]
    fn rejects_a_file_for_directory_mapping() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("one.png");
        std::fs::write(&file, b"png").unwrap();

        let mut assets = AssetSet::new();
        assets.textures(&file);

        assert!(matches!(
            assets.collect(),
            Err(Error::AssetDirExpected(path)) if path == file
        ));
    }

    #[test]
    fn rejects_parent_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("preview.png");
        std::fs::write(&file, b"png").unwrap();

        let mut assets = AssetSet::new();
        assets.map_file(&file, "../outside.png");

        assert!(assets.collect().is_err());
    }

    #[test]
    fn rejects_two_mappings_to_the_same_destination() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first/a.png");
        std::fs::create_dir_all(first.parent().unwrap()).unwrap();
        std::fs::write(&first, b"first").unwrap();
        let second = dir.path().join("second/a.png");
        std::fs::create_dir_all(second.parent().unwrap()).unwrap();
        std::fs::write(&second, b"second").unwrap();

        let mut assets = AssetSet::new();
        assets.map_file(&first, "Textures/a.png");
        assets.map_file(&second, "Textures/a.png");

        assert!(matches!(
            assets.collect(),
            Err(Error::DuplicateOutputPath(path)) if path == "Textures/a.png"
        ));
    }
}
