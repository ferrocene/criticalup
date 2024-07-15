// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::config::Config;
use crate::errors::{DownloadServerError, Error};
use crate::state::State;
use criticaltrust::keys::PublicKey;
use criticaltrust::manifests::ReleaseManifest;
use criticaltrust::manifests::{KeysManifest, ReleaseArtifactFormat};
use criticaltrust::signatures::Keychain;
use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::StatusCode;
use reqwest::{Client, RequestBuilder, Response, Url};
use serde::Deserialize;
use std::thread;
use std::time::Duration;

const CLIENT_TIMEOUT_SECONDS: u64 = 30;
const CLIENT_MAX_RETRIES: u8 = 5;
const CLIENT_RETRY_BACKOFF: u64 = 100;

pub struct DownloadServerClient {
    base_url: String,
    client: Client,
    state: State,
    trust_root: PublicKey,
}

impl DownloadServerClient {
    pub fn new(config: &Config, state: &State) -> Self {
        let client = Client::builder()
            .user_agent(config.whitelabel.http_user_agent)
            // Do not call `.timeout(Some(short_number))` as some slow connections may take awhile to download the full request body.
            .connect_timeout(Duration::from_secs(CLIENT_TIMEOUT_SECONDS))
            .read_timeout(Duration::from_secs(CLIENT_TIMEOUT_SECONDS))
            .build()
            .expect("failed to configure http client");

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

    pub async fn get_keys(&self) -> Result<Keychain, Error> {
        let mut keychain = Keychain::new(&self.trust_root).map_err(Error::KeychainInitFailed)?;

        let resp: KeysManifest = self
            .json(self.send(self.client.get(self.url("/v1/keys"))).await?)
            .await?;
        for key in &resp.keys {
            // Invalid keys are silently ignored, as they might be signed by a different root key
            // used by a different release of criticalup, or they might be using an algorithm not
            // supported by the current version of criticaltrust.
            let _ = keychain.load(key);
        }

        Ok(keychain)
    }

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

        // set path to token file for docker
        let path_to_token_file = if std::path::Path::new("/.dockerenv").exists() {
            Some("/run/secrets/CRITICALUP_TOKEN")
        } else {
            None
        };

        let header = self
            .state
            .authentication_token(path_to_token_file)
            .as_ref()
            .and_then(|token| HeaderValue::from_str(&format!("Bearer {}", token.unseal())).ok());

        match header {
            Some(header) => self.send(builder.header(AUTHORIZATION, header)).await,
            None => Err(self.err_from_request(builder, DownloadServerError::AuthenticationFailed)),
        }
    }

    async fn send(&self, builder: RequestBuilder) -> Result<Response, Error> {
        let req = builder.build().expect("failed to prepare the http request");
        let url = req.url().to_string();

        // This section implements the logic for retrying a failed request.
        // This logic will be superseded by changes to this code base when we move this to async.
        let mut current_retries: u8 = 0;
        let mut current_retry_backoff = CLIENT_RETRY_BACKOFF;

        // This `try_clone()` will be `None` if request body is a stream.
        let req_outer = req.try_clone().ok_or(Error::RequestCloningFailed)?;
        let mut response_result = self.client.execute(req_outer).await;

        while current_retries < CLIENT_MAX_RETRIES && response_result.is_err() {
            thread::sleep(Duration::from_millis(current_retry_backoff));

            let req = req.try_clone().ok_or(Error::RequestCloningFailed)?;

            response_result = self.client.execute(req).await;
            if response_result.is_ok() {
                break;
            }

            current_retries += 1;
            current_retry_backoff *= 2;
        }

        let response = response_result.map_err(|e| Error::DownloadServerError {
            kind: DownloadServerError::Network(e),
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
        let test_env = TestEnvironment::with().download_server().prepare();

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
        let test_env = TestEnvironment::with().download_server().prepare();
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
        let test_env = TestEnvironment::with().download_server().prepare();
        test_env
            .state()
            .set_authentication_token(Some(AuthenticationToken::seal("wrong")));
        assert_auth_failed(&test_env).await;

        assert_eq!(1, test_env.requests_served_by_mock_download_server());
    }

    #[tokio::test]
    async fn test_get_current_token_with_no_token() {
        let test_env = TestEnvironment::with().download_server().prepare();
        test_env.state().set_authentication_token(None);
        assert_auth_failed(&test_env).await;

        // No token was configured, so no request could've been made.
        assert_eq!(0, test_env.requests_served_by_mock_download_server());
    }

    #[tokio::test]
    async fn test_get_keys() {
        let test_env = TestEnvironment::with().download_server().prepare();
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
