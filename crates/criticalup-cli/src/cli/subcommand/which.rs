// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::cli::CommandExecute;
use crate::errors::Error;
use crate::errors::Error::BinaryNotInstalled;
use crate::Context;
use clap::Parser;
use criticalup_core::project_manifest::ProjectManifest;
use std::env::current_dir;
use std::path::PathBuf;
use tracing::Span;

/// Display which binary will be run for a given command
#[derive(Debug, Parser)]
pub(crate) struct Which {
    /// Name of the binary to find the absolute path of
    command: String,
    /// Path to the manifest `criticalup.toml`
    #[arg(long)]
    project: Option<PathBuf>,
}

impl CommandExecute for Which {
    #[tracing::instrument(level = "debug", skip_all, fields(project,))]
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        let span = Span::current();
        let project = if let Some(project) = self.project {
            project.clone()
        } else {
            ProjectManifest::discover(&current_dir()?, None)?
        };
        span.record("project", tracing::field::display(project.display()));
        let project_manifest = ProjectManifest::load(&project)?;

        let installation_dir = &ctx.config.paths.installation_dir;

        for product in project_manifest.products() {
            let abs_installation_dir_path = installation_dir.join(product.installation_id());

            let bin_path = PathBuf::from("bin");

            let mut tool_executable = PathBuf::new();
            tool_executable.set_file_name(&self.command);

            let tools_bin_path = abs_installation_dir_path.join(bin_path.join(&tool_executable));

            if tools_bin_path.exists() {
                println!("{}\n", tools_bin_path.display());
            } else {
                // On Windows, the user can pass (for example) `cargo` or `cargo.exe`
                #[cfg(windows)]
                {
                    let mut tools_bin_path_with_exe = tools_bin_path.clone();
                    tools_bin_path_with_exe.set_extension("exe");
                    if tools_bin_path_with_exe.exists() {
                        println!("{}\n", tools_bin_path_with_exe.display());
                    } else {
                        return Err(BinaryNotInstalled(self.command));
                    }
                }
                #[cfg(not(windows))]
                return Err(BinaryNotInstalled(self.command));
            }
        }

        Ok(())
    }
}
