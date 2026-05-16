//! `tracing` subscriber setup.

use tracing_subscriber::EnvFilter;

use crate::cli::Cli;

/// Initialize logging. Honors `RUST_LOG` when set; otherwise uses `info`, or `debug` if `--verbose`.
pub fn init(cli: &Cli) {
    let default = if cli.verbose { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}
