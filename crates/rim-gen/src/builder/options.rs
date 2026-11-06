//! Build configuration ([`BuildOptions`]).

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use crate::dotnet::CommandRunner;

/// Options controlling a build.
#[derive(Clone)]
pub struct BuildOptions {
    /// Write `dist/<ModName>.zip`.
    pub zip: bool,
    /// Copy the finished mod into `<game>/Mods/<ModName>`.
    ///
    /// The destination directory is replaced wholesale. Before the replace, an
    /// existing `About.xml` there is checked: it must identify the same mod
    /// (via `packageId`, falling back to a case-insensitive `name`
    /// comparison), or the build is refused instead of silently deleting
    /// another mod.
    pub deploy: bool,
    /// Keep `.rim-rs/stage/<ModName>` after a successful build.
    pub keep_staging: bool,
    /// `dotnet` executable.
    pub dotnet: String,
    /// MSBuild configuration.
    pub configuration: String,
    /// Scratch directory (default `.rim-rs`).
    pub work_dir: PathBuf,
    /// Output directory for zips (default `dist`).
    pub dist_dir: PathBuf,
    /// Override for the deploy destination.
    pub mods_dir: Option<PathBuf>,
    /// Detect the game installation for the C# build phase (default `true`).
    ///
    /// With the default, `build()` locates a RimWorld 1.6 install and uses its
    /// `Managed/` DLLs (`Assembly-CSharp`, `UnityEngine.CoreModule`, a detected
    /// Harmony, ...) as references. With `game_probe(false)`, the probe and the
    /// automatic references are skipped entirely: every assembly the C# sources
    /// need must be passed explicitly via `ModBuilder::csharp_reference`. A
    /// probe-disabled build without any explicit reference is rejected before
    /// any file is written, instead of failing later inside `dotnet`.
    ///
    /// Deployment ([`BuildOptions::deploy`] without an explicit
    /// [`BuildOptions::mods_dir`]) is independent of this switch and still
    /// requires a local game install. Likewise, when the probe is disabled,
    /// [`crate::ModBuilder::game_dir`] has no effect on the C# phase; it only applies
    /// to deployment probing.
    pub game_probe: bool,
    /// Process runner used to invoke `dotnet` (default: [`crate::dotnet::SystemRunner`]).
    ///
    /// `SystemRunner` spawns a real process; tests can inject a fake
    /// [`CommandRunner`] to build without the .NET SDK.
    pub runner: Option<Arc<dyn CommandRunner>>,
    /// Stage and validate only, without compiling C#, zipping or deploying
    /// (default `false`).
    ///
    /// A dry run stops after staging and [`crate::package::validate_mod_dir`];
    /// no `dist/` directory is created, no `dotnet` process runs and nothing
    /// is deployed. All fields of [`crate::BuildReport`] except `mod_dir` (which
    /// still follows `keep_staging`) are `None`.
    ///
    /// Mods with C# input are rejected: a skipped C# compile stage would
    /// stage a mod without any DLL, which can never load, so this is refused
    /// up front instead of silently producing a broken layout.
    pub dry_run: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            zip: true,
            deploy: false,
            keep_staging: true,
            dotnet: "dotnet".to_string(),
            configuration: "Release".to_string(),
            work_dir: PathBuf::from(".rim-rs"),
            dist_dir: PathBuf::from("dist"),
            mods_dir: None,
            game_probe: true,
            runner: None,
            dry_run: false,
        }
    }
}

impl fmt::Debug for BuildOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildOptions")
            .field("zip", &self.zip)
            .field("deploy", &self.deploy)
            .field("keep_staging", &self.keep_staging)
            .field("dotnet", &self.dotnet)
            .field("configuration", &self.configuration)
            .field("work_dir", &self.work_dir)
            .field("dist_dir", &self.dist_dir)
            .field("mods_dir", &self.mods_dir)
            .field("game_probe", &self.game_probe)
            .field("runner", &self.runner.as_ref().map(|_| "injected"))
            .field("dry_run", &self.dry_run)
            .finish()
    }
}

impl BuildOptions {
    /// Sets both `work_dir` and `dist_dir` relative to a project root.
    pub fn base_dir(mut self, directory: impl Into<PathBuf>) -> Self {
        let directory = directory.into();
        self.work_dir = directory.join(".rim-rs");
        self.dist_dir = directory.join("dist");
        self
    }

    pub fn zip(mut self, value: bool) -> Self {
        self.zip = value;
        self
    }

    pub fn deploy(mut self, value: bool) -> Self {
        self.deploy = value;
        self
    }

    pub fn keep_staging(mut self, value: bool) -> Self {
        self.keep_staging = value;
        self
    }

    pub fn dotnet(mut self, program: impl Into<String>) -> Self {
        self.dotnet = program.into();
        self
    }

    pub fn configuration(mut self, configuration: impl Into<String>) -> Self {
        self.configuration = configuration.into();
        self
    }

    pub fn work_dir(mut self, directory: impl Into<PathBuf>) -> Self {
        self.work_dir = directory.into();
        self
    }

    pub fn dist_dir(mut self, directory: impl Into<PathBuf>) -> Self {
        self.dist_dir = directory.into();
        self
    }

    pub fn mods_dir(mut self, directory: impl Into<PathBuf>) -> Self {
        self.mods_dir = Some(directory.into());
        self
    }

    /// Sets whether the C# build phase probes the game installation.
    ///
    /// See [`BuildOptions::game_probe`] for the documented semantics.
    pub fn game_probe(mut self, value: bool) -> Self {
        self.game_probe = value;
        self
    }

    /// Overrides the process runner used to invoke `dotnet`.
    ///
    /// See [`BuildOptions::runner`] for the documented semantics.
    pub fn runner(mut self, value: Arc<dyn CommandRunner>) -> Self {
        self.runner = Some(value);
        self
    }

    /// Sets whether the build only stages and validates.
    ///
    /// See [`BuildOptions::dry_run`] for the documented semantics.
    pub fn dry_run(mut self, value: bool) -> Self {
        self.dry_run = value;
        self
    }
}
