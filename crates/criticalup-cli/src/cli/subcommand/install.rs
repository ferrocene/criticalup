// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::env::current_dir;
use std::path::{Path, PathBuf};

use crate::cli::connectivity::Network;
use crate::cli::CommandExecute;
use crate::errors::Error;
use crate::errors::Error::{IntegrityErrorsWhileInstallation, PackageDependenciesNotSupported};
use crate::Context;
use clap::Parser;
use criticaltrust::integrity::IntegrityVerifier;
use criticaltrust::manifests::{Release, ReleaseArtifactFormat};
use criticalup_core::download_server_client::DownloadServerClient;
use criticalup_core::project_manifest::{ProjectManifest, ProjectManifestProduct};
use criticalup_core::state::State;
use tracing::Span;

pub const DEFAULT_RELEASE_ARTIFACT_FORMAT: ReleaseArtifactFormat = ReleaseArtifactFormat::TarXz;

/// Install the toolchain for the given project based on the manifest `criticalup.toml`
#[derive(Debug, Parser)]
pub(crate) struct Install {
    /// Path to the manifest `criticalup.toml`
    #[arg(long)]
    project: Option<PathBuf>,
    /// Reinstall products that may have already been installed
    #[arg(long)]
    reinstall: bool,
    #[clap(flatten)]
    network: Network,
}

impl CommandExecute for Install {
    #[tracing::instrument(level = "debug", skip_all, fields(
        project,
        %connectivity = self.network.connectivity
    ))]
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        // TODO: If `std::io::stdout().is_terminal() == true``, provide a nice, fancy progress bar using indicatif.
        //       Retain existing behavior to support non-TTY usage.

        let span = Span::current();
        let project = if let Some(project) = self.project {
            project.clone()
        } else {
            ProjectManifest::discover(&current_dir()?)?
        };
        span.record("project", tracing::field::display(project.display()));

        let state = State::load(&ctx.config).await?;
        let client = DownloadServerClient::new(&ctx.config, &state, self.network.connectivity);

        // Parse and serialize the project manifest.
        let project_manifest = ProjectManifest::load(&project)?;

        let installation_dir = &ctx.config.paths.installation_dir;

        for product in project_manifest.products() {
            let abs_installation_dir_path = installation_dir.join(product.installation_id());

            if !abs_installation_dir_path.exists() {
                install_product_afresh(ctx, &state, &client, &project, product).await?;
            } else {
                // Check if the state file has no mention of this installation.
                let does_this_installation_exist_in_state = state
                    .installations()
                    .contains_key(&product.installation_id());
                if !does_this_installation_exist_in_state || self.reinstall {
                    // If the installation directory exists, but the State has no installation of that
                    // InstallationId, then re-run the install command and go through installation.
                    install_product_afresh(ctx, &state, &client, &project, product).await?;
                } else {
                    // If the installation directory exists AND there is an existing installation with
                    // that InstallationId, then merely update the installation in the State file to
                    // reflect this manifest/project.
                    state.update_installation_manifests(&product.installation_id(), &project)?;
                    tracing::info!("Skipping installation for product '{}' because it seems to be already installed.\n\
                        If you want to reinstall it, please run 'criticalup install --reinstall'.",
                            product.name());
                }
            }
            // Even though we do not install the existing packages again, we still need to add
            // the manifest to the state.json.
            state.persist().await?;
        }

        criticalup_core::binary_proxies::update(&ctx.config, &state, &std::env::current_exe()?)
            .await?;

        Ok(())
    }
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
    client: &DownloadServerClient,
    manifest_path: &Path,
    product: &ProjectManifestProduct,
) -> Result<(), Error> {
    let product_name = product.name();
    let release = product.release();
    let installation_dir = &ctx.config.paths.installation_dir;
    let abs_installation_dir_path = installation_dir.join(product.installation_id());
    let keys = client.keys().await?;

    tracing::info!("Installing product '{product_name}' ({release})",);

    let mut integrity_verifier = IntegrityVerifier::new(&keys);

    // Get the release manifest for the product from the server and verify it.
    let release_manifest_from_server = client
        .product_release_manifest(product_name, product.release())
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
        let package_data = client
            .package(
                product_name,
                release_name,
                package,
                DEFAULT_RELEASE_ARTIFACT_FORMAT,
            )
            .await?;

        tracing::info!("Installing component '{package}' for '{product_name}' ({release})",);
        // iiuc the paths are added inside the integrity verifier, so we should not need to return a vector
        install_one_release(
            &mut integrity_verifier,
            &abs_installation_dir_path,
            package_data,
        )
        .await?;
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

fn check_for_package_dependencies(verified_release_manifest: &Release) -> Result<(), Error> {
    for package in verified_release_manifest.packages.iter() {
        if !package.dependencies.is_empty() {
            return Err(PackageDependenciesNotSupported(package.package.clone()));
        }
    }
    Ok(())
}

async fn install_one_release(
    integrity_verifier: &mut IntegrityVerifier<'_>,
    abs_installation_dir_path: &PathBuf,
    package_data: Vec<u8>,
) -> Result<(), Error> {
    let decoder = xz2::read::XzDecoder::new(package_data.as_slice());
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

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
