// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Serializable and deserializable representation of criticaltrust v2 manifests.

use serde_semver::SemverReq;
use std::path::PathBuf;

use crate::keys::{KeyRole, PublicKey};
use crate::signatures::{Signable, SignedPayload};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, SemverReq)]
#[version("0.0.1")]
pub struct MetadataVersion;

impl std::fmt::Debug for MetadataVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("MetadataVersion: {}", MetadataVersion::version())[..])
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
    pub signature: String
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy)]
pub enum MetadataKind {
  #[serde(rename = "Ferrocene-Release")]
  FerroceneRelease
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Yanked {
    pub code: u16,
    pub human: String,
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
    pub metadata_version: MetadataVersion,
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
    #[serde(with = "crate::serde_base64")]
    pub sha256: Vec<u8>,
    #[serde(with = "crate::serde_base64")]
    pub md5: Vec<u8>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_version_debug() {
        assert_eq!("MetadataVersion: 0.0.1", format!("{:?}", MetadataVersion));
    }

    #[test]
    fn test_manifest_version_serialize() {
        assert_eq!("\"0.0.1\"", serde_json::to_string(&MetadataVersion).unwrap());
    }

    #[test]
    fn test_manifest_version_deserialize() {
        assert_eq!(MetadataVersion, serde_json::from_str("\"0.0.1\"").unwrap());
        assert!(serde_json::from_str::<MetadataVersion>("0.0.0").is_err());
    }

    #[test]
    fn test_release_package_json_serialize() {
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
                    human: "<human readable reason>".parse().unwrap(),
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
                    sha256: "sha256_abcdefgh".as_bytes().to_vec(),
                    md5: "md5_abcdefgh".as_bytes().to_vec(),
                }
            }]
        };

        assert_eq!(
            package,
            serde_json::from_str::<ReleasePackage>(&serde_json::to_string_pretty(&package).unwrap()[..])
                .unwrap()
        );

        insta::assert_snapshot!(&serde_json::to_string_pretty(&package).unwrap()[..]);
    }
}
