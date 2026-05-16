//! Shared error types for device resolution and I/O.

use std::path::PathBuf;

use crate::cli::CliError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Cli(#[from] CliError),

    #[error("multiple devices match --device-name {name:?}: use --device with a single path")]
    AmbiguousDeviceName {
        name: String,
        matches: Vec<(PathBuf, String)>,
    },

    #[error("no device found matching --device-name {name:?}")]
    NoMatchingDevice { name: String },

    #[error("no input device path specified (use --device, --device-name, or --config once wired)")]
    NoDevice,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{msg}")]
    Msg { msg: String },
}

pub type Result<T> = std::result::Result<T, Error>;
