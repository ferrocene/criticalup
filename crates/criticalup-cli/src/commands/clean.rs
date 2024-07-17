// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};
use tokio::fs;

use owo_colors::OwoColorize;

use criticalup_core::project_manifest::InstallationId;
use criticalup_core::state::State;

use crate::errors::Error;
use crate::Context;

pub(crate) async fn run(ctx: &Context) -> Result<(), Error> {
    let installations_dir = &ctx.config.paths.installation_dir;
    let state = State::load(&ctx.config).await?;

    delete_unused_installations(installations_dir, &state).await?;
    delete_untracked_installation_dirs(installations_dir, state).await?;

    Ok(())
}

/// Deletes installation from `State` wl; ith `InstallationId`s that have empty manifest section, and
/// deletes the installation directory from the disk if present.
async fn delete_unused_installations(installations_dir: &Path, state: &State) -> Result<(), Error> {
    let unused_installations: Vec<InstallationId> = state
        .installations()
        .iter()
        .filter(|item| item.1.manifests().is_empty())
        .map(|item| item.0.to_owned())
        .collect();

    if unused_installations.is_empty() {
        println!("{} no unused installations found", "info:".bold());
        return Ok(());
    }

    for installation in unused_installations {
        println!(
            "{} deleting unused installation {}",
            "info:".bold(),
            installation.0
        );

        // Remove installation from the state.
        state.remove_installation(&installation);
        // The state will be saved onto the disk but the removal of the installation directory
        // will be done after this which may not exist.
        state.persist().await?;

        // Remove installation directory from physical location.
        let installation_dir_to_delete = installations_dir.join(&installation.0);
        if installation_dir_to_delete.exists() {
            println!(
                "{} deleting unused installation directory {}",
                "info:".bold(),
                &installation_dir_to_delete.display()
            );
            fs::remove_dir_all(&installation_dir_to_delete)
                .await
                .map_err(|err| Error::DeletingUnusedInstallationDir {
                    path: installation_dir_to_delete,
                    kind: err,
                })?;
        }
    }
    Ok(())
}

/// Deletes the installation directories from the disk that do not exist in the State.
async fn delete_untracked_installation_dirs(
    installations_dir: &PathBuf,
    state: State,
) -> Result<(), Error> {
    let installations_in_state = state.installations().clone();
    let mut are_untracked_installation_dirs_present = false;

    let mut entries = fs::read_dir(installations_dir).await?;
    while let Some(item) = entries.next_entry().await? {
        if item.file_type().await?.is_dir() {
            let installation_dir_name = item.file_name();
            if let Some(name) = installation_dir_name.to_str() {
                if !installations_in_state.contains_key(&InstallationId(name.into())) {
                    are_untracked_installation_dirs_present = true;
                    println!(
                        "{} deleting untracked installation directory {}",
                        "info:".bold(),
                        item.path().to_path_buf().display()
                    );

                    fs::remove_dir_all(item.path()).await.map_err(|err| {
                        Error::DeletingUntrackedInstallationDir {
                            path: item.path().to_path_buf(),
                            kind: err,
                        }
                    })?;
                }
            }
        }
    }

    if !are_untracked_installation_dirs_present {
        println!(
            "{} no untracked installation directories found",
            "info:".bold()
        );
    }

    Ok(())
}
