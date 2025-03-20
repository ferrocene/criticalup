// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::Path;

use crate::config::Config;
use crate::envvars;
use crate::errors::{DownloadServerError, Error};
use crate::state::{AuthenticationToken, State};
use criticaltrust::keys::PublicKey;
use criticaltrust::manifests::ReleaseManifest;
use criticaltrust::manifests::{KeysManifest, ReleaseArtifactFormat};
use criticaltrust::signatures::Keychain;
use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::StatusCode;
use reqwest::{Response, Url};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, RequestBuilder};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use serde::Deserialize;
use tokio::fs::read_to_string;

const CLIENT_MAX_RETRIES: u32 = 5;

pub struct DownloadServerClient {
    base_url: String,
    client: ClientWithMiddleware,
    state: State,
    trust_root: PublicKey,
}

impl DownloadServerClient {
    pub fn new(config: &Config, state: &State) -> Self {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(CLIENT_MAX_RETRIES);
        let client = reqwest::ClientBuilder::new()
            .user_agent(config.whitelabel.http_user_agent)
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
        }
    }

    pub async fn get_current_token_data(&self) -> Result<CurrentTokenData, Error> {
        self.json(
            self.send_with_auth(self.client.get(self.url("/v1/tokens/current")))
                .await?,
        )
        .await
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn get_keys(&self) -> Result<Keychain, Error> {
        let resp: KeysManifest = self
            .json(self.send(self.client.get(self.url("/v1/keys"))).await?)
            .await?;
        let mut keychain = Keychain::new(&self.trust_root).map_err(Error::KeychainInitFailed)?;
        let _ = keychain.load_all(&resp);
        Ok(keychain)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(
        %product,
        %release,
    ))]
    pub async fn get_product_release_manifest(
        &self,
        product: &str,
        release: &str,
    ) -> Result<ReleaseManifest, Error> {
        let p = format!("/v1/releases/{product}/{release}");
        self.json(
            self.send_with_auth(self.client.get(self.url(p.as_str())))
                .await?,
        )
        .await
    }

    #[tracing::instrument(level = "debug", skip_all, fields(
        %product,
        %release,
        %package,
        %format
    ))]
    pub async fn download_package(
        &self,
        product: &str,
        release: &str,
        package: &str,
        format: ReleaseArtifactFormat,
    ) -> Result<Vec<u8>, Error> {
        let artifact_format = format.to_string();

        let download_url =
            format!("/v1/releases/{product}/{release}/download/{package}/{artifact_format}");

        tracing::info!("Downloading component '{package}' for '{product}' ({release})",);
        let response = self
            .send_with_auth(self.client.get(self.url(download_url.as_str())))
            .await?;
        let resp_body = response.bytes().await?.to_vec();
        Ok(resp_body)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    async fn send_with_auth(&self, builder: RequestBuilder) -> Result<Response, Error> {
        // We're constructing the `HeaderValue` manually instead of using the `bearer_token` method
        // of `RequestBuilder` as the latter panics when it receives a token not representable
        // inside HTTP headers (for example containing the `\r` byte).
        //
        // If the token contains such chars treat the authentication as failed due to an invalid
        // token, as the server wouldn't be able to validate it either anyway.

        // Get token from file path.
        let docker_env_file = Path::new("/.dockerenv");
        let criticalup_token_secret_file = Path::new("/run/secrets/CRITICALUP_TOKEN");
        let token_from_file = if docker_env_file.exists() {
            tracing::trace!("Detected `{}`, in a Docker environment, looking for `{}`", docker_env_file.display(), criticalup_token_secret_file.display());
            let token = read_to_string(criticalup_token_secret_file).await
                .map_or(None, |item| Some(AuthenticationToken::from(item)));
            tracing::trace!("Got token from Docker secret from `{}`", criticalup_token_secret_file.display());
            token
        } else {
            None
        };

        let token_from_env = envvars::EnvVars::new()
            .criticalup_token
            .map(|item| item.into());

        let token_from_state = self.state.authentication_token().await;

        // Set precedence for tokens.
        let token = match (token_from_file, token_from_env, token_from_state) {
            (Some(token), _, _) => {
                tracing::trace!("Using token from {}", criticalup_token_secret_file.display());
                Some(token)
            },
            (_, Some(token), _) => {
                tracing::trace!("Using token from environment variable `CRITICALUP_TOKEN`");
                Some(token)
            },
            (_, _, Some(token)) => {
                tracing::trace!("Using token from CriticalUp state file");
                Some(token)
            },
            _ => None,
        };

        let header = token
            .as_ref()
            .and_then(|token| HeaderValue::from_str(&format!("Bearer {}", token.unseal())).ok());

        match header {
            Some(header) => {
                tracing::trace!("Sending request");
                let res = self.send(builder.header(AUTHORIZATION, header)).await;
                tracing::trace!("Got response");
                res
            },
            None => Err(self.err_from_request(builder, DownloadServerError::AuthenticationFailed)),
        }
    }

    async fn send(&self, builder: RequestBuilder) -> Result<Response, Error> {
        let req = builder.build().expect("failed to prepare the http request");
        let url = req.url().to_string();

        let response_result = self.client.execute(req).await;

        let response = response_result.map_err(|e| Error::DownloadServerError {
            kind: DownloadServerError::NetworkWithMiddleware(e),
            url,
        })?;

        Err(self.err_from_response(
            response.url(),
            match response.status() {
                StatusCode::OK => return Ok(response),

                StatusCode::BAD_REQUEST => DownloadServerError::BadRequest,
                StatusCode::FORBIDDEN => DownloadServerError::AuthenticationFailed,
                StatusCode::NOT_FOUND => DownloadServerError::NotFound,
                StatusCode::TOO_MANY_REQUESTS => DownloadServerError::RateLimited,

                s if s.is_server_error() => DownloadServerError::InternalServerError(s),
                s => DownloadServerError::UnexpectedResponseStatus(s),
            },
        ))
    }

    async fn json<T: for<'de> Deserialize<'de>>(&self, response: Response) -> Result<T, Error> {
        let url = response.url().clone();
        response
            .json()
            .await
            .map_err(|e| self.err_from_response(&url, DownloadServerError::Network(e)))
    }

    fn err_from_request(&self, builder: RequestBuilder, kind: DownloadServerError) -> Error {
        Error::DownloadServerError {
            kind,
            url: builder
                .build()
                .expect("failed to prepare the http request")
                .url()
                .to_string(),
        }
    }

    fn err_from_response(&self, url: &Url, kind: DownloadServerError) -> Error {
        Error::DownloadServerError {
            kind,
            url: url.to_string(),
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
        assert_eq!(1, test_env.requests_served_by_mock_download_server());
    }

    #[tokio::test]
    async fn test_get_current_token_with_unrepresentable_token() {
        let test_env = TestEnvironment::with().download_server().prepare().await;
        test_env
            .state()
            .set_authentication_token(Some(AuthenticationToken::seal("wrong\0")));
        assert_auth_failed(&test_env).await;

        // No request was actually made since the authentication token can't be represented in
        // HTTP headers.
        assert_eq!(0, test_env.requests_served_by_mock_download_server());
    }

    #[tokio::test]
    async fn test_get_current_token_with_wrong_token() {
        let test_env = TestEnvironment::with().download_server().prepare().await;
        test_env
            .state()
            .set_authentication_token(Some(AuthenticationToken::seal("wrong")));
        assert_auth_failed(&test_env).await;

        assert_eq!(1, test_env.requests_served_by_mock_download_server());
    }

    #[tokio::test]
    async fn test_get_current_token_with_no_token() {
        let test_env = TestEnvironment::with().download_server().prepare().await;
        test_env.state().set_authentication_token(None);
        assert_auth_failed(&test_env).await;

        // No token was configured, so no request could've been made.
        assert_eq!(0, test_env.requests_served_by_mock_download_server());
    }

    #[tokio::test]
    async fn test_get_keys() {
        let test_env = TestEnvironment::with().download_server().prepare().await;
        test_env.state().set_authentication_token(None); // The endpoint requires no authentication.

        let keys = test_env.keys();
        let keychain = test_env.download_server().get_keys().await.unwrap();

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
