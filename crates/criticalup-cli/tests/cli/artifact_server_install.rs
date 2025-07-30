// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::utils::TestEnvironment;
use hyper::StatusCode;
use mock_artifact_server::MockArtifactServer;
use tempfile::tempdir;

#[tokio::test]
async fn run_install_successfully() {
    let mut test_env = TestEnvironment::prepare().await;

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

    let server: &mut MockArtifactServer = test_env.artifact_server();

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

    run_install_cmd(&test_env, manifest_path.to_str().unwrap(), false).await;

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

    run_install_cmd(&test_env, manifest_path.to_str().unwrap(), true).await;

    assert_url_called_n_times_and_returns_status_code(
        &mut test_env,
        2,
        "/v1/keys",
        StatusCode::PERMANENT_REDIRECT,
    )
    .await;
    assert_url_called_n_times_and_returns_status_code(
        &mut test_env,
        2,
        "/keys.json",
        StatusCode::OK,
    )
    .await;

    assert_url_called_n_times_and_returns_status_code(
        &mut test_env,
        2,
        "/v1/releases/ferrocene/25.05.0",
        StatusCode::PERMANENT_REDIRECT,
    )
    .await;
    assert_url_called_n_times_and_returns_status_code(
        &mut test_env,
        2,
        "/artifacts/products/ferrocene/releases/25.05.0/manifest.json",
        StatusCode::OK,
    )
    .await;

    assert_url_called_n_times_and_returns_status_code(
        &mut test_env,
        2,
        "/v1/releases/ferrocene/25.05.0/download/rustc/tar.xz",
        StatusCode::PERMANENT_REDIRECT,
    )
    .await;
    assert_url_called_n_times_and_returns_status_code(
        &mut test_env,
        2,
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
            let history = test_env.artifact_server().history().await;
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

async fn run_install_cmd(test_env: &TestEnvironment, manifest_path: &str, reinstall: bool) {
    let mut command = test_env.cmd_with_artifact_server();
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
