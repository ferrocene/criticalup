// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

mod handlers;
mod server;

pub use crate::server::MockServer;
use axum::body::Body;
use axum::http::{Request, Response};
use axum::routing::get;
use axum::Router;
use criticaltrust::keys::{EphemeralKeyPair, PublicKey};
use criticaltrust::manifests::ReleaseManifest;
use criticaltrust::signatures::SignedPayload;
use handlers::{handle_v1_keys, handle_v1_package, handle_v1_release, handle_v1_tokens_current};
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    pub release_manifests: HashMap<(String, String), ReleaseManifest>,
    pub release_packages: HashMap<(String, String, String), Vec<u8>>,
    pub history: Vec<(Request<Body>, Response<Body>)>,
}

pub struct Builder {
    data: Data,
}

impl Builder {
    pub fn new() -> Builder {
        Builder {
            data: Data {
                keypairs: HashMap::new(),
                tokens: HashMap::new(),
                keys: Vec::new(),
                release_manifests: HashMap::new(),
                release_packages: HashMap::new(),
                history: Vec::new(),
            },
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
        MockServer::spawn(self.data).await
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

fn v1_routes() -> Router<Arc<Mutex<Data>>> {
    Router::new()
        .route("/keys", get(handle_v1_keys))
        .route("/releases/{product}/{release}", get(handle_v1_release))
        .route(
            "/releases/{product}/{release}/download/{package}/{format}",
            get(handle_v1_package),
        )
        .route("/tokens", get(handle_v1_tokens_current))
        .route("/tokens/current", get(handle_v1_tokens_current))
}
