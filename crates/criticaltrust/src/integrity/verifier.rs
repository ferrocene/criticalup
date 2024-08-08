// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::integrity::detect_manifest::{is_package_manifest, FoundPackageManifest};
use crate::integrity::IntegrityError;
use crate::manifests::{PackageFile, PackageManifest};
use crate::sha256::hash_sha256;
use crate::signatures::Keychain;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Verify the integrity of a CriticalUp archive or installation.
///
/// The verifier does **no I/O**: instead it allows verifying individual files incrementally, and
/// delegates I/O to the caller. This allows verifying an archive on the fly as it's being
/// downloaded, in addition to verifying the contents of a directory in the filesystem.
///
/// The verifier will collect and return all the integrity errors it finds, instead of
/// short-circuiting at the first encountered error. The list of errors is only accessible after
/// [`verify`](IntegrityVerifier::verify) is called.
pub struct IntegrityVerifier<'a> {
    keychain: &'a Keychain,
    errors: Vec<IntegrityError>,
    verified_packages: Vec<VerifiedPackage>,
    allow_external_files: bool,

    managed_prefixes: HashSet<PathBuf>,
    loaded_files: HashSet<PathBuf>,
    referenced_by_manifests_but_missing: HashMap<PathBuf, PackageFile>,
    added_but_not_referenced_by_manifests: HashMap<PathBuf, FoundFile>,
}

impl<'a> IntegrityVerifier<'a> {
    /// Create a new verifier instance, using the provided keyring to verify the digital signatures
    /// of the package manifests.
    pub fn new(keychain: &'a Keychain) -> Self {
        Self {
            keychain,
            errors: Vec::new(),
            verified_packages: Vec::new(),
            allow_external_files: false,

            managed_prefixes: HashSet::new(),
            loaded_files: HashSet::new(),
            referenced_by_manifests_but_missing: HashMap::new(),
            added_but_not_referenced_by_manifests: HashMap::new(),
        }
    }

    /// Decide whether all files must be referenced by a verified manifest, or "external" files not
    /// referenced by any manifest are allowed. By default, external files are **not** allowed.
    ///
    /// Note that even when external files are allowed, they must not be present in path prefixes a
    /// manifest considers managed exclusively by CriticalUp, and errors will be returned in those
    /// cases.
    ///
    /// This setting can be toggled at any point before calling
    /// [`verify`](IntegrityVerifier::verify), as the checks for external files are performed in
    /// that method.
    pub fn allow_external_files(&mut self, allow: bool) {
        self.allow_external_files = allow;
    }

    /// Include the provided path and contents in the files pending verification.
    ///
    /// The order in which files are added does not matter, but the same file can't be added twice.
    /// The verifier will not store in memory the contents of the file, but it will keep track of
    /// the metadata potentially until [`verify`](IntegrityVerifier::verify) is called.
    pub fn add(&mut self, path: &Path, mode: u32, contents: &[u8]) {
        let path_str = path.to_string_lossy().to_string();
        if !self.loaded_files.insert(path.to_owned()) {
            self.errors.push(IntegrityError::FileLoadedMultipleTimes {
                path: path.to_owned(),
            });
            return;
        }

        if let Some(found) = is_package_manifest(&path_str) {
            if let Err(err) = self.add_package_manifest(path, &found, contents) {
                self.errors.push(err);
            }
        } else {
            let entry = FoundFile {
                mode,
                sha256: hash_sha256(contents),
            };

            if let Some(manifest) = self.referenced_by_manifests_but_missing.remove(path) {
                self.verify_file(&path_str, &manifest, &entry);
            } else {
                self.added_but_not_referenced_by_manifests
                    .insert(path.into(), entry);
            }
        }
    }

    /// Perform the final checks and return the outcome of the verification. The method either
    /// returns all the packages it successfully verified, or if any error occured during
    /// verification it will return all encountered errors.
    pub fn verify(mut self) -> Result<Vec<VerifiedPackage>, Vec<IntegrityError>> {
        if self.verified_packages.is_empty() {
            self.errors.push(IntegrityError::NoPackageManifestFound);
        }

        for path in self.referenced_by_manifests_but_missing.into_keys() {
            self.errors.push(IntegrityError::MissingFile { path });
        }

        for path in self.added_but_not_referenced_by_manifests.into_keys() {
            if self.allow_external_files {
                for prefix in &self.managed_prefixes {
                    if path.starts_with(prefix) {
                        self.errors
                            .push(IntegrityError::UnexpectedFileInManagedPrefix {
                                path,
                                prefix: prefix.clone(),
                            });
                        break;
                    }
                }
            } else {
                self.errors.push(IntegrityError::UnexpectedFile { path });
            }
        }

        if self.errors.is_empty() {
            Ok(self.verified_packages)
        } else {
            Err(self.errors)
        }
    }

    fn add_package_manifest(
        &mut self,
        path: &Path,
        found: &FoundPackageManifest,
        contents: &[u8],
    ) -> Result<(), IntegrityError> {
        let manifest = serde_json::from_slice::<PackageManifest>(contents)
            .map_err(|e| IntegrityError::PackageManifestDeserialization {
                path: path.into(),
                inner: e,
            })?
            .signed
            .into_verified(self.keychain, self.keychain.revocation_info())
            .map_err(|e| IntegrityError::PackageManifestVerification {
                path: path.into(),
                inner: e,
            })?;

        if found.product != manifest.product {
            return Err(IntegrityError::WrongProductName {
                path: path.into(),
                expected: manifest.product,
            });
        }
        if found.package != manifest.package {
            return Err(IntegrityError::WrongPackageName {
                path: path.into(),
                expected: manifest.package,
            });
        }

        let mut proxies_paths = BTreeMap::new();
        let prefix = found.prefix.map(PathBuf::from).unwrap_or_default();
        for file in manifest.files {
            let file_path = prefix.join(&file.path);
            let file_str = file_path.to_string_lossy().to_string();

            if file.needs_proxy {
                let proxy_name = file_path
                    .file_name()
                    .map(PathBuf::from)
                    .unwrap_or(file_path.clone());

                proxies_paths.insert(proxy_name, file_path.clone());
            }

            if let Some(found) = self
                .added_but_not_referenced_by_manifests
                .remove(&file_path)
            {
                self.verify_file(&file_str, &file, &found);
            } else if self.loaded_files.contains(&file_path)
                || self
                    .referenced_by_manifests_but_missing
                    .insert(file_path.clone(), file)
                    .is_some()
            {
                self.errors
                    .push(IntegrityError::FileReferencedByMultipleManifests { path: file_path });
            }
        }

        for managed_prefix in manifest.managed_prefixes {
            self.managed_prefixes.insert(prefix.join(&managed_prefix));
        }

        self.verified_packages.push(VerifiedPackage {
            product: manifest.product,
            package: manifest.package,
            proxies_paths,
        });

        Ok(())
    }

    fn verify_file(&mut self, path: &str, manifest: &PackageFile, actual: &FoundFile) {
        #[cfg(not(windows))] // Windows does not do file modes.
        if manifest.posix_mode != actual.mode {
            self.errors.push(IntegrityError::WrongPosixPermissions {
                path: path.into(),
                expected: manifest.posix_mode,
                found: actual.mode,
            });
        }
        if manifest.sha256 != actual.sha256 {
            self.errors
                .push(IntegrityError::WrongChecksum { path: path.into() });
        }
    }
}

/// Information about a package verified by [`IntegrityVerifier`].
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct VerifiedPackage {
    /// Name of the product this package belongs to.
    pub product: String,
    /// Name of the package.
    pub package: String,
    /// List of the paths of all binaries that need a proxy.
    pub proxies_paths: BTreeMap<PathBuf, PathBuf>,
}

struct FoundFile {
    #[cfg_attr(windows, allow(dead_code))] // Windows does not do file modes
    mode: u32,
    sha256: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{EphemeralKeyPair, KeyRole};
    use crate::manifests::{ManifestVersion, Package};
    use crate::signatures::SignedPayload;
    use crate::test_utils::TestEnvironment;
    use crate::Error;
    use itertools::Itertools;
    use once_cell::sync::Lazy;
    use std::borrow::Cow;
    use std::ffi::OsStr;

    // Note that the tests verify all possible permutations of input files, ensuring the expected
    // behavior regardless of the order files are provided to the verifier.

    static BIN_A: Lazy<TestFile> = Lazy::new(|| TestFile::new("bin/a", 0o755, b"foo binary"));
    static BIN_B: Lazy<TestFile> = Lazy::new(|| TestFile::new("bin/b", 0o755, b"bar binary"));
    static SHARE_A: Lazy<TestFile> = Lazy::new(|| TestFile::new("share/a", 0o644, b"a file"));
    static SHARE_B: Lazy<TestFile> = Lazy::new(|| TestFile::new("share/b", 0o644, b"b file"));

    macro_rules! btreemap {
        ($($key:expr => $value:expr),*$(,)?) => {{
            let mut map = BTreeMap::new();
            $(map.insert($key.into(), $value.into());)*
            map
        }}
    }

    macro_rules! errors {
        ($($pat:pat $(if $if:expr)?),*$(,)?) => {{
            let errors: &[(&str, fn(&IntegrityError) -> bool)] = &[$(
                (
                    stringify!($pat $(if $if)?),
                    |error| match error {
                        $pat $(if $if)? => true,
                        _ => false,
                    },
                ),
            )*];
            errors
        }}
    }

    #[test]
    fn test_no_manifests() {
        IntegrityTest::new().assert_errors(errors![IntegrityError::NoPackageManifestFound]);
    }

    #[test]
    fn test_one_manifest_with_files() {
        IntegrityTest::new()
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A).file(&SHARE_A))
            .file(&BIN_A)
            .file(&SHARE_A)
            .assert_verified(&[("a", "b")]);
    }

    #[test]
    fn test_one_manifest_with_files_in_a_prefix() {
        IntegrityTest::new()
            .manifest(
                ManifestBuilder::new("a", "b")
                    .file(&BIN_A)
                    .file(&SHARE_A)
                    .prefix("foo/"),
            )
            .file(&BIN_A.clone().prefix("foo/"))
            .file(&SHARE_A.clone().prefix("foo/"))
            .assert_verified(&[("a", "b")]);
    }

    #[test]
    fn test_multiple_manifests_in_different_prefixes() {
        IntegrityTest::new()
            .manifest(
                ManifestBuilder::new("a", "b")
                    .file(&BIN_A)
                    .file(&SHARE_A)
                    .prefix("foo/"),
            )
            .manifest(
                ManifestBuilder::new("a", "c")
                    .file(&BIN_A)
                    .file(&SHARE_A)
                    .prefix("bar/"),
            )
            .file(&BIN_A.clone().prefix("foo/"))
            .file(&BIN_A.clone().prefix("bar/"))
            .file(&SHARE_A.clone().prefix("foo/"))
            .file(&SHARE_A.clone().prefix("bar/"))
            .assert_verified(&[("a", "b"), ("a", "c")]);
    }

    #[test]
    fn test_multiple_manifests_in_the_same_prefix() {
        IntegrityTest::new()
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A).file(&SHARE_A))
            .manifest(ManifestBuilder::new("a", "c").file(&BIN_B).file(&SHARE_B))
            .file(&BIN_A)
            .file(&BIN_B)
            .file(&SHARE_A)
            .file(&SHARE_B)
            .assert_verified(&[("a", "b"), ("a", "c")]);
    }

    #[test]
    fn test_manifest_nested_inside_other_manifest() {
        IntegrityTest::new()
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A).file(&SHARE_A))
            .manifest(
                ManifestBuilder::new("a", "c")
                    .file(&BIN_A)
                    .file(&SHARE_A)
                    .prefix("share/foo/"),
            )
            .file(&BIN_A)
            .file(&BIN_A.clone().prefix("share/foo/"))
            .file(&SHARE_A.clone().prefix("share/foo/"))
            .file(&SHARE_A)
            .assert_verified(&[("a", "b"), ("a", "c")]);
    }

    #[test]
    fn test_same_file_in_multiple_manifests() {
        IntegrityTest::new()
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A))
            .manifest(ManifestBuilder::new("a", "c").file(&BIN_A).file(&BIN_B))
            .file(&BIN_A)
            .file(&BIN_B)
            .assert_errors(errors![
                IntegrityError::FileReferencedByMultipleManifests { path } if path == Path::new("bin/a"),
            ]);
    }

    #[test]
    fn test_files_with_wrong_checksum() {
        IntegrityTest::new()
            .manifest(
                ManifestBuilder::new("a", "b")
                    .file(&BIN_A)
                    .file(&BIN_B)
                    .file(&SHARE_A),
            )
            .file(&BIN_A.clone().add_content(b"!"))
            .file(&BIN_B.clone().add_content(b"!"))
            .file(&SHARE_A)
            .assert_errors(errors![
                IntegrityError::WrongChecksum { path } if path == Path::new("bin/a"),
                IntegrityError::WrongChecksum { path } if path == Path::new("bin/b"),
            ]);
    }

    #[cfg(not(windows))] // Windows does not have file modes
    #[test]
    fn test_files_with_wrong_mode() {
        IntegrityTest::new()
            .manifest(
                ManifestBuilder::new("a", "b")
                    .file(&BIN_A)
                    .file(&BIN_B)
                    .file(&SHARE_A),
            )
            .file(&BIN_A.clone().mode(0o644))
            .file(&BIN_B.clone().mode(0o644))
            .file(&SHARE_A)
            .assert_errors(errors![
                IntegrityError::WrongPosixPermissions {
                    path,
                    expected: 0o755,
                    found: 0o644,
                } if path == Path::new("bin/a"),
                IntegrityError::WrongPosixPermissions {
                    path,
                    expected: 0o755,
                    found: 0o644,
                } if path == Path::new("bin/b"),
            ]);
    }

    #[cfg(not(windows))] // Windows does not have file modes
    #[test]
    fn test_files_with_both_wrong_mode_and_wrong_checksum() {
        IntegrityTest::new()
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A).file(&BIN_B))
            .file(&BIN_A.clone().add_content(b"!").mode(0o644))
            .file(&BIN_B)
            .assert_errors(errors![
                IntegrityError::WrongPosixPermissions {
                    path,
                    expected: 0o755,
                    found: 0o644,
                } if path == Path::new("bin/a"),
                IntegrityError::WrongChecksum { path } if path == Path::new("bin/a"),
            ]);
    }

    #[test]
    fn test_mismatched_product_name() {
        IntegrityTest::new()
            .manifest_in(
                "share/criticaltrust/z/b.json",
                ManifestBuilder::new("a", "b").file(&BIN_A),
            )
            .file(&BIN_A)
            .assert_errors(errors![
                IntegrityError::WrongProductName { path, expected }
                    if expected == "a" && path == Path::new("share/criticaltrust/z/b.json"),
                // The manifest is completely ignored, resulting in more errors.
                IntegrityError::NoPackageManifestFound,
                IntegrityError::UnexpectedFile { path } if path == Path::new("bin/a"),
            ]);
    }

    #[test]
    fn test_mismatched_package_name() {
        IntegrityTest::new()
            .manifest_in(
                "share/criticaltrust/a/z.json",
                ManifestBuilder::new("a", "b").file(&BIN_A),
            )
            .file(&BIN_A)
            .assert_errors(errors![
                IntegrityError::WrongPackageName { path, expected }
                    if expected == "b" && path == Path::new("share/criticaltrust/a/z.json"),
                // The manifest is completely ignored, resulting in more errors.
                IntegrityError::NoPackageManifestFound,
                IntegrityError::UnexpectedFile { path } if path == Path::new("bin/a"),
            ]);
    }

    #[test]
    fn test_files_not_in_manifest() {
        IntegrityTest::new()
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A))
            .file(&BIN_A)
            .file(&SHARE_A)
            .assert_errors(errors![
                IntegrityError::UnexpectedFile { path } if path == Path::new("share/a"),
            ]);
    }

    #[test]
    fn test_files_in_manifest_not_present() {
        IntegrityTest::new()
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A).file(&SHARE_A))
            .file(&BIN_A)
            .assert_errors(errors![
                IntegrityError::MissingFile { path } if path == Path::new("share/a"),
            ]);
    }

    #[test]
    fn test_untrusted_manifest() {
        // This key is not trusted by the keychain created by IntegrityTest.
        let key = EphemeralKeyPair::generate(
            crate::keys::KeyAlgorithm::EcdsaP256Sha256Asn1SpkiDer,
            KeyRole::Packages,
            None,
        )
        .unwrap();

        IntegrityTest::new()
            .file(&TestFile {
                path: "share/criticaltrust/a/b.json".into(),
                mode: 0o644,
                contents: ManifestBuilder::new("a", "b")
                    .file(&BIN_A)
                    .finish(&key)
                    .into(),
                needs_proxy: false,
            })
            .assert_errors(errors![
                IntegrityError::PackageManifestVerification {
                    path,
                    inner: Error::VerificationFailed,
                } if path == Path::new("share/criticaltrust/a/b.json"),
                // No valid one was found:
                IntegrityError::NoPackageManifestFound,
            ]);
    }

    #[test]
    fn test_invalid_json_in_manifest() {
        IntegrityTest::new()
            .file(&TestFile::new(
                "share/criticaltrust/a/b.json",
                0o644,
                b"{not valid json}",
            ))
            .assert_errors(errors![
                IntegrityError::PackageManifestDeserialization { path, inner }
                     if path == Path::new("share/criticaltrust/a/b.json") && inner.is_syntax(),
                // No valid one was found:
                IntegrityError::NoPackageManifestFound,
            ]);
    }

    #[test]
    fn test_unprefixed_manifest_with_prefixed_files() {
        IntegrityTest::new()
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A).file(&SHARE_A))
            .file(&BIN_A.clone().prefix("foo/"))
            .file(&SHARE_A.clone().prefix("foo/"))
            .assert_errors(errors![
                IntegrityError::MissingFile { path } if path == Path::new("bin/a"),
                IntegrityError::MissingFile { path } if path == Path::new("share/a"),
                IntegrityError::UnexpectedFile { path } if path == Path::new("foo/bin/a"),
                IntegrityError::UnexpectedFile { path } if path == Path::new("foo/share/a"),
            ]);
    }

    #[test]
    fn test_prefixed_manifest_with_unprefixed_files() {
        IntegrityTest::new()
            .manifest(
                ManifestBuilder::new("a", "b")
                    .file(&BIN_A)
                    .file(&SHARE_A)
                    .prefix("foo/"),
            )
            .file(&BIN_A)
            .file(&SHARE_A)
            .assert_errors(errors![
                IntegrityError::MissingFile { path } if path == Path::new("foo/bin/a"),
                IntegrityError::MissingFile { path } if path == Path::new("foo/share/a"),
                IntegrityError::UnexpectedFile { path } if path == Path::new("bin/a"),
                IntegrityError::UnexpectedFile { path } if path == Path::new("share/a"),
            ]);
    }

    #[test]
    fn test_file_loaded_multiple_times() {
        IntegrityTest::new()
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A))
            .file(&BIN_A)
            .file(&BIN_A)
            .assert_errors(errors![
                IntegrityError::FileLoadedMultipleTimes { path } if path == Path::new("bin/a"),
            ]);
    }

    #[test]
    fn test_manifest_loaded_multiple_times() {
        IntegrityTest::new()
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A))
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A))
            .file(&BIN_A)
            .assert_errors(errors![
                IntegrityError::FileLoadedMultipleTimes { path }
                    if path == Path::new("share/criticaltrust/a/b.json")
            ]);
    }

    #[test]
    fn test_collecting_needs_proxy_binaries() {
        IntegrityTest::new()
            .manifest(
                ManifestBuilder::new("a", "b")
                    .file(&BIN_A.clone().needs_proxy())
                    .file(&BIN_B),
            )
            .file(&BIN_A)
            .file(&BIN_B)
            .assert_verified(&[VerifiedPackage {
                product: "a".into(),
                package: "b".into(),
                proxies_paths: btreemap! {"a" => "bin/a"},
            }]);
    }

    #[test]
    fn test_collecting_needs_proxy_binaries_inside_a_prefix() {
        IntegrityTest::new()
            .manifest(
                ManifestBuilder::new("a", "b")
                    .file(&BIN_A.clone().needs_proxy())
                    .file(&BIN_B)
                    .prefix("foo/"),
            )
            .file(&BIN_A.clone().prefix("foo/"))
            .file(&BIN_B.clone().prefix("foo/"))
            .assert_verified(&[VerifiedPackage {
                product: "a".into(),
                package: "b".into(),
                proxies_paths: btreemap! {"a" => "foo/bin/a"},
            }]);
    }

    #[test]
    fn test_allowing_external_files() {
        IntegrityTest::new()
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A))
            .file(&BIN_A)
            .file(&BIN_B)
            .allow_external_files()
            .assert_verified(&[("a", "b")]);
    }

    #[test]
    fn test_allowing_external_files_in_managed_prefixes() {
        IntegrityTest::new()
            .manifest(
                ManifestBuilder::new("a", "b")
                    .file(&BIN_A)
                    .managed_prefix("bin/"),
            )
            .file(&BIN_A)
            .file(&BIN_B)
            .allow_external_files()
            .assert_errors(errors![
                IntegrityError::UnexpectedFileInManagedPrefix { path, prefix }
                    if path == Path::new("bin/b") && prefix == Path::new("bin/")
            ]);
    }

    #[test]
    fn test_allowing_external_files_in_managed_prefixes_inside_a_prefix() {
        IntegrityTest::new()
            .manifest(
                ManifestBuilder::new("a", "b")
                    .file(&BIN_A)
                    .managed_prefix("bin/")
                    .prefix("foo/"),
            )
            .file(&BIN_A.clone().prefix("foo/"))
            .file(&BIN_B.clone().prefix("foo/"))
            .file(&BIN_A)
            .allow_external_files()
            .assert_errors(errors![
                IntegrityError::UnexpectedFileInManagedPrefix { path, prefix }
                    if path == Path::new("foo/bin/b") && prefix == Path::new("foo/bin/")
            ]);
    }

    #[test]
    fn test_allowing_external_files_inside_a_prefix() {
        IntegrityTest::new()
            .manifest(ManifestBuilder::new("a", "b").file(&BIN_A).prefix("foo/"))
            .file(&BIN_A.clone().prefix("foo/"))
            .file(&BIN_B.clone().prefix("foo/"))
            .file(&BIN_A)
            .allow_external_files()
            .assert_verified(&[("a", "b")]);
    }

    #[derive(Clone)]
    struct TestFile {
        path: PathBuf,
        mode: u32,
        contents: Cow<'static, [u8]>,
        needs_proxy: bool,
    }

    impl TestFile {
        fn new(path: impl AsRef<OsStr>, mode: u32, contents: &'static [u8]) -> Self {
            Self {
                path: path.as_ref().into(),
                mode,
                contents: Cow::Borrowed(contents),
                needs_proxy: false,
            }
        }

        fn prefix(mut self, prefix: impl AsRef<OsStr>) -> Self {
            let path = self.path.to_owned();
            self.path = PathBuf::from(prefix.as_ref()).join(path);
            self
        }

        #[cfg_attr(windows, allow(dead_code))]
        fn mode(mut self, new: u32) -> Self {
            self.mode = new;
            self
        }

        fn needs_proxy(mut self) -> Self {
            self.needs_proxy = true;
            self
        }

        fn add_content(mut self, extra: &[u8]) -> Self {
            let mut contents = self.contents.into_owned();
            contents.extend_from_slice(extra);
            self.contents = Cow::Owned(contents);
            self
        }
    }

    type ErrorMatcher = fn(&IntegrityError) -> bool;

    struct ManifestBuilder {
        manifest: Package,
        prefix: PathBuf,
    }

    impl ManifestBuilder {
        fn new(product: &str, package: &str) -> Self {
            Self {
                manifest: Package {
                    product: product.into(),
                    package: package.into(),
                    commit: String::new(),
                    files: Vec::new(),
                    managed_prefixes: Vec::new(),
                },
                prefix: Default::default(),
            }
        }

        fn managed_prefix(mut self, prefix: &str) -> Self {
            self.manifest.managed_prefixes.push(prefix.into());
            self
        }

        fn file(mut self, file: &TestFile) -> Self {
            self.manifest.files.push(PackageFile {
                path: file.path.clone(),
                posix_mode: file.mode,
                sha256: hash_sha256(&file.contents),
                needs_proxy: file.needs_proxy,
            });
            self
        }

        fn prefix(mut self, prefix: impl AsRef<OsStr>) -> Self {
            self.prefix = prefix.as_ref().into();
            self
        }

        fn finish(self, key: &EphemeralKeyPair) -> Vec<u8> {
            let mut signed = SignedPayload::new(&self.manifest).unwrap();
            signed.add_signature(key).unwrap();

            serde_json::to_vec(&PackageManifest {
                version: ManifestVersion,
                signed,
            })
            .unwrap()
        }
    }

    struct IntegrityTest {
        env: TestEnvironment,
        key: EphemeralKeyPair,
        allow_external_files: bool,
        files: Vec<TestFile>,
    }

    impl IntegrityTest {
        fn new() -> Self {
            let mut env = TestEnvironment::prepare();
            let key = env.create_key(KeyRole::Packages);
            Self {
                env,
                key,
                allow_external_files: false,
                files: Vec::new(),
            }
        }

        fn allow_external_files(mut self) -> Self {
            self.allow_external_files = true;
            self
        }

        fn file(mut self, file: &TestFile) -> Self {
            self.files.push(file.clone());
            self
        }

        fn manifest(self, builder: ManifestBuilder) -> Self {
            self.manifest_in(
                builder.prefix.join(format!(
                    "share/criticaltrust/{}/{}.json",
                    builder.manifest.product, builder.manifest.package
                )),
                builder,
            )
        }

        fn manifest_in(mut self, path: impl AsRef<OsStr>, builder: ManifestBuilder) -> Self {
            self.files.push(TestFile {
                path: path.as_ref().into(),
                mode: 0o644,
                contents: builder.finish(&self.key).into(),
                needs_proxy: false,
            });
            self
        }

        #[track_caller]
        fn assert_verified(self, found: &[impl AsVerifiedPackage]) {
            let mut expected = found
                .iter()
                .map(|vp| vp.as_verified_package())
                .collect::<Vec<_>>();
            expected.sort();

            self.permutations(|result| {
                let mut result = result.unwrap();
                result.sort();

                assert_eq!(result, expected);
            })
        }

        #[track_caller]
        fn assert_errors(self, matchers: &[(&str, ErrorMatcher)]) {
            self.permutations(|result| {
                let mut matchers = matchers.iter().map(Some).collect::<Vec<_>>();

                let result = result.unwrap_err();
                for error in &result {
                    let mut matched = false;
                    for matcher in &mut matchers {
                        if let Some((_, m)) = matcher {
                            if m(error) {
                                matched = true;
                                *matcher = None;
                                break;
                            }
                        }
                    }
                    if !matched {
                        panic!(
                            "\n\nreturned errors: {result:?}\n\
                        -> found error not matching any pattern: {error:?}\n"
                        );
                    }
                }

                let unmatched = matchers
                    .into_iter()
                    .flatten()
                    .map(|m| m.0)
                    .collect::<Vec<_>>();
                if !unmatched.is_empty() {
                    panic!(
                        "\n\nreturned errors: {result:?}\n\
                    -> some matchers were not matched: {unmatched:?}\n"
                    );
                }
            })
        }

        #[track_caller]
        fn permutations(self, f: impl Fn(Result<Vec<VerifiedPackage>, Vec<IntegrityError>>)) {
            self.files
                .iter()
                .permutations(self.files.len())
                .for_each(|files| {
                    println!(
                        "current permutation: {:?}",
                        files.iter().map(|f| &f.path).collect::<Vec<_>>()
                    );

                    let mut verifier = IntegrityVerifier::new(self.env.keychain());
                    verifier.allow_external_files(self.allow_external_files);
                    for file in files {
                        verifier.add(&file.path, file.mode, &file.contents);
                    }
                    f(verifier.verify());
                })
        }
    }

    trait AsVerifiedPackage {
        fn as_verified_package(&self) -> VerifiedPackage;
    }

    impl AsVerifiedPackage for (&str, &str) {
        fn as_verified_package(&self) -> VerifiedPackage {
            VerifiedPackage {
                product: self.0.into(),
                package: self.1.into(),
                proxies_paths: BTreeMap::new(),
            }
        }
    }

    impl AsVerifiedPackage for VerifiedPackage {
        fn as_verified_package(&self) -> VerifiedPackage {
            self.clone()
        }
    }
}
