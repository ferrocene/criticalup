#![cfg_attr(docsrs, feature(doc_auto_cfg))]

pub mod errors;
pub mod integrity;
pub mod keys;
pub mod manifests;
mod serde_base64;
mod sha256;
pub mod signatures;

#[cfg(test)]
mod test_utils;

pub use errors::Error;

/// Trait to make sure that verification of only certain types does not check for revocation.
pub trait NoRevocationCheck {}
