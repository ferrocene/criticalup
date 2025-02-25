// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::TestEnvironment;

#[tokio::test]
async fn help_message() {
    let test_env = TestEnvironment::prepare().await;
    assert_output!(test_env.cmd().args(["init", "--help"]));
}

#[tokio::test]
async fn creates_manifest_successfully() {
    let test_env = TestEnvironment::prepare().await;
    let root = test_env.root().to_path_buf();

    let manifest_path = root.parent().unwrap().join("criticalup.toml");

    assert!(!&manifest_path.exists());
    assert_output!(test_env
        .cmd()
        .args(["init", "--release", "the-amazing-ferrocene-release"]));
    assert!(&manifest_path.exists());

    // Current directory for these tests is crates/criticalup-cli which means this
    // creates a 'criticalup.toml' in the crate next to src.
    if manifest_path.exists() {
        std::fs::remove_file(manifest_path).unwrap();
    }
}

#[tokio::test]
async fn prints_manifest_successfully() {
    let test_env = TestEnvironment::prepare().await;
    assert_output!(test_env.cmd().args([
        "init",
        "--release",
        "the-amazing-ferrocene-release",
        "--print"
    ]));
}

#[tokio::test]
async fn error_on_missing_required_arg() {
    let test_env = TestEnvironment::prepare().await;
    assert_output!(test_env.cmd().args(["init"]));
}
