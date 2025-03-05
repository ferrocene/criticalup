// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::TestEnvironment;
use std::fs::File;
use tempfile::tempdir;

#[tokio::test]
async fn help_message() {
    let test_env = TestEnvironment::prepare().await;
    assert_output!(test_env.cmd().args(["init", "--help"]));
}

#[tokio::test]
async fn creates_manifest_successfully() {
    let test_env = TestEnvironment::prepare().await;
    let current_dir = tempdir().unwrap();
    let manifest_path = current_dir.path().join("criticalup.toml");

    assert!(!&manifest_path.exists());
    assert_output!(test_env
        .cmd()
        .args(["init", "--release", "the-amazing-ferrocene-release"])
        .current_dir(current_dir.path()));
    assert!(&manifest_path.exists());
}

#[tokio::test]
async fn shows_error_on_existing_manifest() {
    let test_env = TestEnvironment::prepare().await;
    let current_dir = tempdir().unwrap();
    let manifest_path = current_dir.path().join("criticalup.toml");
    let _ = File::create(&manifest_path).unwrap();

    assert!(&manifest_path.exists());

    assert_output!(test_env
        .cmd()
        .args(["init", "--release", "the-amazing-ferrocene-release"])
        .current_dir(current_dir.path()));
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
