use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;

use crate::error::{Error, Result};

/// Executes external commands.
///
/// The default implementation spawns a real process; tests can inject a fake
/// to assert arguments and to simulate failing builds.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[String], working_dir: &Path) -> std::io::Result<Output>;
}

/// The default [`CommandRunner`]: executes the command as a real process.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[String], working_dir: &Path) -> std::io::Result<Output> {
        Command::new(program)
            .args(args)
            .current_dir(working_dir)
            .output()
    }
}

/// Lets a shared runner be stored in build options and used by reference.
impl<T: CommandRunner + ?Sized> CommandRunner for Arc<T> {
    fn run(&self, program: &str, args: &[String], working_dir: &Path) -> std::io::Result<Output> {
        (**self).run(program, args, working_dir)
    }
}

/// Runs the local `dotnet` CLI.
///
/// ```no_run
/// use rim_gen::dotnet::DotnetRunner;
/// use std::path::Path;
///
/// DotnetRunner::new("dotnet").build(
///     Path::new(".rim-rs/work/Mod.csproj"),
///     Path::new(".rim-rs/work"),
///     Path::new(".rim-rs/out"),
///     "Release",
/// )?;
/// # Ok::<(), rim_gen::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct DotnetRunner<R: CommandRunner = SystemRunner> {
    program: String,
    runner: R,
}

impl DotnetRunner<SystemRunner> {
    pub fn new(program: impl Into<String>) -> Self {
        Self::with_runner(program, SystemRunner)
    }
}

impl<R: CommandRunner> DotnetRunner<R> {
    pub fn with_runner(program: impl Into<String>, runner: R) -> Self {
        Self {
            program: program.into(),
            runner,
        }
    }

    /// Runs `dotnet restore` and `dotnet build --no-restore`.
    pub fn build(
        &self,
        csproj: &Path,
        working_dir: &Path,
        output_dir: &Path,
        configuration: &str,
    ) -> Result<()> {
        self.run(
            &[
                "restore".to_string(),
                csproj.display().to_string(),
                "--nologo".to_string(),
                "-v:minimal".to_string(),
            ],
            working_dir,
        )?;

        self.run(
            &[
                "build".to_string(),
                csproj.display().to_string(),
                "--no-restore".to_string(),
                "-c".to_string(),
                configuration.to_string(),
                "-o".to_string(),
                output_dir.display().to_string(),
                "--nologo".to_string(),
                "-v:minimal".to_string(),
                "-p:ContinuousIntegrationBuild=true".to_string(),
            ],
            working_dir,
        )
    }

    fn run(&self, args: &[String], working_dir: &Path) -> Result<()> {
        let output = self
            .runner
            .run(&self.program, args, working_dir)
            .map_err(|source| Error::DotnetSpawn {
                program: self.program.clone(),
                source,
            })?;

        if output.status.success() {
            return Ok(());
        }

        Err(Error::DotnetBuild(failure_message(
            &self.program,
            args,
            &output,
        )))
    }
}

/// Builds the failure text: a diagnostic summary first, raw output after.
fn failure_message(program: &str, args: &[String], output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let mut message = format!(
        "{program} {} failed with status {}\n",
        args.join(" "),
        output.status
    );

    let diagnostics = diagnostic_lines(&stdout, &stderr);
    if diagnostics.is_empty() {
        message.push_str("no `error CSxxxx` / `error MSBxxxx` diagnostics found\n");
    } else {
        message.push_str("--- diagnostics ---\n");
        for line in diagnostics {
            message.push_str(&line);
            message.push('\n');
        }
    }

    message.push_str("--- stdout ---\n");
    message.push_str(&stdout);
    message.push_str("\n--- stderr ---\n");
    message.push_str(&stderr);

    message
}

/// Extracts unique `error CSxxx` / `error MSBxxx` lines (file/line text kept).
fn diagnostic_lines(stdout: &str, stderr: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in stdout.lines().chain(stderr.lines()) {
        if is_diagnostic_line(line) && !found.iter().any(|existing| existing == line) {
            found.push(line.to_string());
        }
    }
    found
}

fn is_diagnostic_line(line: &str) -> bool {
    let Some((_, rest)) = line.split_once("error ") else {
        return false;
    };
    let token: String = rest
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric())
        .collect();

    let split = token
        .char_indices()
        .find(|(_, character)| character.is_ascii_digit())
        .map(|(index, _)| index);
    let Some(split) = split else {
        return false;
    };

    let (family, digits) = token.split_at(split);
    matches!(family, "CS" | "MSB") && !digits.is_empty()
}

#[cfg(test)]
mod tests {
    use std::process::ExitStatus;
    use std::sync::{Arc, Mutex};

    use super::*;

    fn status(code: u32) -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(code as i32)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(code)
        }
    }

    #[derive(Clone)]
    struct RecordedCall {
        args: Vec<String>,
        working_dir: String,
    }

    #[derive(Clone)]
    struct FakeRunner {
        calls: Arc<Mutex<Vec<RecordedCall>>>,
        output: Arc<Output>,
    }

    impl FakeRunner {
        fn succeeding() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                output: Arc::new(Output {
                    status: status(0),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                }),
            }
        }

        fn failing(stdout: &str, stderr: &str) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                output: Arc::new(Output {
                    status: status(101),
                    stdout: stdout.as_bytes().to_vec(),
                    stderr: stderr.as_bytes().to_vec(),
                }),
            }
        }

        fn calls(&self) -> Vec<RecordedCall> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(
            &self,
            program: &str,
            args: &[String],
            working_dir: &Path,
        ) -> std::io::Result<Output> {
            if program != "dotnet" {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "unexpected program",
                ));
            }
            self.calls.lock().unwrap().push(RecordedCall {
                args: args.to_vec(),
                working_dir: working_dir.display().to_string(),
            });
            Ok((*self.output).clone())
        }
    }

    fn runner(fake: FakeRunner) -> DotnetRunner<FakeRunner> {
        DotnetRunner::with_runner("dotnet", fake)
    }

    #[test]
    fn issues_restore_then_build_with_expected_args() {
        let fake = FakeRunner::succeeding();
        runner(fake.clone())
            .build(
                Path::new("/tmp/rimrs/work/Mod.csproj"),
                Path::new("/tmp/rimrs/work"),
                Path::new("/tmp/rimrs/out"),
                "Release",
            )
            .unwrap();

        let calls = fake.calls();
        assert_eq!(calls.len(), 2);
        assert!(
            calls
                .iter()
                .all(|call| call.working_dir == "/tmp/rimrs/work")
        );

        let restore = &calls[0].args;
        assert_eq!(restore[0], "restore");
        assert_eq!(restore[1], "/tmp/rimrs/work/Mod.csproj");

        let build = &calls[1].args;
        assert_eq!(build[0], "build");
        assert_eq!(build[1], "/tmp/rimrs/work/Mod.csproj");
        for arg in ["--no-restore", "-c", "Release", "-o", "--nologo"] {
            assert!(build.iter().any(|candidate| candidate == arg), "{arg}");
        }
        assert!(
            build
                .iter()
                .any(|candidate| candidate == "-p:ContinuousIntegrationBuild=true")
        );
    }

    #[test]
    fn surfaces_diagnostics_on_failure() {
        let fake = FakeRunner::failing(
            "Compile: 1 error(s)\n  Program.cs(10,5): error CS0103: The name 'x' does not exist\n",
            "warning: some noise\nerror MSB4321: build problem\n",
        );

        let error = runner(fake)
            .build(
                Path::new("/p/Mod.csproj"),
                Path::new("/p"),
                Path::new("/o"),
                "Debug",
            )
            .unwrap_err();

        let text = error.to_string();
        assert!(text.contains("error CS0103"), "{text}");
        assert!(text.contains("error MSB4321"), "{text}");
        assert!(text.contains("failed with status"), "{text}");
        assert!(text.contains("--- stdout ---"), "{text}");
        assert!(text.contains("--- stderr ---"), "{text}");
    }

    #[test]
    fn keeps_full_output_when_no_diagnostics() {
        let fake = FakeRunner::failing("something went wrong without a code\n", "");

        let error = runner(fake)
            .build(
                Path::new("/p/Mod.csproj"),
                Path::new("/p"),
                Path::new("/o"),
                "Debug",
            )
            .unwrap_err();

        let text = error.to_string();
        assert!(
            text.contains("no `error CSxxxx` / `error MSBxxxx` diagnostics found"),
            "{text}"
        );
        assert!(
            text.contains("something went wrong without a code"),
            "{text}"
        );
    }

    #[test]
    fn classifies_diagnostic_lines() {
        assert!(is_diagnostic_line("Program.cs(1,1): error CS0001: boom"));
        assert!(is_diagnostic_line("error MSB3021: copy failed"));
        assert!(!is_diagnostic_line("error XY0001: not a known family"));
        assert!(!is_diagnostic_line("error CS: no code"));
        assert!(!is_diagnostic_line("plain failure"));
    }
}
