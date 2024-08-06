#![cfg_attr(docsrs, feature(doc_auto_cfg))]

// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

extern crate core;

pub mod errors;
pub mod integrity;
pub mod keys;
pub mod manifests;
mod serde_base64;
mod sha256;
pub mod signatures;

pub mod revocation_info;
#[cfg(test)]
mod test_utils;

pub use errors::Error;

/// Trait to make sure that verification of only certain types does not check for revocation.
pub trait NoRevocationsCheck {}
