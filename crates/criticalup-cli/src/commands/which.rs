// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::Error;
use crate::errors::Error::BinaryNotInstalled;
use crate::Context;
use criticalup_core::project_manifest::ProjectManifest;
use std::path::PathBuf;

pub(crate) async fn run(ctx: &Context, tool: String, project: Option<PathBuf>) -> Result<(), Error> {
    let manifest = ProjectManifest::get(project).await?;

    let installation_dir = &ctx.config.paths.installation_dir;

    for product in manifest.products() {
        let abs_installation_dir_path = installation_dir.join(product.installation_id());

        let bin_path = PathBuf::from("bin");

        let mut tool_executable = PathBuf::new();
        tool_executable.set_file_name(&tool);

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
                    return Err(BinaryNotInstalled(tool));
                }
            }
            #[cfg(not(windows))]
            return Err(BinaryNotInstalled(tool));
        }
    }

    Ok(())
}
