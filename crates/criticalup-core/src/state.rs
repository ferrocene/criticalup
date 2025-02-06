// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::{Ref, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::env::VarError;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use criticaltrust::integrity::VerifiedPackage;
use tokio::io::AsyncWriteExt;

use crate::config::Config;
use crate::errors::Error::InstallationDoesNotExist;
use crate::errors::WriteFileError;
use crate::errors::{self, Error};
use crate::project_manifest::InstallationId;
use crate::utils::open_file_for_write;

const CURRENT_FORMAT_VERSION: u32 = 1;
const CRITICALUP_TOKEN_ENV_VAR_NAME: &str = "CRITICALUP_TOKEN";

#[derive(Clone)]
pub struct State {
    inner: Rc<RefCell<StateInner>>,
}

impl State {
    /// Construct the `State` object by loading the content from state file from disk.
    pub async fn load(config: &Config) -> Result<Self, Error> {
        let path = config.paths.state_file.clone();

        let repr = match tokio::fs::read(&path).await {
            Ok(contents) => serde_json::from_slice(&contents)
                .map_err(|e| Error::CorruptStateFile(path.clone(), e))?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => StateRepr::default(),
            Err(err) => return Err(Error::CantReadStateFile(path, err)),
        };

        if repr.version != CURRENT_FORMAT_VERSION {
            return Err(Error::UnsupportedStateFileVersion(path, repr.version));
        }

        Ok(State {
            inner: Rc::new(RefCell::new(StateInner { path, repr })),
        })
    }

    /// Returns the authentication token.
    ///
    /// Attempts to read from:
    ///  1. `token_path` (if present)
    ///  2. The `CRITICALUP_TOKEN_ENV_VAR_NAME` environment
    ///  3. The state
    ///     1. check if token_path was sent in the fn call
    ///     2. if not, then try to see if the env var is set
    ///     3. if that was not set then look at the State
    ///     4. else, None
    pub async fn authentication_token(
        &self,
        token_path: Option<&str>,
        env_vars: &EnvVars,
    ) -> Option<AuthenticationToken> {
        match token_path {
            Some(token_path) => {
                let token_path = std::path::Path::new(token_path);
                if token_path.exists() {
                    match tokio::fs::read_to_string(token_path).await {
                        Ok(token) => Some(AuthenticationToken(token.to_string().trim().into())),
                        Err(_) => self.authentication_token_inner(env_vars),
                    }
                } else {
                    self.authentication_token_inner(env_vars)
                }
            }
            None => self.authentication_token_inner(env_vars),
        }
    }

    /// Returns the authentication token.
    ///
    /// Attempts to read from:
    ///  1. The `CRITICALUP_TOKEN_ENV_VAR_NAME` environment
    ///  2. The state
    fn authentication_token_inner(&self, env_vars: &EnvVars) -> Option<AuthenticationToken> {
        match &env_vars.criticalup_token {
            Some(token) => Some(AuthenticationToken(token.to_string())),
            None => {
                let borrowed = self.inner.borrow();
                borrowed.repr.authentication_token.clone()
            }
        }
    }

    pub fn set_authentication_token(&self, token: Option<AuthenticationToken>) {
        self.inner.borrow_mut().repr.authentication_token = token;
    }

    /// Adds or selectively installation in the State for a given `InstallationId`,
    /// a given Manifest path and verified packages.
    ///
    /// Creates or overrides installation for a given unique `InstallationId`. If you merely want
    /// to update/append more manifest paths then use `Self::update_installation_manifests` method.
    ///
    /// Also, removes the manifest path from older installations.
    ///
    /// We need to check the following to make a decision on what to do with the installation
    /// within the State and also what to do with the manifests within those installations:
    ///     - State file
    ///     - Installation directory
    ///     - Manifest path(s)
    ///
    /// The following table will help figure out the match pattern below.
    ///
    /// +===========+===========+===========================================================+
    /// | In State? | On Disk?  |                 Result                                    |
    /// +===========+===========+===========================================================+
    /// | true      | true      | Update existing installation                              |
    /// +-----------+-----------+-----------------------------------------------------------+
    /// | false     | _         | Create new installation                                   |
    /// +-----------+-----------+-----------------------------------------------------------+
    /// | true      | false     | Remove the older installations from the State, create new |
    /// +-----------+-----------+-----------------------------------------------------------+
    pub fn add_installation(
        &self,
        installation_id: &InstallationId,
        packages: &[VerifiedPackage],
        manifest: &Path,
        config: &Config,
    ) -> Result<(), Error> {
        // Get the canonical path so all platforms are consistent.
        let manifest = canonicalize_or_err(manifest)?;
        let mut inner = self.inner.borrow_mut();
        let existing_installation_in_state_exists =
            inner.repr.installations.contains_key(installation_id);
        let installation_path_on_disk_exists = config
            .paths
            .installation_dir
            .join(&installation_id.0)
            .exists();
        match (
            existing_installation_in_state_exists,
            installation_path_on_disk_exists,
        ) {
            (true, true) => {
                inner.update_installation_manifests(installation_id, &manifest)?;
            }

            (false, _) => {
                inner.remove_manifest_from_all_installations(&manifest);

                // Create the new installation for provided manifest.
                let manifests = BTreeSet::from([manifest]);
                inner.repr.installations.insert(
                    installation_id.clone(),
                    StateInstallation {
                        manifests,
                        binary_proxies: packages
                            .iter()
                            .flat_map(|package| package.proxies_paths.iter())
                            .map(|(k, v)| (k.clone(), v.into()))
                            .collect(),
                    },
                );
            }
            (true, false) => {
                eprintln!(
                    "Installation in the State exists but the installation directory is missing."
                );
                inner.repr.installations.remove(installation_id);
            }
        }
        Ok(())
    }

    /// Updates an existing installation using `InstallationId` by appending manifest path for
    /// a new project using a manifest that has an existing installation.
    ///
    /// Also, removes the manifest path from older installations.
    pub fn update_installation_manifests(
        &self,
        installation_id: &InstallationId,
        manifest_path: &Path,
    ) -> Result<(), Error> {
        // Get the canonical path so all platforms are consistent.
        let manifest = canonicalize_or_err(manifest_path)?;
        let mut inner = self.inner.borrow_mut();
        inner.update_installation_manifests(installation_id, &manifest)
    }

    /// Removes a manifest path from all installations and returns the list of `InstallationId`s
    /// that had the said manifest.
    pub fn remove_manifest_from_all_installations(
        &self,
        manifest_path: &Path,
    ) -> Result<Vec<InstallationId>, Error> {
        // Get the canonical path so all platforms are consistent.
        let manifest = canonicalize_or_err(manifest_path)?;
        let mut inner = self.inner.borrow_mut();
        Ok(inner.remove_manifest_from_all_installations(&manifest))
    }

    /// Remove an installation from the `State` for a given `InstallationId`.
    pub fn remove_installation(&self, installation_id: &InstallationId) {
        self.inner
            .borrow_mut()
            .repr
            .installations
            .remove(installation_id);
    }

    pub fn resolve_binary_proxy(
        &self,
        installation: &InstallationId,
        name: impl AsRef<Path>,
    ) -> Option<PathBuf> {
        self.inner
            .borrow()
            .repr
            .installations
            .get(installation)
            .and_then(|i| i.binary_proxies.get(name.as_ref()))
            .map(|name| name.into())
    }

    /// Gets all the installations listed in the `State` file.
    pub fn installations(&self) -> Ref<BTreeMap<InstallationId, StateInstallation>> {
        Ref::map(self.inner.borrow(), |v| &v.repr.installations)
    }

    pub fn all_binary_proxy_names(&self) -> Vec<PathBuf> {
        let mut result: Vec<_> = self
            .inner
            .borrow()
            .repr
            .installations
            .values()
            .flat_map(|installation| installation.binary_proxies.keys())
            .cloned()
            .collect();

        result.sort();
        result.dedup();
        result
    }

    pub async fn persist(&self) -> Result<(), Error> {
        let (path, serialized) = {
            // Do not hold RefCells over await points, drop at end of scope.
            let inner = self.inner.borrow();

            // According to the serde_json documentation, the only two reasons this could fail is if
            // either the serialize implementation returns an error, or a map has non-string keys. With
            // our schema neither of these are supposed to happen, so if we fail serialization it's a
            // criticalup bug and we shoiuld abort.
            let mut serialized = serde_json::to_vec_pretty(&inner.repr)
                .expect("state file serialization unexpectedly failed");
            serialized.push(b'\n');

            let path = inner.path.clone();
            (path, serialized)
        };

        let mut f = open_file_for_write(&path)
            .await
            .map_err(|e| Error::CantWriteStateFile(path.clone(), e))?;
        f.write_all(&serialized)
            .await
            .map_err(|e| Error::CantWriteStateFile(path.clone(), WriteFileError::Io(e)))?;
        f.flush()
            .await
            .map_err(|e| Error::CantWriteStateFile(path.clone(), errors::WriteFileError::Io(e)))?;
        Ok(())
    }
}

/// Helper for any method or function in State to canonicalize the manifest path.
fn canonicalize_or_err(manifest_path: &Path) -> Result<PathBuf, Error> {
    let manifest =
        manifest_path
            .canonicalize()
            .map_err(|err| Error::FailedToFindCanonicalPath {
                path: manifest_path.to_path_buf(),
                kind: err,
            })?;
    Ok(manifest)
}

struct StateInner {
    path: PathBuf,
    repr: StateRepr,
}

impl StateInner {
    /// Removes a manifest path from all installations and returns the list of `InstallationId`s
    /// that had the said manifest.
    fn remove_manifest_from_all_installations(
        &mut self,
        manifest_path: &Path,
    ) -> Vec<InstallationId> {
        let all_installations_for_given_manifest = self
            .repr
            .installations
            .iter()
            .filter(|installation| installation.to_owned().1.manifests.contains(manifest_path))
            .map(|installation| installation.0.to_owned())
            .collect::<Vec<InstallationId>>();

        for id in &all_installations_for_given_manifest {
            let ins = self.repr.installations.get_mut(id);
            if let Some(state_installation) = ins {
                state_installation.manifests.remove(manifest_path);
            }
        }

        all_installations_for_given_manifest
    }

    /// Updates an existing installation using `InstallationId` by appending manifest path for a new
    /// project that has an existing installation.
    ///
    /// Also, removes the manifest path from older installations.
    fn update_installation_manifests(
        &mut self,
        installation_id: &InstallationId,
        manifest: &Path,
    ) -> Result<(), Error> {
        // Start by removing the manifest from all installations. This function takes care of
        // deleting the installation where this manifest was the last manifest before removal.
        self.remove_manifest_from_all_installations(manifest);

        match self.repr.installations.get_mut(installation_id) {
            Some(installation) => {
                let _ = installation.manifests.insert(manifest.to_path_buf());
                Ok(())
            }
            // Maybe this arm can use some tracing.
            None => Err(InstallationDoesNotExist(installation_id.0.to_owned())),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq, Eq, Debug))]
struct StateRepr {
    version: u32,
    authentication_token: Option<AuthenticationToken>,
    #[serde(default)]
    installations: BTreeMap<InstallationId, StateInstallation>,
}

impl Default for StateRepr {
    fn default() -> Self {
        Self {
            version: CURRENT_FORMAT_VERSION,
            authentication_token: None,
            installations: BTreeMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq, Debug))]
pub struct StateInstallation {
    binary_proxies: BTreeMap<PathBuf, PathBuf>,
    #[serde(default)]
    manifests: BTreeSet<PathBuf>,
}

impl StateInstallation {
    /// Get all manifests for a given `StateInstallation`.
    pub fn manifests(&self) -> &BTreeSet<PathBuf> {
        &self.manifests
    }
}

pub struct EnvVars {
    criticalup_token: Option<String>,
}

impl Default for EnvVars {
    fn default() -> Self {
        let criticalup_token = match std::env::var(CRITICALUP_TOKEN_ENV_VAR_NAME) {
            Ok(value) => {
                if !value.is_empty() {
                    Some(value)
                } else {
                    None
                }
            }
            Err(var_err) => {
                if let VarError::NotUnicode(_) = var_err {
                    tracing::error!(
                        "Environment variable {} is not Unicode.",
                        CRITICALUP_TOKEN_ENV_VAR_NAME
                    );
                }
                None
            }
        };

        EnvVars { criticalup_token }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct AuthenticationToken(String);

impl AuthenticationToken {
    pub fn seal(token: &str) -> Self {
        AuthenticationToken(token.into())
    }

    pub fn unseal(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for AuthenticationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // We don't want to accidentally include the full authentication token in debug reprs or
        // log messages, to avoid leaking it if customers share their criticalup output. This
        // custom debug repr redacts all but the last 3 chars, if the string is long enough that
        // doing so wouldn't compromise the security of the token.

        const PLAINTEXT_TRAILING_CHARS: usize = 3;
        const REDACT_ALL_WHEN_SHORTER_THAN: usize = 9;

        let encoded = if self.0.len() < REDACT_ALL_WHEN_SHORTER_THAN {
            self.0.chars().map(|_| '*').collect::<String>()
        } else {
            self.0
                .char_indices()
                .map(|(i, c)| {
                    if self.0.len() - i > PLAINTEXT_TRAILING_CHARS {
                        '*'
                    } else {
                        c
                    }
                })
                .collect::<String>()
        };

        f.write_str(&encoded)
    }
}

#[cfg(test)]
mod tests {
    use crate::state::EnvVars;
    use crate::test_utils::TestEnvironment;

    use super::*;

    macro_rules! btreemap {
        ($($key:expr => $value:expr),*$(,)?) => {{
            let mut map = std::collections::BTreeMap::new();
            $(map.insert($key.into(), $value.into());)*
            map
        }}
    }

    const ENV_VARS: EnvVars = EnvVars {
        criticalup_token: None,
    };

    #[tokio::test]
    async fn test_load_state_without_existing_file() {
        let test = TestEnvironment::prepare().await;

        assert!(!test.config().paths.state_file.exists());

        let state = State::load(test.config()).await.unwrap();
        assert_eq!(StateRepr::default(), state.inner.borrow().repr);
    }

    #[tokio::test]
    async fn test_load_state_with_existing_file() {
        let test_env = TestEnvironment::prepare().await;

        tokio::fs::write(
            &test_env.config().paths.state_file,
            serde_json::to_vec_pretty(&StateRepr {
                version: CURRENT_FORMAT_VERSION,
                authentication_token: Some(AuthenticationToken("hello".into())),
                installations: BTreeMap::new(),
            })
            .unwrap(),
        )
        .await
        .unwrap();

        let state = State::load(test_env.config()).await.unwrap();
        assert_eq!(
            Some(AuthenticationToken("hello".into())),
            state.authentication_token(None, &ENV_VARS).await
        );
    }

    #[tokio::test]
    async fn save_same_manifest_content_new_proj_if_existing_installation() {
        let test_env = TestEnvironment::with().state().prepare().await;
        let root = test_env.root();
        let state = test_env.state();

        // Prepare env with one installation that has one manifest file path.
        let installation_id = InstallationId("installation-id-1".to_string());
        let binary_proxies: BTreeMap<_, _> = btreemap! {
            PathBuf::from("cargo") => PathBuf::from("/path/to/").join(installation_id.0.clone()).join("bin/cargo"),
            PathBuf::from("rustc") => PathBuf::from("/path/to/").join(installation_id.0.clone()).join("bin/rustc"),
        };
        let verified_package = VerifiedPackage {
            product: "ferrocene".to_string(),
            package: "rusty".to_string(),
            proxies_paths: binary_proxies,
        };

        // Add installation and write the state file.
        let proj1 = root.join("path/to/proj/1");
        tokio::fs::create_dir_all(&proj1).await.unwrap();
        state
            .add_installation(
                &installation_id,
                &[verified_package],
                &proj1,
                test_env.config(),
            )
            .unwrap();
        state.persist().await.unwrap();

        // Add a second project manifest for the same installation and write the state file.
        let proj2 = root.join("path/to/proj/2");
        tokio::fs::create_dir_all(&proj2).await.unwrap();
        let _ = state.update_installation_manifests(&installation_id, &proj2);
        state.persist().await.unwrap();

        // Check that both unique manifests are present in the installation.
        let new_state = State::load(test_env.config()).await.unwrap();
        let new_state_inner = new_state.inner.borrow();
        let manifests_in_state = &new_state_inner
            .repr
            .installations
            .get(&installation_id)
            .unwrap()
            .manifests;
        assert_eq!(
            &BTreeSet::from([
                root.join("path/to/proj/1").canonicalize().unwrap(),
                root.join("path/to/proj/2").canonicalize().unwrap()
            ]),
            manifests_in_state
        );
    }

    #[tokio::test]
    async fn same_manifest_content_new_proj_twice_for_existing_installation_still_unique_manifest_paths_only(
    ) {
        let test_env = TestEnvironment::with().state().prepare().await;
        let root = test_env.root();
        let state = test_env.state();
        // Prepare env with one installation that has one manifest file path.
        let installation_id = InstallationId("installation-id-1".to_string());
        let binary_proxies: BTreeMap<_, _> = btreemap! {
            PathBuf::from("cargo") => PathBuf::from("/path/to/").join(installation_id.0.clone()).join("bin/cargo"),
            PathBuf::from("rustc") => PathBuf::from("/path/to/").join(installation_id.0.clone()).join("bin/rustc"),
        };
        let verified_package = VerifiedPackage {
            product: "ferrocene".to_string(),
            package: "rusty".to_string(),
            proxies_paths: binary_proxies,
        };

        let proj1 = root.join("path/to/proj/1");
        tokio::fs::create_dir_all(&proj1).await.unwrap();
        // Add installation and write the state file.
        state
            .add_installation(
                &installation_id,
                &[verified_package],
                &proj1,
                test_env.config(),
            )
            .unwrap();
        state.persist().await.unwrap();

        // Load the State file and add update installation manifest with another unique path
        // which mimics that for same installation id you can have the new path added
        // here we update the same path multiple times.
        let proj2 = root.join("path/to/proj/2");
        tokio::fs::create_dir_all(&proj2).await.unwrap();
        let state = State::load(test_env.config()).await.unwrap();
        let _ = state.update_installation_manifests(&installation_id, &proj2);
        state.persist().await.unwrap();
        let _ = state.update_installation_manifests(&installation_id, &proj2);
        state.persist().await.unwrap();
        let _ = state.update_installation_manifests(&installation_id, &proj2);
        state.persist().await.unwrap();

        let new_state = State::load(test_env.config()).await.unwrap().inner;
        let new_state = &new_state.borrow_mut();
        let manifests_in_state = &new_state
            .repr
            .installations
            .get(&installation_id)
            .unwrap()
            .manifests;

        assert_eq!(
            &BTreeSet::from([
                root.join("path/to/proj/1").canonicalize().unwrap(),
                root.join("path/to/proj/2").canonicalize().unwrap()
            ]),
            manifests_in_state
        );
    }

    /// Starts with two installations with one manifest/project each and then updates the State
    /// by adding second manifest to the first installation.
    ///
    /// Should result in empty manifests section of second installation and two manifests in the
    /// first installation.
    #[tokio::test]
    async fn two_installations_empty_manifests_section_when_moved() {
        let test_env = TestEnvironment::with().state().prepare().await;
        let root = test_env.root();
        let state = test_env.state();

        // Prepare env with two installations with different manifest paths.
        let proj1 = root.join("path/to/proj/1");
        tokio::fs::create_dir_all(&proj1).await.unwrap();
        let proj2 = root.join("path/to/proj/2");
        tokio::fs::create_dir_all(&proj2).await.unwrap();

        // Installation 1.
        let installation_id_1 = InstallationId("installation-id-1".to_string());
        let binary_proxies_1: BTreeMap<_, _> = btreemap! {
            PathBuf::from("cargo") => PathBuf::from("/path/to/").join(installation_id_1.0.clone()).join("bin/cargo"),
            PathBuf::from("rustc") => PathBuf::from("/path/to/").join(installation_id_1.0.clone()).join("bin/rustc"),
        };
        let verified_package_1 = VerifiedPackage {
            product: "ferrocene".to_string(),
            package: "rusty".to_string(),
            proxies_paths: binary_proxies_1,
        };

        // Add installation 1 and write the state file.
        state
            .add_installation(
                &installation_id_1,
                &[verified_package_1],
                &proj1,
                test_env.config(),
            )
            .unwrap();
        state.persist().await.unwrap();

        // Installation 2.
        let installation_id_2 = InstallationId("installation-id-2".to_string());
        let binary_proxies_2: BTreeMap<_, _> = btreemap! {
            PathBuf::from("amazing") => PathBuf::from("/path/to/").join(installation_id_2.0.clone()).join("bin/amazing"),
            PathBuf::from("stuff") => PathBuf::from("/path/to/").join(installation_id_2.0.clone()).join("bin/stuff"),
        };
        let verified_package_2 = VerifiedPackage {
            product: "ferrocene".to_string(),
            package: "rusty".to_string(),
            proxies_paths: binary_proxies_2,
        };

        // Add installation 2 and write the state file.
        state
            .add_installation(
                &installation_id_2,
                &[verified_package_2],
                &proj2,
                test_env.config(),
            )
            .unwrap();
        state.persist().await.unwrap();

        // Load the State file and add update installation manifest with another unique path
        // which mimics that for same installation id you can have the new path added
        // here we update the same path multiple times.
        let state = State::load(test_env.config()).await.unwrap();
        let _ = state.update_installation_manifests(&installation_id_1, &proj2);
        state.persist().await.unwrap();

        // Check that the installation 1 has both project manifests and the installation 2 has
        // no project manifests (empty manifests).
        let new_state = State::load(test_env.config()).await.unwrap().inner;
        let new_state = &new_state.borrow_mut();
        let manifests_in_installation_1 = &new_state
            .repr
            .installations
            .get(&installation_id_1)
            .unwrap()
            .manifests;

        assert_eq!(
            &BTreeSet::from([
                root.join("path/to/proj/1").canonicalize().unwrap(),
                root.join("path/to/proj/2").canonicalize().unwrap()
            ]),
            manifests_in_installation_1
        );

        let manifests_in_installation_2 = &new_state
            .repr
            .installations
            .get(&installation_id_2)
            .unwrap()
            .manifests;
        assert_eq!(&BTreeSet::from([]), manifests_in_installation_2);
    }

    #[tokio::test]
    async fn test_load_state_with_fs_error() {
        let test_env = TestEnvironment::prepare().await;

        // Creating a directory instead of a file should result in an IO error when we then try to
        // read the contents of the file.
        tokio::fs::create_dir_all(&test_env.config().paths.state_file)
            .await
            .unwrap();

        match State::load(test_env.config()).await {
            Err(Error::CantReadStateFile(path, _)) => {
                assert_eq!(test_env.config().paths.state_file, path);
            }
            Err(err) => panic!("unexpected error when loading the state: {err:?}"),
            Ok(_) => panic!("loading the state file succeeded"),
        }
    }

    #[tokio::test]
    async fn test_load_state_with_unsupported_version() {
        let test_env = TestEnvironment::prepare().await;

        std::fs::write(
            &test_env.config().paths.state_file,
            serde_json::to_vec_pretty(&StateRepr {
                version: CURRENT_FORMAT_VERSION + 1,
                ..StateRepr::default()
            })
            .unwrap(),
        )
        .unwrap();

        match State::load(test_env.config()).await {
            Err(Error::UnsupportedStateFileVersion(path, version)) => {
                assert_eq!(test_env.config().paths.state_file, path);
                assert_eq!(CURRENT_FORMAT_VERSION + 1, version);
            }
            Err(err) => panic!("unexpected error when loading the state: {err:?}"),
            Ok(_) => panic!("loading the state file succeeded"),
        }
    }

    #[tokio::test]
    async fn test_load_state_with_invalid_contents() {
        let test_env = TestEnvironment::prepare().await;

        tokio::fs::write(&test_env.config().paths.state_file, b"Hello world\n")
            .await
            .unwrap();

        match State::load(test_env.config()).await {
            Err(Error::CorruptStateFile(path, error)) => {
                assert_eq!(test_env.config().paths.state_file, path);
                assert!(error.is_syntax());
            }
            Err(err) => panic!("unexpected error when loading the state: {err:?}"),
            Ok(_) => panic!("loading the state file succeeded"),
        }
    }

    #[tokio::test]
    async fn docker_secrets_are_read_from_file() {
        let test_env = TestEnvironment::with()
            .state()
            .root_in_subdir("run/secrets")
            .prepare()
            .await;
        let state = test_env.state();
        state.set_authentication_token(None);

        //  Make sure the state file has authentication token as None.
        assert_eq!(state.authentication_token(None, &ENV_VARS).await, None);

        let file_token_content = "my-awesome-token-from-file";
        let token_name = "CRITICALUP_TOKEN";

        // Add a temp secrets dir and create a token file there and make sure
        // that that token is returned if legit file path was given.
        let secrets_dir = test_env.root().join::<PathBuf>("run/secrets".into());
        tokio::fs::create_dir_all(&secrets_dir).await.unwrap();
        std::fs::write(secrets_dir.join(token_name), file_token_content).unwrap();
        let token = test_env
            .state()
            .authentication_token(
                Some(secrets_dir.join(token_name).to_str().unwrap()),
                &ENV_VARS,
            )
            .await;
        assert_eq!(Some(AuthenticationToken(file_token_content.into())), token)
    }

    #[tokio::test]
    async fn test_set_authentication_token() {
        let test_env = TestEnvironment::with().state().prepare().await;
        let state = test_env.state();

        state.set_authentication_token(None);
        assert_eq!(None, state.authentication_token(None, &ENV_VARS).await);

        state.set_authentication_token(Some(AuthenticationToken("hello world".into())));
        assert_eq!(
            Some(AuthenticationToken("hello world".into())),
            state.authentication_token(None, &ENV_VARS).await
        );
    }

    #[tokio::test]
    async fn test_persist_state() {
        let test_env = TestEnvironment::with().state().prepare().await;

        let token = AuthenticationToken("hello world".into());
        test_env
            .state()
            .set_authentication_token(Some(token.clone()));
        test_env.state().persist().await.unwrap();

        let new_state = State::load(test_env.config()).await.unwrap();
        assert_eq!(
            Some(token),
            new_state.authentication_token(None, &ENV_VARS).await
        );
    }

    #[tokio::test]
    async fn test_persist_state_with_fs_io_error() {
        let test_env = TestEnvironment::with().state().prepare().await;
        test_env
            .state()
            .set_authentication_token(Some(AuthenticationToken("hello world".into())));

        // Simulate a file system error by creating a directory in the path the state file is
        // supposed to be written. The current state was generated in memory, so we don't need to
        // remove the previous contents at that path.
        tokio::fs::create_dir_all(&test_env.config().paths.state_file)
            .await
            .unwrap();

        match test_env.state().persist().await {
            Err(Error::CantWriteStateFile(path, WriteFileError::Io(_))) => {
                assert_eq!(test_env.config().paths.state_file, path);
            }
            Err(err) => panic!("unexpected error when persisting the state: {err:?}"),
            Ok(_) => panic!("persisting the state file succeeded"),
        }
    }

    #[tokio::test]
    async fn test_persist_state_with_fs_parent_directory_error() {
        let test_env = TestEnvironment::with()
            .root_in_subdir("subdir")
            .state()
            .prepare()
            .await;
        test_env
            .state()
            .set_authentication_token(Some(AuthenticationToken("hello world".into())));

        // Simulate a file system error by creating a file in the path the parent directory of the
        // state file is supposed to be written. The current state was generated in memory, so we
        // don't need to remove the previous contents at that path.
        tokio::fs::write(test_env.root().join("subdir"), b"")
            .await
            .unwrap();

        match test_env.state().persist().await {
            Err(Error::CantWriteStateFile(path, WriteFileError::CantCreateParentDirectory(_))) => {
                assert_eq!(test_env.config().paths.state_file, path);
            }
            Err(err) => panic!("unexpected error when persisting the state: {err:?}"),
            Ok(_) => panic!("persisting the state file succeeded"),
        }
    }

    #[tokio::test]
    async fn test_binary_proxies() {
        let test_env = TestEnvironment::with().state().prepare().await;
        let root = test_env.root();
        let state = test_env.state();

        let id1 = InstallationId("sample".into());
        let inst1_manifest_path = root.join("proj/1/manifest");
        tokio::fs::create_dir_all(&inst1_manifest_path)
            .await
            .unwrap();
        let id2 = InstallationId("id".into());
        let inst2_manifest_path = root.join("proj/2/manifest");
        tokio::fs::create_dir_all(&inst2_manifest_path)
            .await
            .unwrap();

        state
            .add_installation(
                &id1,
                &[
                    VerifiedPackage {
                        product: "ferrocene".into(),
                        package: "foo".into(),
                        proxies_paths: btreemap! { "a" => "foo/a" },
                    },
                    VerifiedPackage {
                        product: "ferrocene".into(),
                        package: "bar".into(),
                        proxies_paths: btreemap! { "b" => "foo/b" },
                    },
                ],
                &inst1_manifest_path,
                test_env.config(),
            )
            .unwrap();
        assert_eq!(Some("foo/a".into()), state.resolve_binary_proxy(&id1, "a"));
        assert_eq!(Some("foo/b".into()), state.resolve_binary_proxy(&id1, "b"));
        assert_eq!(
            vec![PathBuf::from("a"), "b".into()],
            state.all_binary_proxy_names()
        );

        state
            .add_installation(
                &id2,
                &[VerifiedPackage {
                    product: "ferrocene".into(),
                    package: "foo".into(),
                    proxies_paths: btreemap! { "a" => "bar/a" },
                }],
                &inst2_manifest_path,
                test_env.config(),
            )
            .unwrap();
        assert_eq!(Some("bar/a".into()), state.resolve_binary_proxy(&id2, "a"));
        assert!(state.resolve_binary_proxy(&id2, "b").is_none());
        assert_eq!(
            vec![PathBuf::from("a"), "b".into()],
            state.all_binary_proxy_names()
        );

        state.remove_installation(&id1);
        assert_eq!(vec![PathBuf::from("a")], state.all_binary_proxy_names());
        state.remove_installation(&id2);

        assert!(state.all_binary_proxy_names().is_empty());
        assert!(state.resolve_binary_proxy(&id1, "a").is_none());
        assert!(state.resolve_binary_proxy(&id1, "b").is_none());
    }

    #[test]
    fn test_default_state_values() {
        // This test ensures the default values for the state file do not change ACCIDENTALLY. If
        // you intentionally made a change that resulted in this test failing you should change it
        // to reflect the new defaults.
        assert_eq!(
            StateRepr {
                version: 1,
                authentication_token: None,
                installations: BTreeMap::new(),
            },
            StateRepr::default()
        );
    }

    #[test]
    fn test_authentication_token_debug_repr() {
        assert_eq!("", format!("{:?}", AuthenticationToken::seal("")));
        assert_eq!("***", format!("{:?}", AuthenticationToken::seal("123")));
        assert_eq!(
            "********",
            format!("{:?}", AuthenticationToken::seal("12345678"))
        );
        assert_eq!(
            "******789",
            format!("{:?}", AuthenticationToken::seal("123456789"))
        );
        assert_eq!(
            "****************789",
            format!("{:?}", AuthenticationToken::seal("1234567890123456789"))
        );
    }

    #[tokio::test]
    async fn all_unsed_installations_only() {
        let test_env = TestEnvironment::with().state().prepare().await;
        let root = test_env.root();
        let state = test_env.state();

        // Prepare env with first installation that has one manifest file path.
        let installation_id_1 = InstallationId("installation-id-1".to_string());
        let verified_package = VerifiedPackage {
            product: "ferrocene".to_string(),
            package: "rusty".to_string(),
            proxies_paths: BTreeMap::default(),
        };

        let proj1 = root.join("path/to/proj/1");
        tokio::fs::create_dir_all(&proj1).await.unwrap();
        // Add installation and write the state file.
        state
            .add_installation(
                &installation_id_1,
                &[verified_package.clone()],
                &proj1,
                test_env.config(),
            )
            .unwrap();
        state.persist().await.unwrap();

        let proj2 = root.join("path/to/proj/2");
        tokio::fs::create_dir_all(&proj2).await.unwrap();
        // Prepare env with second installation that has one manifest file path.
        let installation_id_2 = InstallationId("installation-id-2".to_string());
        state
            .add_installation(
                &installation_id_2,
                &[verified_package.clone()],
                &proj2,
                test_env.config(),
            )
            .unwrap();
        state.persist().await.unwrap();

        // Add a second project manifest to the first installation. This will render the second
        // installation with empty manifests section and will be return as "unused".
        let _ = state.update_installation_manifests(&installation_id_1, &proj2);
        state.persist().await.unwrap();

        let unused_installations = state
            .installations()
            .iter()
            .filter(|item| item.1.manifests().is_empty())
            .map(|item| item.0.to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            Vec::from([installation_id_2.to_owned()]),
            unused_installations
        )
    }

    #[tokio::test]
    async fn test_set_auth_token_env_var() {
        let test_env = TestEnvironment::with().state().prepare().await;
        let state = test_env.state();

        let token = "my_awesome_token".to_string();

        let env_vars = EnvVars {
            criticalup_token: Some(token.clone()),
        };

        state.set_authentication_token(None);
        assert_eq!(
            Some(AuthenticationToken(token)),
            state.authentication_token(None, &env_vars).await
        );

        let env_vars = EnvVars {
            criticalup_token: None,
        };
        assert_eq!(None, state.authentication_token(None, &env_vars).await);
    }

    #[tokio::test]
    async fn test_read_env_var() {
        std::env::set_var(CRITICALUP_TOKEN_ENV_VAR_NAME, "HoustonWeHaveAToken!");
        let ev = EnvVars::default();
        assert_eq!(
            ev.criticalup_token,
            Some("HoustonWeHaveAToken!".to_string())
        );

        std::env::remove_var(CRITICALUP_TOKEN_ENV_VAR_NAME);
        let ev = EnvVars::default();
        assert_eq!(ev.criticalup_token, None);
    }
}
