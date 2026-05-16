//! Enumerate `/dev/input/event*` and classify wheel-capable, non-keyboard devices.

use std::path::PathBuf;

use evdev::{Device, KeyCode, RelativeAxisCode};
use tracing::{debug, warn};

use crate::errors::{Error, Result};

const WHEEL_AXES: [RelativeAxisCode; 4] = [
    RelativeAxisCode::REL_WHEEL,
    RelativeAxisCode::REL_HWHEEL,
    RelativeAxisCode::REL_WHEEL_HI_RES,
    RelativeAxisCode::REL_HWHEEL_HI_RES,
];

/// Human-readable labels matching `plan.md` / linux input-event-codes.
pub fn axis_label(axis: RelativeAxisCode) -> &'static str {
    match axis {
        RelativeAxisCode::REL_WHEEL => "REL_WHEEL",
        RelativeAxisCode::REL_HWHEEL => "REL_HWHEEL",
        RelativeAxisCode::REL_WHEEL_HI_RES => "REL_WHEEL_HI_RES",
        RelativeAxisCode::REL_HWHEEL_HI_RES => "REL_HWHEEL_HI_RES",
        _ => "REL_OTHER",
    }
}

/// Exclude full keyboards: space + letter row key present (see `plan.md`).
pub fn is_likely_keyboard(keys: Option<&evdev::AttributeSetRef<KeyCode>>) -> bool {
    let Some(keys) = keys else {
        return false;
    };
    keys.contains(KeyCode::KEY_SPACE) && keys.contains(KeyCode::KEY_A)
}

pub fn wheel_capability_labels(device: &Device) -> Vec<&'static str> {
    let Some(axes) = device.supported_relative_axes() else {
        return vec![];
    };
    WHEEL_AXES
        .iter()
        .filter(|a| axes.contains(**a))
        .copied()
        .map(axis_label)
        .collect()
}

pub fn has_any_wheel_axis(device: &Device) -> bool {
    !wheel_capability_labels(device).is_empty()
}

/// Candidate = at least one wheel REL axis and not classified as a typical keyboard.
pub fn is_candidate_mouse(device: &Device) -> bool {
    has_any_wheel_axis(device) && !is_likely_keyboard(device.supported_keys())
}

pub fn event_node_paths() -> Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = glob::glob("/dev/input/event*")
        .map_err(|e| Error::Msg {
            msg: format!("glob pattern: {e}"),
        })?
        .filter_map(std::result::Result::ok)
        .collect();
    paths.sort();
    Ok(paths)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i + 1 >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

/// Scan every `/dev/input/event*`: print candidates; permission errors are non-fatal and listed.
pub fn print_candidate_devices() -> Result<()> {
    println!("{:<28} {:<30} WHEEL_CAPABILITIES", "PATH", "NAME");
    for path in event_node_paths()? {
        match Device::open(&path) {
            Ok(device) => {
                let name = device.name().unwrap_or("(no name)").to_string();
                let wheel_caps = wheel_capability_labels(&device);
                if !is_candidate_mouse(&device) {
                    if has_any_wheel_axis(&device) && is_likely_keyboard(device.supported_keys()) {
                        debug!(
                            path = %path.display(),
                            name = %name,
                            "skipped (keyboard-like device with wheel axes)"
                        );
                    }
                    continue;
                }
                let caps = wheel_caps.join(" ");
                println!(
                    "{:<28} {:<30} {}",
                    path.display(),
                    truncate(&name, 28),
                    caps
                );
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    warn!(path = %path.display(), error = %e, "cannot open device");
                } else {
                    debug!(path = %path.display(), error = %e, "skipping device");
                }
                println!("{:<28} {}", path.display(), e);
            }
        }
    }
    Ok(())
}

/// Resolve `--device-name`: deterministic sorted paths, fail if 0 or >1 match.
pub fn resolve_device_name(name_substring: &str) -> Result<PathBuf> {
    let needle = name_substring.to_lowercase();
    let mut matches: Vec<(PathBuf, String)> = Vec::new();
    for path in event_node_paths()? {
        let device = match Device::open(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if !is_candidate_mouse(&device) {
            continue;
        }
        let n = device.name().unwrap_or("").to_lowercase();
        if n.contains(&needle) {
            matches.push((path, device.name().unwrap_or("").to_string()));
        }
    }
    // `event_node_paths()` already returns paths sorted, so `matches` is in
    // path order without an extra sort here.
    match matches.len() {
        0 => Err(Error::NoMatchingDevice {
            name: name_substring.to_string(),
        }),
        1 => Ok(matches[0].0.clone()),
        _ => Err(Error::AmbiguousDeviceName {
            name: name_substring.to_string(),
            matches,
        }),
    }
}

pub fn resolve_device_path(cli: &crate::cli::Cli) -> Result<Option<PathBuf>> {
    if let Some(ref p) = cli.device {
        return Ok(Some(p.clone()));
    }
    if let Some(ref name) = cli.device_name {
        return Ok(Some(resolve_device_name(name)?));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_label_maps_wheel_variants() {
        assert_eq!(axis_label(RelativeAxisCode::REL_WHEEL), "REL_WHEEL");
        assert_eq!(axis_label(RelativeAxisCode::REL_WHEEL_HI_RES), "REL_WHEEL_HI_RES");
    }
}
