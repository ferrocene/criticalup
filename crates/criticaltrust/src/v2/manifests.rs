// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Serializable and deserializable representation of criticaltrust manifests.

use std::path::PathBuf;
use serde_semver::SemverReq;

use crate::keys::{KeyRole, PublicKey};
use crate::signatures::{Signable, SignedPayload};
use serde::{Deserialize, Serialize};

/// Typed representation of a manifest version number.
///
/// The version number is stored as a const generic rather than as a field of the struct. This is
/// done to:
///
/// * Verify that the version number is correct as part of the deserialization process.
/// * Simplify constructing manifests: you don't have to specify the version number, type
///   inference will figure out the right one.
#[derive(Clone, PartialEq, Eq, SemverReq)]
#[version("0.0.1")]
pub struct MetadataVersion;

impl std::fmt::Debug for MetadataVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("MetadataVersion: {}", MetadataVersion::version())[..])
    }
}

// Redirects

#[derive(Debug, Serialize, Deserialize)]
pub struct RedirectManifest {
    pub version: MetadataVersion,
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
    pub version: MetadataVersion,
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
    pub version: MetadataVersion,
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
    pub version: MetadataVersion,
    pub keys: Vec<SignedPayload<PublicKey>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_version_debug() {
        assert_eq!("MetadataVersion: 0.0.1", format!("{:?}", MetadataVersion));
    }

    #[test]
    fn test_manifest_version_serialize() {
        assert_eq!("0.0.1\n", serde_yaml::to_string(&MetadataVersion).unwrap());
    }

    #[test]
    fn test_manifest_version_deserialize() {
        assert_eq!(
            MetadataVersion,
            serde_yaml::from_str("0.0.1").unwrap()
        );
        assert!( serde_yaml::from_str::<MetadataVersion>("0.0.0").is_err());

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
