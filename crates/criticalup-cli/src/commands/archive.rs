// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(not(windows))]
use std::os::unix::fs::MetadataExt;
use std::{
    env::current_dir,
    fs::OpenOptions,
    io::{stdout, Write},
    path::PathBuf,
};

use criticaltrust::{integrity::IntegrityVerifier, signatures::Keychain};
use criticalup_core::{
    download_server_cache::DownloadServerCache, download_server_client::DownloadServerClient,
    project_manifest::ProjectManifest, state::State,
};
use tempfile::TempDir;
use tokio::task::spawn_blocking;
use tracing::Span;
use walkdir::WalkDir;

use crate::{errors::Error, Context};

use super::install::DEFAULT_RELEASE_ARTIFACT_FORMAT;

#[tracing::instrument(level = "debug", skip_all, fields(manifest_path, %offline))]
pub(crate) async fn run(
    ctx: &Context,
    manifest_path: Option<&PathBuf>,
    offline: bool,
    out: Option<&PathBuf>,
) -> Result<(), Error> {
    let span = Span::current();
    let manifest_path = if let Some(manifest_path) = manifest_path {
        manifest_path.clone()
    } else {
        ProjectManifest::discover(&current_dir()?)?
    };
    span.record(
        "manifest_path",
        tracing::field::display(manifest_path.display()),
    );

    let state = State::load(&ctx.config).await?;
    let maybe_client = if !offline {
        Some(DownloadServerClient::new(&ctx.config, &state))
    } else {
        None
    };
    let cache = DownloadServerCache::new(&ctx.config.paths.cache_dir, &maybe_client).await?;
    let keys = cache.keys().await?;

    let project_manifest = ProjectManifest::load(&manifest_path)?;

    archive(cache, &keys, &project_manifest, out).await?;

    Ok(())
}

#[tracing::instrument(level = "debug", skip_all, fields(product_path))]
async fn archive(
    cache: DownloadServerCache<'_>,
    keys: &Keychain,
    project_manifest: &ProjectManifest,
    out: Option<&PathBuf>,
) -> Result<(), Error> {
    // Path to installables we will include in the tarball
    // Note: Do not try to get clever and parallize the building of this, download
    //       bandwidth is not generous for many people.
    let mut installables = vec![];

    // Collect a list of installables
    for product in project_manifest.products() {
        let product_name = product.name();
        let release = product.release();

        for package in product.packages() {
            let package_path = cache
                .package(
                    product_name,
                    release,
                    package,
                    DEFAULT_RELEASE_ARTIFACT_FORMAT,
                )
                .await?;
            installables.push(package_path);
        }
    }

    // Build a sysroot of the installables in a tempdir.
    let working_dir = TempDir::new()?;
    for installable in installables {
        let working_path = working_dir.path().to_path_buf();
        spawn_blocking(move || {
            let file = std::fs::OpenOptions::new().read(true).open(installable)?;
            let decoder = xz2::read::XzDecoder::new(file);
            let mut archive = tar::Archive::new(decoder);
            archive.set_preserve_permissions(true);
            archive.set_preserve_mtime(true);
            archive.set_unpack_xattrs(true);

            archive.unpack(working_path)
        })
        .await??;
    }

    // Run the verifier over the tempdir.
    tracing::info!("Verifying toolchain...");
    let mut integrity_verifier = IntegrityVerifier::new(keys);
    for entry in WalkDir::new(&working_dir) {
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
        .map_err(Error::IntegrityErrorsWhileTarballing)?;
    tracing::info!("Verified toolchain");

    // Wrap it up.
    let out_cloned = out.map(|v| v.to_path_buf());
    let working_dir_owned = working_dir.path().to_path_buf();
    spawn_blocking(move || {
        let mut destination: Box<dyn Write> = if let Some(out) = out_cloned {
            let destination = std::env::current_dir()?.join(&out);
            tracing::info!(path = %out.display(), "Creating archive...");
            Box::new(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(destination)?,
            )
        } else {
            Box::new(stdout())
        };

        // Tarballs kinda suck, we can't create them with absolute paths,
        // so, we're forced to change directory.
        let old_current_dir = std::env::current_dir()?;
        std::env::set_current_dir(working_dir_owned)?;
        let mut archive = tar::Builder::new(&mut destination);
        archive.append_dir_all(".", ".")?;
        archive.finish()?;
        std::env::set_current_dir(old_current_dir)
    })
    .await??;
    if let Some(out) = out {
        tracing::info!(path = %out.display(), "Archive created successfully");
    } else {
        tracing::info!("Archive created successfully");
    }

    Ok(())
}