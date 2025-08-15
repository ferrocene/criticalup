// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::config::Config;
use crate::download_server_client::{Connectivity, DownloadServerClient};
use crate::state::{AuthenticationToken, State};
use axum::http::{Request, Response};
use criticaltrust::keys::{EphemeralKeyPair, KeyAlgorithm, KeyPair, KeyRole, PublicKey};
use criticaltrust::signatures::SignedPayload;
use mock_download_server::MockServer;
use std::path::Path;
use tempfile::TempDir;
use tokio::sync::MappedMutexGuard;

pub(crate) const SAMPLE_AUTH_TOKEN: &str = "criticalup_token_foo";
pub(crate) const SAMPLE_AUTH_TOKEN_NAME: &str = "token name";
pub(crate) const SAMPLE_AUTH_TOKEN_CUSTOMER: &str = "internal";
pub(crate) const SAMPLE_AUTH_TOKEN_EXPIRY: &str = "2022-01-01T00:00:00+00:00";

pub(crate) struct TestEnvironment {
    root: TempDir,
    config: Config,
    state: Option<State>,
    download_server: Option<DownloadServerClient>,
    keys: Option<TestKeys>,

    mock_server: Option<MockServer>,
}

impl TestEnvironment {
    pub(crate) fn with() -> TestEnvironmentBuilder {
        TestEnvironmentBuilder {
            state: false,
            download_server: false,
            keys: false,
            root_in_subdir: None,
        }
    }

    pub(crate) async fn prepare() -> Self {
        Self::with().prepare().await
    }

    pub(crate) fn root(&self) -> &Path {
        self.root.path()
    }

    pub(crate) fn config(&self) -> &Config {
        &self.config
    }

    pub(crate) fn keys(&self) -> &TestKeys {
        self.keys.as_ref().expect("keys not prepared")
    }

    pub(crate) fn state(&self) -> &State {
        self.state.as_ref().expect("state not prepared")
    }

    pub(crate) fn download_server(&self) -> &DownloadServerClient {
        self.download_server
            .as_ref()
            .expect("download server not prepared")
    }

    pub(crate) async fn requests_served_by_mock_download_server(&self) -> usize {
        self.mock_server
            .as_ref()
            .expect("download server not prepared")
            .served_requests_count()
            .await
    }

    /// Get the history of the download server.
    ///
    /// Make sure to `drop()` the value or wrap it in a scope if you have a function
    /// that calls it multiple times.
    pub(crate) async fn history(
        &self,
    ) -> MappedMutexGuard<'_, [(Request<axum::body::Body>, Response<axum::body::Body>)]> {
        self.mock_server
            .as_ref()
            .expect("download server not prepared")
            .history()
            .await
    }
}

pub(crate) struct TestEnvironmentBuilder {
    state: bool,
    download_server: bool,
    keys: bool,
    root_in_subdir: Option<String>,
}

impl TestEnvironmentBuilder {
    pub(crate) fn state(mut self) -> Self {
        self.state = true;
        self
    }

    pub(crate) fn keys(mut self) -> Self {
        self.keys = true;
        self
    }

    pub(crate) fn download_server(mut self) -> Self {
        self.download_server = true;
        self.state().keys()
    }

    pub(crate) fn root_in_subdir(mut self, subdir: &str) -> Self {
        self.root_in_subdir = Some(subdir.into());
        self
    }

    pub(crate) async fn prepare(self) -> TestEnvironment {
        #[cfg(not(target_os = "windows"))]
        let root = TempDir::new().expect("failed to create temp dir");
        #[cfg(target_os = "windows")]
        let root =
            TempDir::new_in(std::env::current_dir().unwrap()).expect("failed to create temp dir");
        let mut root_path = root.path().to_path_buf();

        #[cfg(not(target_os = "windows"))]
        let cache = TempDir::new().expect("failed to create temp dir");
        #[cfg(target_os = "windows")]
        let cache =
            TempDir::new_in(std::env::current_dir().unwrap()).expect("failed to create temp dir");
        let cache_path = cache.path().to_path_buf();

        if let Some(subdir) = self.root_in_subdir {
            // A subdir creation is a requirement because root cannot be changed to anything
            // that does not exist.
            #[cfg(target_os = "windows")]
            tokio::fs::create_dir_all(&subdir).await.unwrap();

            root_path = root_path.join(subdir);
        }

        let mut config = Config::test(root_path, cache_path).expect("failed to create config");

        let keys = if self.keys {
            let keys = TestKeys::generate();
            config.whitelabel.trust_root = keys.trust_root.public().clone();
            Some(keys)
        } else {
            None
        };

        let mock_server = if self.download_server {
            let keys = keys.as_ref().unwrap();
            let server = start_mock_server(keys.signed_public_keys().await, &keys.revocation).await;
            config.whitelabel.download_server_url = server.url();
            Some(server)
        } else {
            None
        };

        let state = if self.state {
            let state = State::load(&config).await.expect("failed to load state");
            state.set_authentication_token(Some(AuthenticationToken::seal(SAMPLE_AUTH_TOKEN)));
            Some(state)
        } else {
            None
        };

        let download_server = if self.download_server {
            Some(DownloadServerClient::new(
                &config,
                state.as_ref().unwrap(),
                Connectivity::Online,
            ))
        } else {
            None
        };

        TestEnvironment {
            root,
            config,
            state,
            keys,
            download_server,
            mock_server,
        }
    }
}

#[derive(Debug)]
pub(crate) struct TestKeys {
    pub(crate) trust_root: EphemeralKeyPair,
    pub(crate) root: EphemeralKeyPair,
    pub(crate) packages: EphemeralKeyPair,
    pub(crate) releases: EphemeralKeyPair,
    pub(crate) redirects: EphemeralKeyPair,
    pub(crate) revocation: EphemeralKeyPair,

    pub(crate) alternate_trust_root: EphemeralKeyPair,
    pub(crate) alternate_root: EphemeralKeyPair,
    pub(crate) alternate_packages: EphemeralKeyPair,
}

impl TestKeys {
    fn generate() -> Self {
        let generate = |role| {
            EphemeralKeyPair::generate(KeyAlgorithm::EcdsaP256Sha256Asn1SpkiDer, role, None)
                .unwrap()
        };

        Self {
            trust_root: generate(KeyRole::Root),
            root: generate(KeyRole::Root),
            packages: generate(KeyRole::Packages),
            releases: generate(KeyRole::Releases),
            redirects: generate(KeyRole::Redirects),
            revocation: generate(KeyRole::Revocation),

            alternate_trust_root: generate(KeyRole::Root),
            alternate_root: generate(KeyRole::Root),
            alternate_packages: generate(KeyRole::Packages),
        }
    }

    async fn signed_public_keys(&self) -> Vec<SignedPayload<PublicKey>> {
        let mut result = Vec::new();

        async fn sign(
            key: &EphemeralKeyPair,
            keys: &[&EphemeralKeyPair],
        ) -> SignedPayload<PublicKey> {
            let mut payload = SignedPayload::new(key.public()).unwrap();
            for key in keys {
                payload.add_signature(*key).await.unwrap();
            }
            payload
        }

        result.push(sign(&self.root, &[&self.trust_root]).await);
        result.push(sign(&self.packages, &[&self.root]).await);
        result.push(sign(&self.releases, &[&self.root]).await);
        result.push(sign(&self.redirects, &[&self.root]).await);
        result.push(sign(&self.revocation, &[&self.root]).await);

        result.push(sign(&self.alternate_root, &[&self.alternate_trust_root]).await);
        result.push(sign(&self.alternate_packages, &[&self.alternate_root]).await);

        result
    }
}

async fn start_mock_server(
    keys: Vec<SignedPayload<PublicKey>>,
    revocation_key: &EphemeralKeyPair,
) -> MockServer {
    use mock_download_server::AuthenticationToken;

    let mut builder = mock_download_server::Builder::default();
    builder = builder.add_token(
        SAMPLE_AUTH_TOKEN,
        AuthenticationToken {
            name: SAMPLE_AUTH_TOKEN_NAME.into(),
            organization_name: SAMPLE_AUTH_TOKEN_CUSTOMER.into(),
            expires_at: Some(SAMPLE_AUTH_TOKEN_EXPIRY.into()),
        },
    );

    for key in keys {
        builder = builder.add_key(key);
    }

    builder = builder.add_revocation_info(revocation_key).await;

    builder.start().await
}
