// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Serializable and deserializable representation of criticaltrust v2 manifests.

use crate::keys::KeyRole;
use crate::signatures::Signable;
use serde::de::Error as _;
use serde::{Deserialize, Serialize};

/// Typed representation of a manifest version number.
///
/// The version number is stored as a const generic rather than as a field of the struct. This is
/// done to:
///
/// * Verify that the version number is correct as part of the deserialization process.
/// * Simplify constructing manifests: you don't have to specify the version number, type
///   inference will figure out the right one.
#[derive(Clone, PartialEq, Eq)]
pub struct MetadataVersion<const V: u32>;

impl<const V: u32> std::fmt::Debug for MetadataVersion<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("MetadataVersion").field(&V).finish()
    }
}

impl<const V: u32> Serialize for MetadataVersion<V> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(V)
    }
}

impl<'de, const V: u32> Deserialize<'de> for MetadataVersion<V> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(deserializer)?;
        if raw != V {
            Err(D::Error::custom(format!(
                "expected version {V}, found version {raw}"
            )))
        } else {
            Ok(MetadataVersion)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Metadata {
    pub release_identifier: String,
    // TODO: channel should be an enum?
    pub channel: String,
    pub ferrocene_version: String,
    pub rust_version: String,
    pub release_date: String,
    pub support_expires_date: String,
    pub yanked: Option<Yanked>,
    pub signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum MetadataKind {
    FerroceneRelease,
}

/// Indicates that a version has been yanked, meaning it is considered problematic or potentially unstable.
/// Yanked versions are not recommended for use in new projects, and developers are strongly advised to avoid them.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Yanked {
    /// A machine-readable code representing the reason for yanking.
    pub code: u16,

    /// A human-readable explanation of why the version was yanked.
    pub reason: String,

    /// A URL pointing to documentation or resources that justify or explain the yanking decision.
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Release {
    pub product: String,
    pub release: String,
    pub commit: String,
    pub packages: Vec<ReleasePackage>,
}

impl Signable for Release {
    const SIGNED_BY_ROLE: KeyRole = KeyRole::Releases;
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct ReleasePackage {
    pub kind: MetadataKind,
    pub metadata_version: MetadataVersion<2>,
    pub metadata: Metadata,
    pub artifacts: Vec<ReleaseArtifact>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReleaseArtifact {
    pub name: String,
    pub format: ReleaseArtifactFormat,
    pub size: usize,
    pub file: String,
    #[serde(with = "crate::serde_base64")]
    pub signature: String,
    pub checksums: Checksums,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Checksums {
    #[serde(with = "hex")]
    pub sha256: [u8; 32],
    #[serde(with = "hex")]
    pub md5: [u8; 16],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy)]
pub enum ReleaseArtifactFormat {
    #[serde(rename = "tar.zst")]
    TarZst,
    #[serde(rename = "tar.xz")]
    TarXz,
    #[serde(other)]
    #[doc(hidden)]
    Unknown,
}

impl std::fmt::Display for ReleaseArtifactFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ReleaseArtifactFormat::TarZst => "tar.zst",
            ReleaseArtifactFormat::TarXz => "tar.xz",
            ReleaseArtifactFormat::Unknown => "unknown",
        };
        write!(f, "{s}")
    }
}

// test
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn test_manifest_version_debug() {
        assert_eq!("MetadataVersion(2)", format!("{:?}", MetadataVersion::<2>));
        assert_eq!(
            "MetadataVersion(42)",
            format!("{:?}", MetadataVersion::<42>)
        );
    }

    #[test]
    fn test_manifest_version_serialize() {
        assert_eq!("2", serde_json::to_string(&MetadataVersion::<2>).unwrap());
        assert_eq!("42", serde_json::to_string(&MetadataVersion::<42>).unwrap());
    }

    #[test]
    fn test_release_package_json_serialize() {
        let mut hasher = Sha256::new();
        hasher.update("Hello, World!");
        let sha256_hash_result = hasher.finalize();

        let mut hasher = md5::Md5::new();
        hasher.update("Hello, World!");
        let md5_hash_result = hasher.finalize();

        let package = ReleasePackage {
            kind: MetadataKind::FerroceneRelease,
            metadata_version: MetadataVersion,
            metadata: Metadata {
                release_identifier: "stable-25.02.0".parse().unwrap(),
                channel: "stable".parse().unwrap(),
                ferrocene_version: "a_ferrocene_version".parse().unwrap(),
                rust_version: "a_rust_version".parse().unwrap(),
                release_date: "10.10.10".parse().unwrap(),
                support_expires_date: "20.20.20".parse().unwrap(),
                yanked: Some(Yanked {
                    code: 123,
                    reason: "<human readable reason>".parse().unwrap(),
                    url: "<url to further information>".parse().unwrap(),
                }),
                signature: "<signature content>".parse().unwrap(),
            },
            artifacts: vec![ReleaseArtifact {
                signature: "<signature>".parse().unwrap(),
                name: "rustc".parse().unwrap(),
                file: "path/to/file/relative/to/repo/root".parse().unwrap(),
                format: ReleaseArtifactFormat::TarXz,
                size: 1024,
                checksums: Checksums {
                    sha256: sha256_hash_result.into(),
                    md5: md5_hash_result.into(),
                },
            }],
        };

        assert_eq!(
            package,
            serde_json::from_str::<ReleasePackage>(
                &serde_json::to_string_pretty(&package).unwrap()[..]
            )
            .unwrap()
        );

        insta::assert_json_snapshot!(&package);
    }
}
