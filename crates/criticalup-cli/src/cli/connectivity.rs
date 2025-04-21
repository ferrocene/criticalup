// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::{
    builder::{BoolishValueParser, TypedValueParser},
    ArgAction,
};
use criticalup_core::download_server_client::Connectivity;

#[derive(clap::Args, Debug)]
pub(crate) struct Network {
    /// Don't download from the server, only use previously cached artifacts
    #[arg(long = "offline", action = ArgAction::SetTrue, value_parser = BoolishValueParser::new()
        .map(|v| -> Connectivity {
            if v { Connectivity::Offline } else { Connectivity::Online }
    }))]
    pub connectivity: Connectivity,
}
