// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Serializable and deserializable representation of criticaltrust manifests.

use std::path::PathBuf;

use crate::keys::{KeyRole, PublicKey};
use crate::signatures::{Signable, SignedPayload};
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
pub struct MetadataVersion<const MAJOR: u32, const MINOR: u32, const PATCH: u32>;

impl<const MAJOR: u32, const MINOR: u32, const PATCH: u32> std::fmt::Debug for MetadataVersion<MAJOR,MINOR,PATCH> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("MetadataVersion").field(&MAJOR).field(&MINOR).field(&PATCH).finish()
    }
}

impl<const MAJOR: u32, const MINOR: u32, const PATCH: u32> Serialize for MetadataVersion<MAJOR,MINOR,PATCH> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let formatted = format!("{}.{}.{}", MAJOR, MINOR, PATCH);
        serializer.serialize_str(&formatted[..])
    }
}

impl<'de, const MAJOR: u32, const MINOR: u32, const PATCH: u32> Deserialize<'de> for MetadataVersion<MAJOR,MINOR,PATCH> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(deserializer)?;
        if raw != MAJOR {
            Err(D::Error::custom(format!(
                "expected version {MAJOR}, found version {raw}"
            )))
        } else {
            Ok(MetadataVersion)
        }
    }
}

// Redirects

#[derive(Debug, Serialize, Deserialize)]
pub struct RedirectManifest {
    pub version: MetadataVersion<0,0,1>,
    #[serde(flatten)]
    pub payload: SignedPayload<Redirect>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Redirect {
    pub nonce: String,
    pub to: String,
}

impl Signable for Redirect {
    const SIGNED_BY_ROLE: KeyRole = KeyRole::Redirects;
}

// Releases

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub version: MetadataVersion<1,2,3>,
    #[serde(flatten)]
    pub signed: SignedPayload<Release>,
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
pub struct ReleasePackage {
    pub package: String,
    pub artifacts: Vec<ReleaseArtifact>,
    pub dependencies: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReleaseArtifact {
    pub format: ReleaseArtifactFormat,
    pub size: usize,
    #[serde(with = "crate::serde_base64")]
    pub sha256: Vec<u8>,
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
        write!(f, "{}", s)
    }
}

// Packages

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageManifest {
    pub version: MetadataVersion<1,2,3>,
    #[serde(flatten)]
    pub signed: SignedPayload<Package>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Package {
    pub product: String,
    pub package: String,
    pub commit: String,
    pub files: Vec<PackageFile>,
    pub managed_prefixes: Vec<String>,
}

impl Signable for Package {
    const SIGNED_BY_ROLE: KeyRole = KeyRole::Packages;
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PackageFile {
    pub path: PathBuf,
    pub posix_mode: u32,
    #[serde(with = "crate::serde_base64")]
    pub sha256: Vec<u8>,
    pub needs_proxy: bool,
}

// Keys

#[derive(Debug, Serialize, Deserialize)]
pub struct KeysManifest {
    pub version: MetadataVersion<1,2,3>,
    pub keys: Vec<SignedPayload<PublicKey>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_version_debug() {
        assert_eq!("MetadataVersion(2, 3, 5)", format!("{:?}", MetadataVersion::<2,3,5>));
        assert_eq!(
            "MetadataVersion(1, 2, 3)",
            format!("{:?}", MetadataVersion::<1,2,3>)
        );
    }

    #[test]
    fn test_manifest_version_serialize() {
        assert_eq!("1.2.3\n", serde_yaml::to_string(&MetadataVersion::<1,2,3>).unwrap());
        assert_eq!("41.42.43\n", serde_yaml::to_string(&MetadataVersion::<41, 42, 43>).unwrap());
    }

    #[test]
    fn test_manifest_version_deserialize() {
        assert_eq!(
            MetadataVersion,
            serde_yaml::from_str::<MetadataVersion<2,3,5>>("1.2.3").unwrap()
        );
        assert_eq!(
            MetadataVersion,
            serde_yaml::from_str::<MetadataVersion<42,3,5>>("42,3,5").unwrap()
        );

        assert!(serde_yaml::from_str::<MetadataVersion<2,3,5>>("42").is_err());
        assert!(serde_yaml::from_str::<MetadataVersion<42,3,5>>("1").is_err());
    }

    #[test]
    fn test_release_package_yaml_serialize() {
        let package = ReleasePackage {
            package: "ferrocene-docs".to_string(),
            artifacts: vec![ReleaseArtifact {
                format: ReleaseArtifactFormat::TarXz,
                size: 1024,
                sha256: "abcdefgh".as_bytes().to_vec(),
            }],
            dependencies: vec![],
        };

        assert_eq!(
            package,
            serde_yaml::from_str::<ReleasePackage>(&serde_yaml::to_string(&package).unwrap()[..])
                .unwrap()
        );

        insta::assert_snapshot!(&serde_yaml::to_string(&package).unwrap()[..]);
    }
}
