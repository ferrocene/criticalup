// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

mod create;
mod remove;
mod show;

use crate::cli::CommandExecute;
use crate::errors::Error;
use crate::Context;
use clap::{Parser, Subcommand};

use create::LinkCreate;
use remove::LinkRemove;
use show::LinkShow;

#[derive(Subcommand, Debug)]
pub(crate) enum LinkSubcommand {
    Show(LinkShow),
    Create(LinkCreate),
    Remove(LinkRemove),
}

/// Manage `rustup` toolchain linking support
#[derive(Debug, Parser)]
pub(crate) struct Link {
    #[command(subcommand)]
    command: LinkSubcommand,
}

impl CommandExecute for Link {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        match self.command {
            LinkSubcommand::Show(show) => return show.execute(ctx).await,
            LinkSubcommand::Create(create) => return create.execute(ctx).await,
            LinkSubcommand::Remove(remove) => return remove.execute(ctx).await,
        }
    }
}
