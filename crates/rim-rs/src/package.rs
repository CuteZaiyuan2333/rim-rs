use std::collections::HashSet;
use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};

use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::error::{Error, Result};

/// Writes files into a staging directory while guarding against path escapes
/// and case-insensitive collisions.
pub struct Staging {
    root: PathBuf,
    seen: HashSet<String>,
}

impl Staging {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|source| Error::io(&root, source))?;
        Ok(Self {
            root,
            seen: HashSet::new(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn write(&mut self, relative: impl AsRef<Path>, bytes: &[u8]) -> Result<()> {
        let relative = relative.as_ref();
        self.record(relative)?;
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
        }
        fs::write(&path, bytes).map_err(|source| Error::io(&path, source))
    }

    pub fn copy(&mut self, source: &Path, relative: impl AsRef<Path>) -> Result<()> {
        let relative = relative.as_ref();
        self.record(relative)?;
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
        }
        fs::copy(source, &path).map_err(|source_err| Error::io(source, source_err))?;
        Ok(())
    }

    fn record(&mut self, relative: &Path) -> Result<()> {
        validate_relative(relative)?;
        let key = relative
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(name.to_string_lossy()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/")
            .to_ascii_lowercase();
        if !self.seen.insert(key) {
            return Err(Error::DuplicateOutputPath(relative.display().to_string()));
        }
        Ok(())
    }
}

pub fn validate_relative(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(Error::InvalidOutputPath(path.display().to_string()));
    }

    let text = path.to_string_lossy();
    if cfg!(not(windows)) && text.contains('\\') {
        return Err(Error::InvalidOutputPath(path.display().to_string()));
    }
    if text.contains(':') {
        return Err(Error::InvalidOutputPath(path.display().to_string()));
    }

    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => return Err(Error::InvalidOutputPath(path.display().to_string())),
        }
    }
    Ok(())
}

/// Validates the final staging directory.
pub fn validate_mod_dir(root: &Path) -> Result<()> {
    let about = root.join("About/About.xml");
    if !about.is_file() {
        return Err(Error::Validation(format!(
            "missing About/About.xml in {}",
            root.display()
        )));
    }

    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|err| Error::Validation(err.to_string()))?;

        if entry.file_type().is_symlink() {
            return Err(Error::Validation(format!(
                "symlink is not allowed in packaged mod: {}",
                entry.path().display()
            )));
        }
        if !entry.file_type().is_file() {
            continue;
        }

        let relative = entry
            .path()
            .strip_prefix(root)
            .expect("walkdir entry is inside root");

        for component in relative.components() {
            if let Component::Normal(name) = component {
                let name = name.to_string_lossy();
                if matches!(
                    name.as_ref(),
                    "obj" | "bin" | "target" | ".rim-rs" | ".git" | "src" | "src-csharp"
                ) {
                    return Err(Error::ForbiddenFile(relative.display().to_string()));
                }
            }
        }

        let file_name = entry.file_name().to_string_lossy();
        if file_name.ends_with(".cs")
            || file_name.ends_with(".csproj")
            || file_name.ends_with(".sln")
            || file_name.ends_with(".rs")
            || file_name.ends_with(".toml")
            || file_name == "Cargo.lock"
        {
            return Err(Error::ForbiddenFile(relative.display().to_string()));
        }
    }

    Ok(())
}

/// Creates a zip archive from a directory.
///
/// Entries are written in sorted path order with a fixed timestamp
/// (1980-01-01), so two builds of identical input produce identical bytes.
pub fn zip_dir(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
    }

    let file = File::create(destination).map_err(|error| Error::io(destination, error))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::DEFAULT);

    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|err| Error::Zip(err.to_string()))?;
        if entry.file_type().is_symlink() {
            return Err(Error::Zip(format!(
                "symlink is not allowed in zip: {}",
                entry.path().display()
            )));
        }
        if !entry.file_type().is_file() {
            continue;
        }

        let relative = entry
            .path()
            .strip_prefix(source)
            .expect("walkdir entry is inside source");
        let name = relative.to_string_lossy().replace('\\', "/");
        entries.push((name, entry.path().to_path_buf()));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));

    for (name, path) in &entries {
        zip.start_file(name.as_str(), options)
            .map_err(|err| Error::Zip(err.to_string()))?;
        let mut input = File::open(path).map_err(|err| Error::io(path, err))?;
        io::copy(&mut input, &mut zip).map_err(|err| Error::Zip(err.to_string()))?;
    }

    zip.finish().map_err(|err| Error::Zip(err.to_string()))?;
    Ok(())
}

/// Recursively copies `source` to `destination`, replacing `destination`.
pub fn copy_dir_all(source: &Path, destination: &Path) -> Result<()> {
    if !source.is_dir() {
        return Err(Error::Message(format!(
            "source directory does not exist: {}",
            source.display()
        )));
    }

    if destination.exists() {
        fs::remove_dir_all(destination).map_err(|error| Error::io(destination, error))?;
    }
    fs::create_dir_all(destination).map_err(|error| Error::io(destination, error))?;

    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|err| Error::Message(err.to_string()))?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .expect("walkdir entry is inside source");
        let target = destination.join(relative);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).map_err(|error| Error::io(&target, error))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
            }
            fs::copy(entry.path(), &target).map_err(|error| Error::io(entry.path(), error))?;
        } else {
            return Err(Error::Message(format!(
                "unsupported file type in source: {}",
                entry.path().display()
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let mut staging = Staging::new(dir.path()).unwrap();
        assert!(staging.write("../escape.txt", b"nope").is_err());
        assert!(staging.write("./escape.txt", b"nope").is_err());
        #[cfg(not(windows))]
        assert!(staging.write("dir\\escape.txt", b"nope").is_err());
    }

    #[test]
    fn rejects_case_insensitive_collisions() {
        let dir = tempfile::tempdir().unwrap();
        let mut staging = Staging::new(dir.path()).unwrap();
        staging.write("Textures/Foo.png", b"a").unwrap();
        assert!(staging.write("Textures/foo.png", b"b").is_err());
    }

    #[test]
    fn zips_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("mod");
        fs::create_dir_all(source.join("About")).unwrap();
        fs::write(source.join("About/About.xml"), b"<ModMetaData/>").unwrap();

        let zip_path = dir.path().join("mod.zip");
        zip_dir(&source, &zip_path).unwrap();
        assert!(zip_path.is_file());
        assert!(fs::metadata(&zip_path).unwrap().len() > 0);
    }

    #[test]
    fn zips_identical_input_to_identical_bytes() {
        let build_tree = |root: &Path, reversed: bool| {
            let names = [
                "Defs/A.xml",
                "Defs/B.xml",
                "About/About.xml",
                "Textures/x.png",
            ];
            let names: Vec<_> = if reversed {
                names.iter().rev().collect()
            } else {
                names.iter().collect()
            };
            for name in names {
                let path = root.join(name);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, format!("content of {name}")).unwrap();
            }
        };

        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        build_tree(&first, false);
        build_tree(&second, true);

        let first_zip = dir.path().join("first.zip");
        let second_zip = dir.path().join("second.zip");
        zip_dir(&first, &first_zip).unwrap();
        zip_dir(&second, &second_zip).unwrap();

        assert_eq!(
            fs::read(&first_zip).unwrap(),
            fs::read(&second_zip).unwrap()
        );
    }

    #[test]
    fn zips_entries_in_sorted_order() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("mod");
        fs::create_dir_all(&source).unwrap();
        for name in ["z.txt", "a.txt", "m.txt"] {
            fs::write(source.join(name), b"x").unwrap();
        }

        let zip_path = dir.path().join("mod.zip");
        zip_dir(&source, &zip_path).unwrap();

        let file = File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name().unwrap().to_string())
            .collect();
        assert_eq!(names, ["a.txt", "m.txt", "z.txt"]);
    }
}
