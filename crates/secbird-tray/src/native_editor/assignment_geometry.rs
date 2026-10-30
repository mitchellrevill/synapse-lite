//! Source Customize control geometry from ordered CSS and inert font/DOM replay.
//! Mouse label bounds are actual source DOM controls, not guessed button boxes.
//! Evidence: assignment-details-replay.json and mouse-customize source index.
use super::assignment_layout::{Rect, Shell};
use super::assignment_model::Category;
#[cfg(test)]
use super::assignment_layout as shell;
fn r(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
pub(super) fn contains(r: Rect, x: f64, y: f64) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}
pub(super) fn translate(r: Rect, dx: f64, dy: f64) -> Rect {
    self::r(r.x + dx, r.y + dy, r.width, r.height)
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Form {
    pub heading: Rect,
    pub display: Rect,
    pub dropdown: Rect,
    pub checkbox: Rect,
    pub label: Rect,
    pub stepper: Rect,
    pub input: Rect,
    pub up: Rect,
    pub down: Rect,
    pub slider: Rect,
    pub track: Rect,
    pub cancel: Rect,
    pub save: Rect,
    pub bottom: f64,
}
pub(super) fn form(
    g: Shell,
    pid: u16,
    category: Option<Category>,
    turbo: bool,
    allow_turbo: bool,
    scroll: f64,
) -> Form {
    let x = g.body.x + 20.0;
    let y = g.body.y - scroll;
    let foot = match category {
        Some(Category::Default) => {
            if pid == 605 {
                98.0
            } else {
                108.0
            }
        }
        Some(Category::Windows) => 170.0,
        Some(Category::Disable) => 59.0,
        Some(Category::Launch) => 240.0,
        Some(Category::Mouse) => {
            if !allow_turbo {
                116.0
            } else if turbo {
                238.0
            } else {
                145.0
            }
        }
        Some(Category::Keyboard) => if turbo && allow_turbo { 413.0 } else { 320.0 },
        _ => 149.0,
    };
    let mut form = Form {
        heading: r(x, y + 20.0, 210.0, 19.0),
        display: r(x, y + 59.0, 210.0, 29.0),
        dropdown: r(x, y + 59.0, 210.0, 27.0),
        checkbox: r(x, y + 96.0, 210.0, 20.0),
        label: r(x, y + 125.0, 210.0, 17.0),
        stepper: r(x, y + 147.0, 58.0, 25.0),
        input: r(x + 7.0, y + 148.0, 50.390625, 24.0),
        up: r(x + 39.0, y + 148.0, 18.0, 12.0),
        down: r(x + 39.0, y + 159.0, 18.0, 12.0),
        slider: r(x, y + 182.0, 210.0, 36.0),
        track: r(x, y + 187.0, 210.0, 6.0),
        cancel: r(x, y + foot, 100.0, 27.0),
        save: r(x + 110.0, y + foot, 100.0, 27.0),
        bottom: y + foot + 27.0,
    };
    if category == Some(Category::Keyboard) {
        form.dropdown.y += 37.0;
        for field in [&mut form.checkbox, &mut form.label, &mut form.stepper, &mut form.input, &mut form.up, &mut form.down, &mut form.slider, &mut form.track] {field.y += 175.0;}
    }
    form
}
pub(super) fn options(dropdown: Rect, viewport_bottom: f64) -> Rect {
    let height = 180.0;
    let top = if dropdown.y + dropdown.height + 2.0 + height > viewport_bottom {
        dropdown.y - height - 1.0
    } else {
        dropdown.y + 28.0
    };
    r(dropdown.x, top, 210.0, height)
}
#[cfg(test)]
pub(super) fn option_rows(panel: Rect, scroll: f64) -> Vec<Rect> {
    option_rows_count(panel, scroll, 10)
}
pub(super) fn option_rows_count(panel: Rect, scroll: f64, count: usize) -> Vec<Rect> {
    let offset = scroll.clamp(0.0, (count as f64 * 25.0 - (panel.height - 2.0)).max(0.0));
    (0..count)
        .map(|i| {
            r(
                panel.x + 1.0,
                panel.y + 1.0 + i as f64 * 25.0 - offset,
                208.0,
                25.0,
            )
        })
        .collect()
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Scrollbar {
    pub track: Rect,
    pub thumb: Rect,
    pub max: f64,
}
pub(super) fn scrollbar(bounds: Rect, content: f64, scroll: f64, width: f64) -> Option<Scrollbar> {
    let max = (content - bounds.height).max(0.0);
    if max <= 0.0 {
        return None;
    }
    let height = (bounds.height * bounds.height / content)
        .max(20.0)
        .min(bounds.height);
    let y = bounds.y + scroll.clamp(0.0, max) / max * (bounds.height - height);
    Some(Scrollbar {
        track: r(
            bounds.x + bounds.width - width,
            bounds.y,
            width,
            bounds.height,
        ),
        thumb: r(bounds.x + bounds.width - width, y, width, height),
        max,
    })
}
impl Scrollbar {
    pub fn drag(self, y: f64, grab: f64) -> f64 {
        ((y - self.track.y - grab) / (self.track.height - self.thumb.height) * self.max)
            .clamp(0.0, self.max)
    }
}
#[derive(Clone, Debug)]
pub(super) struct Mouse {
    pub canvas: Rect,
    pub image: Rect,
    pub buttons: [Rect; 8],
    pub drawer: Rect,
    pub hyper: Rect,
    pub standard: Rect,
    pub shifted: Rect,
    pub help: Rect,
}
pub(super) const MOUSE_LABEL_WIDTHS: [f64; 8] = [
    78.1875, 85.953125, 88.8125, 75.421875, 93.890625, 118.515625, 118.515625, 187.515625,
];
pub(super) fn mouse(
    width: f64,
    drawer: bool,
    scroll_x: f64,
    scroll_y: f64,
    label_widths: [f64; 8],
) -> Mouse {
    let center = if drawer {
        ((width + 230.0) / 2.0).max(750.0)
    } else {
        width / 2.0
    } - scroll_x;
    let top = 148.0 - scroll_y;
    let canvas = r(center - 155.0, top, 310.0, 340.0);
    let mut buttons = [r(0.0, 0.0, 0.0, 0.0); 8];
    for (row, index) in [0usize, 2, 5, 6].into_iter().enumerate() {
        buttons[index] = r(
            center - 175.0 - label_widths[index],
            top + row as f64 * 50.0,
            label_widths[index],
            30.0,
        );
    }
    for (row, index) in [1usize, 3, 4, 7].into_iter().enumerate() {
        buttons[index] = r(
            center + 175.0,
            top + row as f64 * 50.0,
            label_widths[index],
            30.0,
        );
    }
    // Group is 249.625px wide, including the source help control and 10px gaps.
    let control_center = center;
    let toggle_x = control_center - 124.8125;
    Mouse {
        canvas,
        image: r(center - 150.0, top, 300.0, 340.0),
        buttons,
        drawer: r(toggle_x, top + 364.5, 38.0, 27.0),
        hyper: r(toggle_x + 48.0, top + 360.0, 177.609375, 36.0),
        standard: r(toggle_x + 54.0, top + 366.0, 76.234375, 24.0),
        shifted: r(toggle_x + 135.234375, top + 366.0, 84.375, 24.0),
        help: r(toggle_x + 235.609375, top + 371.0, 14.0, 14.0),
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Alert {
    pub panel: Rect,
    pub title: Rect,
    pub first: Rect,
    pub second: Rect,
    pub close: Rect,
    pub discard: Rect,
    pub save: Rect,
}
pub(super) fn alert(width: f64, client_height: f64) -> Alert {
    let product_height = client_height - 42.0;
    let top = 42.0 + product_height * 0.3 - 106.5;
    let x = (width - 400.0) / 2.0;
    Alert {
        panel: r(x, top, 400.0, 213.0),
        title: r(x + 31.0, top + 21.0, 338.0, 19.0),
        first: r(x + 31.0, top + 60.0, 338.0, 34.0),
        second: r(x + 31.0, top + 111.0, 338.0, 34.0),
        close: r(x + 372.0, top + 8.0, 20.0, 20.0),
        discard: r(x + 95.0, top + 165.0, 100.0, 27.0),
        save: r(x + 205.0, top + 165.0, 100.0, 27.0),
    }
}
pub(super) fn keyboard_center(width: f64, drawer: bool) -> f64 {
    if drawer {
        ((width + 230.0) / 2.0).max(750.0)
    } else {
        width / 2.0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn populated_source_controls_resolve_keyboard_center_without_empty_flex_artifact() {
        for (width, closed, open) in [
            (1280.0, 640.0, 755.0),
            (1266.0, 633.0, 750.0),
            (1000.0, 500.0, 750.0),
        ] {
            assert_eq!(keyboard_center(width, false), closed);
            assert_eq!(keyboard_center(width, true), open);
        }
    }
    #[test]
    fn forms_match_source_replay_not_a_common_footer() {
        let g = shell::shell(678.0, 42.0, 100.0, 36.0, false).unwrap();
        assert_eq!(
            form(g, 605, Some(Category::Default), false, true, 0.0)
                .cancel
                .y,
            267.0
        );
        assert_eq!(
            form(g, 196, Some(Category::Default), false, true, 0.0)
                .cancel
                .y,
            277.0
        );
        assert_eq!(
            form(g, 605, Some(Category::Disable), false, true, 0.0)
                .cancel
                .y,
            228.0
        );
        let on = form(g, 605, Some(Category::Mouse), true, true, 0.0);
        assert_eq!(on.stepper, r(161.0, 316.0, 58.0, 25.0));
        assert_eq!(on.track, r(161.0, 356.0, 210.0, 6.0));
        assert_eq!(on.cancel.y, 407.0);
        assert_eq!(
            form(g, 605, Some(Category::Mouse), false, true, 0.0)
                .cancel
                .y,
            314.0
        );
    }
    #[test]
    fn mouse_labels_are_source_dom_bounds_and_drawer_minimum_is_preserved() {
        let wide = mouse(1280.0, false, 0.0, 0.0, MOUSE_LABEL_WIDTHS);
        assert_eq!(wide.image, r(490.0, 148.0, 300.0, 340.0));
        assert_eq!(wide.buttons[0], r(386.8125, 148.0, 78.1875, 30.0));
        assert_eq!(wide.buttons[7], r(815.0, 298.0, 187.515625, 30.0));
        let narrow = mouse(1000.0, true, 0.0, 0.0, MOUSE_LABEL_WIDTHS);
        assert_eq!(narrow.image.x, 600.0);
        assert_eq!(narrow.buttons[1].x, 925.0);
        assert_eq!(
            mouse(1266.0, true, 0.0, 0.0, MOUSE_LABEL_WIDTHS).image.x,
            600.0
        );
    }
    #[test]
    fn dialog_matches_source_centred_400_by213_with_separate_host() {
        let g = alert(1280.0, 720.0);
        assert!((g.panel.y - 138.9).abs() < 0.001);
        assert_eq!(g.panel.width, 400.0);
        assert_eq!(g.discard.x, 535.0);
        assert!((g.discard.y - 303.9).abs() < 0.001);
    }
    #[test]
    fn scroll_thumb_drag_clamps_and_option_rows_keep_source25px() {
        let b = scrollbar(r(10.0, 20.0, 250.0, 500.0), 600.0, 0.0, 5.0).unwrap();
        assert_eq!(b.drag(-100.0, 0.0), 0.0);
        assert_eq!(b.drag(10000.0, 0.0), 100.0);
        let box_ = options(r(100.0, 200.0, 210.0, 27.0), 600.0);
        assert_eq!(box_, r(100.0, 228.0, 210.0, 180.0));
        assert_eq!(option_rows(box_, 0.0)[0], r(101.0, 229.0, 208.0, 25.0));
    }
}
