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
}

impl AssetSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Maps a file or directory to a path relative to the mod root.
    ///
    /// Directory contents are copied recursively; relative paths are preserved.
    pub fn map(&mut self, source: impl Into<PathBuf>, dest: impl Into<PathBuf>) -> &mut Self {
        self.mappings.push(AssetMapping {
            source: source.into(),
            dest: dest.into(),
        });
        self
    }

    pub fn textures(&mut self, source: impl Into<PathBuf>) -> &mut Self {
        self.map(source, "Textures")
    }

    pub fn sounds(&mut self, source: impl Into<PathBuf>) -> &mut Self {
        self.map(source, "Sounds")
    }

    pub fn languages(&mut self, source: impl Into<PathBuf>) -> &mut Self {
        self.map(source, "Languages")
    }

    /// Maps a file to `About/Preview.png`.
    pub fn preview(&mut self, source: impl Into<PathBuf>) -> &mut Self {
        self.map(source, "About/Preview.png")
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

            if mapping.source.is_file() {
                output.push((mapping.source.clone(), mapping.dest.clone()));
                continue;
            }

            if !mapping.source.is_dir() {
                return Err(Error::AssetSourceMissing(mapping.source.clone()));
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

        Ok(output)
    }
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
    fn rejects_parent_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("preview.png");
        std::fs::write(&file, b"png").unwrap();

        let mut assets = AssetSet::new();
        assets.map(&file, "../outside.png");

        assert!(assets.collect().is_err());
    }
}
