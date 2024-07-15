// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::WriteFileError;
use sha2::{Digest, Sha256};
use std::hash::Hasher;
use std::path::Path;
use tokio::fs::File;
use tokio::io::BufWriter;

pub(crate) async fn open_file_for_write(path: &Path) -> Result<BufWriter<File>, WriteFileError> {
    // Ensure the parent directory is always present
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(WriteFileError::CantCreateParentDirectory)?;
    }

    Ok(BufWriter::new(
        File::create(path).await.map_err(WriteFileError::Io)?,
    ))
}

/// A `Hasher` helper type which is a wrapper to a choice of cryptographic hashing algorithm
/// to generate cryptographic hash of our types. This is needed to make sure we
/// 1. do not use [`DefaultHasher`], which may change its algorithm, for hash state
/// 2. bridge the gap between normal [`Hash`] and cryptographic hash (e.g. [`Sha256`])
/// 3. better ergonomics to create a hash of our types like [`ProjectManifestProduct`] using `#[derive(Hash)]`
pub struct Sha256Hasher(Sha256);

impl Sha256Hasher {
    pub(crate) fn new() -> Self {
        Self(Sha256::new())
    }

    /// Provides the final hash value
    pub(crate) fn finalize(self) -> String {
        format!("{:x}", self.0.finalize())
    }
}

impl Hasher for Sha256Hasher {
    /// This method is unreachable and here to appease the compiler, mandatory method.
    fn finish(&self) -> u64 {
        unreachable!()
    }

    /// Update the hasher state, mandatory method.
    fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hash;

    #[test]
    fn test_sha256_for_a_struct_works() {
        #[derive(Hash)]
        struct Abc {
            name: String,
            version: String,
            dependencies: Vec<String>,
        }

        let abc = Abc {
            name: "abc".to_string(),
            version: "1.2.3".to_string(),
            dependencies: vec!["dep2".to_string(), "dep1".to_string()],
        };

        let mut hasher = Sha256Hasher::new();
        abc.hash(&mut hasher);
        let final_hash = hasher.finalize();
        assert_eq!(
            "fb9eee112b5cee551f7a5088402e53dbbdaec2a1e1cd3f4663a1b81b1b53015f",
            final_hash
        )
    }
}
