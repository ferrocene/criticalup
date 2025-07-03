// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeMap;
use std::env::current_dir;
use std::path::{Path, PathBuf};

use crate::cli::connectivity::Network;
use crate::cli::CommandExecute;
use crate::errors::Error;
use crate::errors::Error::{
    IntegrityErrorsWhileInstallation, PackageDependenciesNotSupported, RevocationCheckFailed,
    RevocationSignatureExpired,
};
use crate::Context;
use clap::Parser;
use criticaltrust::integrity::{IntegrityError, IntegrityVerifier};
use criticaltrust::manifests::{Release, ReleaseArtifactFormat};
use criticaltrust::revocation_info::RevocationInfo;
use criticalup_core::download_server_client::DownloadServerClient;
use criticalup_core::project_manifest::{ProjectManifest, ProjectManifestProduct};
use criticalup_core::state::State;
use tokio::sync::mpsc;
use tracing::{Instrument, Span};

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
    offline: bool,
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

    // Checks for making sure that there is no revoked content in the incoming packages.
    let revocation_info = &keys
        .revocation_info()
        .ok_or_else(|| Error::MissingRevocationInfo(IntegrityError::MissingRevocationInfo))?;
    check_for_revocation(revocation_info, &verified_release_manifest, offline)?;

    // criticalup 0.1, return error if any of package.dependencies is not empty.
    // We have to use manifest's Release because the information about dependencies
    // only lives in it and not in product's packages which is only a name/String.
    check_for_package_dependencies(&verified_release_manifest)?;

    let release_name = verified_release_manifest.release.as_str();

    product
        .create_product_dir(&ctx.config.paths.installation_dir)
        .await?;
    // Finish channel must be opened in advance and be ready to rx; if not, the code remains sequential.
    let (finish_tx, mut finish_rx) = mpsc::channel(1);
    let (product_name_clone, release_clone): (String, String) =
        (product_name.to_owned(), release.to_owned());

    let (install_tx, mut install_rx) = mpsc::channel(1);
    tokio::spawn(async move {
        while let Some((package_name, package_data)) = install_rx.recv().await {
            tracing::info!(
                "Installing component '{package_name}' for '{product_name_clone}' ({release_clone})",
            );
            let files = install_one_package(&abs_installation_dir_path, package_data).await;
            finish_tx.send(files).await.unwrap();
        }
        // Tx must be dropped to indicate the end of the operation.
        drop(finish_tx);
    }.instrument(Span::current()));

    for package in product.packages() {
        let package_data = client
            .package(
                product_name,
                release_name,
                package,
                DEFAULT_RELEASE_ARTIFACT_FORMAT,
            )
            .await
            .unwrap();
        install_tx
            .send((package.to_owned(), package_data))
            .await
            .unwrap();
    }
    // Same as finish_tx, we send None to indicate the end of the operation.
    drop(install_tx);

    while let Some(res) = finish_rx.recv().await {
        let files = res?;
        for (path, mode) in files {
            let buffer = &tokio::fs::read(&path).await?;
            integrity_verifier.add(path.as_ref(), mode, buffer);
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

fn check_for_revocation(
    revocation_info: &RevocationInfo,
    verified_release_manifest: &Release,
    offline: bool,
) -> Result<(), Error> {
    if !offline && time::OffsetDateTime::now_utc() >= revocation_info.expires_at {
        return Err(RevocationSignatureExpired(
            criticaltrust::Error::RevocationSignatureExpired(revocation_info.expires_at),
        ));
    }

    // Convert Verified Release Manifest packages into a map so we can quickly check.
    let mut base64_bytes_to_package_name: BTreeMap<Vec<u8>, String> = BTreeMap::new();
    for release_package in &verified_release_manifest.packages {
        for release_artifact in &release_package.artifacts {
            base64_bytes_to_package_name.insert(
                release_artifact.sha256.clone(),
                release_package.package.clone(),
            );
        }
    }

    for revoked_sha in &revocation_info.revoked_content_sha256 {
        if let Some(package) = base64_bytes_to_package_name.get(revoked_sha) {
            return Err(RevocationCheckFailed(
                package.to_owned(),
                criticaltrust::Error::ContentRevoked(package.to_owned()),
            ));
        }
    }

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

async fn install_one_package(
    abs_installation_dir_path: &Path,
    package_data: Vec<u8>,
) -> Result<Vec<(PathBuf, u32)>, Error> {
    let decoder = xz2::read::XzDecoder::new(package_data.as_slice());
    let mut archive = tar::Archive::new(decoder);
    archive.set_preserve_permissions(true);
    archive.set_preserve_mtime(true);
    archive.set_unpack_xattrs(true);

    let mut files = Vec::new();

    let entries = archive.entries()?;
    for each in entries {
        let mut entry = each?;

        let p = entry.path()?.into_owned();

        let entry_path_on_disk = abs_installation_dir_path.join(p);
        entry.unpack(&entry_path_on_disk)?;

        if entry_path_on_disk.is_file() {
            files.push((entry_path_on_disk, entry.header().mode().unwrap()));
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use criticaltrust::manifests::{ReleaseArtifact, ReleasePackage};
    use time::macros::datetime;

    const PACKAGE_SHA256: &[u8] = &[
        57, 55, 54, 101, 97, 97, 99, 53, 53, 99, 101, 102, 102, 50, 49, 53, 53, 48, 99, 55, 100,
        52, 97, 57, 100, 52, 97, 101, 100, 101, 52, 101, 48, 49, 102, 48, 57, 100, 99, 57, 53, 51,
        48, 48, 57, 51, 97, 98, 98, 57, 102, 49, 100, 48, 56, 53, 101, 49, 48, 50, 51, 99, 55, 49,
    ];

    const PACKAGE_SHA256_NOT_PRESENT: &[u8] = &[
        57, 55, 54, 101, 99, 97, 99, 53, 53, 99, 101, 102, 102, 50, 49, 53, 53, 48, 99, 55, 100,
        52, 97, 57, 100, 52, 97, 101, 100, 101, 52, 101, 48, 49, 102, 48, 57, 100, 99, 57, 53, 51,
        48, 48, 57, 51, 99, 98, 98, 57, 102, 49, 100, 48, 56, 53, 101, 49, 48, 50, 51, 99, 55, 49,
    ];

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

    // Check if there is a revoked content Sha256 in the package.
    #[test]
    fn revocation_check_normal() {
        let revocation_info =
            RevocationInfo::new(vec![PACKAGE_SHA256.into()], datetime!(2400-10-10 00:00 UTC));
        let release = generate_release();
        assert!(matches!(
            check_for_revocation(&revocation_info, &release, false),
            Err(RevocationCheckFailed(..))
        ))
    }

    // Offline mode but valid expiration date.
    #[test]
    fn revocation_check_offline() {
        let revocation_info =
            RevocationInfo::new(vec![PACKAGE_SHA256.into()], datetime!(2400-10-10 00:00 UTC));
        let release = generate_release();
        assert!(matches!(
            check_for_revocation(&revocation_info, &release, true),
            Err(RevocationCheckFailed(..))
        ))
    }

    // The expired datetime ignored in Offline mode but the package expired hash still catches the
    // error.
    #[test]
    fn revocation_check_offline_mode_expired_datetime_correct_expired_package_hash() {
        let revocation_info =
            RevocationInfo::new(vec![PACKAGE_SHA256.into()], datetime!(2012-10-10 00:00 UTC));
        let release = generate_release();
        assert!(matches!(
            check_for_revocation(&revocation_info, &release, true),
            Err(RevocationCheckFailed(..))
        ))
    }

    // The expired datetime must be ignored in Offline mode with a package expired hash not of the
    // package being checked.
    #[test]
    fn revocation_check_offline_mode_expired_datetime_incorrect_expired_package_hash() {
        let revocation_info = RevocationInfo::new(
            vec![PACKAGE_SHA256_NOT_PRESENT.into()],
            datetime!(2012-10-10 00:00 UTC),
        );
        let release = generate_release();
        assert!(matches!(
            check_for_revocation(&revocation_info, &release, true),
            Ok(())
        ))
    }

    // The expired datetime must be ignored in Offline mode with no package expired hash.
    #[test]
    fn revocation_check_offline_mode_expired_datetime_no_expired_package_hash() {
        let revocation_info = RevocationInfo::new(vec![], datetime!(2012-10-10 00:00 UTC));
        let release = generate_release();
        assert!(matches!(
            check_for_revocation(&revocation_info, &release, true),
            Ok(())
        ))
    }

    // Check if the revocation info signature is expired.
    #[test]
    fn revocation_check_expired() {
        let revocation_info =
            RevocationInfo::new(vec![PACKAGE_SHA256.into()], datetime!(2000-10-10 00:00 UTC));
        let release = generate_release();
        assert!(matches!(
            check_for_revocation(&revocation_info, &release, false),
            Err(RevocationSignatureExpired(..))
        ))
    }

    // Utilities.

    fn generate_release() -> Release {
        Release {
            product: "ferrocene".to_string(),
            release: "amazing".to_string(),
            commit: "bsdf32avsd2312".to_string(),
            packages: vec![ReleasePackage {
                package: "x86".to_string(),
                artifacts: vec![ReleaseArtifact {
                    format: ReleaseArtifactFormat::TarZst,
                    size: 10,
                    sha256: PACKAGE_SHA256.into(),
                }],
                dependencies: vec![],
            }],
        }
    }
}
