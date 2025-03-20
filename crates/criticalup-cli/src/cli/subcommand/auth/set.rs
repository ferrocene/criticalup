// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io;
use std::io::Write;

use crate::cli::CommandExecute;
use crate::errors::{Error, LibError};
use crate::Context;
use clap::Parser;
use criticalup_core::download_server_client::DownloadServerClient;
use criticalup_core::errors::DownloadServerError;
use criticalup_core::state::{AuthenticationToken, State};

/// Set the authentication token used to interact with the download server
#[derive(Debug, Parser)]
pub(crate) struct AuthSet {
    /// Authentication token to use; if not provided, it will be read from stdin
    token: Option<String>,
}

impl CommandExecute for AuthSet {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        let state = State::load(&ctx.config).await?;
        let download_server = DownloadServerClient::new(&ctx.config, &state);

        let token = if let Some(token) = self.token {
            token
        } else if is_tty(ctx, &io::stdin()) && is_tty(ctx, &io::stderr()) {
            token_from_stdin_interactive(ctx).map_err(Error::CantReadTokenFromStdin)?
        } else {
            token_from_stdin_programmatic().map_err(Error::CantReadTokenFromStdin)?
        };

        state.set_authentication_token(Some(AuthenticationToken::seal(&token)));

        tracing::trace!("Getting token data from server");
        match download_server.get_current_token_data().await {
            Ok(_) => {
                tracing::trace!("Got server response, persisting data");
                let res = state.persist().await?;
                tracing::trace!("Persisted data");
                Ok(res)
            },

            Err(LibError::DownloadServerError {
                kind: DownloadServerError::AuthenticationFailed,
                ..
            }) => Err(Error::InvalidAuthenticationToken),
            Err(err) => Err(err.into()),
        }
    }
}

fn token_from_stdin_interactive(ctx: &Context) -> Result<String, std::io::Error> {
    let mut stderr = std::io::stderr();
    let token_loc_message = format!(
        "Visit {}/{} to create a new token, then enter it below.\n",
        ctx.config.whitelabel.customer_portal_url, "users/tokens"
    );
    stderr.write_all(token_loc_message.as_bytes())?;
    stderr.write_all("enter the authentication token: ".as_bytes())?;
    stderr.flush()?;

    let mut token = String::new();
    std::io::stdin().read_line(&mut token)?;

    // `.trim_end()` can trim more than just the last newline.
    if token.ends_with('\n') {
        token.pop();
        if token.ends_with('\r') {
            token.pop();
        }
    } else {
        // Ensure a newline is printed even if the user terminated the line in another way (for
        // example with an EOF / Ctrl+D)
        stderr.write_all(b"\n")?;
    }

    Ok(token)
}

fn token_from_stdin_programmatic() -> Result<String, std::io::Error> {
    let mut token = String::new();
    std::io::stdin().read_line(&mut token)?;

    // `.trim_end()` can trim more than just the last newline.
    if token.ends_with('\n') {
        token.pop();
        if token.ends_with('\r') {
            token.pop();
        }
    }

    Ok(token)
}

fn is_tty(ctx: &Context, stream: &dyn std::io::IsTerminal) -> bool {
    if ctx.config.whitelabel.test_mode {
        // If the environment variable is set, pay attention to it
        if let Some(var) = std::env::var_os("CRITICALUP_TEST_MOCK_TTY") {
            if var == "1" {
                return true;
            } else if var == "0" {
                return false;
            } else {
                panic!("CRITICALUP_TEST_MOCK_TTY should only ever be 0 or 1, or unset");
            }
        }
    }
    stream.is_terminal()
}
