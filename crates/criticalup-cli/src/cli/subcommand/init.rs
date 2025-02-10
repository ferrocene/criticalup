// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::cli::CommandExecute;
use crate::errors::Error;
use crate::Context;
use clap::Parser;
use criticalup_core::project_manifest::v1::sample_manifest;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};

/// Create a manifest file (criticalup.toml) inside current directory
#[derive(Debug, Parser)]
pub(crate) struct Init {
    /// Release version string of Ferrocene for this manifest
    #[arg(long)]
    release: Option<String>,
    /// Only print the contents of manifest instead of saving to file
    #[arg(long)]
    print_only: bool,
}

impl CommandExecute for Init {
    async fn execute(self, _ctx: &Context) -> Result<(), Error> {
        let current_dir = std::env::current_dir()?;
        let manifest_file_name = "criticalup.toml".to_string();
        let manifest_path = current_dir.join(manifest_file_name);

        let manifest_content = sample_manifest(self.release);
        let manifest_content_serialized = toml_edit::ser::to_string_pretty(&manifest_content)?;

        if self.print_only {
            println!("{}", manifest_content_serialized);
        } else {
            let mut manifest_file = BufWriter::new(File::create(&manifest_path).await?);
            manifest_file
                .write_all(manifest_content_serialized.as_bytes())
                .await?;
            manifest_file.flush().await?;
            tracing::info!("Created project manifest at {}", &manifest_path.display());
        }

        Ok(())
    }
}
