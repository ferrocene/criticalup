// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use criticaltrust::integrity::IntegrityError;
pub(crate) use criticaltrust::Error as TrustError;
pub(crate) use criticalup_core::errors::BinaryProxyUpdateError;
pub(crate) use criticalup_core::errors::Error as LibError;
use std::path::PathBuf;
use std::process::Output;
use std::string::FromUtf8Error as Utf8Error;
use tokio::task::JoinError;

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
    WalkDir(#[from] walkdir::Error),
    #[error(transparent)]
    Join(#[from] JoinError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    JoinPaths(#[from] std::env::JoinPathsError),

    #[error("Exiting with code {0}.")]
    Exit(i32),
    #[error("Failed to parse command line arguments.")]
    CliArgumentParsing(#[source] clap::Error),

    #[error("Failed to read the token from stdin.")]
    CantReadTokenFromStdin(#[source] std::io::Error),
    #[error("Invalid authentication token provided.")]
    InvalidAuthenticationToken,

    #[error("Some files did not pass the integrity checks after the download.\n \
        Please clean your installation directory and re-install the project again.\n \
        The following errors were found:\n\n{}",
      .0.iter().map(|err| { err.to_string() }).collect::<Vec<_>>().join("\n")
    )]
    IntegrityErrorsWhileInstallation(Vec<IntegrityError>),

    #[error("Some files did not pass the integrity checks during verification.\n \
        Please clean your installation directory and re-install the project again.\n \
        The following errors were found:\n\n{}",
        .0.iter().map(|err| { err.to_string() }).collect::<Vec<_>>().join("\n")
    )]
    IntegrityErrorsWhileVerifying(Vec<IntegrityError>),
    #[error("Some files did not pass the integrity checks during archiving.\n \
        The following errors were found:\n\n{}",
        .0.iter().map(|err| { err.to_string() }).collect::<Vec<_>>().join("\n")
    )]
    IntegrityErrorsWhileArchiving(Vec<IntegrityError>),

    #[error("arg0 is not encoded in UTF-8")]
    NonUtf8Arg0,
    #[error("Failed to invoke proxied command {}.", .0.display())]
    FailedToInvokeProxiedCommand(PathBuf, #[source] std::io::Error),
    #[error(
        "'{0}' is not installed for this project.\n\n\
    Please make sure that the correct package for '{0}' is listed in the packages section of your \
    project's criticalup.toml and run 'criticalup install' command again.\n"
    )]
    BinaryNotInstalled(String),

    #[error(
        "Ambiguous binary specified while in strict mode. This is an unexpected error as no \
        valid products have duplicated binaries. Please report this. \n
        \n
        Ambiguous binaries: {}\n
        \n
        To work around this, pass one of the absolute paths to the command and disable strict mode.
    ", .0.iter().map(|v| format!("{}", v.display())).collect::<Vec<_>>().join(", "))]
    BinaryAmbiguous(Vec<PathBuf>),

    #[error(
        "Strict mode does not handle multi-component paths. Pass just a binary name, eg `rustc`."
    )]
    StrictModeDoesNotAcceptPaths,

    // This is not *technically* needed, but it provides useful insights when an error happens when
    // invoking a binary proxy. Otherwise people could think the error comes from rustc/cargo/etc.
    #[error("criticalup could not invoke the binary you requested")]
    BinaryProxyInvocationFailed(#[source] Box<Error>),

    #[error(
        "Dependencies are not supported in the current criticalup release.\n \
            Found package {0} with dependencies in the manifest.\n \
            Please updated criticalup to the latest version to resolve this error."
    )]
    PackageDependenciesNotSupported(String),

    #[error("There was an error while trying to delete the unused installation directory at {}.", path.display())]
    DeletingUnusedInstallationDir {
        path: PathBuf,
        #[source]
        kind: std::io::Error,
    },

    #[error("There was an error while trying to delete the untracked installation directory at {}.", path.display())]
    DeletingUntrackedInstallationDir {
        path: PathBuf,
        #[source]
        kind: std::io::Error,
    },

    #[error("Parsing tracing directives.")]
    EnvFilter(
        #[from]
        #[source]
        tracing_subscriber::filter::ParseError,
    ),

    #[error("Parsing tracing directives from environment.")]
    FromEnv(
        #[from]
        #[source]
        tracing_subscriber::filter::FromEnvError,
    ),

    #[error("Initializing tracing.")]
    TryInit(
        #[from]
        #[source]
        tracing_subscriber::util::TryInitError,
    ),

    #[error("Failed to open document in the browser at {}", url)]
    FailedToOpenDoc {
        url: String,
        #[source]
        kind: opener::OpenError,
    },

    #[error(
        "Package 'ferrocene-docs' is not installed for this project. Please add the package \
    'ferrocene-docs' to the project manifest and run 'criticalup install' again."
    )]
    MissingDocPackage(),

    #[error("Manifest serialization failed.")]
    TomlSerializationFailed(#[from] toml_edit::ser::Error),

    #[error(
        "A 'criticalup.toml' already exists in the current directory.\n\
    Please delete or move that file and run the 'criticalup init' command again."
    )]
    ManifestAlreadyExists(),

    #[error(
        "\
        An installation corresponding to `{}` was not found.\n\
        Please ensure your `criticalup.toml` contains what you expect, and then\n\
        run `criticalup install` to complete the installation.
    ", .0.display()
    )]
    InstallationNotFound(PathBuf),

    #[error("Current directory not found.")]
    CurrentDirectoryNotFound,

    #[error("No proxies have been created, it's likely `criticalup install` has not been run")]
    NoProxyDirectory,

    // Rather than a generic "not found" error here, we provide a nice breadcrumb
    #[error("No `rustup` executable found, either install it from https://rustup.rs/, or add the ouptut of `criticalup link show` to your $PATH")]
    RustupMissing,

    #[error("Failed to run command `{}`", .0)]
    CommandFailed(String, #[source] std::io::Error),

    #[error("Non-successful exit for command `{}`, stderr: \n{}", .0, String::from_utf8_lossy(&.1.stderr))]
    CommandExitNonzero(String, Output),

    #[cfg(windows)]
    #[error("Could not set Ctrl-C handler.")]
    CtrlHandler,
}
