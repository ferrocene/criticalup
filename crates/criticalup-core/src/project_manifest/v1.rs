// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ProjectManifest {
    #[allow(unused)]
    manifest_version: u32,
    #[serde(default)]
    pub(super) products: HashMap<String, ProjectManifestProduct>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(super) struct ProjectManifestProduct {
    pub(super) release: String,
    pub(super) packages: Vec<String>,
}

pub fn sample_manifest(release: Option<String>) -> ProjectManifest {
    let packages = vec![
        "cargo-${rustc-host}".into(),
        "rustc-${rustc-host}".into(),
        "clippy-${rustc-host}".into(),
        "rust-std-${rustc-host}".into(),
        "rustfmt-${rustc-host}".into(),
    ];

    let mut product = ProjectManifestProduct {
        release: "".into(),
        packages,
    };

    if let Some(release_val) = release {
        product.release = release_val;
    } else {
        product.release = "stable-24.11.0".to_string();
    }

    ProjectManifest {
        manifest_version: 1,
        products: HashMap::from([("ferrocene".to_string(), product)]),
    }
}
