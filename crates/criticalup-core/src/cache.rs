use md5::Md5;
use reqwest::header::HeaderValue;
use reqwest::{IntoUrl, Request, StatusCode};
use sha2::Digest;
use std::fs;
use std::path::{Path, PathBuf};

use crate::download_server_client::Connectivity;
use tokio::fs as tokio_fs;

use criticaltrust::manifests::ReleaseArtifactFormat;

use reqwest::header::AUTHORIZATION;

use crate::download_server_client::{unexpected_status, DownloadServerClient};

use crate::errors::{DownloadServerError, Error};

pub(crate) fn try_migrating_deprecated_path(
    deprecated_product_name_path: PathBuf,
    new_products_path: PathBuf,
) -> Result<(), Error> {
    // This function should error if we try to overwrite an existing cache
    if matches!(new_products_path.try_exists(), Ok(true)) {
        return Err(Error::DestinationAlreadyExists {
            deprecated: deprecated_product_name_path.clone(),
            new: new_products_path.clone(),
        });
    }
    fs::create_dir_all(&new_products_path)
        .map_err(|e| Error::Create(new_products_path.clone(), e))?;

    tracing::info!(
        "Tidying deprecated binary proxies, they are now located at `{}`",
        &new_products_path.display()
    );
    // The old path is removed by `rename` which is a move
    fs::rename(&deprecated_product_name_path, &new_products_path)
        .map_err(|e| Error::Write(new_products_path.clone(), e))?;

    Ok(())
}

pub(crate) fn keys_cache_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("keys.json")
}

pub(crate) fn product_release_manifest_cache_path(
    download_server_client: &DownloadServerClient,
    product: &str,
    release: &str,
) -> PathBuf {
    product_release_cache_path(download_server_client, product, release).join("manifest.json")
}
pub(crate) fn product_release_cache_path(
    download_server_client: &DownloadServerClient,
    product: &str,
    release: &str,
) -> PathBuf {
    download_server_client
        .cache_dir
        .join("artifacts")
        .join("products")
        .join(product)
        .join("releases")
        .join(release)
}

pub(crate) fn package_cache_path(
    download_server_client: &DownloadServerClient,
    product: &str,
    release: &str,
    package: &str,
    format: ReleaseArtifactFormat,
) -> PathBuf {
    product_release_cache_path(download_server_client, product, release).join({
        let mut file_name = PathBuf::from(package);
        file_name.set_extension(format.to_string());
        file_name
    })
}

pub(crate) async fn cacheable(
    download_server_client: &DownloadServerClient,
    url: String,
    cache_key: PathBuf,
) -> Result<Vec<u8>, Error> {
    let cache_hit = cache_key.exists();

    let data = if download_server_client.connectivity == Connectivity::Offline {
        if cache_hit {
            fs::read(&cache_key).map_err(|e| Error::Read(cache_key, e))?
        } else {
            return Err(Error::OfflineMode);
        }
    } else {
        let req = cacheable_request(download_server_client, &url, &cache_key, cache_hit).await?;

        let resp = download_server_client
            .client
            .execute(req)
            .await
            .map_err(|e| Error::DownloadServerError {
                url: url.clone(),
                kind: DownloadServerError::NetworkWithMiddleware(e),
            })?;

        match resp.status() {
            StatusCode::OK => {
                tracing::trace!(status = %resp.status(), "Downloading");
                let data = resp.bytes().await?;
                if let Some(parent) = cache_key.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| Error::Create(parent.to_path_buf(), e))?;
                }
                tokio_fs::write(&cache_key, &data)
                    .await
                    .map_err(|e| Error::Write(cache_key, e))?;
                data.to_vec()
            }
            StatusCode::NOT_MODIFIED => {
                tracing::trace!(status = %resp.status(), "Cache is fresh & valid");
                tokio_fs::read(&cache_key)
                    .await
                    .map_err(|e| Error::Read(cache_key, e))?
            }
            _ => {
                tracing::trace!(status = %resp.status(), "Unexpected status");
                return Err(unexpected_status(url, resp));
            }
        }
    };

    Ok(data)
}

pub(crate) async fn cacheable_request(
    download_server_client: &DownloadServerClient,
    url: impl IntoUrl,
    cache_key: impl AsRef<Path>,
    cache_hit: bool,
) -> Result<Request, Error> {
    let cache_key = cache_key.as_ref();
    let mut req = download_server_client.client.get(url);
    if let Some(auth_token) = download_server_client.auth_token().await? {
        req = req.header(AUTHORIZATION, auth_token);
    }
    if cache_hit {
        let cache_content =
            fs::read(cache_key).map_err(|e| Error::Read(cache_key.to_path_buf(), e))?;
        let mut hasher = Md5::new();
        hasher.update(cache_content);
        let etag_md5 = format!(r#""{:x}""#, hasher.finalize());
        req = req.header("If-None-Match", HeaderValue::from_str(&etag_md5).unwrap());
        tracing::trace!(cache_key = %cache_key.display(), etag = %etag_md5, "Got cached");
    }
    let req_built = req.build()?;
    Ok(req_built)
}
