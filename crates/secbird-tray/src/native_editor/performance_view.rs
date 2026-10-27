//! Product196 Performance rendering from recovered source and inert DOM bounds.
//! See mouse-performance-spec.json for source hashes, offsets and limitations.
//! Event dispatch and native input ownership remain in the parent editor.

use super::performance_layout::{self, PerformanceGeometry, Rect as SourceRect};
use super::*;
use std::ptr::null_mut;

pub(super) fn layout(state: &State) -> Option<(PerformanceGeometry, Vec<usize>)> {
    let device = state.device()?;
    if state.tab != Tab::Performance || !matches!(device["pid"].as_str(), Some("00C4" | "00C5")) {
        return None;
    }
    let performance = state.performance.as_ref()?;
    let active = usize::from(performance.active.checked_sub(1)?);
    if active >= performance.stages.len() {
        return None;
    }
    let rows: Vec<usize> = if performance.enabled {
        (0..performance.stages.len()).collect()
    } else {
        vec![active]
    };
    let independent: Vec<bool> = rows
        .iter()
        .map(|i| performance.stages[*i].independent)
        .collect();
    let polling_count = device["polling"]["options"].as_array().map_or(0, Vec::len);
    let width = f64::from(state.width) / f64::from(state.scale);
    Some((
        performance_layout::geometry(
            width,
            128.0,
            state.performance_scroll,
            &independent,
            polling_count,
        )?,
        rows,
    ))
}

fn rect(p: &Painter<'_>, r: SourceRect) -> RECT {
    let px = |v: f64| (v * f64::from(p.scale)).round() as i32;
    RECT {
        left: px(r.x),
        top: px(r.y),
        right: px(r.x + r.width),
        bottom: px(r.y + r.height),
    }
}

fn at(x: f64, y: f64, width: f64, height: f64) -> SourceRect {
    SourceRect {
        x,
        y,
        width,
        height,
    }
}

pub(super) fn faded(color: u32, background: u32, alpha: f64) -> u32 {
    let channel = |shift: u32| {
        (((color >> shift) & 255u32) as f64 * alpha
            + ((background >> shift) & 255u32) as f64 * (1.0 - alpha))
            .round() as u32
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}

unsafe fn fill(p: &Painter<'_>, r: SourceRect, color: u32) {
    let brush = CreateSolidBrush(rgb(color));
    FillRect(p.dc, &rect(p, r), brush);
    DeleteObject(brush);
}

pub(super) unsafe fn rounded(p: &Painter<'_>, r: SourceRect, color: u32, border: u32, radius: f64) {
    let brush = CreateSolidBrush(rgb(color));
    let pen = CreatePen(PS_SOLID, (p.scale.round() as i32).max(1), rgb(border));
    let old_b = SelectObject(p.dc, brush);
    let old_p = SelectObject(p.dc, pen);
    let r = rect(p, r);
    let diameter = (radius * 2.0 * f64::from(p.scale)).round() as i32;
    RoundRect(p.dc, r.left, r.top, r.right, r.bottom, diameter, diameter);
    SelectObject(p.dc, old_b);
    SelectObject(p.dc, old_p);
    DeleteObject(brush);
    DeleteObject(pen);
}

pub(super) unsafe fn text(
    p: &Painter<'_>,
    value: &str,
    r: SourceRect,
    font: HFONT,
    color: u32,
    align: u32,
) {
    let old = SelectObject(p.dc, font);
    SetBkMode(p.dc, TRANSPARENT as i32);
    SetTextColor(p.dc, rgb(color));
    let mut r = rect(p, r);
    DrawTextW(
        p.dc,
        wide(value).as_ptr(),
        -1,
        &mut r,
        align | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
    );
    SelectObject(p.dc, old);
}

pub(super) fn hit(p: &mut Painter<'_>, r: SourceRect, action: Action) {
    p.hits.push(Hit {
        rect: rect(p, r),
        action,
    });
}

unsafe fn pointer_inside(state: &State, p: &Painter<'_>, r: SourceRect) -> bool {
    let mut point = POINT { x: 0, y: 0 };
    GetCursorPos(&mut point) != 0
        && ScreenToClient(state.hwnd, &mut point) != 0
        && PtInRect(&rect(p, r), point) != 0
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SourceBlend {
    operation: u8,
    flags: u8,
    alpha: u8,
    format: u8,
}
#[link(name = "msimg32")]
extern "system" {
    #[link_name = "AlphaBlend"]
    fn source_alpha_blend(
        target: HDC,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        source: HDC,
        sx: i32,
        sy: i32,
        sw: i32,
        sh: i32,
        blend: SourceBlend,
    ) -> i32;
}

unsafe fn image(p: &Painter<'_>, bitmap: Option<&Bitmap>, r: SourceRect, alpha: f64) {
    let Some(bitmap) = bitmap else { return };
    let dc = CreateCompatibleDC(p.dc);
    if dc.is_null() {
        return;
    }
    let old = SelectObject(dc, bitmap.handle);
    let dst = rect(p, r);
    SetStretchBltMode(p.dc, HALFTONE as i32);
    SetBrushOrgEx(p.dc, 0, 0, null_mut());
    if alpha < 1.0 {
        source_alpha_blend(
            p.dc,
            dst.left,
            dst.top,
            dst.right - dst.left,
            dst.bottom - dst.top,
            dc,
            0,
            0,
            bitmap.width,
            bitmap.height,
            SourceBlend {
                operation: 0,
                flags: 0,
                alpha: (255.0 * alpha).round() as u8,
                format: 0,
            },
        );
    } else {
        StretchBlt(
            p.dc,
            dst.left,
            dst.top,
            dst.right - dst.left,
            dst.bottom - dst.top,
            dc,
            0,
            0,
            bitmap.width,
            bitmap.height,
            SRCCOPY,
        );
    }
    SelectObject(dc, old);
    DeleteDC(dc);
}

unsafe fn toggle(p: &Painter<'_>, r: SourceRect, enabled: bool, disabled: bool) {
    let a = if disabled { 0.3 } else { 1.0 };
    rounded(
        p,
        r,
        faded(if enabled { GREEN } else { 0x707070 }, BG, a),
        faded(0x4e4e4e, BG, a),
        9.0,
    );
    rounded(
        p,
        at(
            r.x + if enabled { 15.0 } else { 1.0 },
            r.y + 1.0,
            14.0,
            14.0,
        ),
        faded(PANEL, BG, a),
        faded(PANEL, BG, a),
        7.0,
    );
}

unsafe fn spinner(
    p: &mut Painter<'_>,
    state: &State,
    input: SourceRect,
    row: usize,
    axis: Axis,
    hovered: bool,
    value: u16,
) {
    // Native EDIT occupies x+7,y+1,55x24; this module draws no duplicate text.
    rounded(
        p,
        input,
        PANEL,
        if pointer_inside(state, p, input) {
            GREEN
        } else if hovered {
            TEXT
        } else {
            PANEL
        },
        0.0,
    );
    if !hovered {
        return;
    }
    for (up, delta, name, offset) in [
        (true, 1, "dpi-stepper-up", 5.0),
        (false, -1, "dpi-stepper-down", 3.0),
    ] {
        let target = at(
            input.x + input.width - 14.0,
            if up {
                input.y
            } else {
                input.y + input.height - 12.0
            },
            14.0,
            12.0,
        );
        let disabled = if up { value >= 26000 } else { value <= 100 };
        image(
            p,
            state.icons.get(name),
            at(target.x + 3.0, target.y + offset, 8.0, 8.0),
            if disabled { 0.3 } else { 1.0 },
        );
        if !disabled {
            hit(p, target, Action::PerfStep(row, axis, delta));
        }
    }
}

unsafe fn slider(
    p: &mut Painter<'_>,
    state: &State,
    r: SourceRect,
    value: u16,
    independent: bool,
    tag: &str,
    enabled: bool,
    row: usize,
    axis: Axis,
    grid_font: HFONT,
) {
    let alpha = if enabled { 1.0 } else { 0.3 };
    let background = if state.performance_hover == Some(row) {
        BG
    } else {
        PANEL
    };
    let percent = performance_layout::dpi_to_percent(u32::from(value)).unwrap_or(0.0);
    let track_alpha = 77.0 / 255.0; // CSS #44d62c4d
    fill(
        p,
        at(r.x, r.y + 7.0, r.width, 6.0),
        faded(faded(GREEN, background, track_alpha), background, alpha),
    );
    let tick = |p: &Painter<'_>, x: f64| {
        fill(
            p,
            at(x - 0.5, r.y + 2.0, 1.0, 16.0),
            faded(0x204c19, background, alpha),
        );
        // The track is translucent over the grid. Flatten that overlap before
        // applying the whole disabled row's opacity against its background.
        fill(
            p,
            at(x - 0.5, r.y + 7.0, 1.0, 6.0),
            faded(faded(GREEN, 0x204c19, track_alpha), background, alpha),
        );
    };
    let anchors = [
        (0.0, "100"),
        (25.0, "400"),
        (37.5, "800"),
        (50.0, "1600"),
        (75.0, "10000"),
        (100.0, "26000"),
    ];
    for (percent, label) in anchors {
        let x = r.x + r.width * percent / 100.0;
        tick(p, x);
        let (left, width, align) = if percent == 0.0 {
            (x, 44.0, DT_LEFT)
        } else if percent == 100.0 {
            (x - 44.0, 44.0, DT_RIGHT)
        } else {
            (x - 25.0, 50.0, DT_CENTER)
        };
        text(
            p,
            label,
            at(left, r.y + 26.0, width, 14.0),
            grid_font,
            faded(TEXT, background, alpha),
            align,
        );
    }
    for percent in [
        62.5, 65.0, 67.5, 70.0, 72.5, 77.5, 80.0, 82.5, 85.0, 87.5, 90.0, 92.5, 95.0, 97.5,
    ] {
        tick(p, r.x + r.width * percent / 100.0);
    }
    // Source range-item has z-index:auto. Track/fill/range are1/2/3, so
    // these layers must cover the grid rather than being crossed by its ticks.
    fill(
        p,
        at(
            r.x,
            r.y + 7.0,
            performance_layout::source_fill_width(percent, r.width),
            6.0,
        ),
        faded(GREEN, background, alpha),
    );
    // The range input's small thumb is12px; source fill retains its16px formula.
    let center = 6.0 + percent / 100.0 * (r.width - 12.0);
    rounded(
        p,
        at(r.x + center - 6.0, r.y + 4.0, 12.0, 12.0),
        faded(GREEN, background, alpha),
        faded(GREEN, background, alpha),
        6.0,
    );
    if independent {
        text(
            p,
            tag,
            at(
                r.x + performance_layout::source_thumb_tag_left(percent),
                r.y - 1.0,
                16.0,
                22.0,
            ),
            grid_font,
            faded(PANEL, background, alpha),
            DT_LEFT,
        );
    }
    if enabled {
        hit(
            p,
            at(r.x, r.y, r.width, 20.0),
            Action::PerfSlider(row, axis, r),
        );
    }
}

pub(super) unsafe fn paint(state: &State, p: &mut Painter<'_>, device: &Value) -> bool {
    let Some((g, rows)) = layout(state) else {
        return false;
    };
    let Some(performance) = state.performance.as_ref() else {
        return false;
    };
    let grid_font = CreateFontW(
        -((11.0 * p.scale).round() as i32),
        0,
        0,
        0,
        400,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        DEFAULT_PITCH as u32,
        wide("Roboto").as_ptr(),
    );
    let grid_font = if grid_font.is_null() {
        p.fonts.small
    } else {
        grid_font
    };
    for (r, title, description) in [
        (
            g.sensitivity_widget,
            "SENSITIVITY",
            "The number of dots-per-inch (DPI) of mouse movement.",
        ),
        (
            g.polling_widget,
            if device["pid"] == "00C4" {
                "WIRED POLLING RATE"
            } else {
                "POLLING RATE"
            },
            "The frequency (Hz) of data updates in a second.",
        ),
    ] {
        rounded(p, r, PANEL, PANEL, 5.0);
        text(
            p,
            title,
            at(r.x + 40.0, r.y + 30.0, 500.0, 22.0),
            p.fonts.widget_heading,
            GREEN,
            DT_LEFT,
        );
        text(
            p,
            description,
            at(r.x + 40.0, r.y + 72.0, 520.0, 17.0),
            p.fonts.normal,
            TEXT,
            DT_LEFT,
        );
    }
    let w = g.sensitivity_widget;
    text(
        p,
        "Sensitivity Stages",
        at(w.x + 40.0, w.y + 114.0, 140.0, 17.0),
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    toggle(p, g.stage_toggle, performance.enabled, false);
    hit(p, g.stage_toggle, Action::PerfEnabled);
    for (label, x) in [("DPI", 50.0), ("100", 110.0), ("26000", 375.0)] {
        text(
            p,
            label,
            at(g.stage_header.x + x, g.stage_header.y + 3.0, 65.0, 17.0),
            p.fonts.normal,
            TEXT,
            DT_LEFT,
        );
    }
    let mut ordinal = 0;
    let mut xy_tip = None;
    let active = usize::from(performance.active.saturating_sub(1));
    for (index, shape) in rows.iter().zip(&g.stages) {
        let row = &performance.stages[*index];
        let selected = *index == active;
        let hover = state.performance_hover == Some(*index);
        let background = if hover { BG } else { PANEL };
        let alpha = if row.visible { 1.0 } else { 0.3 };
        if hover {
            if pointer_inside(state, p, shape.independent_icon) {
                xy_tip = Some((
                    shape.independent_icon,
                    if row.independent {
                        "Disable X-Y"
                    } else {
                        "Enable X-Y"
                    },
                ));
            }
            rounded(p, shape.row, BG, BG, 3.0);
        }
        if let Some(PerformanceGesture::Reorder { row: from, target }) = &state.performance_gesture
        {
            if from == index {
                let composite = faded(0x44c62d, faded(GREEN, background, 0.2), 0.2);
                rounded(p, shape.row, composite, composite, 3.0);
            }
            if *target == *index {
                fill(
                    p,
                    at(
                        shape.row.x,
                        if target < from {
                            shape.row.y
                        } else {
                            shape.row.y + shape.row.height - 2.0
                        },
                        shape.row.width,
                        2.0,
                    ),
                    GREEN,
                );
            }
        }
        let ordinal_bg = if selected { GREEN } else { BG };
        rounded(
            p,
            shape.ordinal,
            faded(ordinal_bg, background, alpha),
            faded(ordinal_bg, background, alpha),
            15.0,
        );
        if row.visible {
            ordinal += 1;
            let shown = if performance.enabled { ordinal } else { 1 };
            let icon = format!(
                "dpi-stage-{shown}-{}",
                if selected { "active" } else { "default" }
            );
            let saved = SaveDC(p.dc);
            let clip = rect(
                p,
                at(shape.ordinal.x + 11.0, shape.ordinal.y + 3.0, 8.0, 6.0),
            );
            IntersectClipRect(p.dc, clip.left, clip.top, clip.right, clip.bottom);
            image(
                p,
                state.icons.get(icon.as_str()),
                at(shape.ordinal.x + 11.0, shape.ordinal.y + 3.0, 8.0, 8.0),
                alpha,
            );
            RestoreDC(p.dc, saved);
            text(
                p,
                &shown.to_string(),
                at(shape.ordinal.x, shape.ordinal.y + 10.0, 30.0, 17.0),
                p.fonts.normal,
                if selected { PANEL } else { TEXT },
                DT_CENTER,
            );
            hit(p, shape.ordinal, Action::PerfSelect(*index));
        }
        spinner(p, state, shape.x_input, *index, Axis::X, hover, row.x);
        slider(
            p,
            state,
            shape.x_slider,
            row.x,
            row.independent,
            "X",
            row.visible,
            *index,
            Axis::X,
            grid_font,
        );
        if let (Some(input), Some(track)) = (shape.y_input, shape.y_slider) {
            spinner(p, state, input, *index, Axis::Y, hover, row.y);
            slider(
                p,
                state,
                track,
                row.y,
                true,
                "Y",
                row.visible,
                *index,
                Axis::Y,
                grid_font,
            );
        }
        if hover {
            image(
                p,
                state.icons.get(if !row.visible {
                    "dpi-xy-disabled"
                } else if row.independent {
                    "dpi-xy-active"
                } else {
                    "dpi-xy"
                }),
                shape.independent_icon,
                alpha,
            );
            hit(p, shape.independent_icon, Action::PerfIndependent(*index));
            if performance.enabled {
                let disabled = row.visible && performance.visible_count() <= 2;
                toggle(p, shape.enable_switch, row.visible, disabled);
                if !disabled {
                    hit(p, shape.enable_switch, Action::PerfVisible(*index));
                }
                image(
                    p,
                    state.icons.get("dpi-drag"),
                    at(
                        shape.drag_handle.x,
                        shape.drag_handle.y + (shape.drag_handle.height - 16.0) / 2.0,
                        10.0,
                        16.0,
                    ),
                    alpha,
                );
                hit(p, shape.drag_handle, Action::PerfDrag(*index));
            }
        }
    }
    if let Some(options) = device["polling"]["options"].as_array() {
        let active = device["polling"]["hz"].as_u64();
        for (value, r) in options.iter().zip(&g.polling_buttons) {
            if let Some(hz) = value.as_u64() {
                rounded(
                    p,
                    *r,
                    BG,
                    if active == Some(hz) || pointer_inside(state, p, *r) {
                        GREEN
                    } else {
                        BORDER
                    },
                    3.0,
                );
                text(p, &hz.to_string(), *r, p.fonts.normal, TEXT, DT_CENTER);
                hit(p, *r, Action::Poll(hz as u32));
            }
        }
    }
    if let Some((icon, label)) = xy_tip {
        let old = SelectObject(p.dc, p.fonts.normal);
        let value = wide(label);
        let mut size = SIZE { cx: 0, cy: 0 };
        GetTextExtentPoint32W(p.dc, value.as_ptr(), (value.len() - 1) as i32, &mut size);
        SelectObject(p.dc, old);
        let width = f64::from(size.cx) / f64::from(p.scale);
        let tip = at(icon.x + 20.0, icon.y + 35.0, width + 22.0, 34.0);
        rounded(p, tip, 0, BORDER, 0.0);
        text(
            p,
            label,
            at(tip.x + 11.0, tip.y + 9.0, width, 16.0),
            p.fonts.normal,
            TEXT,
            DT_LEFT,
        );
    }
    if grid_font != p.fonts.small {
        DeleteObject(grid_font);
    }
    true
}
