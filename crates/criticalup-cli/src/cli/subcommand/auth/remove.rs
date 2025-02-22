// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::cli::CommandExecute;
use crate::errors::Error;
use crate::Context;
use clap::Parser;
use criticalup_core::state::State;

/// Remove the authentication token used to interact with the download server
#[derive(Debug, Parser)]
pub(crate) struct AuthRemove;

impl CommandExecute for AuthRemove {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        let state = State::load(&ctx.config).await?;

        if state.authentication_token(None).await.is_some() {
            state.set_authentication_token(None);
            state.persist().await?;
        }

        Ok(())
    }
}
