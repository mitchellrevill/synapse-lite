//! Ornata605 Lighting bounds recovered from main.fcf07de2.js/main.f65da71b.css.
//! Normal English Quick mode, source fonts, no alert/Chroma ownership overlay.
//! Controlled inert DOM replay, not stock-window/pixel parity verification.
//! This module contains no device I/O, vendor program or vendor artwork.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
fn at(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[derive(Debug)]
pub struct ColorControl {
    pub label: Rect,
    pub dropdown: Rect,
}

#[derive(Debug)]
pub struct LightingGeometry {
    pub brightness_panel: Rect,
    pub brightness_switch: Rect,
    pub brightness_slider: Rect,
    pub sleep_panel: Rect,
    pub display_off_check: Rect,
    pub idle_check: Rect,
    pub idle_slider: Rect,
    pub effects_panel: Rect,
    pub mode_pill: Rect,
    pub quick_tab: Rect,
    pub advanced_tab: Rect,
    pub quick_message: Rect,
    pub effect_dropdown: Rect,
    pub sync: Rect,
    pub colors: Vec<ColorControl>,
    pub random_check: Option<Rect>,
    pub duration_slider: Option<Rect>,
    pub direction_tabs: Vec<Rect>,
    pub ambient_screens: Vec<Rect>,
    pub color_boost: Option<Rect>,
}

/// Publicly advertised order, not numerical effect-ID order. Source product
/// table is at UTF-8 byte7257187; numeric enum is in webpack module13254.
pub const QUICK_EFFECTS: [(u8, &str); 12] = [
    (11, "Ambient Awareness"),
    (12, "Audio Meter"),
    (2, "Breathing"),
    (8, "Fire"),
    (5, "Reactive"),
    (6, "Ripple"),
    (3, "Spectrum Cycling"),
    (7, "Starlight"),
    (1, "Static"),
    (19, "Tidal"),
    (4, "Wave"),
    (13, "Wheel"),
];

pub fn effect_panel_height(effect: u8) -> Option<f64> {
    Some(match effect {
        11 => 306.0,
        12 | 2 | 7 => 313.0,
        8 | 3 => 239.0,
        5 => 312.0,
        6 | 1 => 308.0,
        19 => 387.0,
        4 | 13 => 323.0,
        _ => return None,
    })
}

/// CSS pixels in product content. Add the host origin separately. At widths
/// below1280 the source columns stack; this preserves the entire left column
/// before placing Effects below it. Native outer width is not CSS width.
pub fn geometry(
    width: f64,
    nav_bottom: f64,
    scroll_top: f64,
    effect: u8,
) -> Option<LightingGeometry> {
    if !width.is_finite() || width < 1000.0 || !nav_bottom.is_finite() || !scroll_top.is_finite() {
        return None;
    }
    let effects_height = effect_panel_height(effect)?;
    let top = nav_bottom + 20.0 - scroll_top;
    let (left, right, effects_top) = if width >= 1280.0 {
        (
            (width - 1240.0) / 2.0 + 10.0,
            (width - 1240.0) / 2.0 + 630.0,
            top,
        )
    } else {
        ((width - 600.0) / 2.0, (width - 600.0) / 2.0, top + 430.0)
    };
    let mut colors = Vec::new();
    if matches!(effect, 1 | 2 | 5 | 6 | 7 | 19) {
        colors.push(ColorControl {
            label: at(right + 40.0, effects_top + 229.0, 73.0, 17.0),
            dropdown: at(right + 40.0, effects_top + 251.0, 53.0, 27.0),
        });
    }
    if matches!(effect, 2 | 7 | 19) {
        colors.push(ColorControl {
            label: at(right + 113.0, effects_top + 229.0, 73.0, 17.0),
            dropdown: at(right + 113.0, effects_top + 251.0, 53.0, 27.0),
        });
    }
    let direction_top = if effect == 19 {
        effects_top + 321.0
    } else {
        effects_top + 257.0
    };
    Some(LightingGeometry {
        brightness_panel: at(left, top, 600.0, 166.0),
        brightness_switch: at(left + 139.703125, top + 33.0, 32.0, 18.0),
        brightness_slider: at(left + 40.0, top + 72.0, 520.0, 64.0),
        sleep_panel: at(left, top + 186.0, 600.0, 224.0),
        display_off_check: at(left + 40.0, top + 258.0, 20.0, 20.0),
        idle_check: at(left + 40.0, top + 287.0, 20.0, 20.0),
        idle_slider: at(left + 70.0, top + 316.0, 490.0, 64.0),
        effects_panel: at(right, effects_top, 600.0, effects_height),
        mode_pill: at(right + 41.0, effects_top + 72.0, 243.65625, 36.0),
        quick_tab: at(right + 47.0, effects_top + 77.0, 102.5625, 26.0),
        advanced_tab: at(right + 149.5625, effects_top + 77.0, 129.09375, 26.0),
        quick_message: at(right + 40.0, effects_top + 128.0, 520.0, 34.0),
        effect_dropdown: at(right + 40.0, effects_top + 182.0, 150.0, 27.0),
        // Source sync overflows its panel content26px. Do not silently shrink it.
        sync: at(right + 210.0, effects_top + 182.5, 376.0, 26.0),
        colors,
        random_check: matches!(effect, 2 | 7 | 19)
            .then(|| at(right + 186.0, effects_top + 254.0, 20.0, 20.0)),
        duration_slider: match effect {
            5 => Some(at(right + 123.0, effects_top + 246.0, 437.0, 36.0)),
            7 => Some(at(right + 330.0, effects_top + 246.0, 230.0, 36.0)),
            _ => None,
        },
        direction_tabs: if matches!(effect, 4 | 13 | 19) {
            vec![
                at(right + 47.0, direction_top, 40.0, 30.0),
                at(right + 92.0, direction_top, 40.0, 30.0),
            ]
        } else {
            Vec::new()
        },
        ambient_screens: if effect == 11 {
            (0..5)
                .map(|i| {
                    at(
                        right + 40.0 + i as f64 * 30.0,
                        effects_top + 249.0,
                        24.0,
                        16.0,
                    )
                })
                .collect()
        } else {
            Vec::new()
        },
        color_boost: (effect == 12).then(|| at(right + 40.0, effects_top + 246.0, 60.0, 27.0)),
    })
}

/// Generic source slider70130: visible track is6px high,25px above the bottom.
pub fn track(container: Rect) -> Rect {
    at(
        container.x,
        container.y + container.height - 31.0,
        container.width,
        6.0,
    )
}

pub fn fill_width(value: f64, min: f64, max: f64, width: f64) -> Option<f64> {
    if ![value, min, max, width].iter().all(|v| v.is_finite())
        || max <= min
        || width < 16.0
        || value < min
        || value > max
    {
        return None;
    }
    Some(8.0 + (value - min) / (max - min) * (width - 16.0))
}

pub fn tip_left(value: f64, min: f64, max: f64, width: f64, tip_width: f64) -> Option<f64> {
    if !tip_width.is_finite() || tip_width < 0.0 {
        return None;
    }
    Some(fill_width(value, min, max, width)? - tip_width / 2.0)
}

/// Slider changes above zero enable lighting; switch toggles separately retain
/// the prior numeric value. No 0..255 hardware scaling belongs in this helper.
#[cfg(test)]
pub fn enabled_after_brightness_slider(value: u8) -> Option<bool> {
    (value <= 100).then_some(value != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spectrum_geometry_matches_replay() {
        let g = geometry(1280.0, 86.0, 0.0, 3).unwrap();
        assert_eq!(g.brightness_panel, at(30.0, 106.0, 600.0, 166.0));
        assert_eq!(g.sleep_panel, at(30.0, 292.0, 600.0, 224.0));
        assert_eq!(g.effect_dropdown, at(690.0, 288.0, 150.0, 27.0));
        assert_eq!(track(g.idle_slider), at(100.0, 455.0, 490.0, 6.0));
    }
    #[test]
    fn narrow_stacks_whole_left_column() {
        let g = geometry(1000.0, 86.0, 0.0, 3).unwrap();
        assert_eq!(g.effects_panel, at(200.0, 536.0, 600.0, 239.0));
        let s = geometry(1000.0, 86.0, 100.0, 3).unwrap();
        assert_eq!(s.effect_dropdown.y, g.effect_dropdown.y - 100.0);
    }
    #[test]
    fn distinct_effect_controls_are_retained() {
        let g = geometry(1280.0, 86.0, 0.0, 7).unwrap();
        assert_eq!(g.colors.len(), 2);
        assert_eq!(g.duration_slider, Some(at(980.0, 352.0, 230.0, 36.0)));
        let wave = geometry(1280.0, 86.0, 0.0, 4).unwrap();
        assert_eq!(wave.direction_tabs[0], at(697.0, 363.0, 40.0, 30.0));
        assert!(wave.colors.is_empty());
    }
    #[test]
    fn source_fill_and_disabled_value_semantics() {
        assert_eq!(fill_width(100.0, 0.0, 100.0, 520.0), Some(512.0));
        assert_eq!(fill_width(1.0, 1.0, 15.0, 490.0), Some(8.0));
        assert_eq!(enabled_after_brightness_slider(0), Some(false));
        assert_eq!(enabled_after_brightness_slider(1), Some(true));
        assert!(geometry(1280.0, 86.0, 0.0, 255).is_none());
    }
}

/// Generic source dropdown (module27734): 25px rows, 180px border-box,
/// chosen-row scrollTo(offsetTop), and up/down collision against innerHeight.
/// CSS main.f65da71b.css .s3-options / global WebKit scrollbar.
#[derive(Clone, Copy, Debug)]
pub struct EffectDropdown {
    pub panel: Rect,
    pub viewport: Rect,
    pub track: Rect,
    pub thumb: Rect,
    pub scroll: f64,
    pub max_scroll: f64,
}
pub fn effect_dropdown(trigger: Rect, height: f64, scroll: f64) -> EffectDropdown {
    let top = if trigger.y + trigger.height + 2.0 + 180.0 > height {
        trigger.y - 180.0 - 1.0
    } else {
        trigger.y + 28.0
    };
    let panel = at(trigger.x, top, trigger.width, 180.0);
    let viewport = at(panel.x + 1.0, top + 1.0, panel.width - 2.0, 178.0);
    let max_scroll = 12.0 * 25.0 - viewport.height;
    let scroll = scroll.clamp(0.0, max_scroll);
    let track = at(
        viewport.x + viewport.width - 8.0,
        viewport.y,
        8.0,
        viewport.height,
    );
    let thumb_height = track.height * viewport.height / 300.0;
    let thumb = at(
        track.x,
        track.y + scroll / max_scroll * (track.height - thumb_height),
        8.0,
        thumb_height,
    );
    EffectDropdown {
        panel,
        viewport,
        track,
        thumb,
        scroll,
        max_scroll,
    }
}
pub fn effect_selected_scroll(selected: Option<u8>) -> f64 {
    QUICK_EFFECTS
        .iter()
        .position(|(id, _)| Some(*id) == selected)
        .map_or(0.0, |i| (i as f64 * 25.0).min(122.0))
}
/// Source q_=Z_ invokes toggleTab for either half (also for outer pill).
/// OI Wave, mI Wheel both select the second tab unless direction == 1.
pub fn direction_toggle(selected: u8, direction: Option<i64>) -> i64 {
    if selected == 19 {
        if direction == Some(1) {
            0
        } else {
            1
        }
    } else if direction == Some(1) {
        2
    } else {
        1
    }
}
#[cfg(test)]
mod effect_dropdown_tests {
    use super::*;
    #[test]
    fn replayed_source_dropdown_preserves_partial_rows_and_selected_wave() {
        let d = effect_dropdown(
            at(660.0, 300.0, 150.0, 27.0),
            720.0,
            effect_selected_scroll(Some(4)),
        );
        assert_eq!(d.panel, at(660.0, 328.0, 150.0, 180.0));
        assert_eq!(d.viewport.height, 178.0);
        assert_eq!(d.scroll, 122.0);
        assert_eq!(300.0 - d.scroll, d.viewport.height);
        assert!((d.thumb.height - 105.6133333333).abs() < 0.0001);
        assert_eq!(effect_selected_scroll(Some(12)), 25.0);
        let up = effect_dropdown(at(660.0, 550.0, 150.0, 27.0), 720.0, 0.0);
        assert_eq!(up.panel.y, 369.0);
    }
    #[test]
    fn shared_source_direction_pill_toggles_instead_of_selecting_clicked_half() {
        assert_eq!(direction_toggle(4, Some(2)), 1);
        assert_eq!(direction_toggle(4, Some(1)), 2);
        assert_eq!(direction_toggle(4, None), 1);
        assert_eq!(direction_toggle(13, Some(2)), 1);
        assert_eq!(direction_toggle(19, Some(1)), 0);
        assert_eq!(direction_toggle(19, Some(0)), 1);
    }
}
