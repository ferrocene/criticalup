// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

mod substitutions;
mod v1;

use crate::errors::Error::FailedToFindCanonicalPath;
use crate::errors::ProjectManifestLoadingError::MultipleProductsNotSupportedInProjectManifest;
use crate::errors::{Error, ProjectManifestLoadingError};
use crate::project_manifest::substitutions::apply_substitutions;
use crate::utils::Sha256Hasher;
use serde::{Deserialize, Serialize};
use std::env;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

const DEFAULT_PROJECT_MANIFEST_NAME: &str = "criticalup.toml";
const DEFAULT_PROJECT_MANIFEST_VERSION: u32 = 1;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ProjectManifest {
    products: Vec<ProjectManifestProduct>,
}

impl ProjectManifest {
    /// Try to find the criticalup.toml project manifest in parent directories.
    pub fn discover(base: &Path) -> Result<PathBuf, Error> {
        let mut search = Some(base);
        while let Some(path) = search.take() {
            search = path.parent();

            let candidate = path.join(DEFAULT_PROJECT_MANIFEST_NAME);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }

        Err(Error::ProjectManifestDetectionFailed)
    }

    /// Find the absolute path to the manifest.
    ///
    /// The path, which is optionally provided by the user could be relative, but we need the
    /// absolute path for state file.
    ///
    /// If the project path is provided then it could be a relative path. In that case, find the
    /// full path to the criticalup.toml.
    ///
    /// If the path is not provided then tries to find the manifest iterating over parent
    /// directories looking for one, and stopping at the closest parent directory with the file.
    pub fn discover_canonical_path(project_path: Option<&Path>) -> Result<PathBuf, Error> {
        match project_path {
            Some(path) => {
                Ok(
                    std::fs::canonicalize(path).map_err(|err| FailedToFindCanonicalPath {
                        path: path.to_path_buf(),
                        kind: err,
                    })?,
                )
            }
            None => {
                let curr_directory = env::current_dir().map_err(Error::FailedToReadDirectory)?;
                let path = ProjectManifest::discover(&curr_directory)?;
                Ok(std::fs::canonicalize(&path)
                    .map_err(|err| FailedToFindCanonicalPath { path, kind: err })?)
            }
        }
    }

    /// Try to parse and return the `ProjectManifest` object.
    pub fn load(path: &Path) -> Result<Self, Error> {
        load_inner(path).map_err(|kind| Error::ProjectManifestLoadingFailed {
            path: path.into(),
            kind: Box::new(kind),
        })
    }

    /// Find the project manifest and parse it.
    ///
    /// This function tries to load the manifest for a given path. If the path is not provided
    /// then tries to find the manifest iterating over parent directories looking for one, and
    /// stopping at the closest parent directory with the file.
    ///
    /// This is a combination of existing functions `Self::load()` and `Self::discover()` for ease
    /// of use.
    pub fn get(project_path: Option<PathBuf>) -> Result<Self, Error> {
        let manifest = match project_path {
            Some(manifest_path) => ProjectManifest::load(&manifest_path)?,
            None => {
                let discovered_manifest = Self::discover_canonical_path(None)?;
                Self::load(discovered_manifest.as_path())?
            }
        };
        Ok(manifest)
    }

    pub fn products(&self) -> &[ProjectManifestProduct] {
        &self.products
    }

    /// Generates a directory for each product under the specified `root`.
    ///
    /// If the directory already exists, then just skips the creation.
    pub fn create_products_dirs(&self, installation_dir: &Path) -> std::io::Result<()> {
        let products = self.products();
        for product in products {
            std::fs::create_dir_all(installation_dir.join(product.installation_id()))?;
        }

        Ok(())
    }
}

/// Keeping packages sorted requires this wrapper newtype pattern.
///
/// Deref and DerefMut are implemented for this type to keep things as smooth as possible
/// with the least amount of breaking changes, if any.
#[derive(Debug, PartialEq, Eq)]
struct Packages(Vec<String>);

impl Hash for Packages {
    /// Packages hash to be done only on sorted packages.
    ///
    /// In-place sorting within a method is not advisable as it is not explicit and can fall
    /// through the cracks.
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut this = self.0.clone();
        this.sort();
        this.hash(state)
    }
}

impl Deref for Packages {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl DerefMut for Packages {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut()
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ProjectManifestProduct {
    name: String,
    release: String,
    packages: Packages,
}

impl ProjectManifestProduct {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn release(&self) -> &str {
        &self.release
    }

    pub fn packages(&self) -> &[String] {
        &self.packages
    }

    pub fn installation_id(&self) -> InstallationId {
        // For now this generates the ID using hash of the product object.
        let mut hasher = Sha256Hasher::new();
        self.hash(&mut hasher);
        InstallationId(hasher.finalize())
    }

    /// Generates a directory for the product under the specified `root`. If the directory already
    /// exists, then just skips the creation.
    pub fn create_product_dir(&self, installation_dir: &Path) -> Result<(), Error> {
        let product_dir_name = self.installation_id();
        let abs_installation_dir_path: PathBuf = [installation_dir, product_dir_name.as_ref()]
            .iter()
            .collect();
        let _res: Result<(), std::io::Error> =
            match std::fs::create_dir_all(abs_installation_dir_path.clone()) {
                Ok(_) => Ok(()),
                Err(err) => {
                    return Err(Error::ProjectManifestProductDirCreationFailed {
                        path: abs_installation_dir_path,
                        product: self.name.clone(),
                        source: err,
                    })
                }
            };
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstallationId(pub String);

impl AsRef<Path> for InstallationId {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl Deref for InstallationId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct VersionDetector {
    manifest_version: u32,
}

fn load_inner(path: &Path) -> Result<ProjectManifest, ProjectManifestLoadingError> {
    let mut products = Vec::new();

    let contents = std::fs::read(path).map_err(ProjectManifestLoadingError::FailedToRead)?;

    // We first deserialize only the `manifest_version` field, which must be present in all
    // past and future versions, and then based on the version we properly deserialize.
    let version: VersionDetector =
        toml_edit::de::from_slice(&contents).map_err(ProjectManifestLoadingError::FailedToParse)?;
    match version.manifest_version {
        DEFAULT_PROJECT_MANIFEST_VERSION => {
            let manifest: v1::ProjectManifest = toml_edit::de::from_slice(&contents)
                .map_err(ProjectManifestLoadingError::FailedToParse)?;

            for (name, product) in manifest.products.into_iter() {
                let mut packages = Packages(
                    product
                        .packages
                        .iter()
                        .map(|p| apply_substitutions(p))
                        .collect::<Result<Vec<_>, ProjectManifestLoadingError>>()?,
                );
                packages.sort();

                products.push(ProjectManifestProduct {
                    name,
                    release: apply_substitutions(&product.release)?,
                    packages,
                });
            }
        }
        small if small < DEFAULT_PROJECT_MANIFEST_VERSION => {
            return Err(ProjectManifestLoadingError::ManifestVersionTooSmall {
                user_version: small,
                default_supported_version: DEFAULT_PROJECT_MANIFEST_VERSION,
            })
        }
        large => {
            return Err(ProjectManifestLoadingError::ManifestVersionTooBig {
                user_version: large,
            })
        }
    }

    products.sort_by(|a, b| a.name.cmp(&b.name));

    if products.len() > 1 {
        return Err(MultipleProductsNotSupportedInProjectManifest(
            products.len(),
        ));
    }

    Ok(ProjectManifest { products })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_sample_manifest(dir: &Path) {
        const SAMPLE: &str = "\
            manifest-version = 1\n\
            \n\
            [products.sample]\n\
            release = \"foo\"\n\
            packages = [\"bar\"]\n\
        ";

        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("criticalup.toml"), SAMPLE.as_bytes()).unwrap();
    }

    mod test_discover {
        use super::*;
        use std::env::set_current_dir;

        #[test]
        fn test_current_directory() {
            let root = tempfile::tempdir().unwrap();
            write_sample_manifest(root.path());
            let discovered_manifest_path = ProjectManifest::discover(root.path()).unwrap();
            assert_sample_parsed(
                ProjectManifest::load(discovered_manifest_path.as_path()).unwrap(),
            );
        }

        #[test]
        fn test_parent_directory() {
            let root = tempfile::tempdir().unwrap();
            write_sample_manifest(root.path());
            let discovered_manifest_path =
                ProjectManifest::discover(&root.path().join("child")).unwrap();
            assert_sample_parsed(
                ProjectManifest::load(discovered_manifest_path.as_path()).unwrap(),
            );
        }

        #[test]
        fn test_two_parent_directories() {
            let root = tempfile::tempdir().unwrap();
            write_sample_manifest(root.path());
            let discovered_manifest_path =
                ProjectManifest::discover(&root.path().join("child").join("grandchild")).unwrap();
            assert_sample_parsed(
                ProjectManifest::load(discovered_manifest_path.as_path()).unwrap(),
            );
        }

        #[test]
        fn test_child_directory() {
            let root = tempfile::tempdir().unwrap();
            write_sample_manifest(&root.path().join("child"));

            assert!(matches!(
                ProjectManifest::discover(root.path()).unwrap_err(),
                Error::ProjectManifestDetectionFailed
            ));
        }

        #[test]
        #[ignore = "Testing manifest discovery while setting current directory will be enabled at a later date."]
        fn discover_canonical_path_matches_current_manifest_canonical_path() {
            let root = tempfile::tempdir().unwrap();
            let expected_project_path = root.path().join("project").join("awesome");
            write_sample_manifest(&expected_project_path);

            // We move into the directory to simulate being in the project directory.
            set_current_dir(&expected_project_path).unwrap();

            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let discovered_abs_path = ProjectManifest::discover_canonical_path(None).unwrap();
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let expected_project_path =
                std::fs::canonicalize(expected_project_path.join("criticalup.toml")).unwrap();

            #[cfg(target_os = "macos")]
            let discovered_abs_path = ProjectManifest::discover_canonical_path(None)
                .unwrap()
                .strip_prefix("/private")
                .unwrap()
                .to_path_buf();
            #[cfg(target_os = "macos")]
            let expected_project_path = expected_project_path
                .join("criticalup.toml")
                .strip_prefix("/")
                .unwrap()
                .to_path_buf();

            #[cfg(target_os = "windows")]
            let discovered_abs_path = ProjectManifest::discover_canonical_path(None).unwrap();
            // We need to canonicalize this side as well because Windows canonical paths
            // add an extra oomph as prefix \\?\.
            // https://learn.microsoft.com/en-us/dotnet/standard/io/file-path-formats#unc-paths
            #[cfg(target_os = "windows")]
            let expected_project_path =
                std::fs::canonicalize(expected_project_path.join("criticalup.toml")).unwrap();

            assert_eq!(discovered_abs_path, expected_project_path);
        }

        #[test]
        fn test_two_child_directories() {
            let root = tempfile::tempdir().unwrap();
            write_sample_manifest(&root.path().join("child").join("grandchild"));

            assert!(matches!(
                ProjectManifest::discover(root.path()).unwrap_err(),
                Error::ProjectManifestDetectionFailed
            ));
        }

        #[test]
        fn test_no_file() {
            assert!(matches!(
                ProjectManifest::discover(tempfile::tempdir().unwrap().path()).unwrap_err(),
                Error::ProjectManifestDetectionFailed
            ));
        }

        #[track_caller]
        fn assert_sample_parsed(manifest: ProjectManifest) {
            assert_eq!(
                ProjectManifest {
                    products: vec![ProjectManifestProduct {
                        name: "sample".into(),
                        release: "foo".into(),
                        packages: Packages(vec!["bar".into()]),
                    }]
                },
                manifest
            );
        }
    }

    mod test_load {
        use super::*;
        use std::env::set_current_dir;

        #[test]
        fn test_read_failure() {
            let root = tempfile::tempdir().unwrap();
            let bad_path = root.path().join("doesnt-exist.toml");
            let actual = ProjectManifest::load(&bad_path).unwrap_err();

            if let Error::ProjectManifestLoadingFailed { path, kind } = actual {
                // A simple function in the spirit of most other closures here for other tests.
                let error_check = |e: Box<ProjectManifestLoadingError>| {
                    matches!(*e, ProjectManifestLoadingError::FailedToRead(_))
                };
                assert_eq!(bad_path, path);
                assert!(error_check(kind));
            }
        }

        #[test]
        fn test_invalid_toml() {
            assert_load_error("\0", |e| {
                matches!(e, ProjectManifestLoadingError::FailedToParse(_))
            });
        }

        #[test]
        fn test_missing_manifest_version() {
            assert_load_error("foo = 1", |e| {
                matches!(e, ProjectManifestLoadingError::FailedToParse(_))
            });
        }

        #[test]
        fn test_unsupported_manifest_version() {
            assert_load_error("manifest-version = 0", |e| {
                matches!(
                    e,
                    ProjectManifestLoadingError::ManifestVersionTooSmall {
                        user_version: 0,
                        default_supported_version: DEFAULT_PROJECT_MANIFEST_VERSION,
                    }
                )
            });

            assert_load_error("manifest-version = 2", |e| {
                matches!(
                    e,
                    ProjectManifestLoadingError::ManifestVersionTooBig { user_version: 2 }
                )
            });
        }

        #[test]
        fn test_v1_empty() {
            assert_load(
                "manifest-version = 1",
                ProjectManifest {
                    products: Vec::new(),
                },
            );
        }

        #[test]
        fn test_v1_one_product() {
            assert_load(
                r#"
                    manifest-version = 1

                    [products.sample]
                    release = "foo"
                    packages = ["bar", "baz"]
                "#,
                ProjectManifest {
                    products: vec![ProjectManifestProduct {
                        name: "sample".into(),
                        release: "foo".into(),
                        packages: Packages(vec!["bar".into(), "baz".into()]),
                    }],
                },
            );
        }

        #[test]
        #[ignore = "Temporarily disabled until support for multiple products is enabled."]
        fn test_v1_multiple_products() {
            // This also tests whether sorting works.
            assert_load(
                r#"
                    manifest-version = 1

                    [products.sample]
                    release = "foo"
                    packages = ["bar", "baz"]

                    [products.demo]
                    release = "@foo/latest"
                    packages = ["b", "a"]
                "#,
                ProjectManifest {
                    products: vec![
                        ProjectManifestProduct {
                            name: "demo".into(),
                            release: "@foo/latest".into(),
                            packages: Packages(vec!["a".into(), "b".into()]),
                        },
                        ProjectManifestProduct {
                            name: "sample".into(),
                            release: "foo".into(),
                            packages: Packages(vec!["bar".into(), "baz".into()]),
                        },
                    ],
                },
            );
        }

        #[test]
        fn test_v1_multiple_products_not_supported() {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("criticalup.toml");
            let contents = r#"
                    manifest-version = 1

                    [products.sample]
                    release = "foo"
                    packages = ["bar", "baz"]

                    [products.demo]
                    release = "@foo/latest"
                    packages = ["b", "a"]
                "#;

            std::fs::write(path, contents.as_bytes()).unwrap();
            assert_load_error(contents, |err| {
                matches!(err, MultipleProductsNotSupportedInProjectManifest(2))
            });
        }

        #[test]
        fn test_v1_substitutions() {
            assert_load(
                r#"
                    manifest-version = 1

                    [products.sample]
                    release = "${rustc-host}"
                    packages = ["foo-${rustc-host}"]
                "#,
                ProjectManifest {
                    products: vec![ProjectManifestProduct {
                        name: "sample".into(),
                        release: env!("TARGET").into(),
                        packages: Packages(vec![concat!("foo-", env!("TARGET")).into()]),
                    }],
                },
            );
        }

        #[test]
        fn test_v1_missing_required_fields() {
            assert_load_error(
                r#"
                    manifest-version = 1

                    [products.sample]
                    release = "foo"
                "#,
                |e| matches!(e, ProjectManifestLoadingError::FailedToParse(_)),
            );
        }

        #[test]
        fn test_v1_extra_unknown_fields() {
            assert_load_error(
                r#"
                    manifest-version = 1
                    foo = 1

                    [products.sample]
                    release = "foo"
                    packages = ["bar"]
                "#,
                |e| matches!(e, ProjectManifestLoadingError::FailedToParse(_)),
            );
            assert_load_error(
                r#"
                    manifest-version = 1

                    [products.sample]
                    release = "foo"
                    packages = ["bar"]
                    foo = 1
                "#,
                |e| matches!(e, ProjectManifestLoadingError::FailedToParse(_)),
            );
        }

        #[test]
        fn test_v1_invalid_substitutions() {
            assert_load_error(
                r#"
                    manifest-version = 1

                    [products.sample]
                    release = "foo"
                    packages = ["${rustc-host"]
                "#,
                |e| {
                    matches!(
                        e,
                        ProjectManifestLoadingError::UnterminatedVariableInSubstitution
                    )
                },
            );
        }

        #[track_caller]
        fn assert_load(contents: &str, expected: ProjectManifest) {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("criticalup.toml");

            std::fs::write(&path, contents.as_bytes()).unwrap();
            assert_eq!(expected, ProjectManifest::load(&path).unwrap());
        }

        #[track_caller]
        fn assert_load_error(
            contents: &str,
            error_check: impl FnOnce(&ProjectManifestLoadingError) -> bool,
        ) {
            let root = tempfile::tempdir().unwrap();
            let bad_path = root.path().join("criticalup.toml");

            std::fs::write(&bad_path, contents.as_bytes()).unwrap();

            let mut supported_versions: Vec<u32> = [1, 3, 5].into();
            supported_versions.sort();

            let err = ProjectManifest::load(&bad_path).unwrap_err();
            if let Error::ProjectManifestLoadingFailed { path, kind } = &err {
                assert_eq!(&bad_path, path);
                assert!(error_check(kind));
            }
        }

        #[test]
        #[ignore = "Testing manifest discovery while setting current directory will be enabled at a later date."]
        fn get_loaded_manifest_by_discovering() {
            let root = tempfile::tempdir().unwrap();
            let awesome_project_path = root.path().join("project").join("awesome");
            write_sample_manifest(&awesome_project_path);

            set_current_dir(&awesome_project_path).unwrap();
            let discovered_manifest = ProjectManifest::get(None).unwrap();
            let direct_loaded_manifest =
                ProjectManifest::load(awesome_project_path.join("criticalup.toml").as_path())
                    .unwrap();
            assert_eq!(discovered_manifest, direct_loaded_manifest);
        }
    }

    mod test_product {
        use crate::project_manifest::{InstallationId, Packages, ProjectManifestProduct};

        #[test]
        fn test_installation_id_generation() {
            let product1 = ProjectManifestProduct {
                name: "dir_name_tester".to_string(),
                release: "1.523231341324".to_string(),
                packages: Packages(vec![]),
            };
            assert_eq!(
                InstallationId(
                    "88ae6c4f87f8b450cef620983f00ac440a0b2dd6c2b7a1f04185b917d7a51c84".into()
                ),
                product1.installation_id(),
            );

            let product2 = ProjectManifestProduct {
                name: "dir_name_tester".to_string(),
                release: "1.523231341324".to_string(),
                packages: Packages(vec!["package 2".to_string(), "package 1".to_string()]),
            };
            assert_eq!(
                InstallationId(
                    "b1eb7dd657b436a540549b2f2adf0cfcdef50233487de50c404ac1510e9d0868".into()
                ),
                product2.installation_id(),
            );
        }

        #[test]
        fn test_create_success() {
            let root = tempfile::tempdir().unwrap();
            let installation_dir = root.path().join("toolchains");
            let product1 = ProjectManifestProduct {
                name: "product1".into(),
                release: "@foo/latest".into(),
                packages: Packages(vec!["b".into(), "a".into()]),
            };

            let product1_id = product1.installation_id();

            let product2 = ProjectManifestProduct {
                name: "product2".into(),
                release: "foo".into(),
                packages: Packages(vec!["bar".into(), "baz".into()]),
            };

            let product2_id = product2.installation_id();

            let test_manifest = crate::project_manifest::ProjectManifest {
                products: vec![product1, product2],
            };

            // Main project dir is created along with product dirs.
            let _ = test_manifest.create_products_dirs(&installation_dir);
            assert!(installation_dir.exists());

            // A dir per product within the project dir.
            assert!(installation_dir.join(product1_id).exists());
            assert!(installation_dir.join(product2_id).exists());
            assert!(!installation_dir
                .join("NEVERGONNAGIVEYOUUPNEVERGONNALETYOUDOWN")
                .exists());
        }
    }
}
