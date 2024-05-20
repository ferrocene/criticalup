// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(super) struct ProjectManifest {
    #[allow(unused)]
    manifest_version: u32,
    #[serde(default)]
    pub(super) products: HashMap<String, ProjectManifestProduct>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(super) struct ProjectManifestProduct {
    pub(super) release: String,
    pub(super) packages: Vec<String>,
}
