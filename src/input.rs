//! Physical evdev reader — wheel REL events only for dry-run / later main loop.

use std::path::Path;

use evdev::{Device, EventSummary};
use tracing::info;

use crate::errors::Result;

const WHEEL: &[evdev::RelativeAxisCode] = &[
    evdev::RelativeAxisCode::REL_WHEEL,
    evdev::RelativeAxisCode::REL_HWHEEL,
    evdev::RelativeAxisCode::REL_WHEEL_HI_RES,
    evdev::RelativeAxisCode::REL_HWHEEL_HI_RES,
];

/// Read from `path` and log only wheel relative-axis events (Phase 3 / `--dry-run`).
pub fn dry_run_wheel_events(path: &Path) -> Result<()> {
    let mut device = Device::open(path)?;
    info!(
        path = %path.display(),
        name = ?device.name(),
        "dry-run: logging wheel REL events (Ctrl+C to stop)"
    );

    loop {
        for event in device.fetch_events()? {
            match event.destructure() {
                EventSummary::RelativeAxis(_, axis, value) if WHEEL.contains(&axis) => {
                    info!(?axis, value, "wheel");
                }
                _ => {}
            }
        }
    }
}
