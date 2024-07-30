// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};

use criticaltrust::integrity::IntegrityVerifier;
use criticaltrust::manifests::{Release, ReleaseArtifactFormat};
use criticalup_core::config::Config;
use criticalup_core::download_server_client::DownloadServerClient;
use criticalup_core::project_manifest::{ProjectManifest, ProjectManifestProduct};
use criticalup_core::state::State;
use tokio::fs::read;

use crate::errors::Error;
use crate::errors::Error::{IntegrityErrorsWhileInstallation, PackageDependenciesNotSupported};
use crate::Context;

pub const DEFAULT_RELEASE_ARTIFACT_FORMAT: ReleaseArtifactFormat = ReleaseArtifactFormat::TarXz;

pub(crate) async fn run(
    ctx: &Context,
    reinstall: bool,
    project: Option<PathBuf>,
) -> Result<(), Error> {
    // TODO: If `std::io::stdout().is_terminal() == true``, provide a nice, fancy progress bar using indicatif.
    //       Retain existing behavior to support non-TTY usage.

    let state = State::load(&ctx.config).await?;

    // Get manifest location if arg `project` is None
    let manifest_path = ProjectManifest::discover_canonical_path(project.as_deref()).await?;

    // Parse and serialize the project manifest.
    let manifest = ProjectManifest::get(project).await?;

    let installation_dir = &ctx.config.paths.installation_dir;

    for product in manifest.products() {
        let abs_installation_dir_path = installation_dir.join(product.installation_id());

        if !abs_installation_dir_path.exists() {
            install_product_afresh(ctx, &state, &manifest_path, product).await?;
        } else {
            // Check if the state file has no mention of this installation.
            let does_this_installation_exist_in_state = state
                .installations()
                .contains_key(&product.installation_id());
            if !does_this_installation_exist_in_state || reinstall {
                // If the installation directory exists, but the State has no installation of that
                // InstallationId, then re-run the install command and go through installation.
                install_product_afresh(ctx, &state, &manifest_path, product).await?;
            } else {
                // If the installation directory exists AND there is an existing installation with
                // that InstallationId, then merely update the installation in the State file to
                // reflect this manifest/project.
                state.update_installation_manifests(&product.installation_id(), &manifest_path)?;
                tracing::info!("Skipping installation for product '{}' because it seems to be already installed.\n\
                    If you want to reinstall it, please run 'criticalup install --reinstall'.",
                         product.name());
            }
        }
        // Even though we do not install the existing packages again, we still need to add
        // the manifest to the state.json.
        state.persist().await?;
    }

    criticalup_core::binary_proxies::update(&ctx.config, &state, &std::env::current_exe()?).await?;

    Ok(())
}

#[tracing::instrument(level = "debug", skip_all, fields(
    manifest_path = %manifest_path.display(),
    installation_id = %product.installation_id(),
    release = %product.release(),
    product = %product.name(),
))]
async fn install_product_afresh(
    ctx: &Context,
    state: &State,
    manifest_path: &Path,
    product: &ProjectManifestProduct,
) -> Result<(), Error> {
    let product_name = product.name();
    let release = product.release();
    let installation_dir = &ctx.config.paths.installation_dir;
    let abs_installation_dir_path = installation_dir.join(product.installation_id());
    let client = DownloadServerClient::new(&ctx.config, state);
    let keys = client.get_keys().await?;

    // TODO: Add tracing to support log levels, structured logging.
    tracing::info!("Installing product '{product_name}' ({release})",);

    let mut integrity_verifier = IntegrityVerifier::new(&keys);

    // Get the release manifest for the product from the server and verify it.
    let release_manifest_from_server = client
        .get_product_release_manifest(product_name, product.release())
        .await?;
    let verified_release_manifest = release_manifest_from_server.signed.into_verified(&keys)?;

    // criticalup 0.1, return error if any of package.dependencies is not empty.
    // We have to use manifest's Release because the information about dependencies
    // only lives in it and not in product's packages which is only a name/String.
    check_for_package_dependencies(&verified_release_manifest)?;

    let release_name = verified_release_manifest.release.as_str();

    product
        .create_product_dir(&ctx.config.paths.installation_dir)
        .await?;

    for package in product.packages() {
        let abs_artifact_compressed_file_path =
            cached_package_path(&ctx.config, product.name(), release, package);

        let package_file = if !abs_artifact_compressed_file_path.exists() {
            tracing::info!("downloading component '{package}' for '{product_name}' ({release})",);
            let package_file = client
                .download_package(
                    product_name,
                    release_name,
                    package,
                    DEFAULT_RELEASE_ARTIFACT_FORMAT,
                )
                .await?;

            // Save the downloaded package archive on disk.
            tracing::debug!(cached = %abs_artifact_compressed_file_path.display(), "Writing package to cache");
            let dest_dir = abs_artifact_compressed_file_path
                .parent()
                .ok_or_else(|| Error::NoParent(abs_artifact_compressed_file_path.clone()))?;
            tokio::fs::create_dir_all(dest_dir).await?;
            tokio::fs::write(&abs_artifact_compressed_file_path, &package_file).await?;

            package_file
        } else {
            // Don't output anything normally, the lack of a "downloading" message is sufficient.
            tracing::debug!(cached = %abs_artifact_compressed_file_path.display(), "Retrieving package from cache");
            read(abs_artifact_compressed_file_path).await?
        };

        tracing::info!("Installing component '{package}' for '{product_name}' ({release})",);

        let decoder = xz2::read::XzDecoder::new(package_file.as_slice());
        let mut archive = tar::Archive::new(decoder);
        archive.set_preserve_permissions(true);
        archive.set_preserve_mtime(true);
        archive.set_unpack_xattrs(true);

        let entries = archive.entries()?;
        for each in entries {
            let mut entry = each?;

            let p = entry.path()?.into_owned();
            let entry_path_on_disk = abs_installation_dir_path.join(p);
            entry.unpack(&entry_path_on_disk)?;

            if entry_path_on_disk.is_file() {
                integrity_verifier.add(
                    &entry_path_on_disk,
                    entry.header().mode()?,
                    &tokio::fs::read(&entry_path_on_disk).await?,
                );
            }
        }
    }

    let verified_packages = integrity_verifier
        .verify()
        .map_err(IntegrityErrorsWhileInstallation)?;

    state.add_installation(
        &product.installation_id(),
        &verified_packages,
        manifest_path,
        &ctx.config,
    )?;
    Ok(())
}

pub fn cached_package_path(
    config: &Config,
    product: &str,
    release: &str,
    package: &str,
) -> PathBuf {
    let release_path = PathBuf::from(product).join(release);
    let package_file = PathBuf::from(format!("{package}.tar.xz"));
    let cache_key = release_path.join(package_file);
    config.paths.cache_dir.join(cache_key)
}

fn check_for_package_dependencies(verified_release_manifest: &Release) -> Result<(), Error> {
    for package in verified_release_manifest.packages.iter() {
        if !package.dependencies.is_empty() {
            return Err(PackageDependenciesNotSupported(package.package.clone()));
        }
    }
    Ok(())
}

#[test]
fn dependencies_check() {
    use criticaltrust::manifests::ReleasePackage;

    let dependencies = vec!["dependency_a".to_string()];

    let good = Release {
        product: "ferrocene".to_string(),
        release: "nightly-2024-02-28".to_string(),
        commit: "123".to_string(),
        packages: vec![ReleasePackage {
            package: "awesome".to_string(),
            artifacts: vec![],
            dependencies: vec![],
        }],
    };

    assert!(check_for_package_dependencies(&good).is_ok());

    let bad = Release {
        product: "ferrocene".to_string(),
        release: "nightly-2024-02-28".to_string(),
        commit: "123".to_string(),
        packages: vec![ReleasePackage {
            package: "awesome".to_string(),
            artifacts: vec![],
            dependencies,
        }],
    };

    assert!(check_for_package_dependencies(&bad).is_err());
    assert!(matches!(
        check_for_package_dependencies(&bad),
        Err(PackageDependenciesNotSupported(..))
    ));
}
