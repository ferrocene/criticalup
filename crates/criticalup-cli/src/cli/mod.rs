// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use instrumentation::Instrumentation;
use subcommand::CriticalupSubcommand;

use crate::{Context, Error};

pub(crate) mod connectivity;
pub(crate) mod instrumentation;
pub(crate) mod subcommand;

pub trait CommandExecute {
    #[allow(async_fn_in_trait)] // This is only used in a limited context for the CLI binaries.
    async fn execute(self, ctx: &Context) -> Result<(), Error>;
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Criticalup {
    #[command(subcommand)]
    command: CriticalupSubcommand,
    #[clap(flatten)]
    pub instrumentation: Instrumentation,
}

impl CommandExecute for Criticalup {
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        // Instrumentation set up in `main_inner`.

        match self.command {
            CriticalupSubcommand::Archive(archive) => archive.execute(ctx).await,
            CriticalupSubcommand::Auth(auth) => auth.execute(ctx).await,
            CriticalupSubcommand::Clean(clean) => clean.execute(ctx).await,
            CriticalupSubcommand::Doc(doc) => doc.execute(ctx).await,
            CriticalupSubcommand::Init(init) => init.execute(ctx).await,
            CriticalupSubcommand::Install(install) => install.execute(ctx).await,
            CriticalupSubcommand::Link(link) => link.execute(ctx).await,
            CriticalupSubcommand::Remove(remove) => remove.execute(ctx).await,
            CriticalupSubcommand::Run(run) => run.execute(ctx).await,
            CriticalupSubcommand::Verify(verify) => verify.execute(ctx).await,
            CriticalupSubcommand::Which(which) => which.execute(ctx).await,
        }
    }
}
