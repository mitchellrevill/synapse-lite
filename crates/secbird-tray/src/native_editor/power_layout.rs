//! Product196 Power layout from the full shipped CSS and inert markup replay.
//! Coordinates are CSS pixels; host offset and scroll are supplied by callers.
//! See outputs/research/mouse-power-layout-replay.json for measurements and limits.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Card {
    pub bounds: Rect,
    pub title: Rect,
    pub description: Rect,
    pub slider: Rect,
    pub warning: Option<Rect>,
}

#[derive(Clone, Copy, Debug)]
pub struct PowerGeometry {
    pub sleep: Card,
    pub threshold: Card,
}

fn card(x: f64, y: f64, warning: bool) -> Card {
    Card {
        bounds: Rect {
            x,
            y,
            width: 600.0,
            height: if warning { 225.0 } else { 193.0 },
        },
        title: Rect {
            x: x + 40.0,
            y: y + 30.0,
            width: 520.0,
            height: 22.0,
        },
        description: Rect {
            x: x + 40.0,
            y: y + 72.0,
            width: 520.0,
            height: 17.0,
        },
        slider: Rect {
            x: x + 40.0,
            y: y + 99.0,
            width: 520.0,
            height: 64.0,
        },
        warning: warning.then_some(Rect {
            x: x + 40.0,
            y: y + 178.0,
            width: 520.0,
            height: 17.0,
        }),
    }
}

/// Source English desktop at CSS width >=1000, no alert banners. Disabled
/// threshold adds its source warning; other unavailable/error states are not
/// established by this CSS replay. Host client clipping remains independent.
pub fn geometry(
    width: f64,
    nav_bottom: f64,
    scroll: f64,
    high_polling: bool,
) -> Option<PowerGeometry> {
    if !width.is_finite() || width < 1000.0 || !nav_bottom.is_finite() || !scroll.is_finite() {
        return None;
    }
    let top = nav_bottom + 20.0 - scroll;
    let (left, right, right_top) = if width >= 1280.0 {
        (
            (width - 1240.0) / 2.0 + 10.0,
            (width - 1240.0) / 2.0 + 630.0,
            top,
        )
    } else {
        ((width - 600.0) / 2.0, (width - 600.0) / 2.0, top + 213.0)
    };
    Some(PowerGeometry {
        sleep: card(left, top, false),
        threshold: card(right, right_top, high_polling),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_wide_and_wrapped_rectangles_match_replay() {
        for (width, left, threshold_x, threshold_y) in [
            (1280.0, 30.0, 650.0, 106.0),
            (1266.0, 333.0, 333.0, 319.0),
            (1000.0, 200.0, 200.0, 319.0),
        ] {
            let g = geometry(width, 86.0, 0.0, false).unwrap();
            assert_eq!(
                g.sleep.bounds,
                Rect {
                    x: left,
                    y: 106.0,
                    width: 600.0,
                    height: 193.0
                }
            );
            assert_eq!(
                g.sleep.slider,
                Rect {
                    x: left + 40.0,
                    y: 205.0,
                    width: 520.0,
                    height: 64.0
                }
            );
            assert_eq!(g.threshold.bounds.x, threshold_x);
            assert_eq!(g.threshold.bounds.y, threshold_y);
            assert!(g.threshold.warning.is_none());
        }
    }

    #[test]
    fn disabled_warning_and_host_scroll_keep_source_spacing() {
        let g = geometry(1266.0, 128.0, 20.0, true).unwrap();
        assert_eq!(g.sleep.bounds.y, 128.0);
        assert_eq!(g.threshold.bounds.y, 341.0);
        assert_eq!(g.threshold.bounds.height, 225.0);
        assert_eq!(g.threshold.warning.unwrap().y, 519.0);
        assert_eq!(g.threshold.description.y - g.threshold.title.y, 42.0);
    }

    #[test]
    fn unsupported_or_nonfinite_viewports_are_not_guessed() {
        assert!(geometry(999.0, 86.0, 0.0, false).is_none());
        assert!(geometry(f64::NAN, 86.0, 0.0, false).is_none());
        assert!(geometry(1280.0, f64::INFINITY, 0.0, false).is_none());
        assert!(geometry(1280.0, 86.0, f64::NAN, false).is_none());
    }
}
