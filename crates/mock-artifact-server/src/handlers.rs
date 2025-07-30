// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;

use crate::Data;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Json;
use criticaltrust::manifests::ManifestVersion;
use tokio::sync::Mutex;

pub(crate) async fn handle_v1_package(
    State(data): State<Arc<Mutex<Data>>>,
    Path((product, release, package)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let data = data.lock().await;

    // we cannot pass 2 parameters in the url /{package}.{format}
    let package = package.replace(".tar.xz", "");

    let bytes = data
        .release_packages
        .get(&(
            product.to_string(),
            release.to_string(),
            package.to_string(),
        ))
        .unwrap()
        .clone();
    bytes.into_response()
}

pub(crate) async fn handle_v1_keys(State(data): State<Arc<Mutex<Data>>>) -> impl IntoResponse {
    let data = data.lock().await;
    Json(criticaltrust::manifests::KeysManifest {
        version: ManifestVersion,
        keys: data.keys.clone(),
        revoked_signatures: data.revoked_signatures.clone(),
    })
}

pub(crate) async fn handle_v1_release(
    State(data): State<Arc<Mutex<Data>>>,
    Path((product, release)): Path<(String, String)>,
) -> impl IntoResponse {
    let data = data.lock().await;
    let rm = data
        .release_manifests
        .get(&(product.to_string(), release.to_string()))
        .expect("Did not get a release manifest")
        .to_owned();
    Json(rm)
}
