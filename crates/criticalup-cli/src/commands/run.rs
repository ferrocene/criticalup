// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::Error;
use crate::errors::Error::BinaryNotInstalled;
use crate::spawn::spawn_command;
use crate::Context;
use criticalup_core::project_manifest::ProjectManifest;
use std::path::PathBuf;
// We *deliberately* use a sync Command here, since we are spawning a process to replace the current one.
use std::process::{Command, Stdio};

pub(crate) async fn run(
    ctx: &Context,
    command: Vec<String>,
    project: Option<PathBuf>,
) -> Result<(), Error> {
    // We try to fetch the manifest early on because it makes failing fast easy. Given that we need
    // this variable to set the env var later for child process, it is important to try to get the
    // canonical path first.
    let manifest_path = ProjectManifest::discover_canonical_path(project.as_deref()).await?;

    // This dir has all the binaries that are proxied.
    let proxies_dir = &ctx.config.paths.proxies_dir;

    if let Some(binary_command) = command.first() {
        let mut binary_executable = PathBuf::new();
        binary_executable.set_file_name(binary_command);
        // On Windows, the user can pass (for example) `cargo` or `cargo.exe`
        #[cfg(windows)]
        binary_executable.set_extension("exe");

        let binary_path = proxies_dir.join(binary_executable);

        if binary_path.exists() {
            let args = command.get(1..).unwrap_or(&[]);
            let mut cmd = Command::new(binary_path);
            cmd.args(args)
                .stdout(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());

            // Set the manifest path env CRITICALUP_CURRENT_PROJ_MANIFEST_CANONICAL_PATH var which is used
            // by the function `crates::criticalup-cli::binary_proxies::proxy` to find the correct project
            // manifest.
            //
            // Important: This env var is strictly for internal use!
            if manifest_path.exists() {
                cmd.env(
                    "CRITICALUP_CURRENT_PROJ_MANIFEST_CANONICAL_PATH",
                    manifest_path.as_os_str(),
                );
            }

            spawn_command(cmd)?;
        } else {
            return Err(BinaryNotInstalled(binary_command.into()));
        }
    }

    Ok(())
}
