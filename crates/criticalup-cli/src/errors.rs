// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use criticaltrust::integrity::IntegrityError;
pub(crate) use criticaltrust::Error as TrustError;
pub(crate) use criticalup_core::errors::BinaryProxyUpdateError;
pub(crate) use criticalup_core::errors::Error as LibError;
use std::path::PathBuf;
use std::string::FromUtf8Error as Utf8Error;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Lib(#[from] LibError),
    #[error(transparent)]
    BinaryProxyUpdate(#[from] BinaryProxyUpdateError),
    #[error(transparent)]
    Trust(#[from] TrustError),
    #[error(transparent)]
    Utf8(#[from] Utf8Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    JoinPaths(#[from] std::env::JoinPathsError),

    #[error("exiting with code {0}")]
    Exit(i32),
    #[error("failed to parse command line arguments")]
    CliArgumentParsing(#[source] clap::Error),

    #[error("failed to read the token from stdin")]
    CantReadTokenFromStdin(#[source] std::io::Error),
    #[error("invalid authentication token provided")]
    InvalidAuthenticationToken,

    #[error("some files did not pass the integrity checks after the download\n \
        please clean your installation directory and re-install the project again\n \
        the following errors were found:\n\n{}",
      .0.iter().map(|err| { err.to_string() }).collect::<Vec<_>>().join("\n")
    )]
    IntegrityErrorsWhileInstallation(Vec<IntegrityError>),

    #[error(transparent)]
    MissingRevocationInfo(#[from] IntegrityError),

    #[error("arg0 is not encoded in UTF-8")]
    NonUtf8Arg0,
    #[error("failed to invoke proxied command {}", .0.display())]
    FailedToInvokeProxiedCommand(PathBuf, #[source] std::io::Error),
    #[error(
        "'{0}' is not installed for this project.\n\n\
    Please make sure that the correct package for '{0}' is listed in the packages section of your \
    project's criticalup.toml and run 'criticalup install' command again.\n"
    )]
    BinaryNotInstalled(String),

    // This is not *technically* needed, but it provides useful insights when an error happens when
    // invoking a binary proxy. Otherwise people could think the error comes from rustc/cargo/etc.
    #[error("criticalup could not invoke the binary you requested")]
    BinaryProxyInvocationFailed(#[source] Box<Error>),

    #[error(
        "dependencies are not supported in the current criticalup release.\n \
            found package {0} with dependencies in the manifest.\n \
            please updated criticalup to the latest version to resolve this error."
    )]
    PackageDependenciesNotSupported(String),

    #[error("there was an error while trying to delete the unused installation directory at {}", path.display())]
    DeletingUnusedInstallationDir {
        path: PathBuf,
        #[source]
        kind: std::io::Error,
    },

    #[error("there was an error while trying to delete the untracked installation directory at {}", path.display())]
    DeletingUntrackedInstallationDir {
        path: PathBuf,
        #[source]
        kind: std::io::Error,
    },

    #[error("Parsing tracing directives")]
    EnvFilter(
        #[from]
        #[source]
        tracing_subscriber::filter::ParseError,
    ),

    #[error("Parsing tracing directives from environment")]
    FromEnv(
        #[from]
        #[source]
        tracing_subscriber::filter::FromEnvError,
    ),

    #[error("Initializing tracing")]
    TryInit(
        #[from]
        #[source]
        tracing_subscriber::util::TryInitError,
    ),

    #[error("failed to install package '{}' because it has been revoked", .0)]
    RevocationCheckFailed(String, #[source] criticaltrust::Error),

    #[cfg(windows)]
    #[error("Could not set Ctrl-C handler.")]
    CtrlHandler,
}
