//! What a successful build produced ([`BuildReport`]).

use std::path::PathBuf;

/// Result of a successful build.
#[derive(Debug, Clone)]
pub struct BuildReport {
    /// Staging directory, present only when `keep_staging` is enabled.
    pub mod_dir: Option<PathBuf>,
    /// Directory the finished mod was copied to, present only when `deploy` is enabled.
    pub deployed_dir: Option<PathBuf>,
    pub zip_path: Option<PathBuf>,
    pub dll_path: Option<PathBuf>,
}
