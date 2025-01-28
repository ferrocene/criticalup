// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

mod binary_proxies;
mod cli;
mod errors;
mod spawn;

use crate::errors::Error;
use clap::{Command, CommandFactory, FromArgMatches};
use cli::{CommandExecute, Criticalup};
use criticalup_core::config::Config;
pub use criticalup_core::config::WhitelabelConfig;
use std::ffi::OsString;

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

    let mut command = Criticalup::command().name(whitelabel.name);
    override_help_template(&mut command);

    let matches = command
        .try_get_matches_from(args)
        .map_err(Error::CliArgumentParsing)?;

    let cli = Criticalup::from_arg_matches(&matches).map_err(Error::CliArgumentParsing)?;

    cli.instrumentation.setup(whitelabel.name).await?;

    let config = Config::detect(whitelabel)?;
    let ctx = Context { config };

    cli.execute(&ctx).await
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
