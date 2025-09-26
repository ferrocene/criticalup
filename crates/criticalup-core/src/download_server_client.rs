// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::Config;
use crate::envvars;
use crate::errors::{DownloadServerError, Error};
use crate::state::{AuthenticationToken, State};
use criticaltrust::keys::PublicKey;
use criticaltrust::manifests::ReleaseArtifactFormat;
use criticaltrust::manifests::ReleaseManifest;
use criticaltrust::signatures::Keychain;
use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::StatusCode;
use reqwest::{IntoUrl, Request, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use serde::Deserialize;
use sha2::Digest;
use tokio::fs::{self, create_dir_all};

const CLIENT_MAX_RETRIES: u32 = 5;

pub struct DownloadServerClient {
    cache_dir: PathBuf,
    base_url: String,
    client: ClientWithMiddleware,
    state: State,
    trust_root: PublicKey,
    connectivity: Connectivity,
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

        DownloadServerClient {
            base_url: config.whitelabel.download_server_url.clone(),
            client,
            state: state.clone(),
            trust_root: config.whitelabel.trust_root.clone(),
            cache_dir: config.paths.cache_dir.clone(),
            connectivity,
        }
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

    fn keys_cache_path(&self) -> PathBuf {
        self.cache_dir.join("keys.json")
    }

    async fn product_release_manifest_cache_path(&self, product: &str, release: &str) -> PathBuf {
        self.product_release_cache_path(product, release)
            .await
            .join("manifest.json")
    }

    async fn product_release_cache_path(&self, product: &str, release: &str) -> PathBuf {
        let old_cache_dir = self.cache_dir.join("artifacts").join(product).join(release);

        let new_cache_dir = self
            .cache_dir
            .join("artifacts")
            .join("products")
            .join(product)
            .join("releases")
            .join(release);

        if old_cache_dir.exists() {
            self.migration(&old_cache_dir, &new_cache_dir).await;
        }

        new_cache_dir
    }

    async fn migration(&self, old_cache_dir: &PathBuf, new_cache_dir: &PathBuf) {
        let Some(new_cache_dir_parent) = new_cache_dir.parent() else {
            panic!("I need to be handled");
        };

        if let Err(e) = fs::create_dir_all(new_cache_dir_parent.clone()).await {
            tracing::debug!("Failed to created {} with {}", new_cache_dir.display(), e);
        }

        // If an old cache exist, we try moving its contents or delete it, or we fail silently.
        if old_cache_dir.as_os_str().is_empty() {
            if let Err(e) = fs::remove_dir(&old_cache_dir).await {
                tracing::debug!(
                    "Failed to remove empty cache {}: {}",
                    old_cache_dir.display(),
                    e
                );
            }
        } else if let Err(e) = fs::rename(&old_cache_dir, &new_cache_dir).await {
            tracing::debug!(
                "Failed to move {} to {}: {}",
                old_cache_dir.display(),
                &new_cache_dir.display(),
                e
            );
        }
        // We also want to remove the parent:
        if let Some(parent) = old_cache_dir.parent() {
            if let Err(e) = fs::remove_dir(parent).await {
                tracing::debug!("Failing in removing {}, {}", parent.display(), e);
            }
        }
    }
    async fn package_cache_path(
        &self,
        product: &str,
        release: &str,
        package: &str,
        format: ReleaseArtifactFormat,
    ) -> PathBuf {
        self.product_release_cache_path(product, release)
            .await
            .join({
                let mut file_name = PathBuf::from(package);
                file_name.set_extension(format.to_string());
                file_name
            })
    }

    async fn cacheable(&self, url: String, cache_key: PathBuf) -> Result<Vec<u8>, Error> {
        let cache_hit = cache_key.exists();

        let data = if self.connectivity == Connectivity::Offline {
            if cache_hit {
                fs::read(&cache_key)
                    .await
                    .map_err(|e| Error::Read(cache_key, e))?
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
                        create_dir_all(parent)
                            .await
                            .map_err(|e| Error::Create(parent.to_path_buf(), e))?;
                    }
                    fs::write(&cache_key, &data)
                        .await
                        .map_err(|e| Error::Write(cache_key, e))?;
                    data.to_vec()
                }
                StatusCode::NOT_MODIFIED => {
                    tracing::trace!(status = %resp.status(), "Cache is fresh & valid");
                    fs::read(&cache_key)
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

    async fn cacheable_request(
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
            let cache_content = fs::read(cache_key)
                .await
                .map_err(|e| Error::Read(cache_key.to_path_buf(), e))?;
            let mut hasher = md5::Md5::new();
            hasher.update(cache_content);
            let etag_md5 = format!(r#""{:x}""#, hasher.finalize());
            req = req.header("If-None-Match", HeaderValue::from_str(&etag_md5).unwrap());
            tracing::trace!(cache_key = %cache_key.display(), etag = %etag_md5, "Got cached");
        }
        let req_built = req.build()?;
        Ok(req_built)
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub async fn keys(&self) -> Result<Keychain, Error> {
        let url = self.url("/v1/keys");
        let cache_key = self.keys_cache_path();

        let data = self.cacheable(url, cache_key).await?;
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
        let cache_key = self
            .product_release_manifest_cache_path(product, release)
            .await;

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
        let cache_key = self
            .package_cache_path(product, release, package, format)
            .await;

        tracing::info!("Downloading component '{package}' for '{product}' ({release})",);
        let data = self.cacheable(url, cache_key).await?;

        Ok(data)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    async fn auth_token(&self) -> Result<Option<HeaderValue>, Error> {
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

fn unexpected_status(url: String, response: Response) -> Error {
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
    use crate::state::AuthenticationToken;
    use crate::test_utils::{
        TestEnvironment, SAMPLE_AUTH_TOKEN_CUSTOMER, SAMPLE_AUTH_TOKEN_EXPIRY,
        SAMPLE_AUTH_TOKEN_NAME,
    };
    use criticaltrust::keys::KeyPair;
    use criticaltrust::signatures::PublicKeysRepository;
    use md5::Md5;
    use reqwest::header::IF_NONE_MATCH;
    use tokio::fs::write;

    #[tokio::test]
    async fn test_cacheable_requests_set_if_none_match() {
        let test_env = TestEnvironment::with().download_server().prepare().await;

        let test_path = test_env.config().paths.cache_dir.join("tester");
        let test_url = test_env.download_server().url("/tester");

        // Does not yet exist
        let req = test_env
            .download_server()
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
            create_dir_all(parent).await.unwrap()
        }
        write(&test_path, test_slug).await.unwrap();
        let req = test_env
            .download_server()
            .cacheable_request(&test_url, &test_path, true)
            .await
            .unwrap();
        assert_eq!(req.headers().get(IF_NONE_MATCH), Some(&test_hash));
    }

    #[tokio::test]
    async fn cache_content_is_migrated() {
        let test_env = TestEnvironment::with().download_server().prepare().await;
        // creating a file at the incorrect path
        let cache_dir = test_env.config().paths.cache_dir.clone();
        let old_path = cache_dir
            .join("artifacts")
            .join("ferrocene")
            .join("stable-25.05.0");
        let expected = cache_dir
            .join("artifacts")
            .join("products")
            .join("ferrocene")
            .join("releases")
            .join("stable-25.05.0");
        assert!(!expected.exists()); // The new path should not exist yet

        fs::create_dir_all(old_path.clone()).await.unwrap();

        test_env
            .download_server()
            // the tested function that migrates the cache
            .product_release_cache_path("ferrocene", "stable-25.05.0")
            .await;

        assert!(expected.exists());
        // we assert parent in old path was deleted
        assert!(!cache_dir.join("artifacts").join("ferrocene").exists());
    }

    #[tokio::test]
    async fn cache_is_constructed() {
        let test_env = TestEnvironment::with().download_server().prepare().await;
        let res = test_env
            .download_server()
            .product_release_cache_path("ferrocene", "stable-25.05.0")
            .await;

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
}
