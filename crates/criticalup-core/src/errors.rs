// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use criticaltrust::Error as TrustError;
use reqwest::Error as ReqError;
use reqwest::StatusCode;
use std::path::PathBuf;

/// We're using a custom error enum instead of `Box<dyn Error>` or one of the crates providing a
/// `Box<dyn Error>` wrapper because we need to know all the possible errors criticalup could
/// encounter. Using `Box<dyn Error>` makes it too easy to accidentally bubble up a library error
/// without wrapping it into a criticalup-specific error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not detect the criticalup root directory.")]
    CouldNotDetectRootDirectory,

    #[error("Could not detect the criticalup cache directory.")]
    CouldNotDetectCacheDirectory,

    #[error("Failed to download {url}.")]
    DownloadServerError {
        url: String,
        #[source]
        kind: DownloadServerError,
    },

    #[error("Network access required, but in offline mode.")]
    OfflineMode,

    #[error("Creating `{}`.", .0.display())]
    Create(PathBuf, #[source] std::io::Error),

    #[error("Writing to `{}`.", .0.display())]
    Write(PathBuf, #[source] std::io::Error),

    #[error("Reading from `{}`.", .0.display())]
    Read(PathBuf, #[source] std::io::Error),

    #[error("JSON Serialization error")]
    JsonSerialization(#[from] serde_json::Error),

    #[error("State file at {} is not supported by this release (state format version {1}).", .0.display())]
    UnsupportedStateFileVersion(PathBuf, u32),
    #[error("Failed to read the criticalup state file at {}.", .0.display())]
    CantReadStateFile(PathBuf, #[source] std::io::Error),
    #[error("Failed to write the criticalup state file to {}.", .0.display())]
    CantWriteStateFile(PathBuf, #[source] WriteFileError),
    #[error("Failed to parse the criticalup state file at {}, is it corrupt?", .0.display())]
    CorruptStateFile(PathBuf, #[source] serde_json::Error),

    #[error("Could not find a project manifest in the current or parent directories.")]
    ProjectManifestDetectionFailed,
    #[error("Failed to load the project manifest at {}.", .path.display(),)]
    ProjectManifestLoadingFailed {
        path: PathBuf,
        #[source]
        // `Box`ing here is needed because of maintaining the size of errors.
        // Otherwise Clippy will tell you to try reducing the size of `errors::Error`.
        kind: Box<ProjectManifestLoadingError>,
    },
    #[error("Failed to create product directory for product {} at {}.", .product, .path.display())]
    ProjectManifestProductDirCreationFailed {
        path: PathBuf,
        product: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Installation {} does not exist; please run `criticalup install` again.", .0)]
    InstallationDoesNotExist(String),

    #[error("Failed to read the project directory; maybe it is missing?")]
    FailedToReadDirectory(#[source] std::io::Error),

    #[error("Failed to initialize the keychain used to verify signatures.")]
    KeychainInitFailed(#[source] TrustError),

    #[error("Unknown variable substitution: ${{{0}}}.")]
    UnknownVariableSubstitution(String),
    #[error("Unterminated variable.")]
    UnterminatedVariable,

    #[error(transparent)]
    Reqwest(#[from] ReqError),

    #[error("Failed to create request to the download server.")]
    RequestCloningFailed,

    #[error("Failed to find canonical path for {}.", path.display())]
    FailedToFindCanonicalPath {
        path: PathBuf,
        #[source]
        kind: std::io::Error,
    },

    #[error("Failed to load keys into keychain.")]
    KeychainLoadingFailed(#[source] criticaltrust::Error),

    #[error(
        "The `release` in your project manifest does not seem to be correct.\n  \
    If you want to specify latest release from a channel, please use '@' symbol as the first \
    character, then the channel name, then the symbol '/', and then the word 'latest'.\n  \
    Your `release`: .release"
    )]
    MalformedReleaseNameRedirect { release: String },
}

#[derive(Debug, thiserror::Error)]
pub enum WriteFileError {
    #[error(transparent)]
    Io(std::io::Error),
    #[error("Failed to create the parent directory.")]
    CantCreateParentDirectory(#[source] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadServerError {
    #[error("Failed to authenticate; missing or wrong authentication token.")]
    AuthenticationFailed,
    #[error("Resource not found.")]
    NotFound,
    #[error("Invalid request sent to the server.")]
    BadRequest,
    #[error("Too many requests, please try later (rate limited).")]
    RateLimited,
    #[error("Internal error occurred on the download server (status code {0}).")]
    InternalServerError(StatusCode),
    #[error("The response from the download server was not expected (status code {0}).")]
    UnexpectedResponseStatus(StatusCode),
    #[error("The contents in the response from the download server were not expected.")]
    UnexpectedResponseData(#[source] serde_json::Error),
    #[error("Failed to send the network request.")]
    Network(#[source] reqwest::Error),
    #[error("Failed to send the network request.")]
    NetworkWithMiddleware(#[source] reqwest_middleware::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectManifestLoadingError {
    #[error("Failed to read the file.")]
    FailedToRead(#[source] std::io::Error),
    #[error("Failed to parse.")]
    FailedToParse(#[source] toml_edit::de::Error),

    #[error(
        "Current version of criticalup does not support multiple products; found {0} products."
    )]
    MultipleProductsNotSupportedInProjectManifest(usize),

    #[error("The `manifest-version` in your project manifest \
        is smaller than what this release of criticalup supports.\n  \
        Please change the `manifest-version` to {}.\n  \
        Your project manifest version: {}.",
    .default_supported_version,
    .user_version,
    )]
    ManifestVersionTooSmall {
        user_version: u32,
        default_supported_version: u32,
    },

    #[error("The `manifest-version` in your project manifest \
        is greater than what this release of criticalup supports.\n  \
        Please update criticalup to the latest version.\n  \
        Your project manifest version: {}.",
    .user_version,
    )]
    ManifestVersionTooBig { user_version: u32 },

    #[error("The 'packages' list for product '{}' in your project manifest is empty. \
    Please provide at least one package in the 'packages' list.", .product_name)]
    MissingPackagesInManifestProduct { product_name: String },

    #[error("Unknown substitution variable: ${{{0}}}.")]
    UnknownVariableInSubstitution(String),
    #[error("Unterminated substitution.")]
    UnterminatedVariableInSubstitution,
}

#[derive(Debug, thiserror::Error)]
pub enum BinaryProxyUpdateError {
    #[error("Failed to list the {} directory.", .0.display())]
    ListDirectoryFailed(PathBuf, #[source] std::io::Error),
    #[error("Failed to inspect {}.", .0.display())]
    InspectFailed(PathBuf, #[source] std::io::Error),
    #[error("Failed to remove unexpected path {}.", .0.display())]
    UnexpectedPathRemovalFailed(PathBuf, #[source] std::io::Error),
    #[error("Failed to create a symlink from {} to {}.", .source.display(), .dest.display())]
    SymlinkFailed {
        source: PathBuf,
        dest: PathBuf,
        #[source]
        inner: std::io::Error,
    },
    #[error("Failed to create the parent directory {}.", .0.display())]
    ParentDirectoryCreationFailed(PathBuf, #[source] std::io::Error),
}
