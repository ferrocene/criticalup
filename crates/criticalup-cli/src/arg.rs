// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::IsTerminal;

use tracing_subscriber::{
    filter::Directive, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

#[derive(clap::Args, Debug)]
pub(crate) struct Instrumentation {
    /// Enable debug logs, -vv for trace
    #[clap(
        short = 'v',
        long,
        action = clap::ArgAction::Count,
        global = true,
        group = "verbosity",
        conflicts_with = "log_level",
    )]
    pub(crate) verbose: u8,
    /// Tracing directives
    #[clap(long, global = true, group = "verbosity", value_delimiter = ',', num_args = 0.., conflicts_with = "verbose")]
    pub(crate) log_level: Vec<Directive>,
}

impl<'a> Instrumentation {
    pub(crate) fn log_level(&self) -> String {
        match self.verbose {
            0 => "info",
            1 => "debug",
            _ => "trace",
        }
        .to_string()
    }

    pub(crate) async fn setup<'b: 'a>(&'b self, binary_name: &str) -> Result<(), crate::Error> {
        let fmt_layer = self.fmt_layer();
        let filter_layer = self.filter_layer(binary_name)?;

        tracing_subscriber::registry()
            .with(filter_layer)
            .with(fmt_layer)
            .try_init()?;

        tracing::trace!("Instrumentation initialized");

        Ok(())
    }

    pub(crate) fn fmt_layer<S>(&self) -> impl tracing_subscriber::layer::Layer<S>
    where
        S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
    {
        tracing_subscriber::fmt::Layer::new()
            .with_ansi(std::io::stderr().is_terminal())
            .with_writer(std::io::stderr)
            .pretty()
    }

    pub(crate) fn filter_layer(&self, binary_name: &str) -> Result<EnvFilter, crate::Error> {
        // If users pass `--log-level` with directives, we assume exactly those are what they want,
        // and do not infer from defaults or `-vvv` args.
        let mut filter_layer = if self.log_level.is_empty() {
            let from_verbosity = format!(
                "{}={level},criticalup_cli={level},criticaltrust={level},criticalup={level},criticalup_core={level}",
                binary_name,
                level=self.log_level(),
            );
            EnvFilter::try_new(from_verbosity)?
        } else {
            EnvFilter::try_new("")?
        };

        for directive in &self.log_level {
            filter_layer = filter_layer.add_directive(directive.clone());
        }

        Ok(filter_layer)
    }
}
