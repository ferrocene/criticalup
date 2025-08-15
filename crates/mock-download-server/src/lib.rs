// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

mod handlers;
mod server;

pub use crate::server::MockServer;
use axum::body::Body;
use axum::extract::Path;
use axum::http::{Request, Response};
use axum::response::Redirect;
use axum::routing::get;
use axum::Router;
use criticaltrust::keys::{EphemeralKeyPair, PublicKey};
use criticaltrust::manifests::ReleaseManifest;
use criticaltrust::revocation_info::RevocationInfo;
use criticaltrust::signatures::SignedPayload;
use handlers::{
    handle_package, handle_v1_keys, handle_v1_package, handle_v1_release, handle_v1_tokens_current,
};
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;

// Make sure there is enough number of days for expiration so tests don't need constant updates.
const EXPIRATION_EXTENSION_IN_DAYS: Duration = Duration::days(180);

#[derive(Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct AuthenticationToken {
    pub name: Cow<'static, str>,
    pub organization_name: Cow<'static, str>,
    pub expires_at: Option<Cow<'static, str>>,
}

pub struct Data {
    pub keypairs: HashMap<String, EphemeralKeyPair>,
    pub tokens: HashMap<String, AuthenticationToken>,
    pub keys: Vec<SignedPayload<PublicKey>>,
    pub revoked_signatures: SignedPayload<RevocationInfo>,
    pub release_manifests: HashMap<(String, String), ReleaseManifest>,
    pub release_packages: HashMap<(String, String, String), Vec<u8>>,
    pub history: Vec<(Request<Body>, Response<Body>)>,
}

pub struct Builder {
    data: Data,
    routes: fn() -> Router<Arc<Mutex<Data>>>,
}

impl Builder {
    pub fn new(routes: fn() -> Router<Arc<Mutex<Data>>>) -> Builder {
        Builder {
            data: Data {
                keypairs: HashMap::new(),
                tokens: HashMap::new(),
                keys: Vec::new(),
                revoked_signatures: SignedPayload::new(&RevocationInfo::new(
                    Vec::new(),
                    OffsetDateTime::now_utc() + EXPIRATION_EXTENSION_IN_DAYS,
                ))
                .unwrap(),
                release_manifests: HashMap::new(),
                release_packages: HashMap::new(),
                history: Vec::new(),
            },
            routes,
        }
    }

    pub fn add_keypair(mut self, keypair: EphemeralKeyPair, name: &str) -> Self {
        self.data.keypairs.insert(name.to_string(), keypair);
        self
    }

    pub fn add_token(mut self, token: &str, info: AuthenticationToken) -> Self {
        self.data.tokens.insert(token.into(), info);
        self
    }

    pub fn add_key(mut self, key: SignedPayload<PublicKey>) -> Self {
        self.data.keys.push(key);
        self
    }

    pub async fn add_revocation_info(mut self, revocation_key: &EphemeralKeyPair) -> Self {
        self.data
            .revoked_signatures
            .add_signature(revocation_key)
            .await
            .unwrap();
        self
    }

    pub fn add_release_manifest(
        mut self,
        product: String,
        release: String,
        manifest: ReleaseManifest,
    ) -> Self {
        self.data
            .release_manifests
            .insert((product, release), manifest);
        self
    }

    pub async fn start(self) -> MockServer {
        MockServer::spawn(self.data, self.routes).await
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new(v1_routes)
    }
}

fn v1_routes() -> Router<Arc<Mutex<Data>>> {
    Router::new().nest(
        "/v1",
        Router::new()
            .route("/keys", get(handle_v1_keys))
            .route("/releases/{product}/{release}", get(handle_v1_release))
            .route(
                "/releases/{product}/{release}/download/{package}/{format}",
                get(handle_v1_package),
            )
            .route("/tokens", get(handle_v1_tokens_current))
            .route("/tokens/current", get(handle_v1_tokens_current)),
    )
}

pub fn file_server_routes() -> Router<Arc<Mutex<Data>>> {
    Router::new()
        .route("/keys.json", get(handle_v1_keys))
        .route(
            "/artifacts/products/{product}/releases/{release}/manifest.json",
            get(handle_v1_release),
        )
        .route(
            "/artifacts/products/{product}/releases/{release}/{package}",
            get(handle_package),
        )
        .nest(
            "/v1",
            Router::new()
                .route("/keys", get(|| async { Redirect::permanent("/keys.json") }))
                .route(
                    "/releases/{product}/{release}",
                    get(
                        |Path((product, release)): Path<(String, String)>| async move {
                            let uri = format!(
                                "/artifacts/products/{product}/releases/{release}/manifest.json"
                            );
                            Redirect::permanent(uri.as_str())
                        },
                    ),
                )
                .route(
                    "/releases/{product}/{release}/download/{package}/{format}",
                    get(
                        |Path((product, release, package, format)): Path<(
                            String,
                            String,
                            String,
                            String,
                        )>| async move {
                            let uri = format!(
                        "/artifacts/products/{product}/releases/{release}/{package}.{format}"
                    );
                            Redirect::permanent(uri.as_str())
                        },
                    ),
                ),
        )
}
