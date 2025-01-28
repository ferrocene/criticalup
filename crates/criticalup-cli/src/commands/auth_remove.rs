// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::Error;
use crate::Context;
use criticalup_core::state::{EnvVars, State};

pub(crate) async fn run(ctx: &Context) -> Result<(), Error> {
    let state = State::load(&ctx.config).await?;

    let env_vars = EnvVars::read().await?;

    if state.authentication_token(None, &env_vars).await.is_some() {
        state.set_authentication_token(None);
        state.persist().await?;
    }

    Ok(())
}
