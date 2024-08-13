// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::keys::KeyRole;
use crate::signatures::Signable;
use crate::NoRevocationsCheck;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Holds hashes of revoked content which are included as a part of the [`KeysManifest`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevocationInfo {
    // Incoming SHA256 data from the API is in the form of String, but we save each as a `Vec<u8>`.
    pub revoked_content_sha256: Vec<Vec<u8>>,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

impl RevocationInfo {
    pub fn new(revoked_content_sha256: Vec<Vec<u8>>, expires_at: OffsetDateTime) -> Self {
        RevocationInfo {
            revoked_content_sha256,
            expires_at,
        }
    }
}

impl Signable for RevocationInfo {
    const SIGNED_BY_ROLE: KeyRole = KeyRole::Revocation;
}

/// Make sure verification of `RevocationInfo` type does no checks for revocations.
///
/// If we did, then this would be a circular logic and we say No! to such logic.
impl NoRevocationsCheck for RevocationInfo {}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn new_revocation_info() {
        let r = RevocationInfo::new(vec![vec![12, 21, 33]], datetime!(2400-10-10 00:00 UTC));
        assert_eq!("RevocationInfo { revoked_content_sha256: [[12, 21, 33]], expires_at: 2400-10-10 0:00:00.0 +00:00:00 }", format!("{:?}", r));
    }
}
