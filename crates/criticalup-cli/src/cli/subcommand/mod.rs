// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use archive::Archive;
use auth::Auth;
use clap::Subcommand;
use clean::Clean;
use doc::Doc;
use install::Install;
use remove::Remove;
use run::Run;
use verify::Verify;
use which::Which;

pub(crate) mod archive;
pub(crate) mod auth;

pub(crate) mod clean;
pub(crate) mod doc;
pub(crate) mod install;
pub(crate) mod remove;
pub(crate) mod run;
pub(crate) mod verify;
pub(crate) mod which;

#[derive(Subcommand, Debug)]
pub(crate) enum CriticalupSubcommand {
    Archive(Archive),
    Auth(Auth),
    Clean(Clean),
    Doc(Doc),
    Install(Install),
    Remove(Remove),
    Run(Run),
    Verify(Verify),
    Which(Which),
}
