use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use walkdir::WalkDir;

use crate::error::{Error, Result};

/// C# inputs for a mod: in-memory sources, files on disk, and extra references.
///
/// `rim-rs` does not generate C# syntax itself. Callers can use any code
/// generator (or hand-written files) and pass the resulting source as a string.
#[derive(Debug, Default)]
pub(crate) struct CSharpSet {
    sources: Vec<SourceFile>,
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
    references: Vec<PathBuf>,
}

#[derive(Debug)]
struct SourceFile {
    name: String,
    code: String,
}

impl CSharpSet {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Adds an in-memory source file. The code is written verbatim.
    pub(crate) fn add_source(
        &mut self,
        name: impl Into<String>,
        code: impl Into<String>,
    ) -> &mut Self {
        let name = normalize_file_name(&name.into());
        self.sources.push(SourceFile {
            name,
            code: code.into(),
        });
        self
    }

    /// Adds a hand-written `.cs` file.
    pub(crate) fn add_file(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.files.push(path.into());
        self
    }

    /// Adds every `.cs` file under a directory (recursively).
    pub(crate) fn add_dir(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.dirs.push(path.into());
        self
    }

    /// Adds an assembly reference, e.g. a Harmony DLL.
    pub(crate) fn add_reference(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.references.push(path.into());
        self
    }

    pub(crate) fn has_sources(&self) -> bool {
        !self.sources.is_empty() || !self.files.is_empty() || !self.dirs.is_empty()
    }

    pub(crate) fn references(&self) -> &[PathBuf] {
        &self.references
    }

    /// References only make sense when at least one source file is compiled.
    pub(crate) fn validate(&self) -> Result<()> {
        if !self.references.is_empty() && !self.has_sources() {
            return Err(Error::CSharpReferenceWithoutSource);
        }
        Ok(())
    }

    /// Returns validated in-memory sources as `(file_name, code)` pairs.
    pub(crate) fn source_files(&self) -> Result<Vec<(&str, &str)>> {
        let mut seen = HashSet::new();
        let mut rendered = Vec::with_capacity(self.sources.len());

        for source in &self.sources {
            validate_generated_name(&source.name)?;
            let key = source.name.to_ascii_lowercase();
            if !seen.insert(key) {
                return Err(Error::DuplicateOutputPath(source.name.clone()));
            }
            rendered.push((source.name.as_str(), source.code.as_str()));
        }

        Ok(rendered)
    }

    /// Expands raw files/directories into a deduplicated compile list.
    pub(crate) fn compile_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut seen = HashSet::new();

        for path in &self.files {
            push_unique(&mut files, &mut seen, path)?;
        }

        for dir in &self.dirs {
            if !dir.is_dir() {
                return Err(Error::Message(format!(
                    "C# source directory does not exist: {}",
                    dir.display()
                )));
            }
            for entry in WalkDir::new(dir).follow_links(false) {
                let entry = entry.map_err(|err| Error::Message(err.to_string()))?;
                if entry.file_type().is_symlink() {
                    return Err(Error::Message(format!(
                        "symlink in C# source directory is not allowed: {}",
                        entry.path().display()
                    )));
                }
                if entry.file_type().is_file()
                    && entry.path().extension().is_some_and(|ext| ext == "cs")
                {
                    push_unique(&mut files, &mut seen, entry.path())?;
                }
            }
        }

        Ok(files)
    }
}

fn normalize_file_name(name: &str) -> String {
    let name = name.trim();
    if Path::new(name).extension().is_none() {
        format!("{name}.cs")
    } else {
        name.to_string()
    }
}

fn validate_generated_name(name: &str) -> Result<()> {
    let path = Path::new(name);
    let mut components = path.components();
    let single_normal =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();

    let safe = single_normal
        && name.len() > 3
        && !name.contains(':')
        && path.extension().is_some_and(|ext| ext == "cs")
        && (cfg!(windows) || !name.contains('\\'));

    if safe {
        Ok(())
    } else {
        Err(Error::InvalidOutputPath(name.to_string()))
    }
}

fn push_unique(files: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: &Path) -> Result<()> {
    if !path.is_file() {
        return Err(Error::Message(format!(
            "C# source file does not exist: {}",
            path.display()
        )));
    }
    let canonical = path
        .canonicalize()
        .map_err(|source| Error::io(path, source))?;
    if seen.insert(canonical.clone()) {
        files.push(canonical);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_source_code_verbatim() {
        let mut set = CSharpSet::new();
        set.add_source("Mod", "// generated by another crate\nclass Mod {}");

        let files = set.source_files().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "Mod.cs");
        assert_eq!(files[0].1, "// generated by another crate\nclass Mod {}");
    }

    #[test]
    fn keeps_existing_cs_extension() {
        let mut set = CSharpSet::new();
        set.add_source("Mod.cs", "class Mod {}");
        set.add_source("Patches.cs", "class Patches {}");

        let names: Vec<_> = set
            .source_files()
            .unwrap()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(names, ["Mod.cs", "Patches.cs"]);
    }

    #[test]
    fn rejects_path_in_generated_file_names() {
        let mut set = CSharpSet::new();
        set.add_source("../evil.cs", "class Evil {}");
        assert!(set.source_files().is_err());

        let mut set = CSharpSet::new();
        set.add_source("sub/evil.cs", "class Evil {}");
        assert!(set.source_files().is_err());

        let mut set = CSharpSet::new();
        set.add_source("Mod.txt", "class Mod {}");
        assert!(set.source_files().is_err());
    }

    #[test]
    fn rejects_duplicate_generated_file_names() {
        let mut set = CSharpSet::new();
        set.add_source("Mod.cs", "class A {}");
        set.add_source("mod.cs", "class B {}");
        assert!(matches!(
            set.source_files(),
            Err(Error::DuplicateOutputPath(_))
        ));
    }

    #[test]
    fn rejects_references_without_sources() {
        let mut set = CSharpSet::new();
        set.add_reference("/tmp/0Harmony.dll");
        assert!(matches!(
            set.validate(),
            Err(Error::CSharpReferenceWithoutSource)
        ));

        set.add_source("Mod.cs", "class Mod {}");
        assert!(set.validate().is_ok());
    }

    #[test]
    fn deduplicates_compile_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("A.cs");
        std::fs::write(&file, "class A {}").unwrap();

        let mut set = CSharpSet::new();
        set.add_file(&file).add_dir(dir.path());

        let files = set.compile_files().unwrap();
        assert_eq!(files.len(), 1);
    }
}
