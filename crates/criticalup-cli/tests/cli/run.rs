// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::TestEnvironment;
use criticalup_core::project_manifest::ProjectManifest;
use std::io::Write;
use tempfile::tempdir;

#[tokio::test]
async fn help_message() {
    let env = TestEnvironment::prepare().await;
    assert_output!(env.cmd().args(["run", "--help"]));
}

#[tokio::test]
async fn simple_run_command_manifest_not_found() {
    // Manifest does not exist.
    let test_env = TestEnvironment::prepare().await;
    assert_output!(test_env
        .cmd()
        .args(["run", "--project", "/path/to/criticalup.toml", "rustc"]));
}

#[tokio::test]
async fn simple_run_command_did_not_run_install() {
    // Make sure the project manifest exists, but it is not installed
    let test_env = TestEnvironment::prepare().await;
    let current_dir = tempdir().unwrap();
    let manifest = current_dir.path().join("criticalup.toml");

    let project_manifest = "
        manifest-version = 1
        [products.ferrocene]
        release = \"nightly\"
        packages = [\"sample\"]
        ";
    std::fs::write(&manifest, project_manifest.as_bytes()).unwrap();
    assert_output!(test_env.cmd().args([
        "run",
        "--strict",
        "--project",
        manifest.to_str().unwrap(),
        "rustc"
    ]));
}

#[tokio::test]
#[ignore = "This test will be improved upon at a later date"]
async fn simple_run_command_existing_package() {
    let test_env = TestEnvironment::prepare().await;
    let current_dir = tempdir().unwrap();
    std::fs::create_dir_all(test_env.root().join("bin")).unwrap();

    let project_manifest = "
        manifest-version = 1
        [products.ferrocene]
        release = \"nightly\"
        packages = [\"sample\"]
        ";
    std::fs::write(
        current_dir.path().join("criticalup.toml"),
        project_manifest.as_bytes(),
    )
    .unwrap();

    let installation_id =
        ProjectManifest::load(current_dir.path().join("criticalup.toml").as_path())
            .unwrap()
            .products()
            .first()
            .unwrap()
            .installation_id()
            .0;
    // Create a sample state file referencing the binary proxy.
    std::fs::write(
        test_env.root().join("state.json"),
        serde_json::json!({
            "version": 1,
            "installations": {
                &installation_id: {
                    "manifests": ["/path/to/manifest/a", "/path/to/manifest/b"],
                    "binary_proxies": {
                        "sample": "bin/sample",
                    },
                },
            },
        })
        .to_string()
        .as_bytes(),
    )
    .unwrap();

    // Create a sample binary.
    crate::binary_proxies::compile_to(
        &test_env
            .root()
            .join("toolchains")
            .join(&installation_id)
            .join("bin")
            .join("sample"),
        r#"fn main() { println!("success: sample binary was called via run command"); }"#,
    );
    let mut f = std::fs::File::create(test_env.root().join("bin/sample")).unwrap();
    f.write_all(b"").unwrap();

    let mut c = std::process::Command::new(
        test_env
            .root()
            .join("toolchains")
            .join(&installation_id)
            .join("bin")
            .join("sample")
            .as_os_str()
            .to_str()
            .unwrap(),
    );
    std::io::stdout()
        .write_all(&c.output().unwrap().stdout)
        .unwrap();

    // assert_output!(test_env.cmd().args(["run", "sample"]));
}
