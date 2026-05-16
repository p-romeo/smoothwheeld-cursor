//! Command-line interface (`clap`). See `plan.md` for flag semantics.

use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "smoothwheeld")]
#[command(about = "Smooth physical mouse wheel events and emit a virtual uinput mouse (Linux).")]
#[command(version)]
pub struct Cli {
    /// List candidate mouse devices with wheel capabilities (non-wheel-only filter).
    #[arg(long)]
    pub list_devices: bool,

    /// Open this evdev node as the physical source (e.g. /dev/input/event6).
    #[arg(long, value_name = "PATH")]
    pub device: Option<PathBuf>,

    /// Select device whose name contains this substring (case-insensitive). Fails if ambiguous.
    #[arg(long, value_name = "SUBSTRING")]
    pub device_name: Option<String>,

    /// TOML config path (defaults to ~/.config/smoothwheeld/config.toml when implemented).
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Force exclusive grab off for this run (overrides config).
    #[arg(long)]
    pub no_grab: bool,

    /// Force exclusive grab on (overrides config). Conflicts with --no-grab.
    #[arg(long)]
    pub grab: bool,

    /// Read wheel events only; do not create uinput or emit synthetic scroll.
    #[arg(long)]
    pub dry_run: bool,

    /// More verbose logs (debug). Superseded by RUST_LOG when set.
    #[arg(short, long)]
    pub verbose: bool,

    /// Delay in milliseconds before grab or main loop (safety; grab mode).
    #[arg(long, value_name = "MS")]
    pub delay_ms: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("cannot use both --device and --device-name")]
    DeviceAndName,
    #[error("cannot use both --grab and --no-grab")]
    GrabConflict,
}

pub fn validate(cli: &Cli) -> Result<(), CliError> {
    if cli.device.is_some() && cli.device_name.is_some() {
        return Err(CliError::DeviceAndName);
    }
    if cli.grab && cli.no_grab {
        return Err(CliError::GrabConflict);
    }
    Ok(())
}
