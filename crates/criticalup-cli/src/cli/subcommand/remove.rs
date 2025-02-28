// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::cli::CommandExecute;
use crate::errors::Error;
use crate::Context;
use clap::Parser;
use criticalup_core::project_manifest::ProjectManifest;
use criticalup_core::state::State;
use std::env::current_dir;
use std::path::PathBuf;
use tokio::fs;
use tracing::Span;

/// Delete all the products specified in the manifest `criticalup.toml`
#[derive(Debug, Parser)]
pub(crate) struct Remove {
    /// Path to the manifest `criticalup.toml`
    #[arg(long)]
    project: Option<PathBuf>,
}

impl CommandExecute for Remove {
    #[tracing::instrument(level = "debug", skip_all, fields(project,))]
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        let span = Span::current();
        let project = if let Some(project) = self.project {
            project.clone()
        } else {
            ProjectManifest::discover(&current_dir()?, None)?
        };
        span.record("project", tracing::field::display(project.display()));

        let state = State::load(&ctx.config).await?;
        let installation_dir = &ctx.config.paths.installation_dir;

        let installations_from_which_manifest_was_deleted =
            state.remove_manifest_from_all_installations(&project)?;
        state.persist().await?;

        for installation_id in &installations_from_which_manifest_was_deleted {
            tracing::info!("Deleting installation {}", installation_id.0);
            let installation_path = installation_dir.join(installation_id.0.as_str());
            if installation_path.exists() {
                fs::remove_dir_all(&installation_path).await?;
            }
        }

        if installations_from_which_manifest_was_deleted.is_empty() {
            tracing::info!("No existing installations found to be deleted",);
        }

        Ok(())
    }
}
