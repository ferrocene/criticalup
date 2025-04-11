// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::ErrorKind;

use crate::cli::CommandExecute;
use crate::errors::Error;
use crate::Context;
use clap::Parser;
use tokio::process::Command;

/// Remove the authentication token used to interact with the download server
#[derive(Debug, Parser)]
pub(crate) struct LinkRemove;

impl CommandExecute for LinkRemove {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        let proxy_dir = &ctx.config.paths.proxy_dir;

        if !proxy_dir.exists() {
            return Err(Error::NoProxyDirectory);
        }

        let mut rustup_command = Command::new("rustup");
        rustup_command.arg("toolchain");
        rustup_command.arg("remove");
        rustup_command.arg("ferrocene");

        tracing::debug!("Running `{:?}`", rustup_command.as_std());
        let res = rustup_command.output().await;

        match res {
            Ok(output) if !output.status.success() => {
                let command_string = format!("{:?}", rustup_command.as_std());
                Err(Error::CommandExitNonzero(command_string, output))
            }
            Err(err) if err.kind() == ErrorKind::NotFound => Err(Error::RustupMissing),
            Err(err) => {
                let command_string = format!("{:?}", rustup_command.as_std());
                Err(Error::CommandFailed(command_string, err))
            }
            Ok(_) => {
                tracing::info!(
                    "The `ferrocene` rustup toolchain has been removed, or did not exist"
                );
                Ok(())
            }
        }
    }
}
