// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::keys::{EphemeralKeyPair, KeyAlgorithm, KeyPair, KeyRole, PublicKey};
use crate::manifests::{KeysManifest, ManifestVersion};
use crate::revocation_info::RevocationInfo;
use crate::signatures::{Keychain, SignedPayload};
use base64::Engine;
use time::{Duration, OffsetDateTime};

const ALGORITHM: KeyAlgorithm = KeyAlgorithm::EcdsaP256Sha256Asn1SpkiDer;
// Make sure there is enough number of days for expiration so tests don't need constant updates.
const EXPIRATION_EXTENSION_IN_DAYS: Duration = Duration::days(180);

pub(crate) struct TestEnvironment {
    root: EphemeralKeyPair,
    keychain: Keychain,
}

impl TestEnvironment {
    pub(crate) async fn prepare() -> Self {
        let root = EphemeralKeyPair::generate(ALGORITHM, KeyRole::Root, None).unwrap();

        let revocation_key =
            EphemeralKeyPair::generate(ALGORITHM, KeyRole::Revocation, None).unwrap();
        let mut signed_revocation_key = SignedPayload::new(revocation_key.public()).unwrap();
        signed_revocation_key.add_signature(&root).await.unwrap();

        let expiration_datetime = OffsetDateTime::now_utc() + EXPIRATION_EXTENSION_IN_DAYS;
        let mut revoked_signatures =
            SignedPayload::new(&RevocationInfo::new(vec![], expiration_datetime)).unwrap();
        revoked_signatures
            .add_signature(&revocation_key)
            .await
            .unwrap();

        let keys_manifest = KeysManifest {
            version: ManifestVersion,
            keys: vec![signed_revocation_key],
        };
        let mut keychain = Keychain::new(root.public()).unwrap();
        keychain.load_all(&keys_manifest).unwrap();
        Self { root, keychain }
    }

    pub(crate) fn keychain(&self) -> &Keychain {
        &self.keychain
    }

    pub(crate) fn create_untrusted_key(&self, role: KeyRole) -> EphemeralKeyPair {
        EphemeralKeyPair::generate(ALGORITHM, role, None).unwrap()
    }

    pub(crate) async fn create_key(&mut self, role: KeyRole) -> EphemeralKeyPair {
        let key = self.create_untrusted_key(role);
        self.sign_and_add_key(key.public()).await;
        key
    }

    pub(crate) async fn create_key_with_expiry(
        &mut self,
        role: KeyRole,
        expiry_diff_hours: i64,
    ) -> EphemeralKeyPair {
        let expiry = OffsetDateTime::now_utc() + Duration::hours(expiry_diff_hours);
        let key = EphemeralKeyPair::generate(ALGORITHM, role, Some(expiry)).unwrap();
        self.sign_and_add_key(key.public()).await;
        key
    }

    async fn sign_and_add_key(&mut self, key: &PublicKey) {
        let mut payload = SignedPayload::new(key).unwrap();
        payload.add_signature(&self.root).await.unwrap();

        self.keychain.load(&payload).unwrap();
    }
}

pub(crate) fn base64_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

pub(crate) fn base64_decode(encoded: &str) -> Result<Vec<u8>, base64::DecodeError> {
    base64::engine::general_purpose::STANDARD.decode(encoded)
}
