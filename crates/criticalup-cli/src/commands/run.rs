// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::commands::which::locate_binary;
use crate::errors::Error;
use crate::spawn::spawn_command;
use crate::Context;
use std::path::PathBuf;
// We *deliberately* use a sync Command here, since we are spawning a process to replace the current one.
use std::process::{Command, Stdio};

pub(crate) async fn run(
    ctx: &Context,
    command: Vec<String>,
    project: Option<PathBuf>,
) -> Result<(), Error> {
    let binary = if let Some(binary) = command.first() {
        binary.clone()
    } else {
        return Err(Error::BinaryNotInstalled(String::new()));
    };
    let found_binary = locate_binary(ctx, binary, project).await?;

    let args = command.get(1..).unwrap_or(&[]);
    let mut cmd = Command::new(found_binary);
    cmd.args(args)
        .stdout(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    spawn_command(cmd)?;

    Ok(())
}
