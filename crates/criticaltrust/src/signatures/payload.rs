// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::keys::newtypes::{PayloadBytes, SignatureBytes};
use crate::keys::{KeyId, KeyPair, KeyRole, PublicKey};
use crate::Error;
use serde::{Deserialize, Serialize};
use std::cell::{Ref, RefCell};

/// Piece of data with signatures attached to it.
///
/// To prevent misuses, there is no way to access the data inside the payload unless signatures are
/// verified. The signed payload can be freely serialized and deserialized.
#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(bound = "T: Signable")]
pub struct SignedPayload<T: Signable> {
    signatures: Vec<Signature>,
    signed: String,
    #[serde(skip)]
    verified_deserialized: RefCell<Option<T>>,
}

impl<T: Signable> std::fmt::Debug for SignedPayload<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedPayload")
            .field("signatures", &self.signatures)
            .field("signed", &self.signed)
            .finish_non_exhaustive()
    }
}

impl<T: Signable> SignedPayload<T> {
    /// Create a new signed payload. Note that no signature is generated by this method call:
    /// you'll also need to call [`add_signature`](Self::add_signature) with a valid [`KeyPair`] to
    /// generate a valid signed payload.
    pub fn new(to_sign: &T) -> Result<Self, Error> {
        Ok(Self {
            signatures: Vec::new(),
            signed: serde_json::to_string(to_sign)
                .map_err(Error::SignedPayloadSerializationFailed)?,
            verified_deserialized: RefCell::new(None),
        })
    }

    /// Add a new signature to this signed payload, generated using the provided [`KeyPair`].
    pub async fn add_signature<K: KeyPair>(&mut self, keypair: &K) -> Result<(), Error> {
        self.signatures.push(Signature {
            key_sha256: keypair.public().calculate_id(),
            signature: keypair
                .sign(&PayloadBytes::borrowed(self.signed.as_bytes()))
                .await?,
        });
        Ok(())
    }

    /// Verifies the signatures attached to the signed payload and returns the deserialized data
    /// (if the signature matched).
    ///
    /// As signature verification and deserialization is expensive, it is only performed the first
    /// time the method is called. The cached results from the initial call will be returned in the
    /// rest of the cases.
    pub fn get_verified(&self, keys: &dyn PublicKeysRepository) -> Result<Ref<'_, T>, Error> {
        let borrow = self.verified_deserialized.borrow();

        if borrow.is_none() {
            let value = verify_signature(
                keys,
                &self.signatures,
                PayloadBytes::borrowed(self.signed.as_bytes()),
            )?;

            // In theory, `borrow_mut()` could panic if an immutable borrow was alive at the same
            // time. In practice that won't happen, as we only populate the cache before returning
            // any reference to the cached data.
            drop(borrow);
            *self.verified_deserialized.borrow_mut() = Some(value)
        }

        Ok(Ref::map(self.verified_deserialized.borrow(), |b| {
            b.as_ref().unwrap()
        }))
    }

    /// Consumes the signed payload and returns the deserialized payload.
    ///
    /// If the signature verification was already performed before (through the
    /// [`get_verified`](Self::get_verified) method), the cached deserialized payload will be
    /// returned. Otherwise, signature verification will be performed with the provided keychain
    /// before deserializing.
    pub fn into_verified(self, keys: &dyn PublicKeysRepository) -> Result<T, Error> {
        if let Some(deserialized) = self.verified_deserialized.into_inner() {
            Ok(deserialized)
        } else {
            verify_signature(
                keys,
                &self.signatures,
                PayloadBytes::borrowed(self.signed.as_bytes()),
            )
        }
    }
}

fn verify_signature<T: Signable>(
    keys: &dyn PublicKeysRepository,
    signatures: &[Signature],
    signed: PayloadBytes<'_>,
) -> Result<T, Error> {
    for signature in signatures {
        let key = match keys.get(&signature.key_sha256) {
            Some(key) => key,
            None => continue,
        };

        match key.verify(T::SIGNED_BY_ROLE, &signed, &signature.signature) {
            Ok(()) => {}
            Err(Error::VerificationFailed) => continue,
            Err(other) => return Err(other),
        }

        // Deserialization is performed after the signature is verified, to ensure we are not
        // deserializing malicious data.
        return serde_json::from_slice(signed.as_bytes()).map_err(Error::DeserializationFailed);
    }

    Err(Error::VerificationFailed)
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
struct Signature {
    key_sha256: KeyId,
    #[serde(with = "crate::serde_base64")]
    signature: SignatureBytes<'static>,
}

/// Trait representing contents that can be wrapped in a [`SignedPayload`].
pub trait Signable: Serialize + for<'de> Deserialize<'de> {
    /// Key role authorized to verify this type.
    const SIGNED_BY_ROLE: KeyRole;
}

/// Trait representing a collection of public keys that can be used to verify signatures.
///
/// You likely want to use a [`Keychain`](crate::signatures::Keychain) as the public keys
/// repository, as it allows to establish a root of trust and supports multiple keys. For simple
/// cases or tests, individual [`PublicKey`]s also implement this trait.
pub trait PublicKeysRepository {
    /// Retrieve a key by its ID.
    fn get(&self, id: &KeyId) -> Option<&PublicKey>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{EphemeralKeyPair, PublicKey};
    use crate::manifests::{KeysManifest, ManifestVersion};
    use crate::revocation_info::RevocationInfo;
    use crate::signatures::Keychain;
    use crate::test_utils::{base64_encode, TestEnvironment};
    use time::{Duration, OffsetDateTime};

    const SAMPLE_DATA: &str = r#"{"answer":42}"#;
    // Make sure there is enough number of days for expiration so tests don't need constant updates.
    const EXPIRATION_EXTENSION_IN_DAYS: Duration = Duration::days(180);

    #[tokio::test]
    async fn test_verify_no_signatures() {
        let test_env = TestEnvironment::prepare().await;
        let pairs: &[&EphemeralKeyPair] = &[];
        assert_verify_fail(&test_env, pairs).await;
    }

    #[tokio::test]
    async fn test_verify_one_valid_signature() {
        let mut test_env = TestEnvironment::prepare().await;

        let key = test_env.create_key(KeyRole::Packages).await;
        assert_verify_pass(&test_env, &[&key]).await;
    }

    #[tokio::test]
    async fn test_verify_multiple_valid_signatures() {
        let mut test_env = TestEnvironment::prepare().await;

        let key1 = test_env.create_key(KeyRole::Packages).await;
        let key2 = test_env.create_key(KeyRole::Packages).await;

        assert_verify_pass(&test_env, &[&key1, &key2]).await;
        assert_verify_pass(&test_env, &[&key2, &key1]).await;
    }

    // Key roles

    #[tokio::test]
    async fn test_verify_with_invalid_key_role() {
        let mut test_env = TestEnvironment::prepare().await;

        let key = test_env.create_key(KeyRole::Redirects).await;
        assert_verify_fail(&test_env, &[&key]).await;
    }

    #[tokio::test]
    async fn test_verify_with_invalid_and_valid_key_roles() {
        let mut test_env = TestEnvironment::prepare().await;

        let valid = test_env.create_key(KeyRole::Packages).await;
        let invalid = test_env.create_key(KeyRole::Redirects).await;
        assert_verify_pass(&test_env, &[&valid, &invalid]).await;
        assert_verify_pass(&test_env, &[&invalid, &valid]).await;
    }

    // Trusted/untrusted
    #[tokio::test]
    async fn test_verify_with_untrusted_key() {
        let test_env = TestEnvironment::prepare().await;

        let untrusted = test_env.create_untrusted_key(KeyRole::Packages);
        assert_verify_fail(&test_env, &[&untrusted]).await;
    }

    #[tokio::test]
    async fn test_verify_with_trusted_and_untrusted_keys() {
        let mut test_env = TestEnvironment::prepare().await;

        let trusted = test_env.create_key(KeyRole::Packages).await;
        let untrusted = test_env.create_untrusted_key(KeyRole::Packages);

        assert_verify_pass(&test_env, &[&trusted, &untrusted]).await;
        assert_verify_pass(&test_env, &[&untrusted, &trusted]).await;
    }

    #[tokio::test]
    async fn test_verify_with_subset_of_trusted_keys() {
        let mut test_env = TestEnvironment::prepare().await;

        let used_key = test_env.create_key(KeyRole::Packages).await;
        let _other_trusted_key = test_env.create_key(KeyRole::Packages).await;

        assert_verify_pass(&test_env, &[&used_key]).await;
    }

    // Expiry

    #[tokio::test]
    async fn test_verify_with_expired_key() {
        let mut test_env = TestEnvironment::prepare().await;

        let expired = test_env.create_key_with_expiry(KeyRole::Packages, -1).await;
        assert_verify_fail(&test_env, &[&expired]).await;
    }

    #[tokio::test]
    async fn test_verify_with_not_expired_key() {
        let mut env = TestEnvironment::prepare().await;

        let not_expired = env.create_key_with_expiry(KeyRole::Packages, 1).await;
        assert_verify_pass(&env, &[&not_expired]).await;
    }

    #[tokio::test]
    async fn test_verify_with_expired_and_not_expired_keys() {
        let mut test_env = TestEnvironment::prepare().await;

        let expired = test_env.create_key_with_expiry(KeyRole::Packages, -1).await;
        let not_expired = test_env.create_key_with_expiry(KeyRole::Packages, 1).await;

        assert_verify_pass(&test_env, &[&expired, &not_expired]).await;
        assert_verify_pass(&test_env, &[&not_expired, &expired]).await;
    }

    // Signature

    #[tokio::test]
    async fn test_verify_with_bad_signature() {
        let mut test_env = TestEnvironment::prepare().await;

        let bad = BadKeyPair(test_env.create_key(KeyRole::Packages).await);
        assert_verify_fail(&test_env, &[&bad]).await;
    }

    #[tokio::test]
    async fn test_verify_with_bad_and_good_signature() {
        let mut test_env = TestEnvironment::prepare().await;

        let bad = BadKeyPair(test_env.create_key(KeyRole::Packages).await);
        let good = test_env.create_key(KeyRole::Packages).await;
        assert_verify_pass(&test_env, &[&bad.0, &good]).await;
        assert_verify_pass(&test_env, &[&good, &bad.0]).await;
    }

    // Caching

    #[tokio::test]
    async fn test_caching() {
        let mut test_env = TestEnvironment::prepare().await;

        let key = test_env.create_key(KeyRole::Packages).await;
        let payload = prepare_payload(&[&key], SAMPLE_DATA).await;

        assert_eq!(
            42,
            payload.get_verified(test_env.keychain()).unwrap().answer
        );

        // If there was no caching, this method call would fail, as there is no valid key to
        // perform verification in an empty keychain. Still, since there is a cache no signature
        // verification is performed and the previous result is returned.
        assert_eq!(
            42,
            payload
                .get_verified(TestEnvironment::prepare().await.keychain())
                .unwrap()
                .answer
        );
    }

    // Misc tests

    #[tokio::test]
    async fn test_deserialization_failed() {
        let mut test_env = TestEnvironment::prepare().await;
        let key = test_env.create_key(KeyRole::Packages).await;

        let payload = prepare_payload(&[&key], r#"{"answer": 42"#).await;
        assert!(matches!(
            payload.get_verified(test_env.keychain()),
            Err(Error::DeserializationFailed(_))
        ));

        let payload = prepare_payload(&[&key], r#"{"answer": 42"#).await;
        assert!(matches!(
            payload.into_verified(test_env.keychain()),
            Err(Error::DeserializationFailed(_))
        ));
    }

    #[test]
    fn test_verify_deserialized() {
        let root_key: PublicKey = serde_json::from_str(
            r#"{"role":"root","algorithm":"ecdsa-p256-sha256-asn1-spki-der","expiry":null,"public":"MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE4LmlE8W7eDS6WOI9Czcl+SPtoG+7SeLLCFDfYs/sP+TvOtEtYWJo8LZgI/uZu25o5qswadqPYCP3n45luTjJWg=="}"#,
        ).unwrap();

        let revocation_key: SignedPayload<PublicKey> = serde_json::from_str(
            r#"
               {"signatures":[{"key_sha256":"jnAhbVxJNB1iHDtxQ9npLMDc1Erl4UU3RhHbgp331Yw=","signature":"MEUCIQCpZYpke7gKyK99SaeWhiKrybMRWoJb81NOt6Ez5DWQEgIgINufdqRNmVj4cLpXE5cv61NC0cEaOiX/D2NC3yNBiq8="}],"signed":"{\"role\":\"revocation\",\"algorithm\":\"ecdsa-p256-sha256-asn1-spki-der\",\"expiry\":null,\"public\":\"MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEdSi4uLtPOIU0ZbAnwyavVNLfy/Ow0y4jaiK5JMVcsFqfoVUTmG7Z51d94SYqxMuhgQypKi1TSKEbhZSACaqsNg==\"}"}
                "#
        ).unwrap();

        let packages_key: SignedPayload<PublicKey> = serde_json::from_str(
            r#"
            {"signatures":[{"key_sha256":"jnAhbVxJNB1iHDtxQ9npLMDc1Erl4UU3RhHbgp331Yw=","signature":"MEQCIAw6+8erSIsGFKVwsjke1IRpKGBNXYR1iCM7SvUvUR8LAiBBC4+FRmTVaH7o+3J8DiRxifhsAjnLz4YoqtDxhe+CmA=="}],"signed":"{\"role\":\"packages\",\"algorithm\":\"ecdsa-p256-sha256-asn1-spki-der\",\"expiry\":null,\"public\":\"MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAETGvokaoXbOGIb9E55ee/NTGGnBSJME/odqhy9XIGwOJJ4P0oP3upA14m6c7+/qJ7qAWueVc+4V/fAnx0KAyzAw==\"}"}
            "#
        ).unwrap();

        let payload: SignedPayload<TestData> = serde_json::from_str(
            r#"
            {"signatures":[{"key_sha256":"unhy/0hUU3DDAIQzd7x5+BM4l1kDcztuALcVOfjo2yw=","signature":"MEUCIChTmjgHZiZ3O7lZiknoQXkFnOOJsBrUMDZieAMB39yeAiEAjFR6fBXGMQrGdqemPzQlgN8FIACRrlw+tfSbdxCFi9Y="}],"signed":"{\"answer\":42}"}
            "#,
        ).unwrap();

        let revoked_signatures = serde_json::from_str(r#"
        {"signatures":[{"key_sha256":"1LAfvhHLQ0bPmRSEgYcDfas2gr+7ZCSUT8MBjsksqnM=","signature":"MEUCICWz68Ry/cgEbp3hRl1zeEDB7cbAjghR4wRIbmsPZaSmAiEAu7HLBjdOjWMMaUWkj+Sm9saLy2eorY17eHY+PRQMXU0="}],"signed":"{\"revoked_content_sha256\":[],\"expires_at\":\"2025-08-05T00:00:00Z\"}"}
        "#).unwrap();

        let km = KeysManifest {
            version: ManifestVersion,
            keys: vec![revocation_key, packages_key],
            revoked_signatures,
        };

        let mut keychain = Keychain::new(&root_key).unwrap();
        keychain.load_all(&km).unwrap();
        assert_eq!(42, payload.get_verified(&keychain).unwrap().answer);
    }

    // Revocation.

    #[tokio::test]
    async fn test_verify_revocation_info() {
        let mut test_env = TestEnvironment::prepare().await;
        let key_revocation = test_env.create_key(KeyRole::Revocation).await;
        let revoked_content = RevocationInfo::new(
            vec![vec![1, 2, 3]],
            OffsetDateTime::now_utc() + EXPIRATION_EXTENSION_IN_DAYS,
        );
        let mut signed_revoked_content = SignedPayload::new(&revoked_content).unwrap();
        signed_revoked_content
            .add_signature(&key_revocation)
            .await
            .unwrap();

        let revovation_info = signed_revoked_content
            .get_verified(&key_revocation)
            .unwrap();

        let expected: &Vec<u8> = &vec![1, 2, 3];
        assert_eq!(
            revovation_info.revoked_content_sha256.first().unwrap(),
            expected
        );
    }

    #[tokio::test]
    async fn test_verify_revocation_info_incorrect_keyrole() {
        let mut test_env = TestEnvironment::prepare().await;
        let key_not_revocation_role = test_env.create_key(KeyRole::Packages).await;
        let revoked_content = RevocationInfo::new(
            vec![vec![1, 2, 3]],
            OffsetDateTime::now_utc() + EXPIRATION_EXTENSION_IN_DAYS,
        );
        let mut signed_revoked_content = SignedPayload::new(&revoked_content).unwrap();
        signed_revoked_content
            .add_signature(&key_not_revocation_role)
            .await
            .unwrap();

        let revocation_info = signed_revoked_content.get_verified(&key_not_revocation_role);
        assert!(matches!(
            revocation_info.unwrap_err(),
            Error::VerificationFailed
        ));
    }

    #[test]
    fn test_verify_deserialized_with_revocation_info() {
        // We need to recreate and initialize the keys for each these tests separately because
        // for most part the content and datetime etc. are different. So, a new set of keys is
        // generated for each test and used here.
        let mut keychain = Keychain::new(
            &serde_json::from_str(
            r#"{
            "role":"root",
            "algorithm":"ecdsa-p256-sha256-asn1-spki-der",
            "expiry":null,
            "public":"MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAECWHCWK690xv1riGZVu5NtBaDinbHndmOvwYAO71qTEZUC/sI5zWcjI1EedPl7zRidfLToVGvqU/DDMcMg6o0dA=="}
            "#).unwrap()).unwrap();

        let revocation_key: SignedPayload<PublicKey> = serde_json::from_str(
            r#"{
            "signatures":[{"key_sha256":"vNSk+m6gWtw0j9UP0Vz3TwemBHQ1nIIOqWmaGDZ5y6k=",
                    "signature":"MEUCIDzxak++Ybvs1UurFG4ZFwooCfk04qJckv1Qu7rq5EqxAiEA/xQrzmAaXZHOykxrfJnMlaSHQk/GuoXWEDO62pISiio="}],
            "signed":"{\"role\":\"revocation\",\"algorithm\":\"ecdsa-p256-sha256-asn1-spki-der\",\"expiry\":null,\"public\":\"MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEujVreV8hOhE8zzXWFSPGIcopeMX8HPIsmmnLZCy6+ojaPX7N3FwpGVjtoYbFXDdPbn71V1CjMO9hmzYLAUCV/g==\"}"
            }"#).unwrap();

        let packages_key: SignedPayload<PublicKey> = serde_json::from_str(
            r#"{
            "signatures":[{"key_sha256":"vNSk+m6gWtw0j9UP0Vz3TwemBHQ1nIIOqWmaGDZ5y6k=",
                 "signature":"MEQCIEzQxuBBoicimHDF0UCP27h9ER6mlGIq2XtpqiN9f6AOAiBRN/6+l+HiRdTQX/jUHIIHp4kcg3OF34YfsONfzUKr/Q=="}],
            "signed":"{\"role\":\"packages\",\"algorithm\":\"ecdsa-p256-sha256-asn1-spki-der\",\"expiry\":null,\"public\":\"MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEvruMS2cS1lTwcCOU64Nce36iueXudb8/nn0kXy8JHUP44XPMgFMdWwbd1HX3csd0r9rhtUwbERi/7cAZhYKErA==\"}"
            }"#).unwrap();

        let revoked_signatures: SignedPayload<RevocationInfo>  = serde_json::from_str(
            r#"{
            "signatures":[{"key_sha256":"Xb6qYHsmDiHMkBTrijStwOUoduuHq59DxMAQ1HMWzyA=",
                "signature":"MEUCIQCEgDqlYvHTBJCPJmvmSoK2MiicsTYo9MuXWOsVe4HH6AIgCDXulLu4bvX/NVJkr+Ck4g6cW8dllk/yTkyQcI52XUw="}],
                "signed":"{\"revoked_content_sha256\":[],\"expires_at\":\"2025-08-05T00:00:00Z\"}"
                }"#).unwrap();

        let km = KeysManifest {
            version: ManifestVersion,
            keys: vec![revocation_key, packages_key],
            revoked_signatures,
        };

        assert!(keychain.load_all(&km).is_ok());
        assert!(keychain
            .revocation_info()
            .unwrap()
            .revoked_content_sha256
            .is_empty());
    }

    #[test]
    #[ignore = "Needs to be tested along with Install command"]
    fn verify_revoked_payload() {
        let mut keychain = Keychain::new(
            &serde_json::from_str(
            r#"{
            "role":"root",
            "algorithm":"ecdsa-p256-sha256-asn1-spki-der",
            "expiry":null,
            "public":"MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEsmIrrJH8LARwp79Qh6w9cEAFVS/QwDpbJwHQwyGC7LiAFvXpox2Whn2zgVKgs2ehLSCnNNdqDH6H+WTDfcU91Q=="}
            "#
            ).unwrap()).unwrap();

        let revocation_key: SignedPayload<PublicKey> = serde_json::from_str(
            r#"
                {"signatures":[{"key_sha256":"0Hjy0uISLPXHhJygWpfT/subu3C07tvzuaV3xJNIoSU=",
                "signature":"MEQCICEqWyDgJ81t5y9f7xiixTD//5s8/EuYG5laHR6O7rV3AiBx4zpBQmIbci6FXCcYJIBSXjCspJbKgAgeYRcToeSUvw=="}],
                "signed":"{\"role\":\"revocation\",\"algorithm\":\"ecdsa-p256-sha256-asn1-spki-der\",\"expiry\":null,\"public\":\"MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEdPE2wdSb3dqGW/sFa0TYRAXe0hGKL1xTk9XZcrNtz4bfssW7QI8GXXAO/rlTm/n69obkPK8lin69QnUCOpAW5g==\"}"
                }"#).unwrap();

        let packages_key: SignedPayload<PublicKey> = serde_json::from_str(
            r#"
               {"signatures":[{"key_sha256":"0Hjy0uISLPXHhJygWpfT/subu3C07tvzuaV3xJNIoSU=",
               "signature":"MEQCIGeVaDYN5ADdZ3PCsfBJ+f4GvdUN+nELsuVaJyNCx6Z/AiBWaeMTXVez3MEXg51KAgu9Z8uYX9P3VmsNxgzaDtu2Rg=="}],
               "signed":"{\"role\":\"packages\",\"algorithm\":\"ecdsa-p256-sha256-asn1-spki-der\",\"expiry\":null,\"public\":\"MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEDuOHCcbc7DNhLpHBwZolEgX33VOf039pRi0FQH6rfS/0uSRawucX4LSKc6Dg4eim3SAbbtRTf+oSl0tTG3KUUg==\"}"}
            "#).unwrap();

        let revoked_signatures: SignedPayload<RevocationInfo>  = serde_json::from_str(
            r#"
              {"signatures":[{"key_sha256":"jpwiXafZnKIYVd50u9qlqp/X+KXuB/qtu0chxx3bO5w=",
              "signature":"MEUCIQCQoHFae7QtfiSw0Okz+dQ4HOtR4Or0XutByRMySpdhwgIgNoQQpeEPmTK/2Vkg6xWP0oIBUF3PV88/RMIUSwLZATU="}],
              "signed":"{\"revoked_content_sha256\":[[57,55,54,101,97,97,99,53,53,99,101,102,102,50,49,53,53,48,99,55,100,52,97,57,100,52,97,101,100,101,52,101,48,49,102,48,57,100,99,57,53,51,48,48,57,51,97,98,98,57,102,49,100,48,56,53,101,49,48,50,51,99,55,49]],\"expires_at\":\"2025-08-05T00:00:00Z\"}"
              }"#).unwrap();

        let km = KeysManifest {
            version: ManifestVersion,
            keys: vec![revocation_key, packages_key],
            revoked_signatures,
        };

        assert!(keychain.load_all(&km).is_ok());
        assert_eq!(
            keychain
                .revocation_info()
                .unwrap()
                .revoked_content_sha256
                .len(),
            1
        );

        let s: SignedPayload<String> = serde_json::from_str(
            r#"
              {"signatures":[{"key_sha256":"UExDkEYvGWey+Cbllq3lu0gWZnj+k3yXmtKT10E8hUw=",
              "signature":"MEQCIBhecxmblDtvC0LM0Kb/GEZszbUK14XHEVTKY3mKJ70hAiBkzqiQx++aCbUKEn3GWOqlu60BoZJo5JcrwAbGggAueg=="}],
              "signed":"976eaac55ceff21550c7d4a9d4aede4e01f09dc9530093abb9f1d085e1023c71"}
            "#
        ).unwrap();

        // Since the payload is in the revoked signatures, this verification will fail.
        assert!(matches!(
            s.get_verified(&keychain),
            Err(Error::VerificationFailed)
        ));
    }

    #[test]
    #[ignore = "Needs to be tested along with Install command"]
    fn verify_revoked_payload_expired_hashes() {
        let mut keychain = Keychain::new(
            &serde_json::from_str(
                r#"
              {"role":"root",
              "algorithm":"ecdsa-p256-sha256-asn1-spki-der",
              "expiry":null,
              "public":"MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEysuTQtxZPS8brgpNB9drJEVKAw/VKgMBNwj8Z9rgJu2gZvs3lhScO6PYLJF4RlYOeroVKJ5iTQAwvS5+f8fuPw=="}
             "#).unwrap()).unwrap();

        let revocation_key: SignedPayload<PublicKey> = serde_json::from_str(
            r#"
                 {"signatures":[{"key_sha256":"kymyTeYBNiOW8JqBr3FBB96stFb07TdvWmKsYFaASqY=",
                 "signature":"MEYCIQD4op6c7uAoYwENrInZ3+DlUYeCfIzhk3fPZjacSpEZqQIhANmADQcvEFdtSfsIY550Vsozmyk9q+DD8V5bN7VqVALi"}],
                 "signed":"{\"role\":\"revocation\",\"algorithm\":\"ecdsa-p256-sha256-asn1-spki-der\",\"expiry\":null,\"public\":\"MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEqReu6kYhzYa6fI7LB14gG2yecR+jtXChwf1Z5wEHLasU6NDu7iE2eBWUeggOhIMnbKkR66F5B6F4KQIxdp9A2w==\"}"}
        "#).unwrap();

        let packages_key: SignedPayload<PublicKey> = serde_json::from_str(
            r#"
                {"signatures":[{"key_sha256":"kymyTeYBNiOW8JqBr3FBB96stFb07TdvWmKsYFaASqY=",
                "signature":"MEYCIQDSqBcxonf8PhwWl1IrJoRmHJTmDj6kNO283vmpeXyxnwIhAOJckzfu/PQ1J3UjR3xVYwOM8ZUMK/jmPjLb9wmyPFNb"}],
                "signed":"{\"role\":\"packages\",\"algorithm\":\"ecdsa-p256-sha256-asn1-spki-der\",\"expiry\":null,\"public\":\"MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEmZl6pB5HF0fc7hkOfnrP4WNhk+jFDxzXDUoawhRnpu+XrYrdgMTl1+wcobxk5rwSdAtarm63vPPkQJEV6LrTxA==\"}"}
            "#
        ).unwrap();

        let revoked_signatures: SignedPayload<RevocationInfo>  = serde_json::from_str(
            r#"
              {"signatures":[{"key_sha256":"jONxDp7vf+gLKbRwNhriqdZgKrNzKz66hyNTpMLDJNQ=",
              "signature":"MEUCIQCvcU+4YVx2roWJ9Coq/OzUJJxOANLm2VSTyCeCOZptDwIgA7bZYU78oHQPISarXI6mI+BAU0ut3zqWjAh2/bpRejU="}],
              "signed":"{\"revoked_content_sha256\":[],\"expires_at\":\"1999-12-31T00:00:00Z\"}"}
            "#
        ).unwrap();

        let km = KeysManifest {
            version: ManifestVersion,
            keys: vec![revocation_key, packages_key],
            revoked_signatures,
        };

        assert!(keychain.load_all(&km).is_ok());
        assert_eq!(
            keychain
                .revocation_info()
                .unwrap()
                .revoked_content_sha256
                .len(),
            0
        );

        let s: SignedPayload<String> = serde_json::from_str(
            r#"
               {"signatures":[{"key_sha256":"+bdNiRBQ5inCKFRFsoLVFP1hGAdUs1RylZT/SSUQGvI=",
               "signature":"MEQCID8V05t2bFC/GtUFit9jF17AlUqVchRWBFMhFuLjX0PuAiAlofxEfyIc9ZqB5fvmHk5NEP+vis4auT4429xqICv9Sw=="}],
               "signed":"\"976eaac55ceff21550c7d4a9d4aede4e01f09dc9530093abb9f1d085e1023c71\""}
            "#
        ).unwrap();

        // Since revocation info has a date that is long passed, the error is about expiration of signatures.
        assert!(matches!(
            s.get_verified(&keychain),
            Err(Error::RevocationSignatureExpired(..))
        ));
    }

    // Utilities

    async fn assert_verify_pass<K: KeyPair>(test_env: &TestEnvironment, keys: &[&K]) {
        let get_payload = prepare_payload(keys, SAMPLE_DATA).await;
        assert_eq!(
            42,
            get_payload
                .get_verified(test_env.keychain())
                .unwrap()
                .answer
        );

        // Two separate payloads are used to avoid caching.
        let into_payload = prepare_payload(keys, SAMPLE_DATA).await;
        assert_eq!(
            42,
            into_payload
                .into_verified(test_env.keychain())
                .unwrap()
                .answer
        );
    }

    async fn assert_verify_fail<K: KeyPair>(test_env: &TestEnvironment, keys: &[&K]) {
        let get_payload = prepare_payload(keys, SAMPLE_DATA).await;
        assert!(matches!(
            get_payload.get_verified(test_env.keychain()).unwrap_err(),
            Error::VerificationFailed
        ));

        // Two separate payloads are used to avoid caching.
        let into_payload = prepare_payload(keys, SAMPLE_DATA).await;
        assert!(matches!(
            into_payload.into_verified(test_env.keychain()).unwrap_err(),
            Error::VerificationFailed
        ));
    }

    async fn prepare_payload<K: KeyPair>(keys: &[&K], data: &str) -> SignedPayload<TestData> {
        let mut signatures = vec![];
        for key in keys {
            let signature = serde_json::json!({
                "key_sha256": key.public().calculate_id(),
                "signature": base64_encode(key.sign(
                    &PayloadBytes::borrowed(data.as_bytes())
                ).await.unwrap().as_bytes()),
            });
            signatures.push(signature)
        }

        serde_json::from_value(serde_json::json!({
            "signatures": signatures,
            "signed": data
        }))
        .unwrap()
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct TestData {
        answer: i32,
    }

    impl Signable for TestData {
        const SIGNED_BY_ROLE: KeyRole = KeyRole::Packages;
    }

    struct BadKeyPair(pub EphemeralKeyPair);

    impl KeyPair for BadKeyPair {
        fn public(&self) -> &PublicKey {
            self.0.public()
        }

        async fn sign(&self, data: &PayloadBytes<'_>) -> Result<SignatureBytes<'static>, Error> {
            let signature = self.0.sign(data).await?;
            let mut broken_signature = signature.as_bytes().to_vec();
            for byte in &mut broken_signature {
                *byte = byte.wrapping_add(1);
            }

            Ok(SignatureBytes::owned(broken_signature))
        }
    }
}
