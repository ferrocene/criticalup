// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::{auth_set_with_valid_token, construct_toolchains_product_path, TestEnvironment};
use serde_json::json;
use std::io::Write;

#[tokio::test]
async fn help_message() {
    let test_env = TestEnvironment::prepare().await;
    assert_output!(test_env.cmd().args(["install", "--help"]));
}

#[tokio::test]
async fn empty_packages_list() {
    let test_env = TestEnvironment::prepare().await;
    let mut current_dir = std::env::current_dir().unwrap();
    current_dir.push("tests/resources/criticalup-empty-packages.toml");
    let manifest_path = current_dir.to_str().unwrap();

    assert_output!(test_env.cmd().args(["install", "--project", manifest_path]));
}

#[tokio::test]
async fn already_installed_toolchain_should_not_throw_error() {
    let test_env = TestEnvironment::prepare().await;

    let mut current_dir = std::env::current_dir().unwrap();
    current_dir.push("tests/resources/criticalup.toml");
    let manifest_path = current_dir.to_str().unwrap();

    // Generate the manifest object so we can get the installation id hash.
    let manifest =
        criticalup_core::project_manifest::ProjectManifest::load(current_dir.as_path()).unwrap();
    let installation_id = manifest.products()[0].installation_id();

    // Generate and write state.json file because our handy functions like
    // state::update_installation_manifests() check for the state file as well as
    // existing installation directories.
    //
    // This is brittle on subject to criticalup.toml changes in the tests/resource but right now
    // `TestEnvironment` in this crate does not support constructing environment with State.
    // So, we are resorting to creating this state.json by hand. Once the two environments for the
    // test utils are merged, we can use the State API.
    let root = test_env.root().join("state.json");
    let mut state_file = std::fs::File::create(root).unwrap();
    // 6bb4fe4c8205d18a8eaf0b852c3b29f65805fd80e528af74cf2f1463a911e40e is the hash of the
    // current criticalup.toml's product contents which we use here to create state.json by
    // dynamically calculating it from the criticalup.toml.
    let content = json!(
        { "version": 1,
          "authentication_token": "criticalup_token_45_hahaha",
          "installations": {
            &installation_id.0: {
              "binary_proxies": {
                "cargo": "/path/toolchains/bin/cargo"
               },
              "manifests": [
                "/path/to/criticalup.toml"
              ]
            }
          }
        }
    )
    .to_string();

    state_file.write_all(content.as_bytes()).unwrap();

    // Manually create the toolchain directory which allows us to skip installation.
    let product_toolchain_dir =
        construct_toolchains_product_path(&test_env, installation_id.0.as_str());
    std::fs::create_dir_all(product_toolchain_dir).unwrap();

    // Running install command should skip installation.
    // See the `filter()` used in utils::assert_output macro for this test.
    assert_output!(test_env.cmd().args(["install", "--project", manifest_path]))
}

/// Sample test to run the command in test environment without any other computation
#[tokio::test]
#[ignore = "Testing `install` subcommand will be enabled at a later date"]
async fn run_install() {
    let test_env = TestEnvironment::prepare().await;

    let mut current_dir = std::env::current_dir().unwrap();
    current_dir.push("tests/resources/criticalup.toml");
    let manifest_path = current_dir.to_str().unwrap();

    run_install_cmd(&test_env, manifest_path);
}

#[tokio::test]
#[ignore = "Testing `install` subcommand will be enabled at a later date"]
async fn product_dirs_are_created() {
    let test_env = TestEnvironment::prepare().await;

    let mut current_dir =
        std::env::current_dir().expect("Could not read current directory in the test.");
    current_dir.push("tests/resources/criticalup.toml");
    let manifest_path = current_dir.to_str().expect("conversion to str failed");

    run_install_cmd(&test_env, manifest_path);

    let ex1 = construct_toolchains_product_path(
        &test_env,
        "791180e94af037a98410323424f9bfda82d82fdbc991a9cd8da30a091459f5f5",
    );
    assert!(ex1.exists());

    let ex2 = construct_toolchains_product_path(
        &test_env,
        "ceac76fcf73a702d9349a7064679606f90c4d8db09a763c9fd4d5acd9059544d",
    );
    assert!(ex2.exists());

    let ex3 = construct_toolchains_product_path(
        &test_env,
        "723bbd3fb691ce24dc6d59afc5f9d4caabce6b359ac512784c057bef7025b095",
    );
    assert!(ex3.exists());
}

fn run_install_cmd(test_env: &TestEnvironment, manifest_path: &str) {
    auth_set_with_valid_token(test_env); // we need auth set before install command

    let output = test_env
        .cmd()
        .args(["install", "--project", manifest_path])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
