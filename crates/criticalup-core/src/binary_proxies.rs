// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binary proxies are binaries named after the tools included in Ferrocene (like rustc, rustdoc,
//! cargo, etc...), that check which criticalup installation to use before executing the actual
//! binary inside of the chosen criticalup installation.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::errors::BinaryProxyUpdateError;
use crate::state::State;

/// Update the set of binary proxies to reflect the current state of things. This will:
///
/// * Add any new binary proxy added to the state since the last update.
///
/// * Remove any binary proxy not referenced in the state anymore.
///
/// * Replace all binary proxy binaries with new copies if they point to a different
///   `proxy_binary`, to ensure they all point to the latest available version. This is likely to
///   occur after the user updates criticalup.
///
pub async fn update(
    config: &Config,
    state: &State,
    proxy_binary: &Path,
) -> Result<(), BinaryProxyUpdateError> {
    let mut expected_proxies = state
        .all_binary_proxy_names()
        .into_iter()
        .collect::<HashSet<_>>();

    let dir = &config.paths.proxies_dir;
    let list_dir_error = |e| BinaryProxyUpdateError::ListDirectoryFailed(dir.into(), e);
    match tokio::fs::read_dir(dir).await {
        Ok(mut iter) => {
            while let Some(entry) = iter.next_entry().await.map_err(list_dir_error)? {
                let entry = entry;

                let entry_name = PathBuf::from(entry.file_name());

                if expected_proxies.remove(&*entry_name) {
                    ensure_link(proxy_binary, &entry.path()).await?;
                } else {
                    remove_unexpected(&entry.path())?;
                }
            }
        }
        // If the directory is missing we can skip trying to update its contents, as the next loop
        // will then create all the proxies.
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(list_dir_error(err)),
    }

    for proxy in expected_proxies {
        let target = &config.paths.proxies_dir.join(&proxy);
        ensure_link(proxy_binary, target).await?;
    }

    Ok(())
}

#[cfg(unix)]
async fn ensure_link(proxy_binary: &Path, target: &Path) -> Result<(), BinaryProxyUpdateError> {
    async fn canonicalize(path: &Path) -> Result<PathBuf, BinaryProxyUpdateError> {
        tokio::fs::canonicalize(path)
            .await
            .map_err(|e| BinaryProxyUpdateError::InspectFailed(path.into(), e))
    }

    let should_create = match target.read_link() {
        Ok(target_dest) => {
            if canonicalize(proxy_binary).await? == canonicalize(&target_dest).await? {
                false
            } else {
                remove_unexpected(target)?;
                true
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => {
            remove_unexpected(target)?;
            true
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
        Err(err) => return Err(BinaryProxyUpdateError::InspectFailed(target.into(), err)),
    };

    if should_create {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                BinaryProxyUpdateError::ParentDirectoryCreationFailed(parent.into(), e)
            })?;
        }
        std::os::unix::fs::symlink(proxy_binary, target).map_err(|e| {
            BinaryProxyUpdateError::SymlinkFailed {
                source: proxy_binary.into(),
                dest: target.into(),
                inner: e,
            }
        })?;
    }

    Ok(())
}

#[cfg(windows)]
async fn ensure_link(proxy_binary: &Path, target: &Path) -> Result<(), BinaryProxyUpdateError> {
    // We cannot use `canonicalize` safely here since it basically doesn't work on Windows.
    // For example, on even a relatively uncomplicated dev machine attempting to canonicalize a link
    // between two files in the same folder on the same disk fails with
    //
    // ```
    // The file or directory is not a reparse point.
    // ```
    //
    // So, instead of checking to see if the link exists and is correct, we just blindly rewrite it.
    if target.exists() {
        remove_unexpected(target)?;
    };

    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| BinaryProxyUpdateError::ParentDirectoryCreationFailed(parent.into(), e))?;
    }

    // We opt against symlinks on Windows. Many of our users are not on Windows 11 which does
    // support unpriviledged symlinks.
    //
    // On Windows 10, symlinks can be done by priviledged users, or users with "Developer Mode"
    // enabled, but not all of our users have that.
    tokio::fs::copy(proxy_binary, target)
        .await
        .map_err(|e| BinaryProxyUpdateError::SymlinkFailed {
            source: proxy_binary.into(),
            dest: target.into(),
            inner: e,
        })
        .await?;

    Ok(())
}

fn remove_unexpected(path: &Path) -> Result<(), BinaryProxyUpdateError> {
    let result = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    match result {
        Ok(()) => Ok(()),
        Err(err) => Err(BinaryProxyUpdateError::UnexpectedPathRemovalFailed(
            path.into(),
            err,
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::path::PathBuf;

    use tempfile::{tempdir, NamedTempFile};

    use criticaltrust::integrity::VerifiedPackage;

    use crate::project_manifest::InstallationId;
    use crate::test_utils::TestEnvironment;

    use super::*;

    #[tokio::test]
    async fn test_update() {
        let test_env = TestEnvironment::with().state().prepare();
        let root = test_env.root();
        let installation_dir = &test_env.config().paths.installation_dir;
        let state = test_env.state();

        // Installation 1, with only one project manifest.
        let inst1 = InstallationId("1".into());
        std::fs::create_dir_all(installation_dir.clone().join("1")).unwrap();
        let inst1_first_manifest_path = root.join("proj/1/manifest");
        std::fs::create_dir_all(&inst1_first_manifest_path).unwrap();

        // Installation 2, with two project manifests in different locations.
        let inst2 = InstallationId("2".into());
        std::fs::create_dir_all(installation_dir.clone().join("2")).unwrap();
        let inst2_first_manifest_path = root.join("proj/2/manifest-1");
        std::fs::create_dir_all(&inst2_first_manifest_path).unwrap();
        // Another manifest for the same project.
        let inst2_second_manifest_path = root.join("project/2/manifest-2");
        std::fs::create_dir_all(&inst2_second_manifest_path).unwrap();

        let mut proxy1 = NamedTempFile::new_in(test_env.root()).unwrap();
        proxy1.write_all(b"proxied binary 1").unwrap();
        let proxy1 = proxy1.path();

        let mut proxy2 = NamedTempFile::new_in(test_env.root()).unwrap();
        proxy2.write_all(b"proxied binary 2").unwrap();
        let proxy2 = proxy2.path();

        // Add a first installation with a few binaries with manifest1.
        state
            .add_installation(
                &inst1,
                &verified_packages(&["bin1", "bin2"]),
                &inst1_first_manifest_path,
                test_env.config(),
            )
            .unwrap();
        update(test_env.config(), state, proxy1).await.unwrap();
        assert_proxies(test_env.config(), proxy1, &["bin1", "bin2"]);

        // Add a second installation, ensure the new binary is added.
        state
            .add_installation(
                &inst2,
                &verified_packages(&["bin3"]),
                &inst2_first_manifest_path,
                test_env.config(),
            )
            .unwrap();
        update(test_env.config(), state, proxy1).await.unwrap();
        assert_proxies(test_env.config(), proxy1, &["bin1", "bin2", "bin3"]);

        // Same installation but a different location of a manifest, which means that another
        // project with the same manifest content.
        state
            .add_installation(
                &inst2,
                &verified_packages(&[]),
                &inst2_second_manifest_path,
                test_env.config(),
            )
            .unwrap();
        update(test_env.config(), state, proxy1).await.unwrap();
        assert_proxies(test_env.config(), proxy1, &["bin1", "bin2", "bin3"]);

        // Remove the first installation *and* change the path of the proxy binary (to simulate a
        // new criticalup binary after an update).
        state.remove_installation(&inst1);
        update(test_env.config(), state, proxy2).await.unwrap();
        assert_proxies(test_env.config(), proxy2, &["bin3"]);

        // Remove the last installation to ensure all proxies are removed.
        state.remove_installation(&inst2);
        update(test_env.config(), state, proxy2).await.unwrap();
        assert_proxies(test_env.config(), proxy2, &[]);

        fn verified_packages(proxies: &[&str]) -> Vec<VerifiedPackage> {
            let mut proxies_paths = BTreeMap::new();
            for proxy in proxies {
                proxies_paths.insert(PathBuf::from(proxy), Path::new("bin").join(proxy));
            }

            vec![VerifiedPackage {
                product: String::new(),
                package: String::new(),
                proxies_paths,
            }]
        }

        #[track_caller]
        fn assert_proxies(config: &Config, expected_proxy: &Path, expected: &[&str]) {
            let expected_proxy_content = std::fs::read(expected_proxy).unwrap();

            let mut found_proxies = Vec::new();
            for file in config.paths.proxies_dir.read_dir().unwrap() {
                let file = file.unwrap().path();
                found_proxies.push(file.file_name().unwrap().to_str().unwrap().to_string());

                // To chech whether the proxy links to the right binary we read the content and
                // compare it. We do this compared to (for example) checking the target of the
                // symlink to make this test resilient to changes in how we create links.
                let proxy_content = std::fs::read(&file).unwrap();
                assert_eq!(
                    expected_proxy_content,
                    proxy_content,
                    "wrong content for {}",
                    file.display()
                );
            }
            found_proxies.sort();

            let mut expected_proxies = expected.to_vec();
            expected_proxies.sort();

            assert_eq!(expected_proxies, found_proxies);
        }
    }

    #[tokio::test]
    async fn test_ensure_link() {
        let dir = tempdir().unwrap();
        assert!(dir.path().is_absolute());

        let create_file = |name: &str| {
            let path = dir.path().join(name);
            std::fs::write(&path, name.as_bytes()).unwrap();
            path
        };

        let source1 = create_file("source1");
        let source2 = create_file("source2");

        // Test creating the link when no existing link was present.
        let link1 = dir.path().join("link1");
        ensure_link(&source1, &link1).await.unwrap();
        assert_link(&source1, &link1);

        // Test calling the function again with the same inputs.
        ensure_link(&source1, &link1).await.unwrap();
        assert_link(&source1, &link1);

        // Test replacing the link with a new target.
        ensure_link(&source2, &link1).await.unwrap();
        assert_link(&source2, &link1);

        // Test creating a link when a non-link file exists in its place.
        let link2 = create_file("link2");
        ensure_link(&source1, &link2).await.unwrap();
        assert_link(&source1, &link2);

        // Test creating a link when a directory with contents exists in its place.
        let link3 = dir.path().join("link3");
        std::fs::create_dir(&link3).unwrap();
        std::fs::write(link3.join("file"), b"").unwrap();
        ensure_link(&source1, &link3).await.unwrap();
        assert_link(&source1, &link3);

        #[track_caller]
        fn assert_link(source: &Path, link: &Path) {
            let source_content = std::fs::read(source).unwrap();
            let link_content = std::fs::read(link).unwrap();
            assert_eq!(
                source_content,
                link_content,
                "{} doesn't link to {}",
                link.display(),
                source.display()
            );
        }
    }
}
