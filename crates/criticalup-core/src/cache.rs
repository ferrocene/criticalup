use crate::download_server_client::DownloadServerClient;
use crate::errors::Error;
use criticaltrust::manifests::ReleaseArtifactFormat;
use std::fs;
use std::path::{Path, PathBuf};

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
