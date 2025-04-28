// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::cli::CommandExecute;
use crate::errors::Error;
use crate::Context;
use clap::Parser;

/// Output the path of the binary proxies
#[derive(Debug, Parser)]
pub(crate) struct LinkShow;

impl CommandExecute for LinkShow {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        let proxy_dir = &ctx.config.paths.proxy_dir;

        if !proxy_dir.exists() {
            return Err(Error::NoProxyDirectory);
        }

        println!("{}", proxy_dir.display());
        Ok(())
    }
}
