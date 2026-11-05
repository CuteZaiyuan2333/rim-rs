use std::path::PathBuf;

/// Errors produced by `rim-rs`.
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

    #[error("rim-rs currently supports only RimWorld 1.6, got version `{0}`")]
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

    #[error("asset source does not exist: {0}")]
    AssetSourceMissing(PathBuf),

    #[error("invalid relative output path: {0}")]
    InvalidOutputPath(String),

    #[error("duplicate output path (case-insensitive): {0}")]
    DuplicateOutputPath(String),

    #[error("forbidden file in packaged mod: {0}")]
    ForbiddenFile(String),

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

    #[error("invalid mod: {0}")]
    Validation(String),

    #[error("zip error: {0}")]
    Zip(String),

    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
