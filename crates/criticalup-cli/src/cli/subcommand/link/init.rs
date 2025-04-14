// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::ErrorKind;

use crate::cli::CommandExecute;
use crate::errors::Error;
use crate::Context;
use clap::Parser;
use tokio::process::Command;

/// Run a `rustup toolchain link` command to create a `ferrocene` toolchain
#[derive(Debug, Parser)]
pub(crate) struct LinkCreate;

impl CommandExecute for LinkCreate {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        let proxy_dir = &ctx.config.paths.proxy_dir;

        if !proxy_dir.exists() {
            return Err(Error::NoProxyDirectory);
        }

        let mut rustup_command = Command::new("rustup");
        rustup_command.arg("toolchain");
        rustup_command.arg("link");
        rustup_command.arg("ferrocene");
        rustup_command.arg(proxy_dir);

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
                tracing::info!("You can now use `ferrocene` as a rustup toolchain, for example, `cargo +ferrocene build`");
                Ok(())
            }
        }
    }
}
