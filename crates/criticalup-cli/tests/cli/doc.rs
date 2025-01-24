// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::{construct_toolchains_product_path, TestEnvironment};
use criticalup_core::project_manifest::ProjectManifest;
use std::fs::File;

#[tokio::test]
async fn help_message() {
    let test_env = TestEnvironment::prepare().await;
    assert_output!(test_env.cmd().args(["doc", "--help"]));
}

#[tokio::test]
async fn show_path_only() {
    let test_env = TestEnvironment::prepare().await;
    let mut current_dir =
        std::env::current_dir().expect("could not read current directory in the test.");
    current_dir.push("tests/resources/criticalup-doc.toml");

    let manifest_path = current_dir.to_str().expect("conversion to str failed");

    // Generate the manifest object so we can get the installation id hash.
    let p = ProjectManifest::load(current_dir.as_path()).expect("could not load project manifest");
    let id_hash = p.products()[0].installation_id().0;

    // Manually create the toolchain directory which allows us to skip installation.
    // TODO: when tests for `install` command are up, use that instead of manual creation.
    let product_toolchain_dir = construct_toolchains_product_path(&test_env, id_hash.as_str());
    let product_toolchain_doc_dir = product_toolchain_dir.join("share/doc/ferrocene/html");
    std::fs::create_dir_all(&product_toolchain_doc_dir)
        .expect("could not create product directory");

    // Create a file "index.html" in the doc dir to mimic docs.
    let _ = File::create(product_toolchain_doc_dir.join("index.html")).unwrap();

    assert_output!(test_env
        .cmd()
        .args(["doc", "--path", "--project", manifest_path]));
}

#[tokio::test]
async fn error_no_file() {
    let test_env = TestEnvironment::prepare().await;
    let mut current_dir =
        std::env::current_dir().expect("could not read current directory in the test.");
    current_dir.push("tests/resources/criticalup-doc.toml");

    let manifest_path = current_dir.to_str().expect("conversion to str failed");

    // Generate the manifest object so we can get the installation id hash.
    let p = ProjectManifest::load(current_dir.as_path()).expect("could not load project manifest");
    let id_hash = p.products()[0].installation_id().0;

    // Manually create the toolchain directory which allows us to skip installation.
    // TODO: when tests for `install` command are up, use that instead of manual creation.
    let product_toolchain_dir = construct_toolchains_product_path(&test_env, id_hash.as_str());
    let product_toolchain_doc_dir = product_toolchain_dir.join("share/doc/ferrocene/html");
    std::fs::create_dir_all(&product_toolchain_doc_dir)
        .expect("could not create product directory");

    assert_output!(test_env
        .cmd()
        .args(["doc", "--path", "--project", manifest_path]));
}

#[tokio::test]
async fn error_no_package() {
    let test_env = TestEnvironment::prepare().await;
    let mut current_dir =
        std::env::current_dir().expect("could not read current directory in the test.");
    current_dir.push("tests/resources/criticalup-doc-no-package.toml");

    let manifest_path = current_dir.to_str().expect("conversion to str failed");

    // Generate the manifest object so we can get the installation id hash.
    let p = ProjectManifest::load(current_dir.as_path()).expect("could not load project manifest");
    let id_hash = p.products()[0].installation_id().0;

    // Manually create the toolchain directory which allows us to skip installation.
    // TODO: when tests for `install` command are up, use that instead of manual creation.
    let product_toolchain_dir = construct_toolchains_product_path(&test_env, id_hash.as_str());
    let product_toolchain_doc_dir = product_toolchain_dir.join("share/doc/ferrocene/html");
    std::fs::create_dir_all(&product_toolchain_doc_dir)
        .expect("could not create product directory");

    // Create a file "index.html" in the doc dir to mimic docs.
    let _ = File::create(product_toolchain_doc_dir.join("index.html")).unwrap();

    // Even if the file is available on the disk, it should show error because the manifest
    // does not have the 'ferrocene-docs' package.
    assert_output!(test_env
        .cmd()
        .args(["doc", "--path", "--project", manifest_path]));
}
