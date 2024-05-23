// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::Error;
use crate::errors::Error::BinaryNotInstalled;
use crate::Context;
use criticalup_core::project_manifest::ProjectManifest;
use std::path::PathBuf;

pub(crate) fn run(ctx: &Context, tool: String, project: Option<PathBuf>) -> Result<(), Error> {
    let manifest = ProjectManifest::get(project)?;

    let installation_dir = &ctx.config.paths.installation_dir;

    for product in manifest.products() {
        let abs_installation_dir_path = installation_dir.join(product.installation_id());
        let tools_bin_path = abs_installation_dir_path.join(format!("bin/{}", tool));

        if tools_bin_path.exists() {
            println!("{}\n", tools_bin_path.display());
        } else {
            return Err(BinaryNotInstalled(tool));
        }
    }

    Ok(())
}
