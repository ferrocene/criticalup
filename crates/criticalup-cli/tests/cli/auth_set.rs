// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::{stdin, TestEnvironment, MOCK_AUTH_TOKENS};
use regex::Regex;
use serde::Deserialize;
use tokio::process::Command;

const TOKEN_A: &str = MOCK_AUTH_TOKENS[0].0;
const TOKEN_B: &str = MOCK_AUTH_TOKENS[1].0;
const TOKEN_INVALID: &str = "criticalup_token_invalid";

#[tokio::test]
async fn help_message() {
    let test_env = TestEnvironment::prepare().await;
    assert_output!(test_env.cmd().args(["auth", "set", "--help"]));
}

#[tokio::test]
async fn byte_zero_via_stdin() {
    let test_env = TestEnvironment::prepare().await;

    // Byte zero is not allowed in HTTP headers: we should get a proper error message instead of a
    // panic, and no requests should be made to the server.
    assert_output!(test_env.cmd().args(["auth", "set"]).stdin(stdin("\0")));
    assert_eq!(0, test_env.requests_served_by_mock_download_server().await);
}

// This is a macro instead of a function because otherwise insta detects the name of the helper
// function as the name of the test.
macro_rules! run_cmd {
    ($expected:ident, $env:ident, $variant:ident, $token:ident, $re:ident) => {
        let out = build_command(&$env, $variant, $token)
            .output()
            .await
            .expect("failed to execute command");
        match &$expected {
            Some(expected) => {
                // This regex replacement dance is required because this nested macro tests
                // set is instantiating the test server twice which means each run gives
                // a different local port. We replace with a stable port just for this test.

                let left_str = String::from_utf8(out.stderr.clone())
                    .expect("string creation from bytes failed.");
                let right_str = String::from_utf8(expected.stderr.clone())
                    .expect("string creation from bytes failed.");
                let left = $re.replace_all(left_str.as_str(), "127.0.0.1:1312");
                let right = $re.replace_all(right_str.as_str(), "127.0.0.1:1312");
                assert_eq!(left, right);
            }
            None => {
                assert_output!(out.clone());
                $expected = Some(out);
            }
        }
    };
}

macro_rules! test_matrix {
    ($($module:ident => [$($variant:expr,)*],)*) => {
        $(mod $module {
            use std::process::Output;
            use super::*;

            #[tokio::test]
            async fn set_valid_token() {
                let mut expected: Option<Output> = None;
                let re = Regex::new(r"127.0.0.1:\d+").expect("regex creation failed.");
                for variant in [$($variant,)*] {
                    let test_env = TestEnvironment::prepare().await;

                    assert_token(&test_env, None);
                    run_cmd!(expected, test_env, variant, TOKEN_A, re);
                    assert_token(&test_env, Some(TOKEN_A));

                    // The download server was called to validate the token.
                    assert_eq!(1, test_env.requests_served_by_mock_download_server().await);
                }
            }

            #[tokio::test]
            async fn set_valid_token_with_existing_token() {
                let mut expected: Option<Output>  = None;
                let re = Regex::new(r"127.0.0.1:\d+").expect("regex creation failed.");
                for variant in [$($variant,)*] {
                    let test_env = TestEnvironment::prepare().await;
                    set_token(&test_env, TOKEN_A).await;

                    run_cmd!(expected, test_env, variant, TOKEN_B, re);
                    assert_token(&test_env, Some(TOKEN_B));

                    // The download server was called by both the `set_token` function and what we want
                    // to test (to validate the token).
                    assert_eq!(2, test_env.requests_served_by_mock_download_server().await);
                }
            }

            #[tokio::test]
            async fn set_invalid_token() {
                let mut expected: Option<Output>  = None;
                let re = Regex::new(r"127.0.0.1:\d+").expect("regex creation failed.");
                for variant in [$($variant,)*] {
                    let test_env = TestEnvironment::prepare().await;

                    assert_token(&test_env, None);
                    run_cmd!(expected, test_env, variant, TOKEN_INVALID, re);
                    assert_token(&test_env, None);

                    // The download server was called to validate the token.
                    assert_eq!(1, test_env.requests_served_by_mock_download_server().await);
                }
            }

            #[tokio::test]
            async fn set_invalid_token_with_existing_token() {
                let mut expected: Option<Output>  = None;
                let re = Regex::new(r"127.0.0.1:\d+").expect("regex creation failed.");
                for variant in [$($variant,)*] {
                    let test_env = TestEnvironment::prepare().await;

                    set_token(&test_env, TOKEN_A).await;
                    run_cmd!(expected, test_env, variant, TOKEN_INVALID, re);
                    assert_token(&test_env, Some(TOKEN_A));

                    // The download server was called by both the `set_token` function and what we want
                    // to test (to validate the token).
                    assert_eq!(2, test_env.requests_served_by_mock_download_server().await);
                }
            }
        })*
    };
}

test_matrix! {
   via_args => [
        Variant::Args,
    ],
    via_stdin => [
        Variant::Stdin { newline: None, tty: false },
        Variant::Stdin { newline: Some("\n"), tty: false },
        Variant::Stdin { newline: Some("\r\n"), tty: false },
    ],
    via_tty_eod => [
        Variant::Stdin { newline: None, tty: true },
    ],
    // In these tests, the output might seem incorrect at a glance, because there's no newline
    // between the prompt and the following line. That's actually correct though, because the
    // newline will be part of stdin, as the user writes it.
    via_tty_nl => [
        Variant::Stdin { newline: Some("\n"), tty: true },
        Variant::Stdin { newline: Some("\r\n"), tty: true },
    ],
}

enum Variant {
    Args,
    Stdin {
        newline: Option<&'static str>,
        tty: bool,
    },
}

fn build_command(test_env: &TestEnvironment, variant: Variant, token: &str) -> Command {
    let mut cmd = test_env.cmd();
    match variant {
        Variant::Args => {
            cmd.args(["auth", "set", token]);
        }
        Variant::Stdin { newline, tty } => {
            cmd.args(["auth", "set"]);
            cmd.stdin(stdin(&match newline {
                Some(nl) => format!("{token}{nl}"),
                None => token.into(),
            }));
            if tty {
                cmd.env("CRITICALUP_TEST_MOCK_TTY", "1");
            }
        }
    };
    cmd
}

#[track_caller]
fn assert_token(test_env: &TestEnvironment, expected: Option<&str>) {
    #[derive(Deserialize)]
    struct State {
        authentication_token: Option<String>,
    }

    let actual = match std::fs::read(test_env.root().join("state.json")) {
        Ok(contents) => {
            serde_json::from_slice::<State>(&contents)
                .unwrap()
                .authentication_token
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => panic!("failed to get state file: {err}"),
    };

    assert_eq!(expected, actual.as_deref());
}

async fn set_token(test_env: &TestEnvironment, token: &str) {
    assert_token(test_env, None);

    // We shouldn't write directly to state.json, as in the test we don't know which other params
    // are required. Let `auth set` initialize the state instead, and ensure it worked.
    let out = test_env
        .cmd()
        .args(["auth", "set", token])
        .output()
        .await
        .unwrap();
    assert!(out.status.success());
    assert_token(test_env, Some(token));
}
