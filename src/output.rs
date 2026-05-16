//! uinput virtual mouse — Phase 4: declare REL wheel axes (including hi-res)
//! and emit smoother scroll segments produced by [`crate::smoother`].

use std::io;

use evdev::{
    uinput::VirtualDevice, AttributeSet, EventType, InputEvent, RelativeAxisCode,
};
use tracing::info;

use crate::smoother::{Axis, OutSegment};

const DEVICE_NAME: &str = "smoothwheeld virtual mouse";

pub struct Output {
    device: VirtualDevice,
}

impl Output {
    pub fn open() -> io::Result<Self> {
        let axes = AttributeSet::from_iter([
            RelativeAxisCode::REL_WHEEL,
            RelativeAxisCode::REL_HWHEEL,
            RelativeAxisCode::REL_WHEEL_HI_RES,
            RelativeAxisCode::REL_HWHEEL_HI_RES,
        ]);
        let mut device = VirtualDevice::builder()?
            .name(DEVICE_NAME)
            .with_relative_axes(&axes)?
            .build()?;
        for path in device.enumerate_dev_nodes_blocking()? {
            match path {
                Ok(p) => info!(path = %p.display(), "uinput device available"),
                Err(e) => info!(error = %e, "uinput device path enumeration error"),
            }
        }
        Ok(Self { device })
    }

    pub fn emit_segments(&mut self, segments: &[OutSegment]) -> io::Result<()> {
        if segments.is_empty() {
            return Ok(());
        }
        // Group everything into one batched emit; evdev appends SYN_REPORT.
        let events: Vec<InputEvent> = segments
            .iter()
            .map(|seg| {
                let code = match seg.axis {
                    Axis::Vertical => RelativeAxisCode::REL_WHEEL_HI_RES,
                    Axis::Horizontal => RelativeAxisCode::REL_HWHEEL_HI_RES,
                };
                InputEvent::new_now(EventType::RELATIVE.0, code.0, seg.hi_res_value)
            })
            .collect();
        self.device.emit(&events)
    }
}
