//! Source color-editor facts from Ornata605 main.fcf07de2.js classCr/pr/lr.
//! No renderer, OS capture, vendor program execution or hardware operations.
//! The plane is hue/saturation; value uses a separate canvas. Canvas pixel
//! interpolation must not be replaced by an assumed standard SV picker.

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Source filterHexValue. In particular,3digits repeat the triplet instead of
/// applying CSS shorthand expansion. Invalid and empty inputs become black.
pub fn normalize_hex(value: &str) -> String {
    let expanded = match value.len() {
        1 => value.repeat(6),
        2 => value.repeat(3),
        3 => value.repeat(2),
        4 => format!("{value}ff"),
        5 => format!("{value}f"),
        6 => value.to_string(),
        _ => return "000000".into(),
    };
    if expanded.bytes().all(|b| b.is_ascii_hexdigit()) {
        expanded
    } else {
        "000000".into()
    }
}

/// lr's numeric regex has no start anchor, so any nonempty text ending in an
/// ASCII digit passes its lexical check. Browser maxLength is3UTF-16 units.
pub fn accepts_rgb_edit(value: &str) -> bool {
    value.encode_utf16().count() <= 3
        && (value.is_empty() || value.as_bytes().last().is_some_and(u8::is_ascii_digit))
}

pub fn accepts_hex_edit(value: &str) -> bool {
    value.len() <= 6 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn numeric_rgb_text(value: &str) -> Option<f64> {
    let text = value.trim_matches(
        |c: char| matches!(c as u32,9..=13|32|160|5760|8192..=8202|8232|8233|8239|8287|12288|65279),
    );
    if text.is_empty() {
        return Some(0.0);
    }
    for (prefix, base) in [
        ("0x", 16),
        ("0X", 16),
        ("0b", 2),
        ("0B", 2),
        ("0o", 8),
        ("0O", 8),
    ] {
        if let Some(tail) = text.strip_prefix(prefix) {
            return u64::from_str_radix(tail, base).ok().map(|v| v as f64);
        }
    }
    text.parse().ok()
}

/// Source filterRgbValue returns original accepted text, including empty or
/// fractional text. Out-of-range values become0; they are not clamped to255.
/// This is a draft-input helper, not an integer RGB validator or persistence API.
pub fn filter_rgb_text(value: &str) -> String {
    if numeric_rgb_text(value).is_some_and(|n| (0.0..=255.0).contains(&n)) {
        value.to_string()
    } else {
        "0".into()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hsv {
    pub h: u16,
    pub s: u8,
    pub v: u8,
}

/// Exact positive-domain rounded HSV helper used for cursor placement. Hue360
/// can occur near red and is retained rather than silently wrapped to0.
pub fn rgb_to_hsv(rgb: Rgb) -> Hsv {
    let r = f64::from(rgb.r) / 255.0;
    let g = f64::from(rgb.g) / 255.0;
    let b = f64::from(rgb.b) / 255.0;
    let max = r.max(g).max(b);
    let delta = max - r.min(g).min(b);
    let (mut hue, saturation) = (0.0, if delta == 0.0 { 0.0 } else { delta / max });
    if delta != 0.0 {
        let c = |channel: f64| (max - channel) / 6.0 / delta + 0.5;
        hue = if r == max {
            c(b) - c(g)
        } else if g == max {
            1.0 / 3.0 + c(r) - c(b)
        } else {
            2.0 / 3.0 + c(g) - c(r)
        };
        if hue < 0.0 {
            hue += 1.0
        } else if hue > 1.0 {
            hue -= 1.0
        }
    }
    Hsv {
        h: (360.0 * hue).round() as u16,
        s: (100.0 * saturation).round() as u8,
        v: (100.0 * max).round() as u8,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cursors {
    pub plane_left: f64,
    pub plane_top: f64,
    pub value_left: f64,
}

/// Plane cursor coordinates are relative to cp-wrapper's content origin;
/// value_left is relative to picked-row. They intentionally use different math.
pub fn cursors(rgb: Rgb) -> Cursors {
    let hsv = rgb_to_hsv(rgb);
    Cursors {
        plane_left: (f64::from(hsv.h) / 360.0 * 198.0).floor() - 7.0,
        plane_top: 118.0 - (f64::from(hsv.s) / 100.0 * 118.0).floor() - 7.0,
        value_left: (f64::from(hsv.v) / 100.0 * 122.0).floor() + 34.0,
    }
}

pub fn plane_event_point(x: f64, y: f64) -> Option<(f64, f64)> {
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some((x.clamp(0.0, 198.0), y.clamp(0.0, 118.0)))
}

pub fn value_event_sample(x: f64, css_width: f64) -> Option<(f64, f64)> {
    if !x.is_finite() || !css_width.is_finite() || css_width < 8.0 {
        return None;
    }
    let sample = x.max(4.0).min(css_width - 4.0);
    Some((sample, 30.0 + sample))
}

#[derive(Debug)]
pub struct Geometry {
    #[cfg(test)]
    pub opens_up: bool,
    pub palette: Rect,
    pub editor: Rect,
    pub plane_wrapper: Rect,
    pub plane_canvas: Rect,
    pub preview: Rect,
    pub value_bar: Rect,
    pub eyedropper: Rect,
    pub hex_input: Rect,
    pub rgb_inputs: [Rect; 3],
    pub input_labels: [Rect; 4],
    pub cancel: Rect,
    pub save: Rect,
    pub presets: Vec<Rect>,
    pub no_color: Rect,
    pub custom_label: Rect,
    pub custom_slots: Vec<Rect>,
}

/// Source chooses direction from250px palette height, even when opening the
/// taller269px editor. This can leave19px more overflow; no horizontal fitting
/// or additional editor-height clamping is established by the source.
#[cfg(test)]
pub fn geometry(anchor: Rect, viewport_height: f64) -> Option<Geometry> {
    if ![
        anchor.x,
        anchor.y,
        anchor.width,
        anchor.height,
        viewport_height,
    ]
    .iter()
    .all(|v| v.is_finite())
        || anchor.height != 27.0
    {
        return None;
    }
    let up = anchor.y + anchor.height + 2.0 + 250.0 >= viewport_height;
    geometry_with_direction(anchor,up)
}

/// Use the direction retained when opening. The source does not recompute its
/// direction on every viewport resize while the palette remains open.
pub fn geometry_with_direction(anchor:Rect,up:bool)->Option<Geometry>{
    if ![anchor.x,anchor.y,anchor.width,anchor.height].iter().all(|v|v.is_finite())||anchor.height!=27.0{return None}
    let py = if up {
        anchor.y - 251.0
    } else {
        anchor.y + 28.0
    };
    let ey = if up {
        anchor.y - 270.0
    } else {
        anchor.y + 29.0
    };
    let x = anchor.x;
    Some(Geometry {
        #[cfg(test)]
        opens_up: up,
        palette: at(x, py, 250.0, 250.0),
        editor: at(x, ey, 220.0, 269.0),
        plane_wrapper: at(x + 10.0, ey + 10.0, 200.0, 120.0),
        plane_canvas: at(x + 11.0, ey + 11.0, 198.0, 118.0),
        preview: at(x + 10.0, ey + 140.0, 24.0, 24.0),
        value_bar: at(x + 44.0, ey + 144.0, 130.0, 16.0),
        eyedropper: at(x + 186.0, ey + 141.0, 24.0, 24.0),
        hex_input: at(x + 10.0, ey + 174.0, 65.0, 26.0),
        rgb_inputs: [
            at(x + 80.0, ey + 174.0, 40.0, 26.0),
            at(x + 125.0, ey + 174.0, 40.0, 26.0),
            at(x + 170.0, ey + 174.0, 40.0, 26.0),
        ],
        input_labels: [
            at(x + 10.0, ey + 203.0, 65.0, 19.0),
            at(x + 80.0, ey + 203.0, 40.0, 19.0),
            at(x + 125.0, ey + 203.0, 40.0, 19.0),
            at(x + 170.0, ey + 203.0, 40.0, 19.0),
        ],
        cancel: at(x + 15.0, ey + 232.0, 90.0, 27.0),
        save: at(x + 115.0, ey + 232.0, 90.0, 27.0),
        presets: (0..40)
            .map(|i| {
                at(
                    x + 10.0 + (i % 8) as f64 * 30.0,
                    py + 11.0 + (i / 8) as f64 * 30.0,
                    20.0,
                    20.0,
                )
            })
            .collect(),
        no_color: at(x + 220.0, py + 161.0, 20.0, 20.0),
        custom_label: at(x + 1.0, py + 166.0, 248.0, 20.0),
        custom_slots: (0..16)
            .map(|i| {
                at(
                    x + 10.0 + (i % 8) as f64 * 30.0,
                    py + 191.0 + (i / 8) as f64 * 30.0,
                    20.0,
                    20.0,
                )
            })
            .collect(),
    })
}

pub fn context_menu(swatch: Rect) -> Rect {
    at(swatch.x + 10.0, swatch.y + 10.0, 90.0, 58.0)
}

/// pr.openPicker chooses an earlier empty slot rather than leave a gap.
pub fn picker_target(picked: &[bool], requested: usize) -> Option<usize> {
    if picked.len() != 16 || requested >= 16 {
        return None;
    }
    Some(
        picked
            .iter()
            .position(|picked| !*picked)
            .filter(|i| *i < requested)
            .unwrap_or(requested),
    )
}

/// Middleware persists all non-sentinel entries, including duplicates; caller
/// retains the complete foreign slot object instead of rebuilding its fields.
#[cfg(test)]
pub fn persistent_slot_indices(colors: &[&str]) -> Vec<usize> {
    colors
        .iter()
        .enumerate()
        .filter_map(|(i, color)| (*color != "#111").then_some(i))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hex_is_not_css_shorthand() {
        assert_eq!(normalize_hex("abc"), "abcabc");
        assert_eq!(normalize_hex("ab"), "ababab");
        assert_eq!(normalize_hex("f"), "ffffff");
        assert_eq!(normalize_hex("abcd"), "abcdff");
        assert_eq!(normalize_hex("abcde"), "abcdef");
        assert_eq!(normalize_hex(""), "000000");
        assert_eq!(normalize_hex("badz"), "000000");
    }
    #[test]
    fn rgb_drafts_preserve_source_quirks() {
        assert!(accepts_rgb_edit("a12"));
        assert!(accepts_rgb_edit("1.5"));
        assert!(accepts_rgb_edit("1e2"));
        assert!(!accepts_rgb_edit("."));
        assert_eq!(filter_rgb_text("999"), "0");
        assert_eq!(filter_rgb_text("-1"), "0");
        assert_eq!(filter_rgb_text(""), "");
        assert_eq!(filter_rgb_text("1.5"), "1.5");
        assert_eq!(filter_rgb_text("a12"), "0");
    }
    #[test]
    fn plane_is_hue_saturation_with_separate_value() {
        assert_eq!(
            rgb_to_hsv(Rgb { r: 255, g: 0, b: 0 }),
            Hsv {
                h: 0,
                s: 100,
                v: 100
            }
        );
        assert_eq!(rgb_to_hsv(Rgb { r: 255, g: 0, b: 1 }).h, 360);
        assert_eq!(
            cursors(Rgb { r: 255, g: 0, b: 0 }),
            Cursors {
                plane_left: -7.0,
                plane_top: -7.0,
                value_left: 156.0
            }
        );
        assert_eq!(value_event_sample(500.0, 130.0), Some((126.0, 156.0)));
        assert_eq!(plane_event_point(250.0, -2.0), Some((198.0, 0.0)));
    }
    #[test]
    fn measured_popup_boxes_and_flip_rule() {
        let a = at(690.0, 357.0, 53.0, 27.0);
        let down = geometry(a, 678.0).unwrap();
        assert_eq!(down.editor, at(690.0, 386.0, 220.0, 269.0));
        assert_eq!(down.plane_canvas, at(701.0, 397.0, 198.0, 118.0));
        assert_eq!(down.custom_slots[0], at(700.0, 576.0, 20.0, 20.0));
        assert_eq!(
            context_menu(down.custom_slots[0]),
            at(710.0, 586.0, 90.0, 58.0)
        );
        let up = geometry(a, 600.0).unwrap();
        assert_eq!(up.editor, at(690.0, 87.0, 220.0, 269.0));
        let overflow = geometry(a, 645.0).unwrap();
        assert!(!overflow.opens_up);
        assert!(overflow.editor.y + overflow.editor.height > 645.0);
    }
    #[test]
    fn custom_slots_use_exact_sentinel_and_earliest_gap() {
        let mut slots = [true; 16];
        slots[2] = false;
        assert_eq!(picker_target(&slots, 5), Some(2));
        assert_eq!(
            persistent_slot_indices(&["#111", "#111111", "#00ff00", "#00ff00"]),
            vec![1, 2, 3]
        );
    }
}
