//! Scroll smoothing — Phase 5: physical wheel ticks → hi-res output segments.
//!
//! The kernel convention is that one wheel notch == 120 `REL_WHEEL_HI_RES` units
//! (`WHEEL_NOTCH_HI_RES`). When a source device emits both `REL_WHEEL` and
//! `REL_WHEEL_HI_RES`, the hi-res stream is the source of truth and the notch
//! event is a coarse summary — feeding both would double-scroll, so we drop
//! the coarse event whenever the source advertises hi-res for that axis.

use evdev::RelativeAxisCode;

/// One wheel notch in hi-res units (kernel convention).
const NOTCH_HI_RES: i32 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutSegment {
    pub axis: Axis,
    pub hi_res_value: i32,
}

#[derive(Debug, Clone)]
pub struct SmootherConfig {
    pub multiplier: f64,
    pub invert_scroll: bool,
    pub enable_horizontal: bool,
    /// Number of sub-steps a single notch is split into when the source lacks hi-res.
    pub sub_steps: u32,
}

impl Default for SmootherConfig {
    fn default() -> Self {
        Self {
            multiplier: 1.0,
            invert_scroll: false,
            enable_horizontal: true,
            sub_steps: 8,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Smoother {
    cfg: SmootherConfig,
    source_has_hi_res_v: bool,
    source_has_hi_res_h: bool,
}

impl Smoother {
    pub fn new(cfg: SmootherConfig, source_has_hi_res_v: bool, source_has_hi_res_h: bool) -> Self {
        Self {
            cfg,
            source_has_hi_res_v,
            source_has_hi_res_h,
        }
    }

    /// Map one physical `REL_*` wheel event into zero or more hi-res output segments.
    pub fn process(&self, axis_code: RelativeAxisCode, value: i32) -> Vec<OutSegment> {
        let (axis, is_hi_res) = match axis_code {
            RelativeAxisCode::REL_WHEEL => (Axis::Vertical, false),
            RelativeAxisCode::REL_WHEEL_HI_RES => (Axis::Vertical, true),
            RelativeAxisCode::REL_HWHEEL => (Axis::Horizontal, false),
            RelativeAxisCode::REL_HWHEEL_HI_RES => (Axis::Horizontal, true),
            _ => return vec![],
        };

        if axis == Axis::Horizontal && !self.cfg.enable_horizontal {
            return vec![];
        }

        let source_has_hi_res = match axis {
            Axis::Vertical => self.source_has_hi_res_v,
            Axis::Horizontal => self.source_has_hi_res_h,
        };
        if !is_hi_res && source_has_hi_res {
            // Drop coarse notch event when the source also sends hi-res for this axis.
            return vec![];
        }

        let sign = if axis == Axis::Vertical && self.cfg.invert_scroll {
            -1.0
        } else {
            1.0
        };

        if is_hi_res {
            let scaled = (f64::from(value) * self.cfg.multiplier * sign).round() as i32;
            if scaled == 0 {
                return vec![];
            }
            return vec![OutSegment {
                axis,
                hi_res_value: scaled,
            }];
        }

        // Coarse notch and source lacks hi-res: split into sub-steps.
        let steps = self.cfg.sub_steps.max(1);
        let total = f64::from(value) * f64::from(NOTCH_HI_RES) * self.cfg.multiplier * sign;
        let per_step = total / f64::from(steps);
        let mut out = Vec::with_capacity(steps as usize);
        for _ in 0..steps {
            let v = per_step.round() as i32;
            if v != 0 {
                out.push(OutSegment {
                    axis,
                    hi_res_value: v,
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smoother(cfg: SmootherConfig) -> Smoother {
        // Pretend the source has hi-res for both axes by default in tests.
        Smoother::new(cfg, true, true)
    }

    #[test]
    fn hi_res_passthrough_keeps_sign_and_value() {
        let s = smoother(SmootherConfig::default());
        let out = s.process(RelativeAxisCode::REL_WHEEL_HI_RES, 120);
        assert_eq!(
            out,
            vec![OutSegment {
                axis: Axis::Vertical,
                hi_res_value: 120
            }]
        );
    }

    #[test]
    fn multiplier_two_doubles_the_hi_res_delta() {
        let s = smoother(SmootherConfig {
            multiplier: 2.0,
            ..Default::default()
        });
        let out = s.process(RelativeAxisCode::REL_WHEEL_HI_RES, 60);
        assert_eq!(out[0].hi_res_value, 120);
    }

    #[test]
    fn invert_scroll_flips_vertical_sign_only() {
        let s = smoother(SmootherConfig {
            invert_scroll: true,
            ..Default::default()
        });
        let v = s.process(RelativeAxisCode::REL_WHEEL_HI_RES, 120);
        assert_eq!(v[0].hi_res_value, -120);
        let h = s.process(RelativeAxisCode::REL_HWHEEL_HI_RES, 120);
        assert_eq!(h[0].hi_res_value, 120);
    }

    #[test]
    fn disable_horizontal_drops_horizontal_ticks() {
        let s = smoother(SmootherConfig {
            enable_horizontal: false,
            ..Default::default()
        });
        assert!(s.process(RelativeAxisCode::REL_HWHEEL, 1).is_empty());
        assert!(s
            .process(RelativeAxisCode::REL_HWHEEL_HI_RES, 120)
            .is_empty());
        assert!(!s.process(RelativeAxisCode::REL_WHEEL_HI_RES, 120).is_empty());
    }

    #[test]
    fn coarse_notch_dropped_when_source_has_hi_res() {
        let s = Smoother::new(SmootherConfig::default(), true, true);
        assert!(s.process(RelativeAxisCode::REL_WHEEL, 1).is_empty());
    }

    #[test]
    fn coarse_notch_split_into_sub_steps_when_source_lacks_hi_res() {
        let s = Smoother::new(
            SmootherConfig {
                sub_steps: 4,
                ..Default::default()
            },
            false,
            false,
        );
        let out = s.process(RelativeAxisCode::REL_WHEEL, 1);
        assert_eq!(out.len(), 4);
        let total: i32 = out.iter().map(|o| o.hi_res_value).sum();
        assert_eq!(total, NOTCH_HI_RES);
        assert!(out.iter().all(|o| o.axis == Axis::Vertical));
    }
}
