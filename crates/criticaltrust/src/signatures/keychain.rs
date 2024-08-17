// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::keys::{KeyId, KeyRole, PublicKey};
use crate::manifests::KeysManifest;
use crate::revocation_info::RevocationInfo;
use crate::signatures::{PublicKeysRepository, SignedPayload};
use crate::Error;
use std::collections::HashMap;

/// Collection of all trusted public keys.
pub struct Keychain {
    keys: HashMap<KeyId, PublicKey>,
    revocation_info: Option<RevocationInfo>,
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
            revocation_info: None,
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

    pub fn revocation_info(&self) -> Option<RevocationInfo> {
        self.revocation_info.clone()
    }

    /// Load the following into [`Keychain`] provided the [`KeysManifest`].
    /// 1. All the verified keys.
    /// 2. Revocation information from the revoked content.
    pub fn load_all(&mut self, keys_manifest: &KeysManifest) -> Result<(), Error> {
        // Load all keys from KeysManifest.
        for key in &keys_manifest.keys {
            let _ = self.load(key)?;
        }

        // Special case: verify and load only RevocationInfo.
        let revocation_info = keys_manifest.revoked_signatures.get_verified(self)?;
        self.revocation_info = Some(revocation_info.clone());

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
    fn get<'a>(&'a self, id: &KeyId) -> Option<&'a PublicKey> {
        self.keys.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{EphemeralKeyPair, KeyAlgorithm, KeyPair};
    use crate::manifests::ManifestVersion;
    use crate::signatures::{Signable, SignedPayload};
    use time::macros::datetime;
    use time::{Duration, OffsetDateTime};

    // Make sure there is enough number of days for expiration so tests don't need constant updates.
    const EXPIRATION_EXTENSION_IN_DAYS: Duration = Duration::days(180);

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

    #[test]
    fn test_add_key_trusted_by_root() {
        let root = generate_key(KeyRole::Root);
        let mut keychain = Keychain::new(root.public()).unwrap();

        let (key, public) = generate_trusted_key(KeyRole::Packages, &root);
        keychain.load(&public).unwrap();

        assert_eq!(
            Some(key.public()),
            keychain.get(&key.public().calculate_id())
        );
    }

    #[test]
    fn test_add_key_trusted_by_root_key_trusted_by_root() {
        let root = generate_key(KeyRole::Root);
        let mut keychain = Keychain::new(root.public()).unwrap();

        let (key1, public1) = generate_trusted_key(KeyRole::Root, &root);
        keychain.load(&public1).unwrap();

        let (key2, public2) = generate_trusted_key(KeyRole::Packages, &key1);
        keychain.load(&public2).unwrap();

        assert_eq!(
            Some(key2.public()),
            keychain.get(&key2.public().calculate_id())
        );
    }

    #[test]
    fn test_add_key_trusted_by_non_root_key_trusted_by_root() {
        let root = generate_key(KeyRole::Root);
        let mut keychain = Keychain::new(root.public()).unwrap();

        let (key1, public1) = generate_trusted_key(KeyRole::Packages, &root);
        keychain.load(&public1).unwrap();

        let (_, public2) = generate_trusted_key(KeyRole::Packages, &key1);
        assert!(matches!(
            keychain.load(&public2),
            Err(Error::VerificationFailed)
        ));
    }

    #[test]
    fn test_add_key_trusted_by_nothing_else() {
        let mut keychain = Keychain::new(generate_key(KeyRole::Root).public()).unwrap();

        let another_root = generate_key(KeyRole::Root);
        let (_, public) = generate_trusted_key(KeyRole::Packages, &another_root);
        assert!(matches!(
            keychain.load(&public),
            Err(Error::VerificationFailed)
        ));
    }

    #[test]
    fn test_add_key_with_unsupported_algorithm() {
        let root = generate_key(KeyRole::Root);
        let mut keychain = Keychain::new(root.public()).unwrap();

        let mut other: SignedPayload<PublicKey> = SignedPayload::new(
            &serde_json::from_str(
                r#"{"algorithm": "foo", "role": "root", "expiry": null, "public": "aGk="}"#,
            )
            .unwrap(),
        )
        .unwrap();
        other.add_signature(&root).unwrap();

        assert!(matches!(keychain.load(&other), Err(Error::UnsupportedKey)));
    }

    impl Signable for String {
        const SIGNED_BY_ROLE: KeyRole = KeyRole::Packages;
    }

    impl Signable for Vec<u8> {
        const SIGNED_BY_ROLE: KeyRole = KeyRole::Revocation;
    }

    #[test]
    fn test_load_all_revoked_content_empty() {
        // Test `load_all` method with RevocationInfo but empty list.
        let root = generate_key(KeyRole::Root);
        let revocation = generate_trusted_key(KeyRole::Revocation, &root);

        let revoked_content = RevocationInfo::new(vec![], datetime!(2400-10-10 00:00 UTC));
        let mut signed_revoked_content = SignedPayload::new(&revoked_content).unwrap();
        signed_revoked_content.add_signature(&revocation.0).unwrap();

        let mut keychain = Keychain::new(root.public()).unwrap();

        let keys_manifest = KeysManifest {
            version: ManifestVersion,
            keys: vec![revocation.1],
            revoked_signatures: signed_revoked_content,
        };

        keychain.load_all(&keys_manifest).unwrap();
        assert_eq!(
            keychain.revocation_info.unwrap().expires_at,
            datetime!(2400-10-10 00:00 UTC)
        )
    }

    #[test]
    fn test_load_all_revoked_content_one_item() {
        // Test `load_all` method with RevocationInfo but with one item in the list.
        // The call to `load_all` should not fail in verifying the revocation key.
        let root = generate_key(KeyRole::Root);
        let revocation = generate_trusted_key(KeyRole::Revocation, &root);
        let revoked_content = RevocationInfo::new(
            vec![vec![1, 2, 3]],
            OffsetDateTime::now_utc() + EXPIRATION_EXTENSION_IN_DAYS,
        );
        let mut signed_revoked_content = SignedPayload::new(&revoked_content).unwrap();
        signed_revoked_content.add_signature(&revocation.0).unwrap();

        let mut keychain = Keychain::new(root.public()).unwrap();

        let keys_manifest = KeysManifest {
            version: ManifestVersion,
            keys: vec![revocation.1],
            revoked_signatures: signed_revoked_content,
        };

        keychain.load_all(&keys_manifest).unwrap();
        let binding = keychain.revocation_info.unwrap();
        let actual = binding.revoked_content_sha256.first().unwrap();
        let expected: &Vec<u8> = &vec![1, 2, 3];
        assert_eq!(actual, expected);
    }

    // Utilities

    fn generate_key(role: KeyRole) -> EphemeralKeyPair {
        EphemeralKeyPair::generate(KeyAlgorithm::EcdsaP256Sha256Asn1SpkiDer, role, None).unwrap()
    }

    fn generate_trusted_key(
        role: KeyRole,
        trusted_by: &dyn KeyPair,
    ) -> (EphemeralKeyPair, SignedPayload<PublicKey>) {
        let key = generate_key(role);
        let mut payload = SignedPayload::new(key.public()).unwrap();
        payload.add_signature(trusted_by).unwrap();
        (key, payload)
    }
}
