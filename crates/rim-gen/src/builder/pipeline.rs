//! The build pipeline: staging, optional C# compilation, zipping, deployment.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::csharp::CSharpSet;
use crate::dotnet::{DotnetRunner, SystemRunner};
use crate::error::{Error, Result};
use crate::game::GameInstall;
use crate::package::{self, Staging};
use crate::project::{self, ProjectSpec};

use super::{BuildReport, ModBuilder};

impl ModBuilder {
    /// Builds the mod.
    ///
    /// On failure, the staging directory created for this build is removed so
    /// that no partial (unusable) mod directory is left behind
    /// (`Error::ModDirOccupied` is raised before anything is deleted when an
    /// overwrite target belongs to a different mod).
    pub fn build(self) -> Result<BuildReport> {
        self.validate()?;

        let mod_folder = project::sanitize_dir_name(&self.name);
        let mod_dir = self.options.work_dir.join("stage").join(&mod_folder);
        let package_id = self.package_id.as_deref().expect("validated package_id");

        if mod_dir.exists() {
            // `remove_dir_all` is a wholesale replace; refuse when a different
            // mod owns the directory (e.g. a case-variant name on a
            // case-insensitive filesystem).
            package::ensure_can_replace(&mod_dir, &self.name, package_id)?;
            fs::remove_dir_all(&mod_dir).map_err(|error| Error::io(&mod_dir, error))?;
        }
        fs::create_dir_all(&mod_dir).map_err(|error| Error::io(&mod_dir, error))?;

        let mut staging = Staging::new(&mod_dir)?;

        match self.assemble(&mut staging, &mod_folder) {
            Ok((zip_path, deployed_dir, dll_path)) => {
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
            Err(error) => {
                // A failed build never leaves a partial mod directory behind,
                // which could be mistaken for a loadable mod. Intermediate
                // directories (`csharp/`, `work/`, `out/`) are kept for
                // debugging the failure.
                let _ = fs::remove_dir_all(&mod_dir);
                Err(error)
            }
        }
    }

    /// Stages every input and, unless `dry_run` is set, compiles C#, zips and
    /// deploys. Returns `(zip_path, deployed_dir, dll_path)`.
    fn assemble(
        &self,
        staging: &mut Staging,
        mod_folder: &str,
    ) -> Result<(Option<PathBuf>, Option<PathBuf>, Option<PathBuf>)> {
        let package_id = self.package_id.as_deref().expect("validated package_id");

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
            Some(self.build_csharp(&self.csharp, staging, mod_folder)?)
        } else {
            None
        };

        package::validate_mod_dir(staging.path())?;

        let zip_path = if self.options.zip && !self.options.dry_run {
            fs::create_dir_all(&self.options.dist_dir)
                .map_err(|error| Error::io(&self.options.dist_dir, error))?;
            let zip_path = self.options.dist_dir.join(format!("{mod_folder}.zip"));
            package::zip_dir(staging.path(), &zip_path)?;
            Some(zip_path)
        } else {
            None
        };

        let deployed_dir = if self.options.deploy && !self.options.dry_run {
            let mods_dir = match &self.options.mods_dir {
                Some(directory) => directory.clone(),
                None => GameInstall::locate(self.game_dir.as_deref())?.mods_dir(),
            };
            let destination = mods_dir.join(mod_folder);
            package::ensure_can_replace(&destination, &self.name, package_id)?;
            package::copy_dir_all(staging.path(), &destination)?;
            Some(destination)
        } else {
            None
        };

        Ok((zip_path, deployed_dir, dll_path))
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
            return Err(Error::DllNotProduced(dll));
        }

        let destination = PathBuf::from("Assemblies").join(format!("{assembly_name}.dll"));
        staging.copy(&dll, destination)?;

        Ok(dll)
    }
}
