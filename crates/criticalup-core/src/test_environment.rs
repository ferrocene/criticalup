// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::Path;
use tempfile::TempDir;
use criticaltrust::keys::{EphemeralKeyPair, KeyAlgorithm, KeyPair, KeyRole, PublicKey};
use criticaltrust::signatures::SignedPayload;
use mock_download_server::MockServer;
use crate::config::Config;
use crate::download_server_client::DownloadServerClient;
use crate::state::{AuthenticationToken, State};

pub const SAMPLE_AUTH_TOKEN: &str = "criticalup_token_foo";
pub const SAMPLE_AUTH_TOKEN_NAME: &str = "token name";
pub const SAMPLE_AUTH_TOKEN_CUSTOMER: &str = "internal";
pub const SAMPLE_AUTH_TOKEN_EXPIRY: &str = "2022-01-01T00:00:00+00:00";

pub struct TestEnvironment {
    root: TempDir,
    config: Config,
    state: Option<State>,
    download_server: Option<DownloadServerClient>,
    keys: Option<TestKeys>,

    mock_server: Option<MockServer>,
}

impl TestEnvironment {
    pub fn with() -> TestEnvironmentBuilder {
        TestEnvironmentBuilder {
            state: false,
            download_server: false,
            keys: false,
            root_in_subdir: None,
        }
    }

    pub async fn prepare() -> Self {
        Self::with().prepare().await
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn keys(&self) -> &TestKeys {
        self.keys.as_ref().expect("keys not prepared")
    }

    pub fn state(&self) -> &State {
        self.state.as_ref().expect("state not prepared")
    }

    pub fn download_server(&self) -> &DownloadServerClient {
        self.download_server
            .as_ref()
            .expect("download server not prepared")
    }

    pub fn requests_served_by_mock_download_server(&self) -> usize {
        self.mock_server
            .as_ref()
            .expect("download server not prepared")
            .served_requests_count()
    }
}

pub struct TestEnvironmentBuilder {
    state: bool,
    download_server: bool,
    keys: bool,
    root_in_subdir: Option<String>,
}


impl TestEnvironmentBuilder {
    pub fn state(mut self) -> Self {
        self.state = true;
        self
    }

    pub fn keys(mut self) -> Self {
        self.keys = true;
        self
    }

    pub fn download_server(mut self) -> Self {
        self.download_server = true;
        self.state().keys()
    }

    pub fn root_in_subdir(mut self, subdir: &str) -> Self {
        self.root_in_subdir = Some(subdir.into());
        self
    }

    pub async fn prepare(self) -> TestEnvironment {
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
            let server = start_mock_server(keys.signed_public_keys(), &keys.revocation);
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
            Some(DownloadServerClient::new(&config, state.as_ref().unwrap()))
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

pub struct TestKeys {
    pub trust_root: EphemeralKeyPair,
    pub root: EphemeralKeyPair,
    pub packages: EphemeralKeyPair,
    pub releases: EphemeralKeyPair,
    pub redirects: EphemeralKeyPair,
    pub revocation: EphemeralKeyPair,

    pub alternate_trust_root: EphemeralKeyPair,
    pub alternate_root: EphemeralKeyPair,
    pub alternate_packages: EphemeralKeyPair,
}

impl TestKeys {
    pub fn generate() -> Self {
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

    pub fn signed_public_keys(&self) -> Vec<SignedPayload<PublicKey>> {
        let mut result = Vec::new();
        let mut sign = |key: &EphemeralKeyPair, keys: &[&EphemeralKeyPair]| {
            let mut payload = SignedPayload::new(key.public()).unwrap();
            for key in keys {
                payload.add_signature(*key).unwrap();
            }
            result.push(payload);
        };

        sign(&self.root, &[&self.trust_root]);
        sign(&self.packages, &[&self.root]);
        sign(&self.releases, &[&self.root]);
        sign(&self.redirects, &[&self.root]);
        sign(&self.revocation, &[&self.root]);

        sign(&self.alternate_root, &[&self.alternate_trust_root]);
        sign(&self.alternate_packages, &[&self.alternate_root]);

        result
    }
}

fn start_mock_server(
    keys: Vec<SignedPayload<PublicKey>>,
    revocation_key: &EphemeralKeyPair,
) -> MockServer {
    use mock_download_server::AuthenticationToken;

    let mut builder = mock_download_server::new();
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

    builder = builder.add_revocation_info(revocation_key);

    builder.start()
}
