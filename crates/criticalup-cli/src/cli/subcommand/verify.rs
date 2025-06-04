// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(not(windows))]
use std::os::unix::fs::MetadataExt;
use std::{
    env::current_dir,
    path::{Path, PathBuf},
};

use clap::Parser;
use criticaltrust::{integrity::IntegrityVerifier, signatures::Keychain};
use criticalup_core::{
    download_server_client::DownloadServerClient,
    project_manifest::{ProjectManifest, ProjectManifestProduct},
    state::State,
};
use tracing::Span;
use walkdir::WalkDir;

use crate::{
    cli::{connectivity::Network, CommandExecute},
    errors::Error,
    Context,
};

/// Verify a given toolchain
#[derive(Debug, Parser)]
pub(crate) struct Verify {
    #[clap(flatten)]
    network: Network,

    /// Path to the manifest `criticalup.toml`
    #[arg(long)]
    project: Option<PathBuf>,
}

impl CommandExecute for Verify {
    #[tracing::instrument(level = "debug", skip_all, fields(
        project,
        %connectivity = self.network.connectivity
    ))]
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        let span = Span::current();
        let project = if let Some(project) = self.project {
            project.clone()
        } else {
            ProjectManifest::discover(&current_dir()?)?
        };
        span.record("project", tracing::field::display(project.display()));

        let state = State::load(&ctx.config).await?;
        let client: DownloadServerClient =
            DownloadServerClient::new(&ctx.config, &state, self.network.connectivity);
        let keys = client.keys().await?;

        let project_manifest = ProjectManifest::load(&project)?;

        let installation_dir = &ctx.config.paths.installation_dir;

        verify(&keys, installation_dir, &project_manifest).await
    }
}

async fn verify(
    keys: &Keychain,
    installation_dir: &Path,
    project_manifest: &ProjectManifest,
) -> Result<(), Error> {
    let products = project_manifest.products();

    // We don't actually care the order of verification of products, simply that they are verified.
    let mut working_set = Vec::with_capacity(products.len());
    for product in project_manifest.products() {
        working_set.push(verify_product(keys, installation_dir, product));
    }
    futures::future::join_all(working_set)
        .await
        .into_iter()
        .collect::<Result<(), Error>>()?;

    Ok(())
}

#[tracing::instrument(level = "debug", skip_all, fields(product_path))]
async fn verify_product(
    keys: &Keychain,
    installation_dir: &Path,
    product: &ProjectManifestProduct,
) -> Result<(), Error> {
    let span = Span::current();

    let mut integrity_verifier = IntegrityVerifier::new(keys);

    let product_name = product.name();
    let product_path = installation_dir.join(product.installation_id());
    span.record(
        "product_path",
        tracing::field::display(product_path.display()),
    );

    tracing::info!("Verifying product '{product_name}'");

    for entry in WalkDir::new(product_path) {
        let entry = entry?;

        if entry.file_type().is_file() {
            tracing::trace!("Adding {}", tracing::field::display(entry.path().display()));

            #[cfg(not(windows))]
            let mode = entry.metadata()?.mode();
            // Windows does not have the same concept of permissions, we just no-op mode.
            #[cfg(windows)]
            let mode = 0;

            integrity_verifier.add(entry.path(), mode, &tokio::fs::read(&entry.path()).await?);
        }
    }

    integrity_verifier
        .verify()
        .map_err(Error::IntegrityErrorsWhileVerifying)?;

    tracing::info!("Successfully verified '{product_name}'");

    Ok(())
}
