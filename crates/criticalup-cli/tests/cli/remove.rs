use crate::assert_output;
use crate::utils::{construct_toolchains_product_path, TestEnvironment};
use serde_json::{json, Value};
use std::fs::File;
use std::io::{BufReader, Write};

#[test]
fn help_message() {
    let test_env = TestEnvironment::prepare();
    assert_output!(test_env.cmd().args(["remove", "--help"]));
}

#[test]
fn remove_deletes_only_manifest_from_list_and_dir() {
    let test_env = TestEnvironment::prepare();
    let mut current_dir = std::env::current_dir().unwrap();
    current_dir.push("tests/resources/criticalup.toml");
    let manifest_path = current_dir.canonicalize().unwrap();

    // Generate the manifest object so we can get the installation id hash.
    let manifest =
        criticalup_core::project_manifest::ProjectManifest::load(current_dir.as_path()).unwrap();
    let installation_id = manifest.products()[0].installation_id();

    let state_file_path = test_env.root().join("state.json");
    let mut state_file = File::create(&state_file_path).unwrap();
    // 6bb4fe4c8205d18a8eaf0b852c3b29f65805fd80e528af74cf2f1463a911e40e is the hash of the
    // current criticalup.toml's product contents which we use here to create state.json by
    // dynamically calculating it from the criticalup.toml.
    let content = json!(
        { "version": 1,
          "authentication_token": "criticalup_token_45_hahaha",
          "installations": {
            &installation_id.0: {
              "binary_proxies": {
                "cargo": "/path/toolchains/bin/cargo"
               },
              "manifests": [
                    manifest_path
              ]
            }
          }
        }
    )
    .to_string();

    state_file.write_all(content.as_bytes()).unwrap();
    // Manually create the toolchain directory which allows us to skip installation.
    let product_toolchain_dir =
        construct_toolchains_product_path(&test_env, installation_id.0.as_str());
    std::fs::create_dir_all(&product_toolchain_dir).unwrap();

    assert_output!(test_env
        .cmd()
        .args(["remove", "--project", manifest_path.to_str().unwrap()]));

    let state_file_actual: Value =
        serde_json::from_reader(BufReader::new(File::open(&state_file_path).unwrap())).unwrap();

    // Installation's manifest is an empty array because the manifest path was removed.
    assert_eq!(
        state_file_actual
            .pointer(format!("/installations/{}/manifests", installation_id.0).as_str())
            .unwrap(),
        &json!([])
    );

    // Directory is gone.
    assert!(!product_toolchain_dir.exists());
}
