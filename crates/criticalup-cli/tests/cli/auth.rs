// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::{TestEnvironment, MOCK_AUTH_TOKENS};

#[test]
fn help_message() {
    let test_env = TestEnvironment::prepare();
    assert_output!(test_env.cmd().args(["auth", "--help"]));
}

#[test]
fn no_token() {
    let test_env = TestEnvironment::prepare();

    assert_output!(test_env.cmd().arg("auth"));
    assert_eq!(0, test_env.requests_served_by_mock_download_server());
}

#[test]
fn invalid_token() {
    let test_env = TestEnvironment::prepare();
    set_token(&test_env, MOCK_AUTH_TOKENS[2].0);
    test_env.revoke_token(MOCK_AUTH_TOKENS[2].0);

    assert_output!(test_env.cmd().arg("auth"));
    assert_eq!(2, test_env.requests_served_by_mock_download_server());
}

#[test]
fn token_without_expiry() {
    let test_env = TestEnvironment::prepare();
    set_token(&test_env, MOCK_AUTH_TOKENS[0].0);

    assert_output!(test_env.cmd().arg("auth"));
    assert_eq!(2, test_env.requests_served_by_mock_download_server());
}

#[test]
fn token_with_expiry() {
    let test_env = TestEnvironment::prepare();
    set_token(&test_env, MOCK_AUTH_TOKENS[1].0);

    assert_output!(test_env.cmd().arg("auth"));
    assert_eq!(2, test_env.requests_served_by_mock_download_server());
}

fn set_token(test_env: &TestEnvironment, token: &str) {
    assert!(test_env
        .cmd()
        .args(["auth", "set", token])
        .output()
        .expect("failed to set the token")
        .status
        .success());
}
