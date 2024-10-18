// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

mod arg;
mod binary_proxies;
mod commands;
mod errors;
mod spawn;

use crate::errors::Error;
use clap::{command, Command, CommandFactory, FromArgMatches, Parser, Subcommand};
use criticalup_core::config::Config;
pub use criticalup_core::config::WhitelabelConfig;
use std::ffi::OsString;
use std::path::PathBuf;

/// Use a custom help template to solve some issues with Clap's default one, namely the
/// command-subcommand-subsubcomand at the top of each heading.
///
/// The syntax is available in the documentation for [`clap::Command::help_template`].
const HELP_TEMPLATE: &str = "{about}\n\n{usage-heading}\n{tab}{usage}\n\n{all-args}";

async fn main_inner(whitelabel: WhitelabelConfig, args: &[OsString]) -> Result<(), Error> {
    let arg0 = binary_proxies::arg0(&whitelabel)?;
    #[cfg(windows)]
    let arg0 = arg0
        .strip_suffix(".exe")
        .map(|v| v.to_string())
        .unwrap_or(arg0);

    if arg0 != whitelabel.name {
        return binary_proxies::proxy(whitelabel)
            .await
            .map_err(|e| Error::BinaryProxyInvocationFailed(Box::new(e)));
    }

    let mut command = Cli::command().name(whitelabel.name);
    override_help_template(&mut command);

    let matches = command
        .try_get_matches_from(args)
        .map_err(Error::CliArgumentParsing)?;

    let cli = Cli::from_arg_matches(&matches).map_err(Error::CliArgumentParsing)?;

    cli.instrumentation.setup(whitelabel.name).await?;

    let config = Config::detect(whitelabel)?;
    let ctx = Context { config };

    tracing::trace!(command = ?cli.commands, "Got command");
    match cli.commands {
        Commands::Auth { commands } => match commands {
            Some(AuthCommands::Set { token }) => commands::auth_set::run(&ctx, token).await?,
            Some(AuthCommands::Remove) => commands::auth_remove::run(&ctx).await?,
            None => commands::auth::run(&ctx).await?,
        },
        Commands::Install {
            project,
            reinstall,
            offline,
        } => commands::install::run(&ctx, reinstall, offline, project).await?,
        Commands::Clean => commands::clean::run(&ctx).await?,
        Commands::Remove { project } => commands::remove::run(&ctx, project).await?,
        Commands::Run { command, project } => commands::run::run(&ctx, command, project).await?,
        Commands::Verify { project, offline } => {
            commands::verify::run(&ctx, project, offline).await?
        }
        Commands::Which {
            binary: tool,
            project,
        } => commands::which::run(&ctx, tool, project).await?,
    }

    Ok(())
}

pub async fn main(whitelabel: WhitelabelConfig, args: &[OsString]) -> i32 {
    match main_inner(whitelabel, args).await {
        Ok(()) => 0,
        Err(Error::Exit(code)) => code,
        Err(Error::CliArgumentParsing(err)) => {
            eprint!("{err}");
            match err.kind() {
                clap::error::ErrorKind::DisplayHelp => 0,
                clap::error::ErrorKind::DisplayVersion => 0,
                _ => 1,
            }
        }
        Err(err) => {
            eprintln!("error: {err}");

            let mut err = &err as &dyn std::error::Error;
            while let Some(source) = err.source() {
                err = source;
                eprintln!("  caused by: {source}");
            }

            1
        }
    }
}

/// There is no Clap option to set the global help template, it has to be set for each individual
/// command and subcommand. Since that's error-prone this function updates all subcommands after
/// the fact to set the correct template.
fn override_help_template(command: &mut Command) {
    *command = command.clone().help_template(HELP_TEMPLATE);
    for subcommand in command.get_subcommands_mut() {
        override_help_template(subcommand);
    }
}

struct Context {
    config: Config,
}

/// CriticalUp is the official tool to download and install Ferrocene.
#[derive(Parser, Debug)]
#[command(name = "criticalup-cli")]
#[command(author, version, about, long_about = None, disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    commands: Commands,
    #[clap(flatten)]
    pub(crate) instrumentation: arg::Instrumentation,
}

#[derive(Debug, Subcommand, Clone)]
enum Commands {
    /// Show and change authentication with the download server
    Auth {
        #[command(subcommand)]
        commands: Option<AuthCommands>,
    },
    /// Install the toolchain for the given project based on the manifest `criticalup.toml`
    Install {
        /// Path to the manifest `criticalup.toml`
        #[arg(long)]
        project: Option<PathBuf>,
        /// Reinstall products that may have already been installed
        #[arg(long)]
        reinstall: bool,
        /// Don't download from the server, only use previously cached artifacts
        #[arg(long)]
        offline: bool,
    },

    /// Delete all unused and untracked installations
    Clean,

    /// Run a command for a given toolchain
    Run {
        /// Command with possible args to run
        #[clap(trailing_var_arg = true, required = true)]
        command: Vec<String>,

        /// Path to the manifest `criticalup.toml`
        #[arg(long)]
        project: Option<PathBuf>,
    },

    /// Delete all the products specified in the manifest `criticalup.toml`
    Remove {
        /// Path to the manifest `criticalup.toml`
        #[arg(long)]
        project: Option<PathBuf>,
    },

    /// Verify a given toolchain
    Verify {
        /// Don't download from the server, only use previously cached artifacts
        #[arg(long)]
        offline: bool,

        /// Path to the manifest `criticalup.toml`
        #[arg(long)]
        project: Option<PathBuf>,
    },

    /// Display which binary will be run for a given command
    Which {
        /// Name of the binary to find the absolute path of
        binary: String,
        /// Path to the manifest `criticalup.toml`
        #[arg(long)]
        project: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand, Clone)]
enum AuthCommands {
    /// Remove the authentication token used to interact with the download server
    Remove,
    /// Set the authentication token used to interact with the download server
    Set {
        /// Authentication token to use; if not provided, it will be read from stdin
        token: Option<String>,
    },
}
