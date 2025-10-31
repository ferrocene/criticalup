// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::cache::{
    keys_cache_path, package_cache_path, product_release_manifest_cache_path,
    try_migrating_deprecated_path,
};
use crate::config::Config;
use crate::envvars;
use crate::errors::{DownloadServerError, Error};
use crate::state::{AuthenticationToken, State};
use criticaltrust::keys::PublicKey;
use criticaltrust::manifests::{ReleaseArtifactFormat, ReleaseManifest};
use criticaltrust::signatures::Keychain;
use md5::Md5;
use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::{IntoUrl, Request, Response, StatusCode};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use serde::Deserialize;
use sha2::Digest;
use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs as tokio_fs;

const CLIENT_MAX_RETRIES: u32 = 5;

pub struct DownloadServerClient {
    pub(crate) cache_dir: PathBuf,
    base_url: String,
    pub(crate) client: ClientWithMiddleware,
    state: State,
    trust_root: PublicKey,
    pub(crate) connectivity: Connectivity,
}

impl DownloadServerClient {
    pub fn new(config: &Config, state: &State, connectivity: Connectivity) -> Self {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(CLIENT_MAX_RETRIES);
        let client = reqwest::ClientBuilder::new()
            .user_agent(config.whitelabel.http_user_agent)
            .read_timeout(Duration::from_secs(90))
            .connect_timeout(Duration::from_secs(90))
            .pool_idle_timeout(Duration::from_secs(90))
            // In rare cases we were encountering a hang in networking in Docker-in-Docker
            // `docker buildx build` situations. This workaround seems to help.
            // ref: https://github.com/hyperium/hyper/issues/2312#issuecomment-778005053
            .pool_max_idle_per_host(0)
            .build()
            .expect("failed to configure http client");
        let client = ClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        let download_server_client = DownloadServerClient {
            base_url: config.whitelabel.download_server_url.clone(),
            client,
            state: state.clone(),
            trust_root: config.whitelabel.trust_root.clone(),
            cache_dir: config.paths.cache_dir.clone(),
            connectivity,
        };

        // Trying migrating the obsolete cache.
        // old path: ~/.cache/criticalup/artifacts/ferrocene
        let deprecated_product_name_path = download_server_client
            .cache_dir
            .join("artifacts")
            .join("ferrocene");
        // new destination path ~/.cache/criticalup/artifacts/products/ferrocene/releases
        let new_product_name_path = download_server_client
            .cache_dir
            .join("artifacts")
            .join("products")
            .join("ferrocene")
            .join("releases");
        // We want to do the migration iff:
        // 1. the deprecated path exists and we have premissions
        // 2. the new path was never created (to not overwrite existing cache)
        // This function returns three types of errors,
        // when writing, creating or not being able to migrate.
        if matches!(deprecated_product_name_path.try_exists(), Ok(true)) {
            try_migrating_deprecated_path(
                deprecated_product_name_path.clone(),
                new_product_name_path.clone(),
            )
            .inspect_err(|e| tracing::warn!("{:?}", e))
            .ok();
        }

        download_server_client
    }

    pub async fn get_current_token_data(&self) -> Result<CurrentTokenData, Error> {
        let url = self.url("/v1/tokens/current");

        let mut req = self.client.get(&url);
        if let Some(auth_token) = self.auth_token().await? {
            req = req.header(AUTHORIZATION, auth_token);
        } else {
            return Err(Error::DownloadServerError {
                url: url.clone(),
                kind: DownloadServerError::AuthenticationFailed,
            });
        }

        let resp = req.send().await.map_err(|e| Error::DownloadServerError {
            url: url.clone(),
            kind: DownloadServerError::NetworkWithMiddleware(e),
        })?;
        match resp.status() {
            StatusCode::OK => {
                let data = resp.bytes().await?;
                let token_data = serde_json::from_slice(&data).map_err(Error::JsonSerialization)?;
                Ok(token_data)
            }
            _ => Err(unexpected_status(url, resp)),
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub async fn keys(&self) -> Result<Keychain, Error> {
        let url = self.url("/v1/keys");
        let cache_key = keys_cache_path(&self.cache_dir);

        let data: Vec<u8> = self.cacheable(url, cache_key).await?;
        let keys_manifest = serde_json::from_slice(&data).map_err(Error::JsonSerialization)?;

        let mut keychain = Keychain::new(&self.trust_root).map_err(Error::KeychainInitFailed)?;
        let _ = keychain.load_all(&keys_manifest);
        Ok(keychain)
    }

    #[tracing::instrument(level = "trace", skip_all, fields(
        %product,
        %release,
    ))]
    pub async fn product_release_manifest(
        &self,
        product: &str,
        release: &str,
    ) -> Result<ReleaseManifest, Error> {
        let url = self.url(&format!("/v1/releases/{product}/{release}"));
        let cache_key = product_release_manifest_cache_path(&self.cache_dir, product, release);

        let data = self.cacheable(url, cache_key).await?;

        serde_json::from_slice(&data).map_err(Error::JsonSerialization)
    }

    #[tracing::instrument(level = "trace", skip_all, fields(
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
    ) -> Result<Vec<u8>, Error> {
        let artifact_format = format.to_string();
        let url = self.url(&format!(
            "/v1/releases/{product}/{release}/download/{package}/{artifact_format}"
        ));
        let cache_key = package_cache_path(&self.cache_dir, product, release, package, format);
        tracing::info!("Downloading component '{package}' for '{product}' ({release})",);
        let data = self.cacheable(url, cache_key).await?;

        Ok(data)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }
    pub(crate) async fn cacheable(
        &self,
        url: String,
        cache_key: PathBuf,
    ) -> Result<Vec<u8>, Error> {
        let cache_hit = cache_key.exists();

        let data = if self.connectivity == Connectivity::Offline {
            if cache_hit {
                fs::read(&cache_key).map_err(|e| Error::Read(cache_key, e))?
            } else {
                return Err(Error::OfflineMode);
            }
        } else {
            let req = self.cacheable_request(&url, &cache_key, cache_hit).await?;

            let resp = self
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
        &self,
        url: impl IntoUrl,
        cache_key: impl AsRef<Path>,
        cache_hit: bool,
    ) -> Result<Request, Error> {
        let cache_key = cache_key.as_ref();
        let mut req = self.client.get(url);
        if let Some(auth_token) = self.auth_token().await? {
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

    pub(crate) async fn auth_token(&self) -> Result<Option<HeaderValue>, Error> {
        let token_from_env: Option<AuthenticationToken> = envvars::EnvVars::new()
            .criticalup_token
            .map(|item| item.into());

        let token_from_state = self.state.authentication_token().await;

        // Set precedence for tokens.
        let token = match (token_from_env, token_from_state) {
            (Some(token), _) => {
                tracing::trace!("Using token from `CRITICALUP_TOKEN` environment variable");
                Some(token)
            }
            (_, Some(token)) => {
                tracing::trace!("Using token from state");
                Some(token)
            }
            _ => None,
        };

        if let Some(token) = token {
            Ok(Some(
                HeaderValue::from_str(&format!("Bearer {}", token.unseal()))
                    .map_err(|_| Error::InvalidAuthenicationToken)?,
            ))
        } else {
            Ok(None)
        }
    }

    /// Sets the base url of this [`DownloadServerClient`].
    pub fn set_base_url(&mut self, base_url: String) {
        self.base_url = base_url;
    }

    /// Returns a reference to the base url of this [`DownloadServerClient`].
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

pub(crate) fn unexpected_status(url: String, response: Response) -> Error {
    let kind = match response.status() {
        StatusCode::BAD_REQUEST => DownloadServerError::BadRequest,
        StatusCode::FORBIDDEN => DownloadServerError::AuthenticationFailed,
        StatusCode::NOT_FOUND => DownloadServerError::NotFound,
        StatusCode::TOO_MANY_REQUESTS => DownloadServerError::RateLimited,

        s if s.is_server_error() => DownloadServerError::InternalServerError(s),
        s => DownloadServerError::UnexpectedResponseStatus(s),
    };
    Error::DownloadServerError { url, kind }
}

#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub enum Connectivity {
    #[default]
    Online,
    Offline,
}

impl Display for Connectivity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Connectivity::Offline => f.write_str("Offline"),
            Connectivity::Online => f.write_str("Online"),
        }
    }
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
#[serde(rename_all = "kebab-case")]
pub struct CurrentTokenData {
    pub name: String,
    pub organization_name: String,
    pub expires_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::product_release_cache_path;
    use crate::state::AuthenticationToken;
    use crate::test_utils::{
        TestEnvironment, SAMPLE_AUTH_TOKEN_CUSTOMER, SAMPLE_AUTH_TOKEN_EXPIRY,
        SAMPLE_AUTH_TOKEN_NAME,
    };
    use criticaltrust::keys::KeyPair;
    use criticaltrust::signatures::PublicKeysRepository;
    use md5::Md5;
    use reqwest::header::IF_NONE_MATCH;
    use sha2::Digest;
    use std::fs;
    use tempfile::tempdir;
    use tokio::fs::write;

    #[tokio::test]
    async fn test_cacheable_requests_set_if_none_match() {
        let test_env = TestEnvironment::with().download_server().prepare().await;

        let test_path = test_env.config().paths.cache_dir.join("tester");
        let test_url = test_env.download_server().url("/tester");

        // Does not yet exist
        let download_server_client = test_env.download_server();
        let req = download_server_client
            .cacheable_request(&test_url, &test_path, false)
            .await
            .unwrap();
        assert!(req.headers().get(IF_NONE_MATCH).is_none());

        // Create it, so we should include the header
        let test_slug = "Cute dogs with boopable snoots";
        let mut hasher = Md5::new();
        hasher.update(test_slug);
        let test_hash = HeaderValue::from_str(&format!(r#""{:x}""#, hasher.finalize())).unwrap();
        if let Some(parent) = test_path.parent() {
            fs::create_dir_all(parent).unwrap()
        }
        write(&test_path, test_slug).await.unwrap();
        let download_server_client = test_env.download_server();

        let req = download_server_client
            .cacheable_request(&test_url, &test_path, true)
            .await
            .unwrap();
        assert_eq!(req.headers().get(IF_NONE_MATCH), Some(&test_hash));
    }

    #[tokio::test]
    async fn cache_is_constructed() {
        let test_env = TestEnvironment::with().download_server().prepare().await;
        let download_server_client = test_env.download_server();
        let res = product_release_cache_path(
            &download_server_client.cache_dir,
            "ferrocene",
            "stable-25.05.0",
        );

        let cache_dir: PathBuf = test_env.config().paths.cache_dir.clone();
        let expected = "artifacts/products/ferrocene/releases/stable-25.05.0";
        let cache_dir = cache_dir.join(expected);
        assert_eq!(cache_dir, res);
    }

    #[tokio::test]
    async fn test_get_current_token_while_authenticated() {
        let test_env = TestEnvironment::with().download_server().prepare().await;

        assert_eq!(
            CurrentTokenData {
                name: SAMPLE_AUTH_TOKEN_NAME.into(),
                organization_name: SAMPLE_AUTH_TOKEN_CUSTOMER.into(),
                expires_at: Some(SAMPLE_AUTH_TOKEN_EXPIRY.into()),
            },
            test_env
                .download_server()
                .get_current_token_data()
                .await
                .unwrap(),
        );
        assert_eq!(1, test_env.requests_served_by_mock_download_server().await);
    }

    #[tokio::test]
    async fn test_get_current_token_with_unrepresentable_token() {
        let test_env = TestEnvironment::with().download_server().prepare().await;
        test_env
            .state()
            .set_authentication_token(Some(AuthenticationToken::seal("wrong\0")));

        assert!(matches!(
            test_env
                .download_server()
                .get_current_token_data()
                .await
                .unwrap_err(),
            Error::InvalidAuthenicationToken,
        ));

        // No request was actually made since the authentication token can't be represented in
        // HTTP headers.
        assert_eq!(0, test_env.requests_served_by_mock_download_server().await);
    }

    #[tokio::test]
    async fn test_get_current_token_with_wrong_token() {
        let test_env = TestEnvironment::with().download_server().prepare().await;
        test_env
            .state()
            .set_authentication_token(Some(AuthenticationToken::seal("wrong")));
        assert_auth_failed(&test_env).await;

        assert_eq!(1, test_env.requests_served_by_mock_download_server().await);
    }

    #[tokio::test]
    async fn test_get_current_token_with_no_token() {
        let test_env = TestEnvironment::with().download_server().prepare().await;
        test_env.state().set_authentication_token(None);
        assert_auth_failed(&test_env).await;

        // No token was configured, so no request could've been made.
        assert_eq!(0, test_env.requests_served_by_mock_download_server().await);
    }

    #[tokio::test]
    async fn test_get_keys() {
        let test_env = TestEnvironment::with().download_server().prepare().await;
        test_env.state().set_authentication_token(None); // The endpoint requires no authentication.

        let keys = test_env.keys();

        let keychain = test_env.download_server().keys().await.unwrap();

        assert_eq!(
            {
                let history = test_env.history().await;
                let (_, last_res) = history.last().unwrap();
                last_res.status()
            },
            StatusCode::OK,
        );

        for expected_present in &[
            // Trust root included from the whitelabel config
            &keys.trust_root,
            // Retrieved from the download server
            &keys.root,
            &keys.packages,
            &keys.releases,
            &keys.redirects,
            &keys.revocation,
        ] {
            assert!(keychain
                .get(&expected_present.public().calculate_id())
                .is_some());
        }

        for expected_missing in &[
            // Not served or provided anywhere
            &keys.alternate_trust_root,
            // Retrieved from the download server
            &keys.alternate_root,
            &keys.alternate_packages,
        ] {
            assert!(keychain
                .get(&expected_missing.public().calculate_id())
                .is_none());
        }
    }

    async fn assert_auth_failed(test_env: &TestEnvironment) {
        assert!(matches!(
            test_env
                .download_server()
                .get_current_token_data()
                .await
                .unwrap_err(),
            Error::DownloadServerError {
                kind: DownloadServerError::AuthenticationFailed,
                ..
            },
        ));
    }
    #[test]
    fn assert_cache_is_migrated() -> Result<(), Box<dyn std::error::Error>> {
        let cache_path = tempdir()?;

        let deprecated_product_name_path = cache_path
            .path()
            .join("artifacts")
            .join("ferrocene")
            .join("stable-25.05.0");
        fs::create_dir_all(&deprecated_product_name_path)?;
        assert!(&deprecated_product_name_path.exists());
        assert!(&deprecated_product_name_path.is_dir());
        let old_path = &deprecated_product_name_path.join("package.txt");
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(old_path)?;

        let new_products_path = cache_path
            .path()
            .join("artifacts")
            .join("products")
            .join("ferrocene")
            .join("releases")
            .join("stable-25.05.0");
        let res = try_migrating_deprecated_path(
            deprecated_product_name_path.clone(),
            new_products_path.clone(),
        );
        assert!(res.is_ok());
        assert!(&new_products_path.exists());
        assert!(!&deprecated_product_name_path.exists());
        assert!(new_products_path.is_dir());
        let migrated_product_path = &new_products_path.join("package.txt");
        assert!(&migrated_product_path.exists());
        assert!(&migrated_product_path.is_file());
        cache_path.close()?;
        Ok(())
    }

    #[test]
    fn assert_cache_is_not_overwritten() -> Result<(), Box<dyn std::error::Error>> {
        let cache_path = tempdir()?;
        let deprecated_product_name_path = cache_path.path().join("artifacts").join("ferrocene");
        fs::create_dir_all(&deprecated_product_name_path)?;

        let new_products_path = cache_path.path().join("artifacts").join("products");
        // This will return an error as we do not want to overwrite an
        // existing fresh cache.
        fs::create_dir_all(&new_products_path)?;
        let res = try_migrating_deprecated_path(
            deprecated_product_name_path.clone(),
            new_products_path.clone(),
        );
        assert!(res.is_err());
        Ok(())
    }
}
