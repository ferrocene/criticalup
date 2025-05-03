// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

//! High-level interface to verify the integrity of archives and installations.

mod detect_manifest;
mod verifier;

use std::path::PathBuf;

pub use verifier::{IntegrityVerifier, VerifiedPackage};

/// Integrity error detected by [`IntegrityVerifier`].
#[derive(Debug, thiserror::Error)]
pub enum IntegrityError {
    #[error("failed to deserialize the package manifest at {path}")]
    PackageManifestDeserialization {
        path: PathBuf,
        #[source]
        inner: serde_json::Error,
    },
    #[error("failed to verify the package manifest at {path}")]
    PackageManifestVerification {
        path: PathBuf,
        #[source]
        inner: crate::Error,
    },
    #[error("wrong POSIX permissions for {path} (expected: {expected:o}, found {found:o})")]
    WrongPosixPermissions {
        path: PathBuf,
        expected: u32,
        found: u32,
    },
    #[error("wrong checksum for {path}")]
    WrongChecksum { path: PathBuf },
    #[error("the product name of {path} is not {expected} (the file path is wrong)")]
    WrongProductName { path: PathBuf, expected: String },
    #[error("the package name of {path} is not {expected} (the file path is wrong)")]
    WrongPackageName { path: PathBuf, expected: String },
    #[error("no package manifest found")]
    NoPackageManifestFound,
    #[error("expected file {path} is not present")]
    MissingFile { path: PathBuf },
    #[error("unexpected file {path} is present")]
    UnexpectedFile { path: PathBuf },
    #[error("unexpected file {path} in prefix managed by CriticalUp ({prefix})")]
    UnexpectedFileInManagedPrefix { path: PathBuf, prefix: PathBuf },
    #[error("file {path} is referenced by multiple package manifests")]
    FileReferencedByMultipleManifests { path: PathBuf },
    #[error("file {path} was loaded multiple times")]
    FileLoadedMultipleTimes { path: PathBuf },
}
