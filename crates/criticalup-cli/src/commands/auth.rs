// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::{Error, LibError};
use crate::Context;
use criticalup_core::download_server_client::DownloadServerClient;
use criticalup_core::errors::DownloadServerError;
use criticalup_core::state::State;

pub(crate) fn run(ctx: &Context) -> Result<(), Error> {
    let state = State::load(&ctx.config)?;
    let download_server = DownloadServerClient::new(&ctx.config, &state);

    match download_server.get_current_token_data() {
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
