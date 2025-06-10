// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::{TestEnvironment, MOCK_AUTH_TOKENS};

#[tokio::test]
async fn help_message() {
    let test_env = TestEnvironment::prepare().await;
    assert_output!(test_env.cmd().args(["auth", "--help"]));
}

#[tokio::test]
async fn no_token() {
    let test_env = TestEnvironment::prepare().await;

    assert_output!(test_env.cmd().arg("auth"));
    assert_eq!(0, test_env.requests_served_by_mock_download_server().await);
}

#[tokio::test]
async fn invalid_token() {
    let test_env = TestEnvironment::prepare().await;
    set_token(&test_env, MOCK_AUTH_TOKENS[2].0).await;
    test_env.revoke_token(MOCK_AUTH_TOKENS[2].0).await;

    assert_output!(test_env.cmd().arg("auth"));
    assert_eq!(2, test_env.requests_served_by_mock_download_server().await);
}

#[tokio::test]
async fn token_without_expiry() {
    let test_env = TestEnvironment::prepare().await;
    set_token(&test_env, MOCK_AUTH_TOKENS[0].0).await;

    assert_output!(test_env.cmd().arg("auth"));
    assert_eq!(2, test_env.requests_served_by_mock_download_server().await);
}

#[tokio::test]
async fn token_with_expiry() {
    let test_env = TestEnvironment::prepare().await;
    set_token(&test_env, MOCK_AUTH_TOKENS[1].0).await;

    assert_output!(test_env.cmd().arg("auth"));
    assert_eq!(2, test_env.requests_served_by_mock_download_server().await);
}

async fn set_token(test_env: &TestEnvironment, token: &str) {
    let output = test_env
        .cmd()
        .args(["auth", "set", token])
        .output()
        .await
        .expect("failed to set the token");
    assert!(output.status.success());
}
