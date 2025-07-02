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
    /// Which logger to use
    #[clap(long, default_value_t = Default::default(), global = true)]
    pub log_format: Logger,
    /// Tracing directives
    #[clap(long, global = true, group = "verbosity", value_delimiter = ',', num_args = 0.., conflicts_with = "verbose")]
    pub(crate) log_level: Vec<Directive>,
}

impl Instrumentation {
    pub(crate) fn log_level(&self) -> String {
        match self.verbose {
            0 => "info",
            1 => "debug",
            _ => "trace",
        }
        .to_string()
    }

    pub(crate) async fn setup(&self, binary_name: &str) -> Result<(), crate::Error> {
        let filter_layer = self.filter_layer(binary_name)?;

        let registry = tracing_subscriber::registry().with(filter_layer);

        match self.log_format {
            Logger::Default => {
                let fmt_layer = self.default_fmt_layer();
                registry.with(fmt_layer).try_init()?
            }
            Logger::Pretty => {
                let fmt_layer = self.pretty_fmt_layer();
                registry.with(fmt_layer).try_init()?
            }
            Logger::Json => {
                let fmt_layer = self.json_fmt_layer();
                registry.with(fmt_layer).try_init()?
            }
            Logger::Tree => {
                let tree_layer = tracing_tree::HierarchicalLayer::new(2).with_indent_lines(true);
                registry.with(tree_layer).try_init()?
            }
        }
        tracing::trace!("Instrumentation initialized");

        Ok(())
    }

    /// Set up a basic, simple logger that doesn't emit more than it needs.
    pub(crate) fn default_fmt_layer<S>(&self) -> impl tracing_subscriber::layer::Layer<S>
    where
        S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
    {
        tracing_subscriber::fmt::Layer::new()
            .compact()
            .with_ansi(std::io::stderr().is_terminal())
            .with_writer(std::io::stderr)
            .without_time()
            .with_target(self.verbose >= 2)
    }

    /// Set up a 'pretty' formatter that displays structure span information, timestamps,
    /// line numbers/files, etc.
    pub(crate) fn pretty_fmt_layer<S>(&self) -> impl tracing_subscriber::layer::Layer<S>
    where
        S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
    {
        tracing_subscriber::fmt::Layer::new()
            .with_ansi(std::io::stderr().is_terminal())
            .with_writer(std::io::stderr)
            .pretty()
    }

    /// Set up a JSON formatter for machine parseable output    
    pub fn json_fmt_layer<S>(&self) -> impl tracing_subscriber::layer::Layer<S>
    where
        S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
    {
        tracing_subscriber::fmt::Layer::new()
            .with_ansi(std::io::stderr().is_terminal())
            .with_writer(std::io::stderr)
            .json()
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

#[derive(Clone, Default, Debug, clap::ValueEnum)]
pub enum Logger {
    #[default]
    Default,
    Pretty,
    Tree,
    Json,
}

impl std::fmt::Display for Logger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let logger = match self {
            Logger::Default => "default",
            Logger::Pretty => "pretty",
            Logger::Json => "json",
            Logger::Tree => "tree",
        };
        write!(f, "{logger}")
    }
}
