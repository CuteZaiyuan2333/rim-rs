use std::path::Path;
use std::process::Command;

use crate::error::{Error, Result};

/// Runs the local `dotnet` CLI.
#[derive(Debug, Clone)]
pub struct DotnetRunner {
    program: String,
}

impl DotnetRunner {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
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
        let output = Command::new(&self.program)
            .args(args)
            .current_dir(working_dir)
            .output()
            .map_err(|source| Error::DotnetSpawn {
                program: self.program.clone(),
                source,
            })?;

        if output.status.success() {
            return Ok(());
        }

        let mut message = String::new();
        message.push_str(&format!(
            "dotnet {} failed with status {}\n",
            args.join(" "),
            output.status
        ));
        message.push_str("--- stdout ---\n");
        message.push_str(&String::from_utf8_lossy(&output.stdout));
        message.push_str("\n--- stderr ---\n");
        message.push_str(&String::from_utf8_lossy(&output.stderr));

        Err(Error::DotnetBuild(message))
    }
}
