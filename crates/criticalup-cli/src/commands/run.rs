// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use criticalup_core::project_manifest::ProjectManifest;

use crate::binary_proxies::prepend_path_to_var_for_command;
use crate::errors::Error;
use crate::spawn::spawn_command;
use crate::Context;
use std::path::PathBuf;
// We *deliberately* use a sync Command here, since we are spawning a process to replace the current one.
use std::process::{Command, Stdio};

pub(crate) async fn run(
    ctx: &Context,
    user_command: Vec<String>,
    project: Option<PathBuf>,
) -> Result<(), Error> {
    let installations = locate_installations(ctx, project).await?;
    let mut bin_paths = vec![];
    let mut lib_paths = vec![];
    for installation in installations {
        let bin_dir = installation.join("bin");
        if bin_dir.exists() {
            bin_paths.push(bin_dir);
        }
        let lib_dir = installation.join("lib");
        if lib_dir.exists() {
            lib_paths.push(lib_dir)
        }
    }

    let binary = user_command
        .first()
        .ok_or(Error::BinaryNotInstalled(String::new()))?;
    let args = user_command.get(1..).unwrap_or(&[]);
    let mut command = Command::new(binary);

    command
        .args(args)
        .stdout(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    // For some particularly niche use cases, users may find themselves wanting
    // to override the `rustc` called, and they may want to do that by setting
    // `PATH` themselves, but they:
    // 1) Shouldn't do that, and
    // 2) Can set `RUSTC` which `cargo` already supports.

    #[cfg(target_os = "macos")]
    prepend_path_to_var_for_command(&mut command, "DYLD_FALLBACK_LIBRARY_PATH", lib_paths)?;
    #[cfg(target_os = "linux")]
    prepend_path_to_var_for_command(&mut command, "LD_LIBRARY_PATH", lib_paths)?;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    prepend_path_to_var_for_command(&mut command, "PATH", bin_paths)?;
    #[cfg(target_os = "windows")]
    prepend_path_to_var_for_command(&mut command, "PATH", [lib_paths, bin_paths].concat())?;

    spawn_command(command)?;

    Ok(())
}

#[tracing::instrument(level = "debug", skip_all, fields(binary, project))]
pub(crate) async fn locate_installations(
    ctx: &Context,
    project: Option<PathBuf>,
) -> Result<Vec<PathBuf>, Error> {
    let manifest = ProjectManifest::get(project).await?;

    let installation_dir = &ctx.config.paths.installation_dir;

    let mut found_installation_dirs = vec![];
    for product in manifest.products() {
        let abs_installation_dir_path = installation_dir.join(product.installation_id());
        found_installation_dirs.push(abs_installation_dir_path);
    }

    Ok(found_installation_dirs)
}
