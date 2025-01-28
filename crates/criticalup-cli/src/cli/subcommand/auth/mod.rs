// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

mod remove;
mod set;

use crate::cli::CommandExecute;
use crate::errors::{Error, LibError};
use crate::Context;
use clap::{Parser, Subcommand};
use criticalup_core::download_server_client::DownloadServerClient;
use criticalup_core::errors::DownloadServerError;
use criticalup_core::state::State;
use remove::AuthRemove;
use set::AuthSet;

#[derive(Subcommand, Debug)]
pub(crate) enum AuthSubcommand {
    Set(AuthSet),
    Remove(AuthRemove),
}

/// Show and change authentication with the download server
#[derive(Debug, Parser)]
pub(crate) struct Auth {
    #[command(subcommand)]
    command: Option<AuthSubcommand>,
}

impl CommandExecute for Auth {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        match self.command {
            Some(AuthSubcommand::Remove(remove)) => return remove.execute(ctx).await,
            Some(AuthSubcommand::Set(set)) => return set.execute(ctx).await,
            None => (),
        };

        let state = State::load(&ctx.config).await?;
        let download_server = DownloadServerClient::new(&ctx.config, &state);

        match download_server.get_current_token_data().await {
            Ok(data) => {
                eprintln!("valid authentication token present");
                eprintln!();
                eprintln!("token name:         {}", data.name);
                eprintln!("organization name:  {}", data.organization_name);
                eprintln!(
                    "expires at:         {}",
                    data.expires_at.as_deref().unwrap_or("none")
                );

                Ok(())
            }
            Err(LibError::DownloadServerError {
                kind: DownloadServerError::AuthenticationFailed,
                ..
            }) => {
                eprintln!("error: failed to authenticate with the download server");
                eprintln!();
                eprintln!("The authentication token could be missing, invalid or expired.");
                eprintln!("You can set a new authentication token by running:");
                eprintln!();
                eprintln!("    criticalup auth set");
                eprintln!();

                Err(Error::Exit(1))
            }
            Err(err) => Err(err.into()),
        }
    }
}
