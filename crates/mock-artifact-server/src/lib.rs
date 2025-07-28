// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

mod handlers;
mod server;

pub use crate::server::MockArtifactServer;
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
use std::collections::HashMap;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;

// Make sure there is enough number of days for expiration so tests don't need constant updates.
const EXPIRATION_EXTENSION_IN_DAYS: Duration = Duration::days(180);

pub struct Data {
    pub keypairs: HashMap<String, EphemeralKeyPair>,
    pub keys: Vec<SignedPayload<PublicKey>>,
    pub release_manifests: HashMap<(String, String), ReleaseManifest>,
    pub revoked_signatures: SignedPayload<RevocationInfo>,
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
        }
    }

    pub fn add_keypair(mut self, keypair: EphemeralKeyPair, name: &str) -> Self {
        self.data.keypairs.insert(name.to_string(), keypair);
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

    pub async fn start(self) -> MockArtifactServer {
        MockArtifactServer::spawn(self.data).await
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

fn v1_routes() -> Router<Arc<Mutex<Data>>> {
    Router::new()
        .route("/keys", get(|| async { Redirect::permanent("/keys.json") }))
        .route(
            "/releases/{product}/{release}",
            get(
                |Path((product, release)): Path<(String, String)>| async move {
                    let uri =
                        format!("/artifacts/products/{product}/releases/{release}/manifest.json");
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
        )
}
