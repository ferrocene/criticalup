// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::{
    auth_set_with_valid_token, construct_toolchains_product_path, Server, TestEnvironment,
};
use hyper::StatusCode;
use mock_download_server::MockServer;
use serde_json::json;
use std::io::Write;
use tempfile::tempdir;

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

#[tokio::test]
async fn run_install_successfully() {
    let mut test_env = TestEnvironment::prepare().await;

    // Create a release with one package.
    let package_ref = "rustc";
    let product_ref = "ferrocene";
    let release_ref = "25.02.0";

    let work_dir_binding = tempdir().unwrap();
    let work_dir = work_dir_binding.path();
    let output_dir = work_dir.join("output");
    tokio::fs::create_dir_all(&output_dir).await.unwrap();

    let input_dir = work_dir.join("input");
    tokio::fs::create_dir_all(input_dir.join("bin"))
        .await
        .unwrap();
    tokio::fs::write(input_dir.join("bin").join("rustc"), "hello")
        .await
        .unwrap();
    assert!(input_dir.join("bin/rustc").exists());

    let server: &mut MockServer = test_env.server();

    server
        .create_package(package_ref, product_ref, &input_dir, &output_dir)
        .await
        .unwrap();
    server
        .create_release(product_ref, release_ref, vec![package_ref], &output_dir)
        .await
        .unwrap();

    let manifest = toml::toml! {
        manifest-version = 1

        [products.ferrocene]
        release = release_ref
        packages = [
            package_ref,
        ]
    }
    .to_string();

    let manifest_path = work_dir.join("criticalup.toml");
    tokio::fs::write(&manifest_path, manifest).await.unwrap();

    run_install_cmd(&test_env, manifest_path.to_str().unwrap(), false, true).await;

    run_install_cmd(&test_env, manifest_path.to_str().unwrap(), true, true).await;
    assert_eq!(
        {
            let history = test_env.server().history().await;
            let downloads = history
                .iter()
                .filter(|(req, _)| {
                    req.uri() == "/v1/releases/ferrocene/25.02.0/download/rustc/tar.xz"
                })
                .collect::<Vec<_>>();
            assert_eq!(downloads.len(), 2);
            let (_, res) = downloads.last().unwrap();
            res.status()
        },
        StatusCode::NOT_MODIFIED,
    );
}

#[tokio::test]
async fn run_install_with_generic_file_server_successfully() {
    let mut test_env = TestEnvironment::prepare_with(Server::FileServer).await;

    // Create a release with one package.
    let package_ref = "rustc";
    let product_ref = "ferrocene";
    let release_ref = "25.05.0";

    let work_dir_binding = tempdir().unwrap();
    let work_dir = work_dir_binding.path();
    let output_dir = work_dir.join("output");
    tokio::fs::create_dir_all(&output_dir).await.unwrap();

    let input_dir = work_dir.join("input");
    tokio::fs::create_dir_all(input_dir.join("bin"))
        .await
        .unwrap();
    tokio::fs::write(input_dir.join("bin").join("rustc"), "hello")
        .await
        .unwrap();
    assert!(input_dir.join("bin/rustc").exists());

    let server: &mut MockServer = test_env.server();

    server
        .create_package(package_ref, product_ref, &input_dir, &output_dir)
        .await
        .unwrap();
    server
        .create_release(product_ref, release_ref, vec![package_ref], &output_dir)
        .await
        .unwrap();

    let manifest = toml::toml! {
        manifest-version = 1

        [products.ferrocene]
        release = release_ref
        packages = [
            package_ref,
        ]
    }
    .to_string();

    let manifest_path = work_dir.join("criticalup.toml");
    tokio::fs::write(&manifest_path, manifest).await.unwrap();

    run_install_cmd(&test_env, manifest_path.to_str().unwrap(), false, false).await;

    assert_url_called_n_times_and_returns_status_code(
        &mut test_env,
        1,
        "/v1/keys",
        StatusCode::PERMANENT_REDIRECT,
    )
    .await;
    assert_url_called_n_times_and_returns_status_code(
        &mut test_env,
        1,
        "/keys.json",
        StatusCode::OK,
    )
    .await;

    assert_url_called_n_times_and_returns_status_code(
        &mut test_env,
        1,
        "/v1/releases/ferrocene/25.05.0",
        StatusCode::PERMANENT_REDIRECT,
    )
    .await;
    assert_url_called_n_times_and_returns_status_code(
        &mut test_env,
        1,
        "/artifacts/products/ferrocene/releases/25.05.0/manifest.json",
        StatusCode::OK,
    )
    .await;

    assert_url_called_n_times_and_returns_status_code(
        &mut test_env,
        1,
        "/v1/releases/ferrocene/25.05.0/download/rustc/tar.xz",
        StatusCode::PERMANENT_REDIRECT,
    )
    .await;
    assert_url_called_n_times_and_returns_status_code(
        &mut test_env,
        1,
        "/artifacts/products/ferrocene/releases/25.05.0/rustc.tar.xz",
        StatusCode::OK,
    )
    .await;
}

async fn assert_url_called_n_times_and_returns_status_code(
    test_env: &mut TestEnvironment,
    n: u8,
    url: &str,
    status_code: StatusCode,
) {
    assert_eq!(
        {
            let history = test_env.server().history().await;
            let downloads = history
                .iter()
                .filter(|(req, _)| req.uri() == url)
                .collect::<Vec<_>>();

            let uri_history = history.iter().map(|(req, _)| req.uri()).collect::<Vec<_>>();
            assert_eq!(
                downloads.len(),
                n as usize,
                "filtered history {downloads:?} total history {uri_history:?}"
            );
            let (_, res) = downloads.last().unwrap();
            res.status()
        },
        status_code,
    );
}

async fn run_install_cmd(
    test_env: &TestEnvironment,
    manifest_path: &str,
    reinstall: bool,
    auth: bool,
) {
    if auth {
        auth_set_with_valid_token(test_env).await; // we need auth set before install command
    }

    let mut command = test_env.cmd();
    command.args(["install", "--project", manifest_path]);

    if reinstall {
        command.arg("--reinstall");
    }

    let output = command.output().await.unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
