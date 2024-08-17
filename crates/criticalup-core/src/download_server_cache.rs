// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{download_server_client::DownloadServerClient, errors::Error};
use std::path::{Path, PathBuf};

use criticaltrust::{
    manifests::{ReleaseArtifactFormat, ReleaseManifest},
    signatures::Keychain,
};
use tokio::fs::{create_dir_all, read, write};

/// A cache for artifacts from the download server
pub struct DownloadServerCache<'a> {
    root: &'a Path,
    /// The cache will lazily populate if provided a client.
    client: Option<&'a DownloadServerClient>,
}

impl<'a> DownloadServerCache<'a> {
    /// Create a new cache from a given root, and optionally a client.
    pub async fn new(
        root: &'a Path,
        client: impl Into<Option<&'a DownloadServerClient>>,
    ) -> Result<Self, Error> {
        let client = client.into();

        Ok(Self { root, client })
    }

    fn release_path(&self, product: &str, release: &str) -> PathBuf {
        self.root.join("artifacts").join(product).join(release)
    }

    fn package_path(
        &self,
        product: &str,
        release: &str,
        package: &str,
        format: ReleaseArtifactFormat,
    ) -> PathBuf {
        self.release_path(product, release).join({
            let mut file_name = PathBuf::from(package);
            file_name.set_extension(format.to_string());
            file_name
        })
    }

    fn product_release_manfest_path(&self, product: &str, release: &str) -> PathBuf {
        self.release_path(product, release).join("manifest.json")
    }

    fn keys_path(&self) -> PathBuf {
        self.root.join("keys.json")
    }

    #[tracing::instrument(level = "debug", skip_all, fields(
        %product,
        %release,
        %package,
        %format
    ))]
    pub async fn package(
        &self,
        product: &str,
        release: &str,
        package: &str,
        format: ReleaseArtifactFormat,
    ) -> Result<PathBuf, Error> {
        let cache_key = self.package_path(product, release, package, format);

        let cache_hit = cache_key.exists();
        tracing::trace!(%cache_hit, cache_key = %cache_key.display());

        match (cache_hit, &self.client) {
            (false, Some(client)) => {
                // Cache miss, online mode
                let cache_dir = self.release_path(product, release);
                create_dir_all(&cache_dir)
                    .await
                    .map_err(|e| Error::Create(cache_dir.to_path_buf(), e))?;

                let download = client
                    .download_package(product, release, package, format)
                    .await?;
                tokio::fs::write(&cache_key, download)
                    .await
                    .map_err(|e| Error::Write(cache_key.clone(), e))?;
            }
            (false, None) => {
                // Cache miss, offline mode
                return Err(Error::OfflineMode);
            }
            (true, _) => (), // Cache hit
        }

        Ok(cache_key)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(
        %product,
        %release,
    ))]
    pub async fn product_release_manifest(
        &self,
        product: &str,
        release: &str,
    ) -> Result<ReleaseManifest, Error> {
        let cache_key = self.product_release_manfest_path(product, release);

        let cache_hit = cache_key.exists();
        tracing::trace!(%cache_hit, cache_key = %cache_key.display());

        let data = match (cache_hit, &self.client) {
            (false, Some(client)) => {
                // Cache miss, online mode
                let cache_dir = self.release_path(product, release);
                create_dir_all(&cache_dir)
                    .await
                    .map_err(|e| Error::Create(cache_dir.to_path_buf(), e))?;

                let data = client
                    .get_product_release_manifest(product, release)
                    .await?;
                // It would be preferable to store the raw server response.
                let serialized = serde_json::to_string_pretty(&data)?;
                write(&cache_key, serialized)
                    .await
                    .map_err(|e| Error::Write(cache_key.clone(), e))?;
                data
            }
            (false, None) => {
                // Cache miss, offline mode
                return Err(Error::OfflineMode);
            }
            (true, _) => {
                // Cache hit
                let data = read(&cache_key)
                    .await
                    .map_err(|e| Error::Read(cache_key.clone(), e))?;
                serde_json::from_slice(&data)?
            }
        };

        Ok(data)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn keys(&self) -> Result<Keychain, Error> {
        let cache_key = self.keys_path();

        let cache_hit = cache_key.exists();
        tracing::trace!(%cache_hit, cache_key = %cache_key.display());

        let data = match (cache_hit, &self.client) {
            (_, Some(client)) => {
                // Cache hit or miss, online mode
                // Eagerly refresh keys whenever online in case there are new keys with new expiration dates.
                create_dir_all(&self.root)
                    .await
                    .map_err(|e| Error::Create(self.root.to_path_buf(), e))?;

                let data = client.get_keys().await?;
                // It would be preferable to store the raw server response.
                let serialized = serde_json::to_string_pretty(&data)?;
                write(&cache_key, serialized)
                    .await
                    .map_err(|e| Error::Write(cache_key.clone(), e))?;
                data
            }
            (false, None) => {
                // Cache miss, offline mode
                return Err(Error::OfflineMode);
            }
            (true, None) => {
                // Cache hit, offline mode
                // We cannot refresh keys, so continue as usual
                let data = read(&cache_key)
                    .await
                    .map_err(|e| Error::Read(cache_key.clone(), e))?;
                serde_json::from_slice(&data)?
            }
        };

        Ok(data)
    }
}
