// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::TestEnvironment;
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::tempdir;

const PROJECT_MANIFEST: &str = "
manifest-version = 1

[products.ferrocene]
release = \"nightly\"
packages = [\"rustc\"]
";
// This is specific to the ferrocene product defined in the manifest above.
const INSTALLATION_ID: &str = "1f67f84fa2c0e3d1b99bf72f971b7a10eef29d91b50d9d9f82371c659eff2f0a";

#[tokio::test]
async fn invoking_outside_of_project() {
    let test_env = TestEnvironment::prepare().await;
    let current_dir = tempdir().unwrap();
    assert_output!(test_env
        .binary_proxy("rustc")
        .env_remove("CRITICALUP_CURRENT_PROJ_MANIFEST_CANONICAL_PATH")
        .current_dir(current_dir.into_path()));
}

#[tokio::test]
async fn invoking_inside_of_project_with_no_installed_proxy() {
    let test_env = TestEnvironment::prepare().await;

    let current_dir = tempdir().unwrap();
    std::fs::write(
        current_dir.path().join("criticalup.toml"),
        PROJECT_MANIFEST.as_bytes(),
    )
    .unwrap();

    assert_output!(test_env
        .binary_proxy("sample")
        .current_dir(current_dir.path()));
}

#[tokio::test]
async fn invoking_inside_of_installed_project() {
    let test_env = TestEnvironment::prepare().await;

    let current_dir = tempdir().unwrap();
    std::fs::write(
        current_dir.path().join("criticalup.toml"),
        PROJECT_MANIFEST.as_bytes(),
    )
    .unwrap();

    #[cfg(not(windows))]
    let executable_name = "sample";
    #[cfg(windows)]
    let executable_name = "sample.exe";

    // Create a sample state file referencing the binary proxy.
    std::fs::write(
        test_env.root().join("state.json"),
        serde_json::json!({
            "version": 1,
            "installations": {
                INSTALLATION_ID: {
                    "manifests": ["/path/to/manifest/a", "/path/to/manifest/b"],
                    "binary_proxies": {
                        executable_name: PathBuf::from("bin").join(executable_name),
                    },
                },
            },
        })
        .to_string()
        .as_bytes(),
    )
    .unwrap();

    // Create a sample binary.
    compile_to(
        &test_env
            .root()
            .join("toolchains")
            .join(INSTALLATION_ID)
            .join("bin")
            .join(executable_name),
        r#"fn main() { println!("proxies work!"); }"#,
    );

    assert_output!(test_env
        .binary_proxy("sample")
        .env_remove("CRITICALUP_CURRENT_PROJ_MANIFEST_CANONICAL_PATH")
        .current_dir(current_dir.path()));
}

pub(crate) fn compile_to(dest: &Path, source: &str) {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }

    let mut rustc = Command::new("rustc")
        .arg("-")
        .arg("-o")
        .arg(dest)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = rustc.stdin.take().unwrap();
    stdin.write_all(source.as_bytes()).unwrap();
    drop(stdin);

    let status = rustc.wait().unwrap();
    assert!(status.success());
}
