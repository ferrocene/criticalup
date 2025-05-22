// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::{TestEnvironment, MOCK_AUTH_TOKENS};
use serde::Deserialize;

#[tokio::test]
async fn help_message() {
    let test_env = TestEnvironment::prepare().await;
    assert_output!(test_env.cmd().args(["auth", "remove", "--help"]));
}

#[tokio::test]
async fn token_missing() {
    let test_env = TestEnvironment::prepare().await;

    assert_output!(test_env.cmd().args(["auth", "remove"]));
    assert_eq!(0, test_env.requests_served_by_mock_download_server().await);

    // Ensure no state file was created by just running remove.
    assert!(!test_env.root().join("state.json").exists());
}

#[tokio::test]
async fn token_present() {
    #[derive(Deserialize)]
    struct State {
        authentication_token: Option<String>,
    }

    let test_env = TestEnvironment::prepare().await;
    assert!(test_env
        .cmd()
        .args(["auth", "set", MOCK_AUTH_TOKENS[0].0])
        .output()
        .await
        .expect("failed to set token")
        .status
        .success());

    assert_output!(test_env.cmd().args(["auth", "remove"]));
    assert_eq!(1, test_env.requests_served_by_mock_download_server().await);

    let state: State =
        serde_json::from_slice(&std::fs::read(test_env.root().join("state.json")).unwrap())
            .unwrap();
    assert!(state.authentication_token.is_none());
}
