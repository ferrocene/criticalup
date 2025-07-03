// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::cli::CommandExecute;
use crate::errors::Error;
use crate::Context;
use clap::Parser;
use criticalup_core::project_manifest::ProjectManifest;
use std::path::PathBuf;
use url::Url;

///  Open the documentation for the current toolchain
#[derive(Debug, Parser)]
pub(crate) struct Doc {
    /// Path to the manifest `criticalup.toml`
    #[arg(long)]
    project: Option<PathBuf>,
    /// Only print the path to the documentation location
    #[arg(long)]
    path: bool,
}

impl CommandExecute for Doc {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn execute(self, ctx: &Context) -> Result<(), Error> {
        // Parse and serialize the project manifest.
        let manifest = ProjectManifest::get(self.project).await?;
        let installation_dir = &ctx.config.paths.installation_dir;

        for product in manifest.products() {
            let doc_package_exists_in_manifest =
                product.packages().contains(&"ferrocene-docs".to_string());
            let abs_ferrocene_html_doc_path = installation_dir
                .join(product.installation_id())
                .join("share/doc/")
                .join(product.name())
                .join("html/index.html");

            if !doc_package_exists_in_manifest || !abs_ferrocene_html_doc_path.exists() {
                return Err(Error::MissingDocPackage());
            }

            // Path to the doc root can be clickable so we try to print that.
            match Url::from_file_path(abs_ferrocene_html_doc_path.clone()) {
                Ok(url) => {
                    let url = url.to_string();
                    if self.path {
                        println!("{}", url);
                    } else {
                        // Open in the default browser.
                        tracing::info!(
                            "Opening docs in your browser for product '{}'.",
                            product.name()
                        );
                        opener::open_browser(abs_ferrocene_html_doc_path.clone())
                            .map_err(|err| Error::FailedToOpenDoc { url, kind: err })?
                    }
                }
                Err(_) => {
                    println!("{}", abs_ferrocene_html_doc_path.display());
                }
            }
        }

        Ok(())
    }
}
