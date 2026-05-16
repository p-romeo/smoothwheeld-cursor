//! Physical evdev reader — wheel REL events only for dry-run and main loop.

use std::path::Path;

use evdev::{Device, EventSummary, RelativeAxisCode};
use tracing::info;

use crate::errors::Result;
use crate::output::Output;
use crate::smoother::{Smoother, SmootherConfig};

const WHEEL: &[RelativeAxisCode] = &[
    RelativeAxisCode::REL_WHEEL,
    RelativeAxisCode::REL_HWHEEL,
    RelativeAxisCode::REL_WHEEL_HI_RES,
    RelativeAxisCode::REL_HWHEEL_HI_RES,
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
            if let EventSummary::RelativeAxis(_, axis, value) = event.destructure() {
                if WHEEL.contains(&axis) {
                    info!(?axis, value, "wheel");
                }
            }
        }
    }
}

/// Phase 5 main loop: read wheel events, smooth them, emit on the uinput device.
///
/// Non-wheel events are ignored — the daemon never modifies pointer motion,
/// buttons, or key events (see `plan.md` requirements 7–10).
pub fn run_loop(path: &Path) -> Result<()> {
    let mut device = Device::open(path)?;
    let (has_hi_res_v, has_hi_res_h) = match device.supported_relative_axes() {
        Some(axes) => (
            axes.contains(RelativeAxisCode::REL_WHEEL_HI_RES),
            axes.contains(RelativeAxisCode::REL_HWHEEL_HI_RES),
        ),
        None => (false, false),
    };
    let smoother = Smoother::new(SmootherConfig::default(), has_hi_res_v, has_hi_res_h);
    let mut output = Output::open()?;

    info!(
        path = %path.display(),
        name = ?device.name(),
        hi_res_v = has_hi_res_v,
        hi_res_h = has_hi_res_h,
        "main loop: forwarding smoothed wheel events (Ctrl+C to stop)"
    );

    loop {
        for event in device.fetch_events()? {
            if let EventSummary::RelativeAxis(_, axis, value) = event.destructure() {
                if WHEEL.contains(&axis) {
                    let segments = smoother.process(axis, value);
                    output.emit_segments(&segments)?;
                }
            }
        }
    }
}
