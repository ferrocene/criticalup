// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::Error;
use crate::Context;
use criticalup_core::state::State;

pub(crate) async fn run(ctx: &Context) -> Result<(), Error> {
    let state = State::load(&ctx.config)?;

    if state.authentication_token(None).is_some() {
        state.set_authentication_token(None);
        state.persist().await?;
    }

    Ok(())
}
