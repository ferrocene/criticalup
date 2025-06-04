// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::TestEnvironment;

#[tokio::test]
async fn no_args() {
    let test_env = TestEnvironment::prepare().await;
    assert_output!(test_env.cmd());
}

#[tokio::test]
async fn help_flags() {
    let test_env = TestEnvironment::prepare().await;

    let no_args = test_env.cmd().output().await.unwrap();
    let help_short = test_env.cmd().arg("-h").output().await.unwrap();
    let help_long = test_env.cmd().arg("--help").output().await.unwrap();

    assert_eq!(&no_args.stdout, &help_short.stdout);
    assert_eq!(&no_args.stdout, &help_long.stdout);

    assert_eq!(&no_args.stderr, &help_short.stderr);
    assert_eq!(&no_args.stderr, &help_long.stderr);

    assert!(help_short.status.success());
    assert!(help_long.status.success());
}

#[tokio::test]
async fn version_flags() {
    let test_env = TestEnvironment::prepare().await;

    let version_short = test_env.cmd().arg("-V").output().await.unwrap();
    let version_long = test_env.cmd().arg("--version").output().await.unwrap();
    assert_eq!(version_long, version_short);

    assert_output!(test_env.cmd().arg("--version"));
}
