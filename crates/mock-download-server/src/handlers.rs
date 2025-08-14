// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::str::FromStr;
use std::sync::Arc;

use crate::{AuthenticationToken, Data};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use axum_extra::headers::authorization::Bearer;
use axum_extra::headers::{self, IfNoneMatch};
use axum_extra::{headers::Authorization, TypedHeader};
use criticaltrust::manifests::ManifestVersion;
use md5::Digest;
use tokio::sync::Mutex;

pub(crate) async fn handle_v1_package(
    State(data): State<Arc<Mutex<Data>>>,
    if_none_match: Option<TypedHeader<IfNoneMatch>>,
    Path((product, release, package, _format)): Path<(String, String, String, String)>,
) -> impl IntoResponse {
    let data = data.lock().await;

    let bytes = data
        .release_packages
        .get(&(
            product.to_string(),
            release.to_string(),
            package.to_string(),
        ))
        .unwrap()
        .clone();

    if let Some(if_none_match) = if_none_match {
        // If the user already has the item downloaded, they at one point were permitted to download it.
        // We only validate that it is correct. This is not a license check.
        let mut hasher = md5::Md5::new();

        hasher.update(&bytes);
        let etag_string = format!("{:x}", hasher.finalize());

        let etag = headers::ETag::from_str(&format!(r#""{etag_string}""#)).unwrap();

        // This does not behave how you might think.
        // ```
        // let aaa_etag = headers::ETag::from_str(r#""aaa""#).unwrap();
        // let bbb_etag = headers::ETag::from_str(r#""bbb""#).unwrap();
        // let aaa_if_none_match = IfNoneMatch::from(aaa_etag.clone());
        // println!("{aaa_if_none_match:?}");
        // println!("{aaa_etag:?}");
        // println!("{bbb_etag:?}");
        // assert!(aaa_if_none_match.precondition_passes(&bbb_etag));
        // assert!(!aaa_if_none_match.precondition_passes(&aaa_etag));
        // ```
        if !if_none_match.precondition_passes(&etag) {
            return axum::http::StatusCode::NOT_MODIFIED.into_response();
        }
    };

    bytes.into_response()
}

pub(crate) async fn handle_package(
    State(data): State<Arc<Mutex<Data>>>,
    Path((product, release, package)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let data = data.lock().await;

    // we cannot pass 2 parameters in the url /{package}.{format}
    // the package hash keys use `package` and not `format` information
    // we remove the format information.
    assert!(package.contains(".tar.xz"));
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

pub(crate) async fn handle_v1_tokens_current(
    State(data): State<Arc<Mutex<Data>>>,
    TypedHeader(bearer): TypedHeader<Authorization<Bearer>>,
) -> Result<Json<AuthenticationToken>, StatusCode> {
    let data = data.lock().await;
    let token = authorize(&data, bearer)?;
    Ok(Json(token.clone()))
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

fn authorize(
    data: &Data,
    bearer: Authorization<Bearer>,
) -> Result<&AuthenticationToken, StatusCode> {
    let token = bearer.token();

    if let Some(token) = data.tokens.get(token) {
        Ok(token)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}
