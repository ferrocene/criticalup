// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::TestEnvironment;

#[test]
fn no_args() {
    let test_env = TestEnvironment::prepare();
    assert_output!(test_env.cmd());
}

#[test]
fn help_flags() {
    let test_env = TestEnvironment::prepare();

    let no_args = test_env.cmd().output().unwrap();
    let help_short = test_env.cmd().arg("-h").output().unwrap();
    let help_long = test_env.cmd().arg("--help").output().unwrap();

    assert_eq!(&no_args.stdout, &help_short.stdout);
    assert_eq!(&no_args.stdout, &help_long.stdout);

    assert_eq!(&no_args.stderr, &help_short.stderr);
    assert_eq!(&no_args.stderr, &help_long.stderr);

    assert!(help_short.status.success());
    assert!(help_long.status.success());
}

#[test]
fn version_flags() {
    let test_env = TestEnvironment::prepare();

    let version_short = test_env.cmd().arg("-V").output().unwrap();
    let version_long = test_env.cmd().arg("--version").output().unwrap();
    assert_eq!(version_long, version_short);

    assert_output!(test_env.cmd().arg("--version"));
}
