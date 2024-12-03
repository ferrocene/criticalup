// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::Error;
use crate::errors::Error::BinaryNotInstalled;
use crate::Context;
use criticalup_core::project_manifest::ProjectManifest;
use std::path::PathBuf;

pub(crate) async fn run(
    ctx: &Context,
    binary: String,
    project: Option<PathBuf>,
) -> Result<(), Error> {
    let found_binary = locate_binary(ctx, binary, project).await?;
    println!("{}\n", found_binary.display());

    Ok(())
}

#[tracing::instrument(level = "debug", skip_all, fields(binary, project))]
pub(crate) async fn locate_binary(
    ctx: &Context,
    binary: String,
    project: Option<PathBuf>,
) -> Result<PathBuf, Error> {
    let manifest = ProjectManifest::get(project).await?;

    let installation_dir = &ctx.config.paths.installation_dir;

    let mut found_binary = None;
    for product in manifest.products() {
        let abs_installation_dir_path = installation_dir.join(product.installation_id());

        let bin_path = PathBuf::from("bin");

        let mut tool_executable = PathBuf::new();
        tool_executable.set_file_name(&binary);

        let tools_bin_path = abs_installation_dir_path.join(bin_path.join(&tool_executable));

        if tools_bin_path.exists() {
            found_binary = Some(tools_bin_path);
        } else {
            // On Windows, the user can pass (for example) `cargo` or `cargo.exe`
            #[cfg(windows)]
            {
                let mut tools_bin_path_with_exe = tools_bin_path.clone();
                tools_bin_path_with_exe.set_extension("exe");
                if tools_bin_path_with_exe.exists() {
                    found_binary = Some(tools_bin_path_with_exe);
                }
            }
        }
    }

    if let Some(found_binary) = found_binary {
        Ok(found_binary)
    } else {
        Err(BinaryNotInstalled(binary))
    }
}
