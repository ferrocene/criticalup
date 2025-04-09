// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::config::WhitelabelConfig;
use crate::errors::Error;
use std::env;
use std::path::{Path, PathBuf};

const DEFAULT_INSTALLATION_DIR_NAME: &str = "toolchains";

#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Paths {
    pub(crate) state_file: PathBuf,

    pub proxy_dir: PathBuf,
    pub installation_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub root: PathBuf,
}

impl Paths {
    pub(super) fn detect(
        whitelabel: &WhitelabelConfig,
        root: Option<std::path::PathBuf>,
        cache_dir: Option<std::path::PathBuf>,
    ) -> Result<Paths, Error> {
        let root = if let Some(root) = root {
            if root != Path::new("") {
                root
            } else {
                find_root(whitelabel).ok_or(Error::CouldNotDetectRootDirectory)?
            }
        } else {
            find_root(whitelabel).ok_or(Error::CouldNotDetectRootDirectory)?
        };

        let cache_dir = match cache_dir {
            Some(cache_dir) => cache_dir,
            None => {
                find_cache_dir(whitelabel).ok_or_else(|| Error::CouldNotDetectCacheDirectory)?
            }
        };

        Ok(Paths {
            state_file: root.join("state.json"),
            proxy_dir: root.join("proxy"),
            installation_dir: root.join(DEFAULT_INSTALLATION_DIR_NAME),
            cache_dir,
            root,
        })
    }
}

fn find_root(whitelabel: &WhitelabelConfig) -> Option<PathBuf> {
    match env::var_os("CRITICALUP_ROOT") {
        Some(val) if val.is_empty() => platform_specific_root(whitelabel),
        Some(val) => Some(PathBuf::from(val)),
        None => platform_specific_root(whitelabel),
    }
}

fn platform_specific_root(whitelabel: &WhitelabelConfig) -> Option<PathBuf> {
    dirs::data_dir().map(|v| v.join(whitelabel.name))
}

fn find_cache_dir(whitelabel: &WhitelabelConfig) -> Option<PathBuf> {
    match env::var_os("CRITICALUP_CACHE_DIR") {
        Some(val) if val.is_empty() => platform_specific_cache_dir(whitelabel),
        Some(val) => Some(PathBuf::from(val)),
        None => platform_specific_cache_dir(whitelabel),
    }
}

fn platform_specific_cache_dir(whitelabel: &WhitelabelConfig) -> Option<PathBuf> {
    dirs::cache_dir().map(|v| v.join(whitelabel.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn not_empty(var: &Option<PathBuf>) -> Option<&PathBuf> {
        var.as_ref().filter(|path| !path.as_os_str().is_empty())
    }

    #[test]
    fn test_calculated_paths() {
        assert_eq!(
            Paths {
                state_file: "/opt/criticalup/state.json".into(),
                proxy_dir: "/opt/criticalup/proxy".into(),
                installation_dir: "/opt/criticalup/toolchains".into(),
                cache_dir: "/cache/criticalup".into(),
                root: "/opt/criticalup".into()
            },
            Paths::detect(
                &WhitelabelConfig::test(),
                Some("/opt/criticalup".into()),
                Some("/cache/criticalup".into())
            )
            .unwrap()
        );
    }

    #[test]
    fn test_with_explicit_criticalup_home() {
        let whitelabel1 = WhitelabelConfig::test();

        let mut whitelabel2 = WhitelabelConfig::test();
        whitelabel2.name = "test-name";

        // The value of the CRITICALUP_ROOT environment variable is respected regardless of the
        // whitelabel configuration.
        for whitelabel in &[whitelabel1, whitelabel2] {
            assert_root_is(
                "/opt/criticalup",
                whitelabel,
                Some("/opt/criticalup".into()),
            );

            assert_root_is("/foo/bar", whitelabel, Some("/foo/bar".into()));

            assert_root_is("foo", whitelabel, Some("foo".into()));

            // When the environment variable is empty we're not using it, so the rest of the
            // detection code is used, only works on Linux currently.
            assert_root_is_not("", whitelabel, Some("".into()));
        }
    }

    #[test]

    fn test_with_explicit_root() {
        let mut wl1 = WhitelabelConfig::test();
        wl1.name = "foo";

        let mut wl2 = WhitelabelConfig::test();
        wl2.name = "bar";

        assert_root_is(
            "/usr/local/share/foo",
            &wl1,
            Some("/usr/local/share/foo".into()),
        );
        assert_root_is(
            "/usr/local/share/bar",
            &wl2,
            Some("/usr/local/share/bar".into()),
        );
        assert_root_is("data/foo", &wl1, Some("data/foo".into()));
        assert_root_is("data/bar", &wl2, Some("data/bar".into()));
        assert_root_is(
            "/home/user/.local/share/foo",
            &wl1,
            Some("/home/user/.local/share/foo".into()),
        );
        assert_root_is(
            "/home/pietro/.local/share/bar",
            &wl2,
            Some("/home/pietro/.local/share/bar".into()),
        );

        // When the environment variable is empty we're not using it, so the rest of the
        // detection code is used.
        assert_root_is_not("foo", &wl1, Some("bar".into()));
        assert_root_is_not("bar", &wl2, Some("foo".into()));
    }

    #[test]
    fn test_not_empty() {
        assert_eq!(
            Some(&PathBuf::from("foo")),
            not_empty(&Some(PathBuf::from("foo")))
        );
        assert_eq!(None, not_empty(&Some(PathBuf::from(""))));
        assert_eq!(None, not_empty(&None));
    }

    fn assert_root_is(
        expected: impl AsRef<Path>,
        whitelabel: &WhitelabelConfig,
        root: Option<PathBuf>,
    ) {
        assert_eq!(
            expected.as_ref(),
            Paths::detect(whitelabel, root, None).unwrap().root
        );
    }

    fn assert_root_is_not(
        expected: impl AsRef<Path>,
        whitelabel: &WhitelabelConfig,
        root: Option<PathBuf>,
    ) {
        match Paths::detect(whitelabel, root, None) {
            Ok(paths) => assert_ne!(expected.as_ref(), paths.root),
            Err(err) => assert!(matches!(err, Error::CouldNotDetectRootDirectory)),
        }
    }
}
