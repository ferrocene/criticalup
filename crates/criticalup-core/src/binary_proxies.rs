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
#[tracing::instrument(level = "trace", skip_all, fields(
    proxy_binary = %proxy_binary.display()
))]
pub async fn update(
    config: &Config,
    state: &State,
    proxy_binary: &Path,
) -> Result<(), BinaryProxyUpdateError> {
    if proxy_binary.is_dir() {
        return Err(BinaryProxyUpdateError::ProxyBinaryShouldNotBeDir(
            proxy_binary.into(),
        ));
    }

    let mut expected_proxies = state
        .all_binary_proxy_names()
        .into_iter()
        .collect::<HashSet<_>>();


    tracing::trace!(
        expected_proxies = expected_proxies.len(),
        "Updating binary proxies"
    );

    let bin_dir = &config.paths.proxy_dir.join("bin");
    tokio::fs::create_dir_all(bin_dir)
        .await
        .map_err(|e| BinaryProxyUpdateError::DirectoryCreationFailed(bin_dir.clone(), e))?;
    // Required for `rustup toolchain link`
    tokio::fs::create_dir_all(config.paths.proxy_dir.join("lib"))
        .await
        .map_err(|e| BinaryProxyUpdateError::DirectoryCreationFailed(bin_dir.clone(), e))?;

    // Migrate to new proxy system
    remove_deprecated_proxies(&config.paths.root.join("bin"), &config.paths.proxy_dir).await?;

    let list_dir_error = |e| BinaryProxyUpdateError::ListDirectoryFailed(bin_dir.into(), e);
    match tokio::fs::read_dir(bin_dir).await {
        Ok(mut iter) => {
            while let Some(entry) = iter.next_entry().await.map_err(list_dir_error)? {
                let entry_name = PathBuf::from(entry.file_name());

                if expected_proxies.remove(&*entry_name) {
                    ensure_link(proxy_binary, &entry.path()).await?;
                } else {
                    remove_unexpected(&entry.path()).await?;
                }
            }
        }
        // If the directory is missing we can skip trying to update its contents, as the next loop
        // will then create all the proxies.
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(list_dir_error(err)),
    }

    if expected_proxies.is_empty() {
        tracing::trace!("No new proxies to create")
    } else {
        for proxy in expected_proxies {
            let target = &config.paths.proxy_dir.join("bin").join(&proxy);
            ensure_link(proxy_binary, target).await?;
        }
    }

    Ok(())
}

// Previously, proxies were located at `root/bin`, now they are at `root/proxy/bin`, remove any remains.
async fn remove_deprecated_proxies(
    old: &PathBuf,
    new: &PathBuf,
) -> Result<(), BinaryProxyUpdateError> {
    let old_bin_dir = old;
    if old_bin_dir.exists() {
        tracing::info!(
            "Tidying deprecated binary proxies, they are now located at `{}`",
            new.join("bin").display()
        );
        tracing::info!("You can also now use `rustup toolchain link ferrocene \"{}\"` then use Ferrocene like any other Rust toolchain via `cargo +ferrocene build`", new.display());
        tokio::fs::remove_dir_all(&old_bin_dir)
            .await
            .map_err(|e| BinaryProxyUpdateError::DirectoryRemovalFailed(old_bin_dir.clone(), e))?;
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
                remove_unexpected(target).await?;
                true
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => {
            remove_unexpected(target).await?;
            true
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
        Err(err) => return Err(BinaryProxyUpdateError::InspectFailed(target.into(), err)),
    };

    if should_create {
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| BinaryProxyUpdateError::DirectoryCreationFailed(parent.into(), e))?;
        }
        std::os::unix::fs::symlink(proxy_binary, target).map_err(|e| {
            BinaryProxyUpdateError::SymlinkFailed {
                source: proxy_binary.into(),
                dest: target.into(),
                inner: e,
            }
        })?;

        tracing::debug!(target = %target.display(), "Created binary proxy");
    } else {
        tracing::trace!(target = %target.display(), "Skipped creating binary proxy, already exists");
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
        remove_unexpected(target).await?;
    };

    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| BinaryProxyUpdateError::DirectoryCreationFailed(parent.into(), e))?;
    }

    // We opt against symlinks on Windows. Many of our users are not on Windows 11 which does
    // support unpriviledged symlinks.
    //
    // On Windows 10, symlinks can be done by priviledged users, or users with "Developer Mode"
    // enabled, but not all of our users have that.
    tokio::fs::copy(proxy_binary, target).await.map_err(|e| {
        BinaryProxyUpdateError::SymlinkFailed {
            source: proxy_binary.into(),
            dest: target.into(),
            inner: e,
        }
    })?;

    tracing::debug!(target = %target.display(), "Created binary proxy");

    Ok(())
}

async fn remove_unexpected(path: &Path) -> Result<(), BinaryProxyUpdateError> {
    let result = if path.is_dir() {
        tokio::fs::remove_dir_all(path).await
    } else {
        tokio::fs::remove_file(path).await
    };
    match result {
        Ok(()) => {
            tracing::trace!(path = %path.display(), "Removed binary proxy");
            Ok(())
        }
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

    use tempfile::{tempdir, NamedTempFile, TempDir};

    use criticaltrust::integrity::VerifiedPackage;

    use crate::project_manifest::InstallationId;
    use crate::test_utils::TestEnvironment;

    use super::*;

    #[tokio::test]
    async fn test_update() {
        let test_env = TestEnvironment::with().state().prepare().await;
        let root = test_env.root();
        let installation_dir = &test_env.config().paths.installation_dir;
        let state = test_env.state();

        // Installation 1, with only one project manifest.
        let inst1 = InstallationId("1".into());
        tokio::fs::create_dir_all(installation_dir.clone().join("1"))
            .await
            .unwrap();
        let inst1_first_manifest_path = root.join("proj/1/manifest");
        tokio::fs::create_dir_all(&inst1_first_manifest_path)
            .await
            .unwrap();

        // Installation 2, with two project manifests in different locations.
        let inst2 = InstallationId("2".into());
        tokio::fs::create_dir_all(installation_dir.clone().join("2"))
            .await
            .unwrap();
        let inst2_first_manifest_path = root.join("proj/2/manifest-1");
        tokio::fs::create_dir_all(&inst2_first_manifest_path)
            .await
            .unwrap();
        // Another manifest for the same project.
        let inst2_second_manifest_path = root.join("project/2/manifest-2");
        tokio::fs::create_dir_all(&inst2_second_manifest_path)
            .await
            .unwrap();

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
            for file in config.paths.proxy_dir.join("bin").read_dir().unwrap() {
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

        async fn create_file(dir: &TempDir, name: &str) -> PathBuf {
            let path = dir.path().join(name);
            tokio::fs::write(&path, name.as_bytes()).await.unwrap();
            path
        }

        let source1 = create_file(&dir, "source1").await;
        let source2 = create_file(&dir, "source2").await;

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
        let link2 = create_file(&dir, "link2").await;
        ensure_link(&source1, &link2).await.unwrap();
        assert_link(&source1, &link2);

        // Test creating a link when a directory with contents exists in its place.
        let link3 = dir.path().join("link3");
        tokio::fs::create_dir(&link3).await.unwrap();
        tokio::fs::write(link3.join("file"), b"").await.unwrap();
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

    #[tokio::test]
    async fn update_should_not_accept_dir_for_proxy_binary_arg() {
        let test_env = TestEnvironment::with().state().prepare().await;
        let state = test_env.state();

        // If the directory does not exist, the `.is_dir()` method fails to recognize it's a dir.
        tokio::fs::create_dir_all(&test_env.config().paths.proxy_dir.join("bin"))
            .await
            .unwrap();
        assert!(test_env.config().paths.proxy_dir.join("bin").exists());

        assert!(matches!(
            update(
                test_env.config(),
                state,
                &test_env.config().paths.proxy_dir.join("bin")
            )
            .await,
            Err(BinaryProxyUpdateError::ProxyBinaryShouldNotBeDir(_))
        ))
    }
}
