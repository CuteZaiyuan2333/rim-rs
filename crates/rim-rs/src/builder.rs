use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::assets::AssetSet;
use crate::csharp::CSharpSet;
use crate::defs::DefSet;
use crate::dotnet::{CommandRunner, DotnetRunner, SystemRunner};
use crate::error::{Error, Result};
use crate::game::GameInstall;
use crate::package::{self, Staging};
use crate::patch::PatchSet;
use crate::project::{self, ProjectSpec};
use crate::xml::Element;

/// Options controlling a build.
#[derive(Clone)]
pub struct BuildOptions {
    /// Write `dist/<ModName>.zip`.
    pub zip: bool,
    /// Copy the finished mod into `<game>/Mods/<ModName>`.
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
    /// [`ModBuilder::game_dir`] has no effect on the C# phase; it only applies
    /// to deployment probing.
    pub game_probe: bool,
    /// Process runner used to invoke `dotnet` (default: [`SystemRunner`]).
    ///
    /// `SystemRunner` spawns a real process; tests can inject a fake
    /// [`CommandRunner`] to build without the .NET SDK.
    pub runner: Option<Arc<dyn CommandRunner>>,
    /// Stage and validate only, without compiling C#, zipping or deploying
    /// (default `false`).
    ///
    /// A dry run stops after staging and [`crate::package::validate_mod_dir`];
    /// no `dist/` directory is created, no `dotnet` process runs and nothing
    /// is deployed. All fields of [`BuildReport`] except `mod_dir` (which
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

/// The root builder.
///
/// ```no_run
/// use rim_rs::prelude::*;
///
/// fn main() -> rim_rs::Result<()> {
///     Mod::builder("My Mod")
///         .package_id("me.mymod")
///         .defs(|defs| {
///             defs.add(ThingDef::new("MyThing").parent("BaseThing"));
///         })
///         .build()?;
///     Ok(())
/// }
/// ```
pub struct ModBuilder {
    name: String,
    package_id: Option<String>,
    author: Option<String>,
    description: Option<String>,
    url: Option<String>,
    supported_versions: Vec<String>,
    game_dir: Option<PathBuf>,
    defs: DefSet,
    patches: PatchSet,
    csharp: CSharpSet,
    assets: AssetSet,
    options: BuildOptions,
}

impl ModBuilder {
    pub fn builder(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            package_id: None,
            author: None,
            description: None,
            url: None,
            supported_versions: vec!["1.6".to_string()],
            game_dir: None,
            defs: DefSet::new(),
            patches: PatchSet::new(),
            csharp: CSharpSet::new(),
            assets: AssetSet::new(),
            options: BuildOptions::default(),
        }
    }

    pub fn package_id(mut self, package_id: impl Into<String>) -> Self {
        self.package_id = Some(package_id.into());
        self
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Sets the supported RimWorld versions, replacing the default `["1.6"]`.
    ///
    /// `rim-rs` only supports RimWorld 1.6; any entry other than exactly `1.6`
    /// is rejected by [`ModBuilder::build`].
    pub fn supported_versions<I, S>(mut self, versions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.supported_versions = versions.into_iter().map(Into::into).collect();
        self
    }

    /// Explicit game root. If omitted, `RIMWORLD_DIR` and common paths are tried.
    pub fn game_dir(mut self, directory: impl Into<PathBuf>) -> Self {
        self.game_dir = Some(directory.into());
        self
    }

    pub fn defs(mut self, f: impl FnOnce(&mut DefSet)) -> Self {
        f(&mut self.defs);
        self
    }

    pub fn patches(mut self, f: impl FnOnce(&mut PatchSet)) -> Self {
        f(&mut self.patches);
        self
    }

    /// Adds an in-memory C# source file. The code is written verbatim.
    ///
    /// `.cs` is appended when `file_name` has no extension.
    pub fn csharp(mut self, file_name: impl Into<String>, code: impl Into<String>) -> Self {
        self.csharp.add_source(file_name, code);
        self
    }

    /// Adds a hand-written `.cs` file.
    pub fn csharp_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.csharp.add_file(path);
        self
    }

    /// Adds every `.cs` file under a directory (recursively).
    pub fn csharp_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.csharp.add_dir(path);
        self
    }

    /// Adds an assembly reference, e.g. a Harmony DLL.
    pub fn csharp_reference(mut self, path: impl Into<PathBuf>) -> Self {
        self.csharp.add_reference(path);
        self
    }

    pub fn assets(mut self, f: impl FnOnce(&mut AssetSet)) -> Self {
        f(&mut self.assets);
        self
    }

    pub fn options(mut self, options: BuildOptions) -> Self {
        self.options = options;
        self
    }

    /// Builds the mod.
    pub fn build(self) -> Result<BuildReport> {
        self.validate()?;

        let mod_folder = project::sanitize_dir_name(&self.name);
        let stage_root = self.options.work_dir.join("stage");
        let mod_dir = stage_root.join(&mod_folder);

        if mod_dir.exists() {
            fs::remove_dir_all(&mod_dir).map_err(|error| Error::io(&mod_dir, error))?;
        }
        fs::create_dir_all(&mod_dir).map_err(|error| Error::io(&mod_dir, error))?;

        let mut staging = Staging::new(&mod_dir)?;

        // About.xml
        staging.write(
            "About/About.xml",
            self.about_xml().to_xml_string()?.as_bytes(),
        )?;

        // Defs
        for (file, root) in self.defs.xml_files()? {
            let relative = PathBuf::from("Defs").join(file);
            staging.write(relative, root.to_xml_string()?.as_bytes())?;
        }

        // Patches
        for (file, root) in self.patches.xml_files() {
            let relative = PathBuf::from("Patches").join(file);
            staging.write(relative, root.to_xml_string()?.as_bytes())?;
        }

        // Assets
        for (source, destination) in self.assets.collect()? {
            staging.copy(&source, destination)?;
        }

        // C#
        let dll_path = if self.options.dry_run {
            // Rejected up front for mods with C# input; a dry run never
            // reaches the compile stage.
            None
        } else if self.csharp.has_sources() {
            Some(self.build_csharp(&self.csharp, &mut staging, &mod_folder)?)
        } else {
            None
        };

        package::validate_mod_dir(&mod_dir)?;

        let zip_path = if self.options.zip && !self.options.dry_run {
            fs::create_dir_all(&self.options.dist_dir)
                .map_err(|error| Error::io(&self.options.dist_dir, error))?;
            let zip_path = self.options.dist_dir.join(format!("{mod_folder}.zip"));
            package::zip_dir(&mod_dir, &zip_path)?;
            Some(zip_path)
        } else {
            None
        };

        let deployed_dir = if self.options.deploy && !self.options.dry_run {
            let mods_dir = match &self.options.mods_dir {
                Some(directory) => directory.clone(),
                None => GameInstall::locate(self.game_dir.as_deref())?.mods_dir(),
            };
            let destination = mods_dir.join(&mod_folder);
            package::copy_dir_all(&mod_dir, &destination)?;
            Some(destination)
        } else {
            None
        };

        let staged_dir = if self.options.keep_staging {
            Some(mod_dir)
        } else {
            fs::remove_dir_all(&mod_dir).map_err(|error| Error::io(&mod_dir, error))?;
            None
        };

        Ok(BuildReport {
            mod_dir: staged_dir,
            deployed_dir,
            zip_path,
            dll_path,
        })
    }

    fn build_csharp(
        &self,
        csharp: &CSharpSet,
        staging: &mut Staging,
        mod_folder: &str,
    ) -> Result<PathBuf> {
        let assembly_name = project::sanitize_identifier(&self.name);
        let root_namespace = assembly_name.clone();

        // All intermediate directories are isolated per mod, so consecutive
        // builds of different mods on the same `base_dir` never overwrite
        // each other's C# sources, project files or DLLs.
        let source_dir = self.options.work_dir.join("csharp").join(mod_folder);
        if source_dir.exists() {
            fs::remove_dir_all(&source_dir).map_err(|error| Error::io(&source_dir, error))?;
        }
        fs::create_dir_all(&source_dir).map_err(|error| Error::io(&source_dir, error))?;

        let mut compile_files = csharp.compile_files()?;
        for (name, code) in csharp.source_files()? {
            let path = source_dir.join(name);
            fs::write(&path, code).map_err(|error| Error::io(&path, error))?;
            compile_files.push(path);
        }

        let mut references = Vec::new();
        if self.options.game_probe {
            let game = GameInstall::locate(self.game_dir.as_deref())?;
            game.require_16()?;
            references = game.references.clone();
        }
        for reference in csharp.references() {
            if !reference.is_file() {
                return Err(Error::MissingReference(reference.clone()));
            }
            if !references.iter().any(|existing| existing == reference) {
                references.push(reference.clone());
            }
        }

        let project_dir = self.options.work_dir.join("work").join(mod_folder);
        fs::create_dir_all(&project_dir).map_err(|error| Error::io(&project_dir, error))?;
        let csproj = project_dir.join("Mod.csproj");
        let csproj_xml = project::render_csproj(&ProjectSpec {
            assembly_name: &assembly_name,
            root_namespace: &root_namespace,
            references: &references,
            compile_files: &compile_files,
        });
        fs::write(&csproj, csproj_xml).map_err(|error| Error::io(&csproj, error))?;

        let output_dir = self.options.work_dir.join("out").join(mod_folder);
        if output_dir.exists() {
            fs::remove_dir_all(&output_dir).map_err(|error| Error::io(&output_dir, error))?;
        }
        fs::create_dir_all(&output_dir).map_err(|error| Error::io(&output_dir, error))?;

        DotnetRunner::with_runner(
            &self.options.dotnet,
            self.options
                .runner
                .clone()
                .unwrap_or_else(|| Arc::new(SystemRunner)),
        )
        .build(
            &csproj,
            &project_dir,
            &output_dir,
            &self.options.configuration,
        )?;

        let dll = output_dir.join(format!("{assembly_name}.dll"));
        if !dll.is_file() {
            return Err(Error::Message(format!(
                "dotnet build succeeded but expected DLL was not produced: {}",
                dll.display()
            )));
        }

        let destination = PathBuf::from("Assemblies").join(format!("{assembly_name}.dll"));
        staging.copy(&dll, destination)?;

        Ok(dll)
    }

    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::EmptyName);
        }

        let package_id = self.package_id.as_deref().ok_or(Error::MissingPackageId)?;
        validate_package_id(package_id)?;

        self.csharp.validate()?;
        self.patches.validate()?;

        if !self.options.game_probe
            && self.csharp.has_sources()
            && self.csharp.references().is_empty()
        {
            return Err(Error::GameProbeWithoutReferences);
        }

        if self.options.dry_run && self.csharp.has_sources() {
            return Err(Error::DryRunWithCsharpSources);
        }

        if self.supported_versions.is_empty() {
            return Err(Error::NoSupportedVersions);
        }
        for version in &self.supported_versions {
            if version != "1.6" {
                return Err(Error::UnsupportedVersion(version.clone()));
            }
        }

        Ok(())
    }

    fn about_xml(&self) -> Element {
        let mut root = Element::new("ModMetaData").child_text("name", &self.name);

        if let Some(author) = &self.author {
            root = root.child_text("author", author);
        }
        root = root.child_text(
            "packageId",
            self.package_id.as_deref().expect("validated package_id"),
        );
        if let Some(description) = &self.description {
            root = root.child_text("description", description);
        }
        if let Some(url) = &self.url {
            root = root.child_text("url", url);
        }

        let mut versions = Element::new("supportedVersions");
        let mut seen = Vec::new();
        for version in &self.supported_versions {
            if seen.iter().any(|existing| existing == version) {
                continue;
            }
            seen.push(version.clone());
            versions = versions.child_text("li", version);
        }
        root.child(versions)
    }
}

pub fn validate_package_id(package_id: &str) -> Result<()> {
    let valid = package_id.len() <= 60
        && package_id.contains('.')
        && !package_id.starts_with('.')
        && !package_id.ends_with('.')
        && !package_id.contains("..")
        && !package_id.to_ascii_lowercase().contains("ludeon")
        && package_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '.');

    if valid {
        Ok(())
    } else {
        Err(Error::InvalidPackageId(package_id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::thing::ThingDef;
    use std::path::Path;
    use std::process::{ExitStatus, Output};
    use std::sync::Mutex;

    /// Simulates the local `dotnet` CLI without the .NET SDK.
    ///
    /// Records every invocation and, when it sees `dotnet build -o <dir>`,
    /// reads the generated `.csproj` and writes the expected DLL into the
    /// output directory so the rest of the C# pipeline runs for real.
    #[derive(Clone)]
    struct FakeDotnet {
        calls: Arc<Mutex<Vec<String>>>,
        csproj_text: Arc<Mutex<Option<String>>>,
        assembly_name: String,
        fail: bool,
    }

    impl FakeDotnet {
        fn succeeding(assembly_name: impl Into<String>) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                csproj_text: Arc::new(Mutex::new(None)),
                assembly_name: assembly_name.into(),
                fail: false,
            }
        }

        fn failing() -> Self {
            let mut fake = Self::succeeding("TestMod");
            fake.fail = true;
            fake
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }

        fn csproj_text(&self) -> String {
            self.csproj_text
                .lock()
                .unwrap()
                .clone()
                .expect("dotnet build records the generated .csproj")
        }
    }

    impl CommandRunner for FakeDotnet {
        fn run(
            &self,
            program: &str,
            args: &[String],
            _working_dir: &Path,
        ) -> std::io::Result<Output> {
            assert_eq!(program, "dotnet");
            self.calls.lock().unwrap().push(args.join(" "));

            if self.fail {
                return Ok(Output {
                    status: failing_status(),
                    stdout: Vec::new(),
                    stderr: b"Mod.cs(1,1): error CS0103: the name 'Verse' does not exist\n"
                        .to_vec(),
                });
            }

            if args.first().is_some_and(|first| first == "build") {
                let csproj = PathBuf::from(&args[1]);
                *self.csproj_text.lock().unwrap() = Some(fs::read_to_string(&csproj).unwrap());

                let output_dir = args
                    .iter()
                    .position(|arg| arg == "-o")
                    .map(|index| args[index + 1].as_str())
                    .expect("dotnet build always passes -o");
                let dll = Path::new(&output_dir).join(format!("{}.dll", self.assembly_name));
                fs::write(&dll, b"fake dll").unwrap();
            }

            Ok(Output {
                status: succeeding_status(),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }

    fn succeeding_status() -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(0)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(0)
        }
    }

    fn failing_status() -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(101)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(101)
        }
    }

    #[test]
    fn accepts_valid_package_ids() {
        for id in ["me.mymod", "Author.ModSeries.Mod", "a.b"] {
            assert!(validate_package_id(id).is_ok(), "{id}");
        }
    }

    #[test]
    fn rejects_invalid_package_ids() {
        for id in [
            "",
            "nodot",
            ".start",
            "end.",
            "double..dot",
            "has space.a",
            "Ludeon.Mod",
            "a_b.c",
        ] {
            assert!(validate_package_id(id).is_err(), "{id}");
        }
        assert!(validate_package_id(&"a".repeat(61)).is_err());
    }

    #[test]
    fn builds_a_pure_xml_mod() {
        let dir = tempfile::tempdir().unwrap();
        let report = ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .author("me")
            .defs(|defs| {
                defs.add(ThingDef::new("TestThing").label("test thing"));
            })
            .options(BuildOptions::default().base_dir(dir.path()).deploy(false))
            .build()
            .unwrap();

        assert!(
            report
                .mod_dir
                .as_ref()
                .expect("staging is kept by default")
                .join("About/About.xml")
                .is_file()
        );
        assert!(
            report
                .mod_dir
                .as_ref()
                .unwrap()
                .join("Defs/Generated.xml")
                .is_file()
        );
        assert!(report.deployed_dir.is_none());
        assert!(report.zip_path.as_ref().unwrap().is_file());
        assert!(report.dll_path.is_none());
    }

    #[test]
    fn drops_staging_path_when_staging_is_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let stage = dir.path().join(".rim-rs/stage/Test Mod");

        let report = ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .options(
                BuildOptions::default()
                    .base_dir(dir.path())
                    .zip(false)
                    .keep_staging(false),
            )
            .build()
            .unwrap();

        assert!(report.mod_dir.is_none());
        assert!(!stage.exists());
    }

    #[test]
    fn rejects_versions_other_than_16() {
        let dir = tempfile::tempdir().unwrap();
        let error = ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .supported_versions(["1.7"])
            .options(BuildOptions::default().base_dir(dir.path()).zip(false))
            .build()
            .unwrap_err();

        assert!(
            matches!(error, Error::UnsupportedVersion(ref version) if version == "1.7"),
            "{error}"
        );
    }

    #[test]
    fn rejects_an_empty_supported_version_list() {
        let dir = tempfile::tempdir().unwrap();
        let error = ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .supported_versions(Vec::<String>::new())
            .options(BuildOptions::default().base_dir(dir.path()).zip(false))
            .build()
            .unwrap_err();

        assert!(matches!(error, Error::NoSupportedVersions), "{error}");
    }

    #[test]
    fn supported_versions_replaces_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let report = ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .supported_versions(["1.6"])
            .options(BuildOptions::default().base_dir(dir.path()).zip(false))
            .build()
            .unwrap();

        let about =
            fs::read_to_string(report.mod_dir.as_ref().unwrap().join("About/About.xml")).unwrap();
        assert_eq!(about.matches("<li>1.6</li>").count(), 1, "{about}");
    }

    #[test]
    fn deploys_to_an_explicit_mods_dir() {
        let dir = tempfile::tempdir().unwrap();
        let mods_dir = dir.path().join("Mods");

        let report = ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .defs(|defs| {
                defs.add(ThingDef::new("TestThing").label("test thing"));
            })
            .options(
                BuildOptions::default()
                    .base_dir(dir.path())
                    .mods_dir(&mods_dir)
                    .deploy(true),
            )
            .build()
            .unwrap();

        let deployed = mods_dir.join("Test Mod");
        assert_eq!(report.deployed_dir.as_deref(), Some(deployed.as_path()));
        assert!(deployed.join("About/About.xml").is_file());
        assert_eq!(
            fs::read(deployed.join("Defs/Generated.xml")).unwrap(),
            fs::read(report.mod_dir.as_ref().unwrap().join("Defs/Generated.xml")).unwrap()
        );
    }

    /// N-008: an explicitly referenced C# build must work without any game
    /// install as long as `game_probe` is disabled.
    #[test]
    fn builds_csharp_without_a_game_when_probe_is_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let libs = dir.path().join("libs");
        fs::create_dir_all(&libs).unwrap();
        let reference = libs.join("0Harmony.dll");
        fs::write(&reference, b"harmony stub").unwrap();

        let fake = FakeDotnet::succeeding("TestMod");
        let report = ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .csharp("Mod.cs", "class Mod {}")
            .csharp_reference(&reference)
            .options(
                BuildOptions::default()
                    .base_dir(dir.path())
                    .zip(false)
                    .game_probe(false)
                    .runner(Arc::new(fake.clone())),
            )
            .build()
            .unwrap();

        // restore + build ran through the injected fake, no probing happened.
        let calls = fake.calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert!(calls[0].starts_with("restore "), "{:?}", calls[0]);
        assert!(calls[1].starts_with("build "), "{:?}", calls[1]);

        // The generated project only references the explicitly supplied DLL.
        let csproj = fake.csproj_text();
        assert!(csproj.contains("0Harmony.dll"), "{csproj}");
        assert!(
            !csproj.contains("Assembly-CSharp"),
            "the game probe must be skipped: {csproj}"
        );

        // The DLL the fake produced lands in the staging Assemblies/ folder.
        let mod_dir = report.mod_dir.as_ref().unwrap();
        assert!(mod_dir.join("Assemblies/TestMod.dll").is_file());
        assert!(report.dll_path.as_ref().unwrap().is_file());
    }

    /// N-008: with the probe disabled, references are the caller's
    /// responsibility and an empty reference list is rejected up front.
    #[test]
    fn rejects_probe_disabled_without_explicit_references() {
        let dir = tempfile::tempdir().unwrap();
        let error = ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .csharp("Mod.cs", "class Mod {}")
            .options(
                BuildOptions::default()
                    .base_dir(dir.path())
                    .zip(false)
                    .game_probe(false),
            )
            .build()
            .unwrap_err();

        assert!(
            matches!(error, Error::GameProbeWithoutReferences),
            "{error}"
        );
    }

    /// N-008: the default must keep probing the game before any `dotnet` call.
    #[test]
    fn csharp_build_still_probes_the_game_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let error = ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .csharp("Mod.cs", "class Mod {}")
            .game_dir(dir.path().join("not-a-game"))
            .options(BuildOptions::default().base_dir(dir.path()).zip(false))
            .build()
            .unwrap_err();

        assert!(
            matches!(error, Error::NotRimWorld(_)),
            "the probe must stay enabled by default: {error}"
        );
    }

    /// An injected runner must fail loudly and carry the dotnet diagnostics.
    #[test]
    fn surfaces_injected_dotnet_failures() {
        let dir = tempfile::tempdir().unwrap();
        let libs = dir.path().join("libs");
        fs::create_dir_all(&libs).unwrap();
        let reference = libs.join("0Harmony.dll");
        fs::write(&reference, b"harmony stub").unwrap();

        let error = ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .csharp("Mod.cs", "class Mod {}")
            .csharp_reference(&reference)
            .options(
                BuildOptions::default()
                    .base_dir(dir.path())
                    .zip(false)
                    .game_probe(false)
                    .runner(Arc::new(FakeDotnet::failing())),
            )
            .build()
            .unwrap_err();

        assert!(
            matches!(error, Error::DotnetBuild(ref text) if text.contains("error CS0103")),
            "{error}"
        );
    }

    /// N-014: end-to-end C# compilation through the real toolchain.
    ///
    /// Opt-in like `game::tests::detects_an_env`: set `RIMWORLD_DIR` to a
    /// RimWorld 1.6 install and run with `--ignored` on a machine that has a
    /// local `dotnet` SDK.
    #[test]
    #[ignore = "requires RIMWORLD_DIR pointing at a RimWorld 1.6 install and a local dotnet SDK; run with --ignored"]
    fn builds_csharp_with_a_real_dotnet_and_game_link() {
        let root =
            std::env::var_os("RIMWORLD_DIR").expect("set RIMWORLD_DIR to a RimWorld 1.6 install");
        let dir = tempfile::tempdir().unwrap();

        const MOD_CODE: &str = r#"using Verse;

[StaticConstructorOnStartup]
public static class RimRsDotnetLinkTest
{
    static RimRsDotnetLinkTest() => Log.Message("[rim-rs] real dotnet link test");
}
"#;

        let report = ModBuilder::builder("RimRs Dotnet Link")
            .package_id("me.dotnetlink")
            .csharp("RimRsDotnetLink.cs", MOD_CODE)
            .game_dir(root)
            .options(BuildOptions::default().base_dir(dir.path()).zip(false))
            .build()
            .unwrap();

        let dll = report
            .dll_path
            .expect("a real dotnet build must produce a DLL");
        assert!(dll.is_file());
        assert!(
            report
                .mod_dir
                .as_ref()
                .unwrap()
                .join("Assemblies/RimRsDotnetLink.dll")
                .is_file()
        );
    }

    /// N-012: a dry run stages and validates but produces no side effects.
    #[test]
    fn dry_run_only_stages_and_validates() {
        let dir = tempfile::tempdir().unwrap();
        let mods_dir = dir.path().join("Mods");

        let report = ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .defs(|defs| {
                defs.add(ThingDef::new("TestThing").label("test thing"));
            })
            .options(
                BuildOptions::default()
                    .base_dir(dir.path())
                    .deploy(true)
                    .mods_dir(&mods_dir)
                    .dry_run(true),
            )
            .build()
            .unwrap();

        let mod_dir = report.mod_dir.as_ref().expect("staging is kept");
        assert!(mod_dir.join("About/About.xml").is_file());
        assert!(mod_dir.join("Defs/Generated.xml").is_file());

        // No side effects beyond staging: no zip, no deployment, no DLL.
        assert!(report.zip_path.is_none());
        assert!(report.deployed_dir.is_none());
        assert!(report.dll_path.is_none());
        assert!(!dir.path().join("dist").exists());
        assert!(!mods_dir.exists());
    }

    /// N-012: dry runs of mods with C# input are rejected up front instead of
    /// staging a mod without its DLL.
    #[test]
    fn rejects_a_dry_run_of_a_mod_with_csharp_sources() {
        let dir = tempfile::tempdir().unwrap();
        let error = ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .csharp("Mod.cs", "class Mod {}")
            .options(
                BuildOptions::default()
                    .base_dir(dir.path())
                    .zip(false)
                    .dry_run(true),
            )
            .build()
            .unwrap_err();

        assert!(matches!(error, Error::DryRunWithCsharpSources), "{error}");
        // Nothing was staged before the rejection.
        assert!(!dir.path().join(".rim-rs").exists());
    }

    /// N-009: consecutive builds of two mods on the same `base_dir` must not
    /// overwrite each other's C# sources, project files or DLL output.
    #[test]
    fn isolates_intermediate_dirs_per_mod() {
        let dir = tempfile::tempdir().unwrap();
        let libs = dir.path().join("libs");
        fs::create_dir_all(&libs).unwrap();
        let reference = libs.join("0Harmony.dll");
        fs::write(&reference, b"harmony stub").unwrap();

        let options_with = |runner: FakeDotnet| {
            BuildOptions::default()
                .base_dir(dir.path())
                .zip(false)
                .game_probe(false)
                .runner(Arc::new(runner))
        };

        ModBuilder::builder("Test Mod")
            .package_id("me.testmod")
            .csharp("Mod.cs", "class First {}")
            .csharp_reference(&reference)
            .options(options_with(FakeDotnet::succeeding("TestMod")))
            .build()
            .unwrap();

        ModBuilder::builder("Other Mod")
            .package_id("me.othermod")
            .csharp("Mod.cs", "class Second {}")
            .csharp_reference(&reference)
            .options(options_with(FakeDotnet::succeeding("OtherMod")))
            .build()
            .unwrap();

        let work_dir = dir.path().join(".rim-rs");
        // The first mod's intermediates survived the second build.
        assert!(work_dir.join("csharp/Test Mod/Mod.cs").is_file());
        assert!(work_dir.join("work/Test Mod/Mod.csproj").is_file());
        assert!(work_dir.join("out/Test Mod/TestMod.dll").is_file());
        // The second mod got its own isolated set.
        assert!(work_dir.join("csharp/Other Mod/Mod.cs").is_file());
        assert!(work_dir.join("work/Other Mod/Mod.csproj").is_file());
        assert!(work_dir.join("out/Other Mod/OtherMod.dll").is_file());
    }
}
