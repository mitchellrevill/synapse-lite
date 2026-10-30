//! Product 196 Performance facts recovered from the cached Synapse renderer.
//!
//! main.bbc961f9.js: product ranges at UTF-8 byte 4832727, slider class TA at
//! 5388288, input normalization in module 4230. Geometry is a controlled inert
//! DOM replay of main.116d273e.css with source Roboto/RazerF5 fonts. Coordinates
//! are CSS pixels in product content; the caller supplies nav bottom and scroll.
//! This does not establish host client bounds or native rasterization parity.

pub const MIN_DPI: u32 = 100;
pub const MAX_DPI: u32 = 26_000;
pub const DPI_STEP: u32 = 50;

#[derive(Clone, Copy, Debug)]
pub struct DpiRange {
    pub percent_from: f64,
    pub percent_to: f64,
    pub dpi_from: f64,
    pub dpi_to: f64,
    pub percent_step: f64,
}

pub const DPI_RANGES: [DpiRange; 5] = [
    DpiRange {
        percent_from: 0.0,
        percent_to: 25.0,
        dpi_from: 100.0,
        dpi_to: 400.0,
        percent_step: 25.0 * 50.0 / 300.0,
    },
    DpiRange {
        percent_from: 25.0,
        percent_to: 37.5,
        dpi_from: 400.0,
        dpi_to: 800.0,
        percent_step: 12.5 * 50.0 / 400.0,
    },
    DpiRange {
        percent_from: 37.5,
        percent_to: 50.0,
        dpi_from: 800.0,
        dpi_to: 1600.0,
        percent_step: 12.5 * 50.0 / 800.0,
    },
    DpiRange {
        percent_from: 50.0,
        percent_to: 75.0,
        dpi_from: 1600.0,
        dpi_to: 10000.0,
        percent_step: 25.0 * 50.0 / 8400.0,
    },
    DpiRange {
        percent_from: 75.0,
        percent_to: 100.0,
        dpi_from: 10000.0,
        dpi_to: 26000.0,
        percent_step: 25.0 * 50.0 / 16000.0,
    },
];

pub fn dpi_to_percent(dpi: u32) -> Option<f64> {
    if !(MIN_DPI..=MAX_DPI).contains(&dpi) {
        return None;
    }
    let value = f64::from(dpi);
    let range = DPI_RANGES.iter().find(|range| value <= range.dpi_to)?;
    Some((value - range.dpi_from) / f64::from(DPI_STEP) * range.percent_step + range.percent_from)
}

/// Positive-domain Math.round from source TA, then the source range origin.
pub fn percent_to_dpi(percent: f64) -> Option<u32> {
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return None;
    }
    let range = DPI_RANGES
        .iter()
        .find(|range| percent <= range.percent_to)?;
    Some(
        (((percent - range.percent_from) / range.percent_step).round() * f64::from(DPI_STEP)
            + range.dpi_from) as u32,
    )
}

/// Numeric input blur rounds UP to the next step, then clamps. This differs
/// from the slider's nearest-step rounding. Invalid/empty parsed input is zero.
pub fn normalize_input_value(parsed_integer: Option<i64>) -> u32 {
    let value = parsed_integer.unwrap_or(0) as f64;
    ((value / f64::from(DPI_STEP)).ceil() * f64::from(DPI_STEP))
        .clamp(f64::from(MIN_DPI), f64::from(MAX_DPI)) as u32
}

/// Step transient empty/minus text from zero, then apply the input bounds.
/// Arithmetic is intentional: the shipped JS concatenates typed strings on
/// increment; reproducing that defect would turn typed800 +50 into26000.
pub fn step_input_value(parsed_integer: Option<i64>, delta: i16) -> u32 {
    normalize_input_value(Some(
        parsed_integer.unwrap_or(0).saturating_add(i64::from(delta)),
    ))
}

/// Source TA uses sizing-16 even with its 12px small thumb styling.
pub fn source_fill_width(percent: f64, sizing: f64) -> f64 {
    percent / 100.0 * (sizing - 16.0) + 8.0
}

pub fn source_thumb_tag_left(percent: f64) -> f64 {
    3.1 + percent / 100.0 * (290.6 - 3.1)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug)]
pub struct StageGeometry {
    pub row: Rect,
    pub ordinal: Rect,
    pub x_input: Rect,
    pub x_slider: Rect,
    pub y_input: Option<Rect>,
    pub y_slider: Option<Rect>,
    /// Present in the DOM, visible on hover; one-stage still shows this icon.
    pub independent_icon: Rect,
    /// Hidden in one-stage mode, otherwise visible only on row hover.
    pub enable_switch: Rect,
    pub drag_handle: Rect,
}

#[derive(Debug)]
pub struct PerformanceGeometry {
    pub sensitivity_widget: Rect,
    pub polling_widget: Rect,
    pub stage_toggle: Rect,
    pub stage_header: Rect,
    pub stages: Vec<StageGeometry>,
    pub polling_buttons: Vec<Rect>,
}

/// Recovered English desktop layout at CSS widths >=1000, with source fonts.
/// `independent_rows` is the rendered row list: all configured rows when stages
/// are enabled, otherwise only the active row. It is not the core profile model.
/// Source CSS wraps the entire polling column below sensitivity below1280px.
/// Polling widget height excludes optional high-rate warning text, in-game
/// polling controls and other conditional product widgets. Host offsets, alert
/// banners and scrollbar widths must be resolved by the caller.
pub fn geometry(
    width: f64,
    nav_bottom: f64,
    scroll_top: f64,
    independent_rows: &[bool],
    polling_button_count: usize,
) -> Option<PerformanceGeometry> {
    if !width.is_finite()
        || width < 1000.0
        || !nav_bottom.is_finite()
        || !scroll_top.is_finite()
        || independent_rows.is_empty()
        || independent_rows.len() > 5
        || polling_button_count > 8
    {
        return None;
    }
    let top = nav_bottom + 20.0 - scroll_top;
    let dpi_height = 191.0
        + independent_rows
            .iter()
            .map(|independent| if *independent { 114.0 } else { 68.0 })
            .sum::<f64>()
        + 4.0 * (independent_rows.len() - 1) as f64;
    let (left, polling_left, polling_top) = if width >= 1280.0 {
        (
            (width - 1240.0) / 2.0 + 10.0,
            (width - 1240.0) / 2.0 + 630.0,
            top,
        )
    } else {
        (
            (width - 600.0) / 2.0,
            (width - 600.0) / 2.0,
            top + dpi_height + 20.0,
        )
    };
    let mut y = top + 161.0;
    let stages = independent_rows
        .iter()
        .map(|independent| {
            let height = if *independent { 114.0 } else { 68.0 };
            let input_top = y + if *independent { 20.5 } else { 16.0 };
            let result = StageGeometry {
                row: Rect::new(left + 20.0, y, 560.0, height),
                ordinal: Rect::new(left + 40.0, y + (height - 30.0) / 2.0, 30.0, 30.0),
                x_input: Rect::new(left + 80.0, input_top, 62.0, 26.0),
                x_slider: Rect::new(left + 152.0, input_top + 3.0, 300.0, 20.0),
                y_input: independent.then(|| Rect::new(left + 80.0, input_top + 37.0, 62.0, 26.0)),
                y_slider: independent
                    .then(|| Rect::new(left + 152.0, input_top + 40.0, 300.0, 20.0)),
                independent_icon: Rect::new(left + 462.0, y + (height - 32.0) / 2.0, 32.0, 32.0),
                enable_switch: Rect::new(left + 509.0, y + (height - 18.0) / 2.0, 32.0, 18.0),
                drag_handle: Rect::new(left + 556.0, y, 10.0, height),
            };
            y += height + 4.0;
            result
        })
        .collect();
    let button_rows = polling_button_count.max(1).div_ceil(6);
    Some(PerformanceGeometry {
        sensitivity_widget: Rect::new(left, top, 600.0, dpi_height),
        polling_widget: Rect::new(
            polling_left,
            polling_top,
            600.0,
            166.0 + (button_rows - 1) as f64 * 37.0,
        ),
        // Source Roboto text width for English "Sensitivity Stages" is109.875.
        stage_toggle: Rect::new(left + 159.875, top + 113.0, 32.0, 18.0),
        stage_header: Rect::new(left + 40.0, top + 141.0, 520.0, 20.0),
        stages,
        polling_buttons: (0..polling_button_count)
            .map(|index| {
                Rect::new(
                    polling_left + 40.0 + (index % 6) as f64 * 82.0,
                    polling_top + 99.0 + (index / 6) as f64 * 37.0,
                    72.0,
                    27.0,
                )
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonlinear_anchors_roundtrip() {
        for (dpi, percent) in [
            (100, 0.0),
            (400, 25.0),
            (800, 37.5),
            (1600, 50.0),
            (10000, 75.0),
            (26000, 100.0),
        ] {
            assert_eq!(dpi_to_percent(dpi), Some(percent));
            assert_eq!(percent_to_dpi(percent), Some(dpi));
        }
    }

    #[test]
    fn every_range_midpoint_uses_its_own_scale() {
        for (dpi, percent) in [
            (250, 12.5),
            (600, 31.25),
            (1200, 43.75),
            (5800, 62.5),
            (18000, 87.5),
        ] {
            assert!((dpi_to_percent(dpi).unwrap() - percent).abs() < 1e-9);
            assert_eq!(percent_to_dpi(percent), Some(dpi));
        }
    }

    #[test]
    fn input_rounds_up_while_slider_rounds_nearest() {
        assert_eq!(normalize_input_value(Some(801)), 850);
        assert_eq!(normalize_input_value(Some(824)), 850);
        assert_eq!(percent_to_dpi(dpi_to_percent(824).unwrap()), Some(800));
        assert_eq!(normalize_input_value(Some(26001)), 26000);
        assert_eq!(normalize_input_value(Some(-10)), 100);
        assert_eq!(normalize_input_value(None), 100);
    }

    #[test]
    fn spinner_handles_transient_text_and_bounds() {
        assert_eq!(step_input_value(None, 50), 100);
        assert_eq!(step_input_value(None, -50), 100);
        assert_eq!(step_input_value(Some(800), 50), 850);
        assert_eq!(step_input_value(Some(800), -50), 750);
        assert_eq!(step_input_value(Some(100), -50), 100);
        assert_eq!(step_input_value(Some(26000), 50), 26000);
        assert_eq!(step_input_value(Some(801), -50), 800);
    }

    #[test]
    fn geometry_matches_source_replays() {
        let wide = geometry(1280.0, 86.0, 0.0, &[false; 5], 3).unwrap();
        assert_eq!(
            wide.sensitivity_widget,
            Rect::new(30.0, 106.0, 600.0, 547.0)
        );
        assert_eq!(
            wide.stages[2].x_slider,
            Rect::new(182.0, 430.0, 300.0, 20.0)
        );
        assert_eq!(wide.polling_buttons[0], Rect::new(690.0, 205.0, 72.0, 27.0));
        let narrow = geometry(1000.0, 86.0, 0.0, &[false; 5], 3).unwrap();
        assert_eq!(narrow.polling_widget, Rect::new(200.0, 673.0, 600.0, 166.0));
        let xy = geometry(1280.0, 86.0, 0.0, &[false, false, true, false, false], 3).unwrap();
        assert_eq!(xy.sensitivity_widget.height, 593.0);
        assert_eq!(
            xy.stages[2].y_input,
            Some(Rect::new(110.0, 468.5, 62.0, 26.0))
        );
    }
}
