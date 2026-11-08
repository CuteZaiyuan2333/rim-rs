//! The root builder ([`ModBuilder`]) and its validation.

use std::path::PathBuf;

use crate::assets::AssetSet;
use crate::csharp::CSharpSet;
use crate::defs::DefSet;
use crate::error::{Error, Result};
use crate::harmony::MaterialCarry;
use crate::package::validate_package_id;
use crate::patch::PatchSet;
use crate::xml::Element;

mod options;
mod pipeline;
mod report;
#[cfg(test)]
mod tests;

pub use options::BuildOptions;
pub use report::BuildReport;

/// The root builder.
///
/// ```no_run
/// use rim_gen::prelude::*;
///
/// fn main() -> rim_gen::Result<()> {
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
    dependencies: Vec<ModDependency>,
    load_after: Vec<String>,
    material_carry: Option<MaterialCarry>,
    game_dir: Option<PathBuf>,
    defs: DefSet,
    patches: PatchSet,
    csharp: CSharpSet,
    assets: AssetSet,
    options: BuildOptions,
}

/// An entry of `About.xml`'s `<modDependencies>` (and `<loadAfter>`).
///
/// Every registered dependency also guarantees load ordering
/// (`loadAfter <packageId>`), matching what RimWorld loads from `About/`.
#[derive(Debug, Clone)]
pub struct ModDependency {
    package_id: String,
    display_name: Option<String>,
    steam_workshop_url: Option<String>,
}

impl ModDependency {
    pub fn new(package_id: impl Into<String>) -> Self {
        Self {
            package_id: package_id.into(),
            display_name: None,
            steam_workshop_url: None,
        }
    }

    /// Sets `<displayName>`, the human-readable name shown in the load order
    /// message.
    pub fn display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    /// Sets `<steamWorkshopUrl>` (Steam distribution only).
    pub fn steam_workshop_url(mut self, url: impl Into<String>) -> Self {
        self.steam_workshop_url = Some(url.into());
        self
    }

    pub(crate) fn package_id(&self) -> &str {
        &self.package_id
    }
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
            dependencies: Vec::new(),
            load_after: Vec::new(),
            material_carry: None,
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
    /// `rim-gen` only supports RimWorld 1.6; any entry other than exactly `1.6`
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

    /// Declares a cross-mod dependency: it writes a `<modDependencies>` entry
    /// in `About.xml` and the matching `loadAfter` entry, replacing the
    /// hand-written `About/Dependencies.xml` pattern.
    ///
    /// The package id must be a valid RimWorld package id
    /// (e.g. `miquella.tamahagane`).
    pub fn dependency(mut self, dependency: ModDependency) -> Self {
        self.dependencies.push(dependency);
        self
    }

    /// Adds a bare `loadAfter` entry without a `modDependencies` entry; use
    /// for ordering against mods whose load is required but not game-reported
    /// (e.g. Harmony itself).
    pub fn load_after(mut self, package_id: impl Into<String>) -> Self {
        self.load_after.push(package_id.into());
        self
    }

    /// Generates a C# Harmony prefix that keeps the material chain alive for
    /// recipes consuming the registered MadeFromStuff intermediates.
    ///
    /// Equivalent to calling `.csharp(<name>, <generated source>)`; still add
    /// the Harmony assembly via `.csharp_reference(...)` and load order via
    /// `.load_after("brrainz.harmony")`.
    pub fn harmony_material_carry(mut self, carry: MaterialCarry) -> Self {
        self.material_carry = Some(carry);
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

    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::EmptyName);
        }

        let package_id = self.package_id.as_deref().ok_or(Error::MissingPackageId)?;
        validate_package_id(package_id)?;

        for dependency in &self.dependencies {
            validate_package_id(dependency.package_id())?;
        }
        for package_id in &self.load_after {
            validate_package_id(package_id)?;
        }
        if let Some(material_carry) = &self.material_carry
            && material_carry.intermediates().is_empty()
        {
            return Err(Error::MaterialCarryWithoutIntermediates);
        }

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
        root = root.child(versions);

        if !self.dependencies.is_empty() {
            let mut dependencies = Element::new("modDependencies");
            for dependency in &self.dependencies {
                let mut li = Element::new("li").child_text("packageId", dependency.package_id());
                if let Some(display_name) = &dependency.display_name {
                    li = li.child_text("displayName", display_name);
                }
                if let Some(steam_workshop_url) = &dependency.steam_workshop_url {
                    li = li.child_text("steamWorkshopUrl", steam_workshop_url);
                }
                dependencies = dependencies.child(li);
            }
            root = root.child(dependencies);
        }

        // Every dependency also orders after itself; caller-added entries
        // keep their registration order and duplicates are skipped.
        let mut ordered_after: Vec<String> = Vec::new();
        for package_id in self
            .dependencies
            .iter()
            .map(|dependency| dependency.package_id().to_string())
            .chain(self.load_after.iter().cloned())
        {
            if !ordered_after.contains(&package_id) {
                ordered_after.push(package_id);
            }
        }
        if !ordered_after.is_empty() {
            let mut list = Element::new("loadAfter");
            for package_id in &ordered_after {
                list = list.child_text("li", package_id);
            }
            root = root.child(list);
        }

        root
    }
}
