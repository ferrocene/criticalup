// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use criticaltrust::keys::{EphemeralKeyPair, KeyAlgorithm, KeyPair, KeyRole, PublicKey};
use criticaltrust::manifests::{Release, ReleaseManifest};
use criticaltrust::signatures::SignedPayload;
use mock_download_server::{file_server_routes, AuthenticationToken, Builder, MockServer};
use std::borrow::Cow;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use tempfile::TempDir;
use tokio::process::Command;

pub(crate) const MOCK_AUTH_TOKENS: &[(&str, AuthenticationToken)] = &[
    (
        "criticalup_token_000000000",
        AuthenticationToken {
            name: Cow::Borrowed("dummy token 1"),
            organization_name: Cow::Borrowed("internal"),
            expires_at: None,
        },
    ),
    (
        "criticalup_token_111111111",
        AuthenticationToken {
            name: Cow::Borrowed("dummy token 2"),
            organization_name: Cow::Borrowed("ferrous-systems"),
            expires_at: Some(Cow::Borrowed("2022-01-01T00:00:00+00:00")),
        },
    ),
    (
        "criticalup_token_222222222",
        AuthenticationToken {
            name: Cow::Borrowed("dummy token 3"),
            organization_name: Cow::Borrowed("ferrous-systems"),
            expires_at: Some(Cow::Borrowed("2022-01-01T00:00:00+00:00")),
        },
    ),
];

// This can't be a const since we call `new()`
pub(crate) fn mock_release_manifests() -> Vec<(&'static str, &'static str, ReleaseManifest)> {
    vec![(
        "ferrocene",
        "dev",
        ReleaseManifest {
            version: criticaltrust::manifests::ManifestVersion,
            signed: SignedPayload::new(&Release {
                product: "ferrocene".into(),
                release: "dev".into(),
                commit: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
                packages: vec![],
            })
            .unwrap(),
        },
    )]
}

pub(crate) struct TestEnvironment {
    root: TempDir,
    trust_root: PublicKey,
    server: MockServer,
    customer_portal_url: String,
}

pub enum Server {
    Default,
    FileServer,
}

impl TestEnvironment {
    pub(crate) async fn prepare() -> Self {
        Self::prepare_with(Server::Default).await
    }

    pub(crate) async fn prepare_with(server: Server) -> Self {
        let root_keypair = EphemeralKeyPair::generate(
            KeyAlgorithm::EcdsaP256Sha256Asn1SpkiDer,
            KeyRole::Root,
            None,
        )
        .unwrap();

        let root = TempDir::new_in(std::env::current_dir().unwrap()).unwrap();

        let server_builder = match server {
            Server::FileServer => mock_download_server::Builder::new(file_server_routes),
            _ => mock_download_server::Builder::default(),
        };

        TestEnvironment {
            root,
            trust_root: root_keypair.public().clone(),
            server: setup_mock_server(server_builder, root_keypair).await,
            customer_portal_url: "https://customers-test.ferrocene.dev".into(),
        }
    }

    pub(crate) fn root(&self) -> &Path {
        self.root.path()
    }

    pub(crate) fn cmd(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_criticalup-test"));
        command.env("CRITICALUP_ROOT", self.root.path());
        command.env("CRITICALUP_TEST_DOWNLOAD_SERVER_URL", self.server.url());
        command.env(
            "CRITICALUP_TEST_CUSTOMER_PORTAL_URL",
            &self.customer_portal_url,
        );
        command.env(
            "CRITICALUP_TEST_TRUST_ROOT",
            serde_json::to_string(&self.trust_root).unwrap(),
        );
        command.env("CRITICALUP_TESTING_IN_PROGRESS", "1");
        command
    }

    pub(crate) fn binary_proxy(&self, name: &str) -> Command {
        let mut command = self.cmd();
        command.env("CRITICALUP_TEST_OVERRIDE_ARG0", name);
        command
    }

    pub(crate) async fn requests_served_by_mock_download_server(&self) -> usize {
        self.server.served_requests_count().await
    }

    pub(crate) async fn revoke_token(&self, token: &str) {
        self.server
            .edit_data(|mut data| {
                data.tokens.remove(token);
            })
            .await;
    }

    // Beware of the consumption.
    pub(crate) fn server(&mut self) -> &mut MockServer {
        &mut self.server
    }
}

pub(crate) fn stdin(content: &str) -> Stdio {
    let mut file = tempfile::tempfile().expect("failed to create temporary file");
    file.write_all(content.as_bytes())
        .expect("failed to write stdin");
    file.rewind().unwrap();
    file.into()
}

async fn setup_mock_server(
    mut server_builder: mock_download_server::Builder,
    root_keypair: EphemeralKeyPair,
) -> MockServer {
    for (token, data) in MOCK_AUTH_TOKENS {
        server_builder = server_builder.add_token(token, data.clone());
    }

    // Root keypair.
    // This is the only keypair that is added without adding a public signed payload because
    // Root is self-trusting and has no parent to sign its public key.
    server_builder = server_builder.add_keypair(root_keypair.to_owned(), "root");

    // Releases keypair.
    server_builder =
        add_non_root_key_to_server(server_builder, "releases", KeyRole::Releases, &root_keypair)
            .await;
    // Revocation keypair.
    server_builder = add_non_root_key_to_server(
        server_builder,
        "revocation",
        KeyRole::Revocation,
        &root_keypair,
    )
    .await;
    // Packages keypair.
    server_builder =
        add_non_root_key_to_server(server_builder, "packages", KeyRole::Packages, &root_keypair)
            .await;

    for (product, release, mut manifest) in mock_release_manifests() {
        manifest.signed.add_signature(&root_keypair).await.unwrap();
        server_builder = server_builder.add_release_manifest(
            product.to_string(),
            release.to_string(),
            manifest.clone(),
        );
    }
    server_builder.start().await
}

pub(crate) trait IntoOutput {
    async fn into_output(&mut self) -> Output;
}

impl IntoOutput for Command {
    async fn into_output(&mut self) -> Output {
        self.output().await.expect("failed to execute command")
    }
}

impl IntoOutput for Output {
    async fn into_output(&mut self) -> Output {
        self.clone()
    }
}

#[macro_export]
macro_rules! assert_output {
    ($out:expr) => {{
        use $crate::utils::IntoOutput;

        let repr = $crate::utils::output_repr(&$out.into_output().await);
        let mut settings = insta::Settings::clone_current();
        settings.set_snapshot_path("../snapshots");

        #[cfg(target_os = "windows")]
        settings.add_filter(
            r"[a-zA-Z]:\\.*\\toolchains\\(?<ins_id>[_a-zA-Z0-9]+)\\.*html.*",
            "/path/to/toolchain/installation/$ins_id/share/doc/ferrocene/html/index.html",
        );

        #[cfg(target_os = "windows")]
        settings.add_filter(
            r"file:.*[a-zA-Z]:.*toolchains/(?<ins_id>[_a-zA-Z0-9]+)/share/doc/ferrocene.*",
            "file:/path/to/toolchain/installation/$ins_id/share/doc/ferrocene/html/index.html",
        );

        // using tempfile in tests changes the output tmp dir on every run
        // so, this is to normalize the data first
        #[cfg(target_os = "linux")]
        settings.add_filter(
            "/.*tmp.*/toolchains/(?<ins_id>[_a-zA-Z0-9]+)/?",
            "/path/to/toolchain/installation/$ins_id/",
        );
        #[cfg(target_os = "macos")]
        settings.add_filter(
            "/.*/toolchains/(?<ins_id>[_a-zA-Z0-9]+)/?",
            "/path/to/toolchain/installation/$ins_id/",
        );
        #[cfg(target_os = "windows")]
        settings.add_filter(
            r"[a-zA-Z]:\\.*\\toolchains\\(?<ins_id>[_a-zA-Z0-9]+)\\?",
            "/path/to/toolchain/installation/$ins_id/",
        );

        #[cfg(windows)]
        settings.add_filter(
            r"error: The system cannot find the path specified\. \(os error 3\)",
            "error: No such file or directory (os error 2)",
        );

        #[cfg(windows)]
        settings.add_filter(
            r"caused by: program not found",
            "caused by: No such file or directory (os error 2)",
        );

        #[cfg(windows)]
        settings.add_filter(
            r"caused by: The system cannot find the path specified\. \(os error 3\)",
            "caused by: No such file or directory (os error 2)",
        );

        settings.add_filter(
            r"error: Failed to load the project manifest at.*criticalup-empty-packages.toml",
            "error: Failed to load the project manifest at /path/to/manifest/criticalup-empty-packages.toml",
        );

        #[cfg(target_os = "linux")]
        settings.add_filter(
            r"/tmp/.*/criticalup.toml",
            "TEMPDIR/criticalup.toml",
        );

        #[cfg(target_os = "macos")]
        settings.add_filter(
            r"/var/folders/.*/criticalup.toml",
            "TEMPDIR/criticalup.toml",
        );

        #[cfg(target_os = "windows")]
        settings.add_filter(
            r"[a-zA-Z]:\\.*\\Temp\\.*\\criticalup.toml",
            "TEMPDIR/criticalup.toml",
        );

        #[cfg(windows)]
        settings.add_filter("exit code: ", "exit status: ");
        #[cfg(windows)]
        settings.add_filter(r"bin\\rustc", r"bin/rustc");
        #[cfg(windows)]
        settings.add_filter("criticalup-test.exe", "criticalup-test");

        settings.add_filter(r"INFO Created project manifest at .+criticalup\.toml", "INFO Created project manifest at /path/to/created/criticalup.toml");

        settings.bind(|| {
            insta::assert_snapshot!(repr);
        });
    }};
}

pub(crate) fn output_repr(output: &Output) -> String {
    let mut snapshot = String::new();
    snapshot.push_str(&format!("exit: {}\n", output.status));

    snapshot.push('\n');
    if output.stdout.is_empty() {
        snapshot.push_str("empty stdout\n");
    } else {
        snapshot.push_str("stdout\n------\n");
        snapshot.push_str(std::str::from_utf8(&output.stdout).expect("non-utf-8 stdout"));
        snapshot.push_str("------\n");
    }

    snapshot.push('\n');
    if output.stderr.is_empty() {
        snapshot.push_str("empty stderr\n");
    } else {
        snapshot.push_str("stderr\n------\n");
        snapshot.push_str(std::str::from_utf8(&output.stderr).expect("non-utf-8 stderr"));
        snapshot.push_str("------\n");
    }

    snapshot
}

pub(crate) fn construct_toolchains_product_path(env: &TestEnvironment, sha: &str) -> PathBuf {
    let toolchains_dir = "toolchains";
    let product_dir_name = sha;
    let mut root = env.root().to_path_buf();
    root.push(toolchains_dir);
    root.push(product_dir_name);
    root
}

pub(crate) async fn auth_set_with_valid_token(env: &TestEnvironment) {
    let second_token = MOCK_AUTH_TOKENS[0].0;

    assert!(env
        .cmd()
        .args(["auth", "set", second_token])
        .output()
        .await
        .expect("sssss")
        .status
        .success());
}

fn generate_key(role: KeyRole) -> EphemeralKeyPair {
    EphemeralKeyPair::generate(KeyAlgorithm::EcdsaP256Sha256Asn1SpkiDer, role, None).unwrap()
}

async fn generate_trusted_key(
    role: KeyRole,
    trusted_by: &EphemeralKeyPair,
) -> (EphemeralKeyPair, SignedPayload<PublicKey>) {
    let key = generate_key(role);
    let mut payload = SignedPayload::new(key.public()).unwrap();
    payload.add_signature(trusted_by).await.unwrap();
    (key, payload)
}

async fn add_non_root_key_to_server(
    mut server_builder: Builder,
    name: &str,
    role: KeyRole,
    trusted_by: &EphemeralKeyPair,
) -> Builder {
    let (keypair, signed_payload) = generate_trusted_key(role, trusted_by).await;
    if name == "revocation" {
        server_builder = server_builder.add_revocation_info(&keypair).await;
    }
    server_builder = server_builder.add_key(signed_payload);
    server_builder = server_builder.add_keypair(keypair, name);

    server_builder
}
