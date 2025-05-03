// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::keys::{KeyId, KeyRole, PublicKey};
use crate::manifests::KeysManifest;
use crate::signatures::{PublicKeysRepository, SignedPayload};
use crate::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Collection of all trusted public keys.
#[derive(Serialize, Deserialize)]
pub struct Keychain {
    keys: HashMap<KeyId, PublicKey>,
}

impl Keychain {
    /// Create a new keychain, using the provided public key as the root of trust.
    ///
    /// The root of trust has to have the `root` key role, and all future keys added to the
    /// keychain will have to be signed by either the root of trust or another key signed by the
    /// root of trust.
    pub fn new(trust_root: &PublicKey) -> Result<Self, Error> {
        let mut keychain = Self {
            keys: HashMap::new(),
        };

        if trust_root.role != KeyRole::Root {
            return Err(Error::WrongKeyRoleForTrustRoot(trust_root.role));
        }
        keychain.load_inner(trust_root)?;

        Ok(keychain)
    }

    pub fn keys(&self) -> &HashMap<KeyId, PublicKey> {
        &self.keys
    }

    /// Update the [`Keychain`] for a given [`KeysManifest`] by verifying and loading all the
    /// verified keys.
    pub fn load_all(&mut self, keys_manifest: &KeysManifest) -> Result<(), Error> {
        // Load all keys from KeysManifest.
        for key in &keys_manifest.keys {
            // Invalid keys are silently ignored, as they might be signed by a different root key
            // used by a different release of criticalup, or they might be using an algorithm not
            // supported by the current version of criticaltrust.
            let _ = self.load(key)?;
        }

        Ok(())
    }

    /// Add a new signed key to the keychain.
    ///
    /// The key has to be signed by either the root of trust or another key with the root role
    /// already part of the keychain.
    pub fn load(&mut self, key: &SignedPayload<PublicKey>) -> Result<KeyId, Error> {
        let key = key.get_verified(self)?;
        self.load_inner(&key)
    }

    fn load_inner(&mut self, key: &PublicKey) -> Result<KeyId, Error> {
        if !key.is_supported() {
            return Err(Error::UnsupportedKey);
        }
        let id = key.calculate_id();
        self.keys.insert(id.clone(), key.clone());
        Ok(id)
    }
}

impl PublicKeysRepository for Keychain {
    fn get(&self, id: &KeyId) -> Option<&PublicKey> {
        self.keys.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{EphemeralKeyPair, KeyAlgorithm, KeyPair};
    use crate::signatures::{Signable, SignedPayload};

    #[test]
    fn test_new_with_root_key_as_trust_root() {
        let root = generate_key(KeyRole::Root);

        let keychain = Keychain::new(root.public()).unwrap();
        assert_eq!(
            Some(root.public()),
            keychain.get(&root.public().calculate_id())
        );
    }

    #[test]
    fn test_new_with_non_root_key_as_trust_root() {
        let non_root = generate_key(KeyRole::Packages);

        assert!(matches!(
            Keychain::new(non_root.public()),
            Err(Error::WrongKeyRoleForTrustRoot(KeyRole::Packages))
        ));
    }

    #[tokio::test]
    async fn test_add_key_trusted_by_root() {
        let root = generate_key(KeyRole::Root);
        let mut keychain = Keychain::new(root.public()).unwrap();

        let (key, public) = generate_trusted_key(KeyRole::Packages, &root).await;
        keychain.load(&public).unwrap();

        assert_eq!(
            Some(key.public()),
            keychain.get(&key.public().calculate_id())
        );
    }

    #[tokio::test]
    async fn test_add_key_trusted_by_root_key_trusted_by_root() {
        let root = generate_key(KeyRole::Root);
        let mut keychain = Keychain::new(root.public()).unwrap();

        let (key1, public1) = generate_trusted_key(KeyRole::Root, &root).await;
        keychain.load(&public1).unwrap();

        let (key2, public2) = generate_trusted_key(KeyRole::Packages, &key1).await;
        keychain.load(&public2).unwrap();

        assert_eq!(
            Some(key2.public()),
            keychain.get(&key2.public().calculate_id())
        );
    }

    #[tokio::test]
    async fn test_add_key_trusted_by_non_root_key_trusted_by_root() {
        let root = generate_key(KeyRole::Root);
        let mut keychain = Keychain::new(root.public()).unwrap();

        let (key1, public1) = generate_trusted_key(KeyRole::Packages, &root).await;
        keychain.load(&public1).unwrap();

        let (_, public2) = generate_trusted_key(KeyRole::Packages, &key1).await;
        assert!(matches!(
            keychain.load(&public2),
            Err(Error::VerificationFailed)
        ));
    }

    #[tokio::test]
    async fn test_add_key_trusted_by_nothing_else() {
        let mut keychain = Keychain::new(generate_key(KeyRole::Root).public()).unwrap();

        let another_root = generate_key(KeyRole::Root);
        let (_, public) = generate_trusted_key(KeyRole::Packages, &another_root).await;
        assert!(matches!(
            keychain.load(&public),
            Err(Error::VerificationFailed)
        ));
    }

    #[tokio::test]
    async fn test_add_key_with_unsupported_algorithm() {
        let root = generate_key(KeyRole::Root);
        let mut keychain = Keychain::new(root.public()).unwrap();

        let mut other: SignedPayload<PublicKey> = SignedPayload::new(
            &serde_json::from_str(
                r#"{"algorithm": "foo", "role": "root", "expiry": null, "public": "aGk="}"#,
            )
            .unwrap(),
        )
        .unwrap();
        other.add_signature(&root).await.unwrap();

        assert!(matches!(keychain.load(&other), Err(Error::UnsupportedKey)));
    }

    impl Signable for String {
        const SIGNED_BY_ROLE: KeyRole = KeyRole::Packages;
    }

    // Utilities

    fn generate_key(role: KeyRole) -> EphemeralKeyPair {
        EphemeralKeyPair::generate(KeyAlgorithm::EcdsaP256Sha256Asn1SpkiDer, role, None).unwrap()
    }

    async fn generate_trusted_key<K: KeyPair>(
        role: KeyRole,
        trusted_by: &K,
    ) -> (EphemeralKeyPair, SignedPayload<PublicKey>) {
        let key = generate_key(role);
        let mut payload = SignedPayload::new(key.public()).unwrap();
        payload.add_signature(trusted_by).await.unwrap();
        (key, payload)
    }
}
