// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::{construct_toolchains_product_path, TestEnvironment};
use criticalup_core::project_manifest::ProjectManifest;
use std::fs::File;

#[test]
fn help_message() {
    let test_env = TestEnvironment::prepare();
    assert_output!(test_env.cmd().args(["which", "--help"]));
}

#[test]
fn which_run_binary_exists() {
    let test_env = TestEnvironment::prepare();

    let mut current_dir =
        std::env::current_dir().expect("could not read current directory in the test.");
    current_dir.push("tests/resources/criticalup-which.toml");

    let manifest_path = current_dir.to_str().expect("conversion to str failed");

    // generate the manifest object so we can get the installation id hash
    let p = ProjectManifest::load(current_dir.as_path()).expect("could not load project manifest");
    let id_hash = p.products()[0].installation_id().0;

    // manually create the toolchain directory which allows us to skip installation
    // TODO: when tests for `install` command are up, use that instead of manual creation
    let product_toolchain_dir = construct_toolchains_product_path(&test_env, id_hash.as_str());
    let product_toolchain_bin_dir = product_toolchain_dir.join("bin");
    std::fs::create_dir_all(&product_toolchain_bin_dir)
        .expect("could not create product directory");

    // create a file "rustc" in the toolchain/.../bin
    let _ = File::create(product_toolchain_bin_dir.join("rustc")).unwrap();

    assert_output!(test_env
        .cmd()
        .args(["which", "rustc", "--project", manifest_path]));
}

#[test]
fn which_run_binary_does_not_exists() {
    let test_env = TestEnvironment::prepare();

    let mut current_dir =
        std::env::current_dir().expect("could not read current directory in the test.");
    current_dir.push("tests/resources/criticalup-which.toml");

    let manifest_path = current_dir.to_str().expect("conversion to str failed");

    // generate the manifest object so we can get the installation id hash
    let p = ProjectManifest::load(current_dir.as_path()).expect("could not load project manifest");
    let id_hash = p.products()[0].installation_id().0;

    // manually create the toolchain directory which allows us to skip installation
    // TODO: when tests for `install` command are up, use that instead of manual creation
    let product_toolchain_dir = construct_toolchains_product_path(&test_env, id_hash.as_str());
    let product_toolchain_bin_dir = product_toolchain_dir.join("bin");
    std::fs::create_dir_all(product_toolchain_bin_dir).expect("could not create product directory");

    assert_output!(test_env
        .cmd()
        .args(["which", "rustc", "--project", manifest_path]));
}
