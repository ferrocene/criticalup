use criticaltrust::keys::{EphemeralKeyPair, KeyAlgorithm, KeyPair, KeyRole, PublicKey};
use criticaltrust::manifests::{Release, ReleaseManifest};
use criticaltrust::signatures::SignedPayload;
use mock_download_server::{AuthenticationToken, MockServer};
use std::borrow::Cow;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;

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

impl TestEnvironment {
    pub(crate) fn prepare() -> Self {
        let keypair = EphemeralKeyPair::generate(
            KeyAlgorithm::EcdsaP256Sha256Asn1SpkiDer,
            KeyRole::Root,
            None,
        )
        .unwrap();

        let root = TempDir::new_in(std::env::current_dir().unwrap()).unwrap();

        TestEnvironment {
            root,
            trust_root: keypair.public().clone(),
            server: setup_mock_server(&keypair),
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

    pub(crate) fn requests_served_by_mock_download_server(&self) -> usize {
        self.server.served_requests_count()
    }

    pub(crate) fn revoke_token(&self, token: &str) {
        self.server.edit_data(|data| {
            data.tokens.remove(token);
        });
    }
}

pub(crate) fn stdin(content: &str) -> Stdio {
    let mut file = tempfile::tempfile().expect("failed to create temporary file");
    file.write_all(content.as_bytes())
        .expect("failed to write stdin");
    file.rewind().unwrap();
    file.into()
}

fn setup_mock_server(keypair: &dyn KeyPair) -> MockServer {
    let mut server = mock_download_server::new();
    for (token, data) in MOCK_AUTH_TOKENS {
        server = server.add_token(token, data.clone());
    }
    for (product, release, mut manifest) in mock_release_manifests() {
        manifest.signed.add_signature(keypair).unwrap();
        server =
            server.add_release_manifest(product.to_string(), release.to_string(), manifest.clone());
    }
    server.start()
}

pub(crate) trait IntoOutput {
    fn into_output(&mut self) -> Output;
}

impl IntoOutput for Command {
    fn into_output(&mut self) -> Output {
        self.output().expect("failed to execute command")
    }
}

impl IntoOutput for Output {
    fn into_output(&mut self) -> Output {
        self.clone()
    }
}

#[macro_export]
macro_rules! assert_output {
    ($out:expr) => {{
        use $crate::utils::IntoOutput;

        let repr = $crate::utils::output_repr(&$out.into_output());
        let mut settings = insta::Settings::clone_current();
        settings.set_snapshot_path("../snapshots");

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
            r"caused by: The system cannot find the path specified\. \(os error 3\)",
            "caused by: No such file or directory (os error 2)",
        );

        #[cfg(windows)]
        settings.add_filter("exit code: ", "exit status: ");
        #[cfg(windows)]
        settings.add_filter("criticalup-test.exe", "criticalup-test");
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

pub(crate) fn auth_set_with_valid_token(env: &TestEnvironment) {
    let second_token = MOCK_AUTH_TOKENS[0].0;

    assert!(env
        .cmd()
        .args(["auth", "set", second_token])
        .output()
        .expect("sssss")
        .status
        .success());
}
