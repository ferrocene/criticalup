// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::keys::KeyRole;
use crate::signatures::Signable;
use crate::NoRevocationsCheck;
use serde::{Deserialize, Serialize};
use time::macros::datetime;
use time::OffsetDateTime;

/// Holds hashes of revoked content which are included as a part of the [`KeysManifest`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevocationInfo {
    pub revoked_content_sha256: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

impl RevocationInfo {
    pub fn new() -> Self {
        RevocationInfo {
            revoked_content_sha256: Vec::new(),
            expires_at: datetime!(2025-01-01 0:00 UTC),
        }
    }
}

impl Default for RevocationInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl Signable for RevocationInfo {
    const SIGNED_BY_ROLE: KeyRole = KeyRole::Revocation;
}

/// Make sure verification of `RevocationInfo` type does no checks for revocations.
///
/// If we did, then this would be a circular logic and we say No! to such logic.
impl NoRevocationsCheck for RevocationInfo {}
