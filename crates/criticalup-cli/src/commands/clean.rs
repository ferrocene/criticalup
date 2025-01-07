// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};
use tokio::fs;

use criticalup_core::binary_proxies;
use criticalup_core::project_manifest::InstallationId;
use criticalup_core::state::State;

use crate::errors::Error;
use crate::Context;

pub(crate) async fn run(ctx: &Context) -> Result<(), Error> {
    let installations_dir = &ctx.config.paths.installation_dir;
    let state = State::load(&ctx.config).await?;

    delete_cache_directory(&ctx.config.paths.cache_dir).await?;
    delete_unused_installations(installations_dir, &state).await?;
    // Deletes unused binary proxies after state cleanup.
    binary_proxies::update(&ctx.config, &state, &std::env::current_exe()?).await?;
    delete_untracked_installation_dirs(installations_dir, state).await?;

    Ok(())
}

async fn delete_cache_directory(cache_dir: &Path) -> Result<(), Error> {
    if cache_dir.exists() {
        tracing::info!("Cleaning cache directory");
        tokio::fs::remove_dir_all(&cache_dir).await?;
    }
    Ok(())
}

/// Deletes installation from `State` with `InstallationId`s that have empty manifest section, and
/// deletes the installation directory from the disk if present.
async fn delete_unused_installations(installations_dir: &Path, state: &State) -> Result<(), Error> {
    // We need to list all the available installations on the disk first, so we can check which
    // installations in state file are absent from the disk.
    let mut all_installations_on_disk: Vec<InstallationId> = Vec::new();
    let mut entries = fs::read_dir(installations_dir).await?;
    while let Some(item) = entries.next_entry().await? {
        if item.file_type().await?.is_dir() {
            let installation_dir_name = item.file_name();
            if let Some(name) = installation_dir_name.to_str() {
                all_installations_on_disk.push(InstallationId(name.into()));
            }
        }
    }

    let unused_installations: Vec<InstallationId> = state
        .installations()
        .iter()
        .filter(|item| item.1.manifests().is_empty() || !all_installations_on_disk.contains(item.0))
        .map(|item| item.0.to_owned())
        .collect();

    if unused_installations.is_empty() {
        tracing::info!("No unused installations found");
        return Ok(());
    }

    for installation in unused_installations {
        tracing::info!("Deleting unused installation {}", installation.0);

        // Remove installation from the state.
        state.remove_installation(&installation);
        // The state will be saved onto the disk but the removal of the installation directory
        // will be done after this which may not exist.
        state.persist().await?;

        // Remove installation directory from physical location.
        let installation_dir_to_delete = installations_dir.join(&installation.0);
        if installation_dir_to_delete.exists() {
            tracing::info!(
                "deleting unused installation directory {}",
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
                    tracing::info!(
                        "deleting untracked installation directory {}",
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
        tracing::info!("no untracked installation directories found",);
    }

    Ok(())
}
