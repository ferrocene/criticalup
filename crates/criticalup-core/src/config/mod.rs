// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

mod paths;

use self::paths::Paths;
use crate::errors::Error;
use criticaltrust::keys::PublicKey;

/// The `Config` struct holds all the configuration of criticalup. It's meant to be created early
/// and passed around the rest of the code.
pub struct Config {
    /// Details about the binary. See [`WhitelabelConfig`] for more information.
    pub whitelabel: WhitelabelConfig,
    /// File system paths criticalup should access. The rest of the code should use the paths
    /// provided by the struct instead of constructing their own. This is for `criticalup`
    /// binary itself, and not for other tools outside this crate.
    pub paths: Paths,
}

impl Config {
    /// Detect and load the criticalup configuration from the execution environment.
    pub fn detect(whitelabel: WhitelabelConfig) -> Result<Self, Error> {
        Self::detect_inner(whitelabel, None, None)
    }

    fn detect_inner(
        whitelabel: WhitelabelConfig,
        root: Option<std::path::PathBuf>,
        cache_dir: Option<std::path::PathBuf>,
    ) -> Result<Self, Error> {
        let paths = Paths::detect(&whitelabel, root, cache_dir)?;
        Ok(Self { whitelabel, paths })
    }

    #[cfg(test)]
    pub(crate) fn test(
        root: std::path::PathBuf,
        cache_dir: std::path::PathBuf,
    ) -> Result<Self, Error> {
        Self::detect_inner(WhitelabelConfig::test(), Some(root), Some(cache_dir))
    }
}

/// CriticalUp supports the creation of multiple "whitelabeled" binaries, each with their own
/// configuration. Binaries are expected to configure their own details in this struct, and pass
/// it to the library. The configuration is not supposed to be dynamically set at runtime.
pub struct WhitelabelConfig {
    /// Name of the program. This influences both the way the binary expects to be called, and the
    /// name of the data directory on disk.
    pub name: &'static str,

    /// User agent to use when making HTTP/HTTPS requests.
    pub http_user_agent: &'static str,
    /// URL of the download server criticalup should use.
    pub download_server_url: String,
    /// URL of the customer portal that user's of criticalup need to set tokens etc.
    pub customer_portal_url: String,

    /// Public key used to verify all other public keys imported from the download server.
    pub trust_root: PublicKey,

    /// Whether test mocking functionality should be enabled for this binary. Must be `false` on
    /// all production criticalup builds, as it's supposed to be used only during tests.
    pub test_mode: bool,
}

impl WhitelabelConfig {
    #[cfg(test)]
    fn test() -> Self {
        use criticaltrust::keys::newtypes::PublicKeyBytes;
        use criticaltrust::keys::{KeyAlgorithm, KeyRole};

        WhitelabelConfig {
            name: "criticalup",

            http_user_agent: "criticalup test suite (https://github.com/ferrocene/criticalup)",
            download_server_url: "http://0.0.0.0:0".into(),
            customer_portal_url: "https://customers-dev.ferrocene.dev".into(),

            // Intentionally broken public key. If a test wants to use a real trust root it needs
            // to override the key with a real one (ideally through TestEnvironment).
            trust_root: PublicKey {
                role: KeyRole::Root,
                algorithm: KeyAlgorithm::Unknown,
                expiry: None,
                public: PublicKeyBytes::borrowed(&[]),
            },

            test_mode: true,
        }
    }
}
