// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::Error;
use crate::Context;
use criticalup_core::project_manifest::ProjectManifest;
use criticalup_core::state::State;
use owo_colors::OwoColorize;
use std::fs;
use std::path::PathBuf;

pub(crate) fn run(ctx: &Context, project: Option<PathBuf>) -> Result<(), Error> {
    let state = State::load(&ctx.config)?;
    let manifest_path = ProjectManifest::discover_canonical_path(project.as_deref())?;
    let installation_dir = &ctx.config.paths.installation_dir;

    let installations_from_which_manifest_was_deleted =
        state.remove_manifest_from_all_installations(&manifest_path)?;
    state.persist()?;

    for installation_id in &installations_from_which_manifest_was_deleted {
        println!(
            "{} deleting installation {}",
            "info:".bold(),
            installation_id.0
        );
        let installation_path = installation_dir.join(installation_id.0.as_str());
        if installation_path.exists() {
            fs::remove_dir_all(&installation_path)?;
        }
    }

    if installations_from_which_manifest_was_deleted.is_empty() {
        println!(
            "{} no existing installations found to be deleted",
            "info:".bold()
        );
    }

    Ok(())
}
