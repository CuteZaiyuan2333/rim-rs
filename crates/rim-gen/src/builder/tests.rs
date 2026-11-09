//! End-to-end builder tests. The fake `dotnet` runner lets the full C#
//! pipeline (csproj -> dotnet -> DLL -> staging) run without a .NET SDK.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::sync::{Arc, Mutex};

use super::*;
use crate::defs::thing::ThingDef;
use crate::dotnet::CommandRunner;

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
    fn run(&self, program: &str, args: &[String], _working_dir: &Path) -> std::io::Result<Output> {
        assert_eq!(program, "dotnet");
        self.calls.lock().unwrap().push(args.join(" "));

        if self.fail {
            return Ok(Output {
                status: failing_status(),
                stdout: Vec::new(),
                stderr: b"Mod.cs(1,1): error CS0103: the name 'Verse' does not exist\n".to_vec(),
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

/// 新-1: the staging directory of a same-named mod must not be silently
/// replaced when the existing `About.xml` belongs to another mod
/// (case-variant names land in the same directory on Windows/macOS).
#[test]
fn refuses_to_replace_a_stage_dir_of_a_different_mod() {
    let dir = tempfile::tempdir().unwrap();
    let stage = dir.path().join(".rim-rs/stage/Test Mod");
    fs::create_dir_all(stage.join("About")).unwrap();
    fs::write(
        stage.join("About/About.xml"),
        "<ModMetaData><name>Test Mod</name><packageId>me.othermod</packageId></ModMetaData>",
    )
    .unwrap();

    let error = ModBuilder::builder("Test Mod")
        .package_id("me.testmod")
        .options(BuildOptions::default().base_dir(dir.path()).zip(false))
        .build()
        .unwrap_err();

    assert!(matches!(error, Error::ModDirOccupied { .. }), "{error}");
    // The foreign mod directory was left untouched.
    assert!(
        fs::read_to_string(stage.join("About/About.xml"))
            .unwrap()
            .contains("me.othermod")
    );
}

/// 新-2: a failed build must not leave a partial staging directory behind
/// that could be mistaken for a loadable mod. Intermediates stay.
#[test]
fn removes_partial_staging_when_the_csharp_stage_fails() {
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

    assert!(matches!(error, Error::DotnetBuild(..)), "{error}");
    assert!(
        !dir.path().join(".rim-rs/stage/Test Mod").exists(),
        "no partial staging may survive a failed build"
    );
    // Intermediates are kept for debugging.
    assert!(dir.path().join(".rim-rs/csharp/Test Mod/Mod.cs").is_file());
    assert!(
        dir.path()
            .join(".rim-rs/work/Test Mod/Mod.csproj")
            .is_file()
    );
}

/// 新-1: the same guard applies to the deploy destination, where only an
/// About.xml shows which mod owns a directory.
#[test]
fn refuses_to_deploy_over_a_foreign_mod_directory() {
    let dir = tempfile::tempdir().unwrap();
    let mods_dir = dir.path().join("Mods");
    let deployed = mods_dir.join("Test Mod");
    fs::create_dir_all(deployed.join("About")).unwrap();
    fs::write(
        deployed.join("About/About.xml"),
        "<ModMetaData><name>Test Mod</name><packageId>me.othermod</packageId></ModMetaData>",
    )
    .unwrap();

    let error = ModBuilder::builder("Test Mod")
        .package_id("me.testmod")
        .defs(|defs| {
            defs.add(ThingDef::new("TestThing").label("test thing"));
        })
        .options(
            BuildOptions::default()
                .base_dir(dir.path())
                .zip(false)
                .deploy(true)
                .mods_dir(&mods_dir),
        )
        .build()
        .unwrap_err();

    assert!(matches!(error, Error::ModDirOccupied { .. }), "{error}");
    // One About.xml later, the other mod's file is still the one there.
    assert!(
        fs::read_to_string(deployed.join("About/About.xml"))
            .unwrap()
            .contains("me.othermod")
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

#[test]
fn about_xml_declares_dependencies_and_load_order() {
    let manager = ModBuilder::builder("Test Mod")
        .package_id("me.testmod")
        .dependency(
            ModDependency::new("miquella.tamahagane")
                .display_name("Tamahagane Expansion")
                .steam_workshop_url("https://steamcommunity.com/workshop/filedetails/?id=1"),
        )
        .dependency(ModDependency::new("me.evenmore").display_name("Even More"))
        .load_after("brrainz.harmony")
        .load_after("miquella.tamahagane");
    let about = manager.about_xml().to_xml_string().unwrap();
    let about = about.replace(['\n', '\t'], "");

    // Only the dependency with a download link survives into
    // modDependencies: vanilla drops url-less entries (with a warning).
    assert!(about.contains(
        "<modDependencies><li><packageId>miquella.tamahagane</packageId>\
         <displayName>Tamahagane Expansion</displayName>"
    ));
    assert!(about.contains("</steamWorkshopUrl>"));
    assert!(!about.contains("me.evenmore</packageId>"));

    // Dependency ids order first, then caller entries; no duplicates.
    // The url-less dependency degrades to a loadAfter-only entry.
    assert!(about.contains(
        "<loadAfter><li>miquella.tamahagane</li><li>me.evenmore</li><li>brrainz.harmony</li>"
    ));
}

#[test]
fn download_url_also_declares_the_dependency() {
    let about = ModBuilder::builder("Test Mod")
        .package_id("me.testmod")
        .dependency(
            ModDependency::new("someone.localmod")
                .display_name("Local Mod")
                .download_url("https://github.com/someone/localmod/releases"),
        )
        .about_xml()
        .to_xml_string()
        .unwrap();
    let about = about.replace(['\n', '\t'], "");

    assert!(about.contains(
        "<modDependencies><li><packageId>someone.localmod</packageId>\
         <displayName>Local Mod</displayName>\
         <downloadUrl>https://github.com/someone/localmod/releases</downloadUrl>"
    ));
}

#[test]
fn rejects_an_invalid_dependency_package_id() {
    let about = ModBuilder::builder("Test Mod")
        .package_id("me.testmod")
        .load_after("not a package id");
    let error = about.validate().unwrap_err();
    assert!(matches!(error, Error::InvalidPackageId(_)), "{error}");
}

#[test]
fn rejects_a_material_carry_without_intermediates() {
    let about = ModBuilder::builder("Test Mod")
        .package_id("me.testmod")
        .harmony_material_carry(MaterialCarry::new("me.testmod"));
    let error = about.validate().unwrap_err();
    assert!(
        matches!(error, Error::MaterialCarryWithoutIntermediates),
        "{error}"
    );
}

/// N-: the registered MaterialCarry is staged and compiled like any other
/// C# source, so the material-chain hook ships without hand-written C#.
#[test]
fn stages_the_generated_material_carry_source() {
    let dir = tempfile::tempdir().unwrap();
    let libs = dir.path().join("libs");
    fs::create_dir_all(&libs).unwrap();
    let reference = libs.join("0Harmony.dll");
    fs::write(&reference, b"harmony stub").unwrap();

    let fake = FakeDotnet::succeeding("TestMod");
    let report = ModBuilder::builder("Test Mod")
        .package_id("me.testmod")
        .harmony_material_carry(
            MaterialCarry::new("me.testmod")
                .namespace("TestMod")
                .intermediate("Ham_Billet"),
        )
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

    let source = fs::read_to_string(
        dir.path()
            .join(".rim-rs/csharp/Test Mod/MaterialCarry.cs"),
    )
    .unwrap();
    assert!(source.contains("namespace TestMod\n{"));
    assert!(source.contains("CalculateDominantIngredient"));
    assert!(source.contains("\"Ham_Billet\""));

    // The compile list includes the generated file.
    let csproj = fake.csproj_text();
    assert!(csproj.contains("MaterialCarry.cs"), "{csproj}");
    // The compiled DLL landed in the staged mod directory as usual.
    let staging = report.mod_dir.as_ref().expect("staging is kept by default");
    assert!(staging.join("Assemblies/TestMod.dll").is_file());
}
