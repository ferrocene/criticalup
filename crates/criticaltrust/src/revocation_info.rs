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
    pub revoked_content_sha256: Vec<RevocationContentSha256>,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct RevocationContentSha256(#[serde(with = "crate::serde_base64")] Vec<u8>);

impl From<Vec<u8>> for RevocationContentSha256 {
    fn from(revoked_content_sha256: Vec<u8>) -> Self {
        RevocationContentSha256(revoked_content_sha256)
    }
}

impl RevocationInfo {
    pub fn new(datetime: OffsetDateTime) -> Self {
        RevocationInfo {
            revoked_content_sha256: Vec::new(),
            expires_at: datetime,
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
