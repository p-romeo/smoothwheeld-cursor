//! smoothwheeld — Linux wheel smoothing daemon (evdev → uinput). See `plan.md`.

mod cli;
mod config;
mod devices;
mod errors;
mod input;
mod logging;
mod output;
mod smoother;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{validate, Cli};
use crate::errors::Error;

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(e) = validate(&cli) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }

    logging::init(&cli);

    if cli.list_devices {
        if let Err(e) = devices::print_candidate_devices() {
            tracing::error!(error = %e, "listing devices failed");
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }

    match run_smoothing(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "smoothwheeld exited with error");
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn run_smoothing(cli: &Cli) -> Result<(), Error> {
    let device_path = devices::resolve_device_path(cli)?.ok_or(Error::NoDevice)?;

    if cli.dry_run {
        return input::dry_run_wheel_events(&device_path).map_err(Into::into);
    }

    tracing::info!("main loop not yet implemented — see plan.md Phase 4–5");
    Err(Error::Msg {
        msg: "main loop not implemented; use --list-devices, --dry-run, or follow plan.md Phase 4+"
            .to_string(),
    })
}
