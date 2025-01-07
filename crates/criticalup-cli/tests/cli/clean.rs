// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::assert_output;
use crate::utils::TestEnvironment;
use serde_json::{json, Value};
use std::fs;
use std::fs::File;
use std::io::{BufReader, Write};

#[tokio::test]
async fn help_message() {
    let env = TestEnvironment::prepare().await;
    assert_output!(env.cmd().args(["clean", "--help"]));
}

#[tokio::test]
async fn clean_deletes_only_unused_installations() {
    let test_env = TestEnvironment::prepare().await;
    let root = test_env.root();
    let toolchains_dir = root.join("toolchains");
    fs::create_dir_all(&toolchains_dir).unwrap();
    let installation_id_1 = "installation_id_1";
    let installation_id_2 = "installation_id_2";
    let installation_id_3 = "installation_id_3";

    let root = test_env.root().join("state.json");
    let mut state_file = std::fs::File::create(&root).unwrap();

    let content = json!({
        "version": 1,
        "authentication_token": "criticalup_token_45_hahaha",
        "installations": {
            installation_id_1: {
                "binary_proxies": {
                    "cargo": "/path/toolchains/bin/cargo"
                },
                "manifests": [
                    "/path/to/proj/1/criticalup.toml",
                    "/path/to/proj/2/criticalup.toml"
                ]
            },
            installation_id_2: {
                "binary_proxies": {
                    "cargo": "/path/toolchains/bin/cargo"
                },
                "manifests": []
            },
            installation_id_3: {
                "binary_proxies": {
                    "cargo": "/path/toolchains/bin/rustc"
                },
                "manifests": []
            }
        }
    })
    .to_string();
    state_file.write_all(content.as_bytes()).unwrap();

    fs::create_dir_all(toolchains_dir.join(installation_id_1)).unwrap();
    fs::create_dir_all(toolchains_dir.join(installation_id_2)).unwrap();
    fs::create_dir_all(toolchains_dir.join(installation_id_3)).unwrap();

    assert!(toolchains_dir.join(installation_id_1).exists());
    assert!(toolchains_dir.join(installation_id_2).exists());
    assert!(toolchains_dir.join(installation_id_3).exists());

    assert_output!(test_env.cmd().args(["clean"]));

    let state_file_actual: Value =
        serde_json::from_reader(BufReader::new(File::open(&root).unwrap())).unwrap();
    // "installation_id_2" is not present.
    assert_eq!(
        state_file_actual.pointer(format!("/installations/{}", installation_id_2).as_str()),
        None
    );
    // "installation_id_3" is not present.
    assert_eq!(
        state_file_actual.pointer(format!("/installations/{}", installation_id_3).as_str()),
        None
    );
    // "installation_id_1" is still present with correct values.
    assert_eq!(
        state_file_actual
            .pointer(format!("/installations/{}", installation_id_1).as_str())
            .unwrap(),
        &json!({
            "binary_proxies": {
                "cargo": "/path/toolchains/bin/cargo"
            },
            "manifests": [
                "/path/to/proj/1/criticalup.toml",
                "/path/to/proj/2/criticalup.toml"
            ]
        })
    );
}

#[tokio::test]
async fn clean_deletes_only_unused_installations_also_from_disk() {
    let test_env = TestEnvironment::prepare().await;
    let root = test_env.root();
    let toolchains_dir = root.join("toolchains");
    fs::create_dir_all(&toolchains_dir).unwrap();

    let installation_id_1 = "installation_id_1";
    let installation_id_2 = "installation_id_2";
    let installation_id_3 = "installation_id_3";

    let state_file_in_root = root.join("state.json");
    let mut state_file = std::fs::File::create(&state_file_in_root).unwrap();
    let content = json!({
        "version": 1,
        "authentication_token": "criticalup_token_45_hahaha",
        "installations": {
            installation_id_1: {
                "binary_proxies": {
                    "cargo": "/path/toolchains/bin/cargo"
                },
                "manifests": [
                    "/path/to/proj/1/criticalup.toml",
                    "/path/to/proj/2/criticalup.toml"
                ]
            },
            installation_id_2: {
                "binary_proxies": {
                    "cargo": "/path/toolchains/bin/cargo"
                },
                "manifests": []
            },
            installation_id_3: {
                "binary_proxies": {
                    "cargo": "/path/toolchains/bin/rustc"
                },
                "manifests": []
            }
        }
    })
    .to_string();
    state_file.write_all(content.as_bytes()).unwrap();

    // Create the corresponding physical directories of installations.
    // TODO: We have to generate these by running `install` command, once tests for those are setup.
    fs::create_dir_all(toolchains_dir.join(installation_id_1)).unwrap();
    fs::create_dir_all(toolchains_dir.join(installation_id_2)).unwrap();
    fs::create_dir_all(toolchains_dir.join(installation_id_3)).unwrap();

    assert!(toolchains_dir.join(installation_id_1).exists());
    assert!(toolchains_dir.join(installation_id_2).exists());
    assert!(toolchains_dir.join(installation_id_3).exists());

    // Run the `clean` command.
    assert_output!(test_env.cmd().args(["clean"]));

    // Test the actual values.
    let state_file_actual: Value =
        serde_json::from_reader(BufReader::new(File::open(&state_file_in_root).unwrap())).unwrap();

    // "installation_id_2" is not present.
    assert_eq!(
        state_file_actual.pointer("/installations/installation_id_2"),
        None
    );
    // "installation_id_3" is not present.
    assert_eq!(
        state_file_actual.pointer("/installations/installation_id_3"),
        None
    );
    // "installation_id_1" is still present with correct values.
    assert_eq!(
        state_file_actual
            .pointer("/installations/installation_id_1")
            .unwrap(),
        &json!({
            "binary_proxies": {
                "cargo": "/path/toolchains/bin/cargo"
            },
            "manifests": [
                "/path/to/proj/1/criticalup.toml",
                "/path/to/proj/2/criticalup.toml"
            ]
        })
    );

    assert!(toolchains_dir.join(installation_id_1).exists());
    assert!(!toolchains_dir.join(installation_id_2).exists()); // Does not exist.
    assert!(!toolchains_dir.join(installation_id_3).exists()); // Does not exist.
}

#[tokio::test]
async fn removes_unused_installations_from_disk_that_do_not_have_state() {
    let test_env = TestEnvironment::prepare().await;
    let root = test_env.root();
    let toolchains_dir = root.join("toolchains");
    fs::create_dir_all(&toolchains_dir).unwrap();

    let installation_id_1 = "installation_id_1";
    let installation_id_2 = "installation_id_2";
    let installation_id_3 = "installation_id_3"; // No State, only directory

    let state_file_in_root = root.join("state.json");
    let mut state_file = std::fs::File::create(&state_file_in_root).unwrap();
    let content = json!({
        "version": 1,
        "authentication_token": "criticalup_token_45_hahaha",
        "installations": {
            installation_id_1: {
                "binary_proxies": {
                    "cargo": "/path/toolchains/bin/cargo"
                },
                "manifests": [
                    "/path/to/proj/1/criticalup.toml",
                    "/path/to/proj/2/criticalup.toml"
                ]
            },
            installation_id_2: {
                "binary_proxies": {
                    "cargo": "/path/toolchains/bin/cargo"
                },
                "manifests": []
            }
        }
    })
    .to_string();
    state_file.write_all(content.as_bytes()).unwrap();

    // Create the corresponding physical directories of installations.
    // TODO: We have to generate these by running `install` command, once tests for those are setup.
    fs::create_dir_all(toolchains_dir.join(installation_id_1)).unwrap();
    fs::create_dir_all(toolchains_dir.join(installation_id_2)).unwrap();
    fs::create_dir_all(toolchains_dir.join(installation_id_3)).unwrap();

    assert!(toolchains_dir.join(installation_id_1).exists());
    assert!(toolchains_dir.join(installation_id_2).exists());
    assert!(toolchains_dir.join(installation_id_3).exists());

    // Run the `clean` command.
    assert_output!(test_env.cmd().args(["clean"]));

    // Test the actual values.
    let state_file_actual: Value =
        serde_json::from_reader(BufReader::new(File::open(&state_file_in_root).unwrap())).unwrap();

    // "installation_id_2" is not present.
    assert_eq!(
        state_file_actual.pointer("/installations/installation_id_2"),
        None
    );
    // "installation_id_1" is still present with correct values.
    assert_eq!(
        state_file_actual
            .pointer("/installations/installation_id_1")
            .unwrap(),
        &json!({
            "binary_proxies": {
                "cargo": "/path/toolchains/bin/cargo"
            },
            "manifests": [
                "/path/to/proj/1/criticalup.toml",
                "/path/to/proj/2/criticalup.toml"
            ]
        })
    );

    assert!(toolchains_dir.join(installation_id_1).exists());
    assert!(!toolchains_dir.join(installation_id_2).exists()); // Does not exist.
    assert!(!toolchains_dir.join(installation_id_3).exists()); // Does not exist.
}

/// Remove the installation from the state, if the installation directory does not exist. This is
/// true even if the installation in the state has a manifest.
#[tokio::test]
async fn clean_deletes_only_unused_installations_that_are_not_on_disk() {
    let test_env = TestEnvironment::prepare().await;
    let root = test_env.root();
    let toolchains_dir = root.join("toolchains");
    fs::create_dir_all(&toolchains_dir).unwrap();

    let installation_id_1 = "installation_id_1";
    let installation_id_2 = "installation_id_2";
    let installation_id_3 = "installation_id_3";

    let state_file_in_root = root.join("state.json");
    let mut state_file = std::fs::File::create(&state_file_in_root).unwrap();
    let content = json!({
        "version": 1,
        "authentication_token": "criticalup_token_45_hahaha",
        "installations": {
            installation_id_1: {
                "binary_proxies": {
                    "cargo": "/path/toolchains/bin/cargo"
                },
                "manifests": [
                    "/path/to/proj/1/criticalup.toml",
                    "/path/to/proj/2/criticalup.toml"
                ]
            },
            installation_id_2: {
                "binary_proxies": {
                    "cargo": "/path/toolchains/bin/cargo"
                },
                "manifests": ["/path/to/proj/3/criticalup.toml"]
            },
            installation_id_3: {
                "binary_proxies": {
                    "cargo": "/path/toolchains/bin/rustc"
                },
                "manifests": []
            }
        }
    })
    .to_string();
    state_file.write_all(content.as_bytes()).unwrap();

    // Create only one of the corresponding physical directories of installations. In this case we
    // create only Installation ID 1.
    // TODO: We have to generate these by running `install` command, once tests for those are setup.
    fs::create_dir_all(toolchains_dir.join(installation_id_1)).unwrap();

    assert!(toolchains_dir.join(installation_id_1).exists());
    assert!(!toolchains_dir.join(installation_id_2).exists());
    assert!(!toolchains_dir.join(installation_id_3).exists());

    // Run the `clean` command.
    assert_output!(test_env.cmd().args(["clean"]));

    // Test the actual values.
    let state_file_actual: Value =
        serde_json::from_reader(BufReader::new(File::open(&state_file_in_root).unwrap())).unwrap();

    // "installation_id_2" is not present.
    assert_eq!(
        state_file_actual.pointer("/installations/installation_id_2"),
        None
    );
    // "installation_id_3" is not present.
    assert_eq!(
        state_file_actual.pointer("/installations/installation_id_3"),
        None
    );
    // "installation_id_1" is still present with correct values.
    assert_eq!(
        state_file_actual
            .pointer("/installations/installation_id_1")
            .unwrap(),
        &json!({
            "binary_proxies": {
                "cargo": "/path/toolchains/bin/cargo"
            },
            "manifests": [
                "/path/to/proj/1/criticalup.toml",
                "/path/to/proj/2/criticalup.toml"
            ]
        })
    );

    assert!(toolchains_dir.join(installation_id_1).exists());
}
