use std::path::{Path, PathBuf};

/// Errors produced by `rim-gen`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("mod name must not be empty")]
    EmptyName,

    #[error("package_id is required")]
    MissingPackageId,

    #[error(
        "invalid packageId `{0}`: use only ASCII letters, digits and dots; \
         at least one dot; no leading/trailing/double dots; <= 60 chars; \
         must not contain `Ludeon`"
    )]
    InvalidPackageId(String),

    #[error("at least one supported version is required")]
    NoSupportedVersions,

    #[error("rim-gen currently supports only RimWorld 1.6, got version `{0}`")]
    UnsupportedVersion(String),

    #[error("game directory not found; set RIMWORLD_DIR or call .game_dir(...)")]
    GameDirNotFound,

    #[error(
        "`{0}` is not a RimWorld installation (missing Data/Core or Managed/Assembly-CSharp.dll)"
    )]
    NotRimWorld(String),

    #[error("missing required game assembly: {0}")]
    MissingAssembly(PathBuf),

    #[error("missing assembly reference: {0}")]
    MissingReference(PathBuf),

    #[error(
        "game_probe is disabled but no explicit csharp_reference(...) was provided; \
         with the probe off every game assembly reference must be supplied by the caller"
    )]
    GameProbeWithoutReferences,

    #[error(
        "dry_run cannot build a mod with C# sources; a skipped C# compile would stage \
         a mod without any DLL. Run a normal build instead"
    )]
    DryRunWithCsharpSources,

    #[error("asset source does not exist: {0}")]
    AssetSourceMissing(PathBuf),

    #[error("asset file mapping source is not a file: {0} (use map_dir for directories)")]
    AssetFileExpected(PathBuf),

    #[error("asset directory mapping source is not a directory: {0} (use map_file for files)")]
    AssetDirExpected(PathBuf),

    #[error("source directory does not exist: {0}")]
    SourceDirMissing(PathBuf),

    #[error("source file does not exist: {0}")]
    SourceFileMissing(PathBuf),

    #[error("symlink in the collected tree is not allowed: {0}")]
    SymlinkNotAllowed(PathBuf),

    #[error("unsupported file type in `{0}` (only plain files and directories can be staged)")]
    UnsupportedFileType(PathBuf),

    #[error("directory traversal failed at `{path}`: {detail}")]
    Walk { path: PathBuf, detail: String },

    #[error("dotnet build succeeded but the expected DLL was not produced: `{0}`")]
    DllNotProduced(PathBuf),

    #[error(
        "invalid defName `{0}`: use ASCII letters, digits, `_`, `-` and `.`; it must not be empty"
    )]
    InvalidDefName(String),

    #[error("invalid relative output path: {0}")]
    InvalidOutputPath(String),

    #[error("duplicate output path (case-insensitive): {0}")]
    DuplicateOutputPath(String),

    #[error("forbidden file in packaged mod: {0}")]
    ForbiddenFile(String),

    #[error(
        "refusing to replace `{path}`: it already contains a different mod ({detail}); \
         pick a different mod name or remove the other mod first"
    )]
    ModDirOccupied { path: PathBuf, detail: String },

    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("xml error: {0}")]
    Xml(String),

    #[error("dotnet command failed to start (`{program}`): {source}")]
    DotnetSpawn {
        program: String,
        #[source]
        source: std::io::Error,
    },

    #[error("dotnet command failed:\n{0}")]
    DotnetBuild(String),

    #[error("C# assembly references were provided without any C# source files")]
    CSharpReferenceWithoutSource,

    #[error("patch file `{0}` contains no PatchOperation")]
    EmptyPatch(String),

    #[error("PatchOperation `{0}` has no xpath, value or content")]
    EmptyOperation(String),

    #[error("invalid mod: {0}")]
    Validation(String),

    #[error("zip error: {0}")]
    Zip(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Maps a failed [`walkdir::WalkDir`] step onto [`Error::Walk`] with the
    /// best available location (the walk error's own path, falling back to the
    /// root that was walked).
    pub(crate) fn walk(root: &Path, error: walkdir::Error) -> Self {
        let path = error.path().unwrap_or(root).to_path_buf();
        Self::Walk {
            path,
            detail: error.to_string(),
        }
    }
}
