// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

mod handlers;
mod server;

pub use crate::server::MockServer;
use criticaltrust::keys::{EphemeralKeyPair, PublicKey};
use criticaltrust::manifests::ReleaseManifest;
use criticaltrust::signatures::SignedPayload;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;

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
}

pub fn new() -> Builder {
    Builder {
        data: Data {
            keypairs: HashMap::new(),
            tokens: HashMap::new(),
            keys: Vec::new(),
            release_manifests: HashMap::new(),
            release_packages: HashMap::new(),
        },
    }
}

pub struct Builder {
    data: Data,
}

impl Builder {
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

    pub fn start(self) -> MockServer {
        MockServer::spawn(self.data)
    }
}
