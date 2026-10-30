//! Installed Customize popup projection. This module never submits a device job.
//! Positions come from assignment_layout + inert shipped CSS/font replay.
use super::assignment_fonts::Role;
use super::assignment_geometry as geo;
use super::assignment_layout::{self as shell, Rect};
use super::assignment_model::{self as model, Anchor, Category, Draft, Input, Transition};
use super::*;
pub(super) const SPEED: u16 = 1600;
#[derive(Clone)]
pub(super) enum Control {
    Block,
    Open(Input, Anchor),
    Close,
    Cancel,
    Save,
    Category(Category),
    Options,
    Option(usize),
    Turbo,
    Step(i8),
    Slider(Rect),
    Drawer,
    Layer(bool),
    Filter,
    FilterChoice(bool),
    Dismiss,
    Discard,
    RailThumb(f64),
    DrawerThumb(f64),
    FormThumb(f64),
    OptionsThumb(f64),
}
fn r(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
fn dimensions(s: &State) -> (f64, f64) {
    (
        s.width as f64 / s.scale as f64,
        s.height as f64 / s.scale as f64,
    )
}
pub(super) fn covered(s: &State) -> bool {
    s.tab == Tab::Customize
        && s.device()
            .is_some_and(|d| matches!(d["pid"].as_str(), Some("025D" | "00C4" | "00C5")))
}
fn viewport(s: &State) -> source_layout::Viewport {
    let (w, _) = dimensions(s);
    let center = geo::keyboard_center(w, s.assignment.drawer) - s.assignment.scroll_x;
    source_layout::Viewport {
        left: center - 365.0,
        top: 154.0 - s.assignment.scroll_y,
        width: 730.0,
        height: 387.0,
    }
}
fn input(k: &source_layout::KeyRegion) -> Input {
    Input {
        key: k.button_key.clone(),
        input_id: k.input_id.clone(),
        input_type: k.input_type.clone(),
        label: k.label.clone(),
        default_label: k.label.clone(),
        functions: k.function_list.clone(),
        enabled: k.enabled,
    }
}
pub(super) fn shell(s: &State) -> Option<shell::Shell> {
    let d = s.assignment.draft.as_ref()?;
    let (w, h) = dimensions(s);
    let anchor = match &d.anchor {
        Anchor::Key(id) => {
            let b = s.keyboard_layout.as_ref()?.bounds(id, viewport(s))?;
            r(b.left, b.top, b.right - b.left, b.bottom - b.top)
        }
        Anchor::Mouse(i) => {
            geo::mouse(
                w,
                s.assignment.drawer,
                s.assignment.scroll_x,
                s.assignment.scroll_y,
                geo::MOUSE_LABEL_WIDTHS,
            )
            .buttons[*i]
        }
        Anchor::Drawer(_) => r(0.0, 0.0, 230.0, 0.0),
    };
    let left = shell::popup_left(
        anchor,
        Some(w),
        s.assignment.scroll_x,
        292.0,
        s.assignment.drawer,
        matches!(d.anchor, Anchor::Drawer(_)),
        230.0,
    );
    shell::shell(
        h - 42.0,
        42.0,
        left,
        36.0 - s.assignment.scroll_y,
        d.rail_expanded,
    )
}
fn box_px(p: &Painter, r: Rect) -> RECT {
    let px = |v: f64| (v * p.scale as f64).round() as i32;
    RECT {
        left: px(r.x),
        top: px(r.y),
        right: px(r.x + r.width),
        bottom: px(r.y + r.height),
    }
}
unsafe fn fill(p: &Painter, r: Rect, c: u32) {
    let b = CreateSolidBrush(rgb(c));
    FillRect(p.dc, &box_px(p, r), b);
    DeleteObject(b);
}
unsafe fn border(p: &Painter, r: Rect, c: u32) {
    let b = CreateSolidBrush(rgb(c));
    FrameRect(p.dc, &box_px(p, r), b);
    DeleteObject(b);
}
unsafe fn text(p: &Painter, t: &str, r: Rect, font: HFONT, c: u32, flags: u32) {
    let old = SelectObject(p.dc, font);
    SetBkMode(p.dc, TRANSPARENT as i32);
    SetTextColor(p.dc, rgb(c));
    let mut b = box_px(p, r);
    DrawTextW(p.dc, wide(t).as_ptr(), -1, &mut b, flags | DT_NOPREFIX);
    SelectObject(p.dc, old);
}
/// Position the glyph baseline from the source line box, rather than centering
/// GDI's larger font metrics inside that box. Glyph advances remain native GDI.
unsafe fn baseline_text(
    p: &Painter,
    t: &str,
    r: Rect,
    font: HFONT,
    color: u32,
    flags: u32,
    role: Role,
) -> bool {
    if matches!(role, Role::Heading)
        && p.source_text(
            t,
            source_text::Role::AssignmentHeading,
            source_text::SourceBox {
                left: r.x as f32,
                top: r.y as f32,
                width: r.width as f32,
            },
            box_px(p, r),
            color,
        )
    {
        return true;
    }
    let Some(y) = role.pixel(r.y, p.scale as f64) else {
        return false;
    };
    // Keep the application clip separate from an open glyph path. The native
    // vector regression exercises the same drawing primitive as the live view.
    let previous_clip = CreateRectRgn(0, 0, 0, 0);
    if previous_clip.is_null() {
        return false;
    }
    let had_clip = GetClipRgn(p.dc, previous_clip);
    if had_clip < 0 {
        DeleteObject(previous_clip);
        return false;
    }
    let previous_font = SelectObject(p.dc, font);
    let previous_align = SetTextAlign(p.dc, TA_BASELINE | TA_LEFT);
    let previous_mode = SetBkMode(p.dc, TRANSPARENT as i32);
    let previous_color = SetTextColor(p.dc, rgb(color));
    let value = wide(t);
    let count = value.len().saturating_sub(1) as u32;
    let bounds = box_px(p, r);
    IntersectClipRect(p.dc, bounds.left, bounds.top, bounds.right, bounds.bottom);
    let mut size: SIZE = std::mem::zeroed();
    GetTextExtentPoint32W(p.dc, value.as_ptr(), count as i32, &mut size);
    let x = if flags & DT_CENTER != 0 {
        bounds.left + (bounds.right - bounds.left - size.cx) / 2
    } else if flags & DT_RIGHT != 0 {
        bounds.right - size.cx
    } else {
        bounds.left
    };
    let emitted = ExtTextOutW(p.dc, x, y, 0, null(), value.as_ptr(), count, null());
    SelectClipRgn(
        p.dc,
        if had_clip == 1 {
            previous_clip
        } else {
            null_mut()
        },
    );
    DeleteObject(previous_clip);
    SetTextAlign(p.dc, previous_align);
    SetBkMode(p.dc, previous_mode);
    SetTextColor(p.dc, previous_color);
    SelectObject(p.dc, previous_font);
    emitted != 0
}

fn hit(p: &mut Painter, r: Rect, c: Control) {
    let mut clip: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        GetClipBox(p.dc, &mut clip);
    }
    let mut bounds = box_px(p, r);
    bounds.left = bounds.left.max(clip.left);
    bounds.top = bounds.top.max(clip.top);
    bounds.right = bounds.right.min(clip.right);
    bounds.bottom = bounds.bottom.min(clip.bottom);
    if bounds.right > bounds.left && bounds.bottom > bounds.top {
        p.hits.push(Hit {
            rect: bounds,
            action: Action::Assignment(c),
        });
    }
}

unsafe fn rounded(p: &Painter, r: Rect, background: u32, stroke: u32, radius: f64) {
    performance_view::rounded(
        p,
        performance_layout::Rect {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        },
        background,
        stroke,
        radius,
    );
}
unsafe fn button(p: &mut Painter, t: &str, r: Rect, control: Option<Control>) {
    let enabled = control.is_some();
    let save = t == "SAVE";
    let opacity = if enabled { 1.0 } else { 0.3 };
    let bg = performance_view::faded(if save { GREEN } else { 0x707070 }, 0x111111, opacity);
    let fg = performance_view::faded(if save { 0 } else { 0xffffff }, 0x111111, opacity);
    rounded(p, r, bg, performance_view::faded(0, 0x111111, opacity), 3.0);
    baseline_text(
        p,
        t,
        r,
        p.fonts.small,
        fg,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        Role::Button,
    );
    hit(p, r, control.unwrap_or(Control::Block));
}
unsafe fn checkbox(p: &Painter, r: Rect, on: bool, over: bool) {
    rounded(
        p,
        r,
        if on { GREEN } else { 0x111111 },
        if on || over { GREEN } else { 0x737373 },
        2.4,
    );
    if on {
        for (left, top, height, degrees) in [(8.6, 16.4, 15.4, -145.0f64), (0.8, 10.2, 9.6, -50.0)]
        {
            let angle = degrees.to_radians();
            let origin = (r.x + 1.0 + left, r.y + 1.0 + top);
            let points: Vec<POINT> = [(0.0, 0.0), (3.0, 0.0), (3.0, height), (0.0, height)]
                .into_iter()
                .map(|(x, y)| POINT {
                    x: ((origin.0 + x * angle.cos() - y * angle.sin()) * p.scale as f64).round()
                        as i32,
                    y: ((origin.1 + x * angle.sin() + y * angle.cos()) * p.scale as f64).round()
                        as i32,
                })
                .collect();
            let b = CreateSolidBrush(rgb(0x111111));
            let old = SelectObject(p.dc, b);
            let pen = SelectObject(p.dc, GetStockObject(NULL_PEN));
            Polygon(p.dc, points.as_ptr(), 4);
            SelectObject(p.dc, pen);
            SelectObject(p.dc, old);
            DeleteObject(b);
        }
    }
}

unsafe fn scroll_thumb(s: &State, p: &Painter, bar: geo::Scrollbar, background: u32, active: bool) {
    let color = if active {
        0x2f941e
    } else if hover(s, bar.thumb) {
        GREEN
    } else {
        performance_view::faded(0xffffff, background, 0.3)
    };
    rounded(
        p,
        r(
            bar.thumb.x + 1.0,
            bar.thumb.y + 1.0,
            (bar.thumb.width - 2.0).max(1.0),
            (bar.thumb.height - 2.0).max(1.0),
        ),
        color,
        color,
        4.0,
    );
}

unsafe fn image(s: &State, p: &Painter, key: &str, r: Rect) {
    p.image(
        s.icons.get(key),
        r.x.round() as i32,
        r.y.round() as i32,
        r.width.round() as i32,
        r.height.round() as i32,
    );
}
fn hover(s: &State, r: Rect) -> bool {
    s.assignment
        .hover
        .is_some_and(|(x, y)| geo::contains(r, x, y))
}
fn inputs(s: &State) -> Vec<Input> {
    if s.device().is_some_and(|d| d["pid"] == "025D") {
        s.keyboard_layout
            .as_ref()
            .map(|l| {
                l.keys
                    .iter()
                    .filter(|k| {
                        k.side_panel != Some(false)
                            && k.key_toggle.is_none_or(|layer| layer == s.assignment.hyper)
                    })
                    .map(input)
                    .collect()
            })
            .unwrap_or_default()
    } else {
        model::mouse_inputs()
    }
}
fn visible_inputs(s: &State) -> Vec<Input> {
    let mut inputs = inputs(s);
    if s.assignment.customized_only {
        let context = s.device().and_then(model::Context::from_device);
        inputs.retain(|i| {
            context
                .as_ref()
                .and_then(|c| c.mappings.as_ref())
                .is_some_and(|m| {
                    m.iter().any(|v| {
                        v["inputID"] == i.input_id
                            && v["isHyperShift"].as_bool().unwrap_or(false) == s.assignment.hyper
                    })
                })
        });
    } else if s.device().is_some_and(|d| d["kind"] == "mouse") {
        inputs.sort_by_key(|i| i.label.parse::<u32>().unwrap_or(u32::MAX));
    }
    inputs
}
unsafe fn trace_keyboard_selection(
    layout: &source_layout::SourceLayout,
    dc: HDC,
    input_id: &str,
    viewport: source_layout::Viewport,
    scale: f64,
) -> bool {
    layout.trace_key_path(dc, input_id, source_layout::Viewport {
        left: viewport.left * scale,
        top: viewport.top * scale,
        width: viewport.width * scale,
        height: viewport.height * scale,
    })
}

pub(super) unsafe fn paint_customize(s: &State, p: &mut Painter) -> bool {
    if !covered(s) {
        return false;
    }
    let (w, h) = dimensions(s);
    let clip = SaveDC(p.dc);
    IntersectClipRect(p.dc, 0, p.px(128), s.width, s.height);
    let center = geo::keyboard_center(w, s.assignment.drawer) - s.assignment.scroll_x;
    let width = (w - if s.assignment.drawer { 230.0 } else { 0.0 }).clamp(770.0, 1220.0);
    let background = r(center - width / 2.0, 148.0 - s.assignment.scroll_y, width, 340.0);
    s.customize_background.borrow_mut().draw(p.dc, box_px(p, background), background, p.scale as f64);
    if s.device().is_some_and(|d| d["pid"] == "025D") {
        let v = viewport(s);
        if s.keyboard_customize.is_none() || s.keyboard_layout.is_none() {
            text(
                p,
                "Source keyboard artwork or key geometry unavailable",
                r(230.0, 250.0, (w - 260.0).max(1.0), 40.0),
                p.fonts.normal,
                0x999999,
                DT_SINGLELINE | DT_CENTER,
            );
        }
        p.image(
            s.keyboard_customize.as_ref(),
            v.left.round() as i32,
            v.top.round() as i32,
            730,
            340,
        );
        if let (Some(l), Some(d)) = (&s.keyboard_layout, &s.assignment.draft) {
            // GDI paths use physical pixels; hits and popup anchors use logical pixels.
            if trace_keyboard_selection(l, p.dc, &d.input.input_id, v, p.scale as f64) {
                let pen = CreatePen(
                    PS_SOLID,
                    p.px(1),
                    rgb(if s.assignment.hyper { 0xfd8611 } else { GREEN }),
                );
                let old = SelectObject(p.dc, pen);
                StrokePath(p.dc);
                SelectObject(p.dc, old);
                DeleteObject(pen);
            }
        }
    } else {
        let g = geo::mouse(
            w,
            s.assignment.drawer,
            s.assignment.scroll_x,
            s.assignment.scroll_y,
            geo::MOUSE_LABEL_WIDTHS,
        );
        let mouse_inputs = model::mouse_inputs();
        let hover_index = mouse_inputs
            .iter()
            .enumerate()
            .find(|(index, item)| item.enabled && hover(s, g.buttons[*index]))
            .map(|(index, _)| index);
        let active_index = s.assignment.draft.as_ref().and_then(|d| {
            mouse_inputs
                .iter()
                .position(|i| i.input_id == d.input.input_id)
        });
        let cycle = hover_index == Some(7) || active_index == Some(7);
        let image = if cycle {
            s.icons.get("assignment-mouse-cycle").or(s.mouse.as_ref())
        } else {
            s.mouse.as_ref()
        };
        p.image(image, g.image.x as i32, g.image.y as i32, 300, 340);
        for (i, item) in model::mouse_inputs().into_iter().enumerate() {
            let b = g.buttons[i];
            let active = s
                .assignment
                .draft
                .as_ref()
                .is_some_and(|d| d.input.input_id == item.input_id);
            if active || hover(s, b) {
                fill(p, b, if active { 0x111111 } else { 0x383838 });
            }
            baseline_text(
                p,
                &item.default_label,
                r(b.x + 10.0, b.y, b.width - 20.0, b.height),
                p.fonts.normal,
                if !item.enabled { 0x555555 } else { 0xcccccc },
                DT_VCENTER | DT_SINGLELINE,
                Role::MouseLabel,
            );
            if item.enabled {
                hit(p, b, Control::Open(item, Anchor::Mouse(i)));
            }
        }
        // Source connector paths end in the clickable label dots; these are decoration.
        let ends = [
            (0, 114.0, 58.0, true),
            (2, 147.0, 88.0, true),
            (5, 88.0, 145.0, true),
            (6, 88.0, 175.0, true),
            (1, 181.0, 58.0, false),
            (3, 148.0, 75.0, false),
            (4, 148.0, 105.0, false),
            (7, 210.0, 166.0, false),
        ];
        for (index, mut x, y, left) in ends {
            if index == 7 && hover_index == Some(7) {
                x = 188.0;
            }
            let focused = active_index == Some(index) || hover_index == Some(index);
            let color = if !mouse_inputs[index].enabled {
                0x333333
            } else if focused {
                if s.assignment.hyper {
                    0xfd8611
                } else {
                    GREEN
                }
            } else {
                0x5d5d5d
            };
            let row = if left {
                [0, 2, 5, 6].iter().position(|v| *v == index).unwrap()
            } else {
                [1, 3, 4, 7].iter().position(|v| *v == index).unwrap()
            };
            let start_x = g.canvas.x + if left { 3.0 } else { 307.0 };
            let start_y = g.canvas.y + 16.0 + row as f64 * 50.0;
            let elbow = start_x + if left { 53.0 } else { -53.0 };
            let end = (g.canvas.x + x + 7.5, g.canvas.y + y + 2.5);
            let pen = CreatePen(PS_SOLID, p.px(1), rgb(color));
            let old = SelectObject(p.dc, pen);
            MoveToEx(
                p.dc,
                (start_x * p.scale as f64).round() as i32,
                (start_y * p.scale as f64).round() as i32,
                null_mut(),
            );
            LineTo(
                p.dc,
                (elbow * p.scale as f64).round() as i32,
                (start_y * p.scale as f64).round() as i32,
            );
            LineTo(
                p.dc,
                (end.0 * p.scale as f64).round() as i32,
                (end.1 * p.scale as f64).round() as i32,
            );
            SelectObject(p.dc, old);
            DeleteObject(pen);
            for (cx, cy, radius, ring) in [
                (start_x, start_y, 2.5, false),
                (end.0, end.1, 2.5, false),
                (end.0, end.1, 5.5, true),
            ] {
                if ring && !focused || index == 7 && !cycle && cx == end.0 {
                    continue;
                }
                let circle = r(cx - radius, cy - radius, radius * 2.0, radius * 2.0);
                let bounds = box_px(p, circle);
                let pen = CreatePen(PS_SOLID, p.px(1), rgb(color));
                let old_pen = SelectObject(p.dc, pen);
                let brush = if ring {
                    GetStockObject(NULL_BRUSH)
                } else {
                    CreateSolidBrush(rgb(color))
                };
                let old_brush = SelectObject(p.dc, brush);
                Ellipse(p.dc, bounds.left, bounds.top, bounds.right, bounds.bottom);
                SelectObject(p.dc, old_brush);
                SelectObject(p.dc, old_pen);
                DeleteObject(pen);
                if !ring {
                    DeleteObject(brush);
                }
            }
        }
    }
    let center = if s.device().is_some_and(|d| d["pid"] == "025D") {
        geo::keyboard_center(w, s.assignment.drawer) - s.assignment.scroll_x
    } else {
        geo::mouse(
            w,
            s.assignment.drawer,
            s.assignment.scroll_x,
            s.assignment.scroll_y,
            geo::MOUSE_LABEL_WIDTHS,
        )
        .image
        .x + 150.0
    };
    let y = if s.device().is_some_and(|d| d["pid"] == "025D") {
        553.0
    } else {
        508.0
    } - s.assignment.scroll_y;
    let source_controls = geo::mouse(w, false, 0.0, 0.0, geo::MOUSE_LABEL_WIDTHS);
    let dx = center - (source_controls.image.x + 150.0);
    let dy = y - (source_controls.image.y + 360.0);
    let panel_button = geo::translate(source_controls.drawer, dx, dy);
    rounded(
        p,
        panel_button,
        0x111111,
        if s.assignment.drawer || hover(s, panel_button) {
            GREEN
        } else {
            0x5d5d5d
        },
        14.0,
    );
    hit(p, panel_button, Control::Drawer);
    image(
        s,
        p,
        if s.assignment.drawer {
            "assignment-panel-active"
        } else {
            "assignment-panel"
        },
        r(panel_button.x + 10.0, panel_button.y + 4.5, 18.0, 18.0),
    );
    rounded(
        p,
        geo::translate(source_controls.hyper, dx, dy),
        0x111111,
        if hover(s, geo::translate(source_controls.hyper, dx, dy)) {
            GREEN
        } else {
            0x5d5d5d
        },
        18.0,
    );
    for (on, label, bounds) in [
        (
            false,
            "Standard",
            geo::translate(source_controls.standard, dx, dy),
        ),
        (
            true,
            "Hypershift",
            geo::translate(source_controls.shifted, dx, dy),
        ),
    ] {
        let selected = s.assignment.hyper == on;
        if selected {
            rounded(
                p,
                bounds,
                if on { 0xfd8611 } else { GREEN },
                if on { 0xfd8611 } else { GREEN },
                12.0,
            );
        }
        baseline_text(
            p,
            label,
            bounds,
            p.fonts.normal,
            if selected { 0x212121 } else { 0xcccccc },
            DT_SINGLELINE | DT_CENTER | DT_VCENTER,
            Role::Layer,
        );
        hit(p, bounds, Control::Layer(on));
    }
    image(
        s,
        p,
        "assignment-help",
        geo::translate(source_controls.help, dx, dy),
    );
    if s.assignment.drawer {
        let g = r(0.0, 128.0, 230.0, h - 128.0);
        fill(p, g, 0x2b2b2b);
        fill(p, r(0.0, 194.0, 230.0, 1.0), 0x5d5d5d);
        let dropdown = r(7.0, 148.0, 180.0, 27.0);
        border(p, dropdown, 0x515151);
        baseline_text(
            p,
            if s.assignment.customized_only {
                "Customized"
            } else {
                "All Buttons"
            },
            r(17.0, 148.0, 150.0, 27.0),
            p.fonts.normal,
            0xcccccc,
            DT_SINGLELINE | DT_VCENTER,
            Role::Dropdown,
        );
        image(s, p, "expand", r(167.0, 156.0, 10.0, 10.0));
        hit(p, dropdown, Control::Filter);
        hit(p, r(197.0, 148.0, 26.0, 26.0), Control::Drawer);
        image(s, p, "assignment-panel-close", r(200.0, 151.0, 20.0, 20.0));
        let entries = visible_inputs(s);
        let context = s
            .device()
            .and_then(model::Context::from_device)
            .map(Arc::new);
        let saved = SaveDC(p.dc);
        IntersectClipRect(p.dc, 0, p.px(195), p.px(230), p.px((h - 12.0) as i32));
        for (index, i) in entries.iter().enumerate() {
            let row = r(
                0.0,
                195.0 + index as f64 * 50.0 - s.assignment.drawer_scroll,
                230.0,
                50.0,
            );
            if hover(s, row) {
                fill(p, row, 0x383838);
            }
            text(
                p,
                &i.label,
                r(-1.0, row.y + 9.0, 56.0, 32.0),
                p.fonts.normal,
                0xcccccc,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            fill(p, r(55.0, row.y + 9.0, 1.0, 32.0), 0x707070);
            let category = context.as_ref().and_then(|context| {
                Draft::new(
                    context.clone(),
                    i.clone(),
                    Anchor::Drawer(i.input_id.clone()),
                    s.assignment.hyper,
                )
                .category
            });
            text(
                p,
                category.map(Category::title).unwrap_or("UNKNOWN"),
                r(61.0, row.y + 9.0, 160.0, 16.0),
                p.fonts.tiny,
                0x707070,
                DT_SINGLELINE,
            );
            text(
                p,
                &i.default_label,
                r(61.0, row.y + 25.0, 160.0, 16.0),
                p.fonts.normal,
                if i.enabled { 0xcccccc } else { 0x555555 },
                DT_SINGLELINE,
            );
            if i.enabled {
                hit(
                    p,
                    row,
                    Control::Open(i.clone(), Anchor::Drawer(i.input_id.clone())),
                );
            }
        }
        RestoreDC(p.dc, saved);
        if let Some(bar) = geo::scrollbar(
            r(0.0, 195.0, 230.0, (h - 207.0).max(1.0)),
            entries.len() as f64 * 50.0,
            s.assignment.drawer_scroll,
            8.0,
        ) {
            scroll_thumb(s, p, bar, 0x2b2b2b, s.assignment.drawer_grab.is_some());
            hit(p, bar.thumb, Control::DrawerThumb(bar.thumb.y));
        }
    }
    if s.assignment.drawer && s.assignment.filter_open {
        fill(p, r(7.0, 176.0, 180.0, 52.0), 0);
        border(p, r(7.0, 176.0, 180.0, 52.0), 0x515151);
        for (value, label, y) in [(false, "All Buttons", 177.0), (true, "Customized", 202.0)] {
            let row = r(8.0, y, 178.0, 25.0);
            if hover(s, row) {
                fill(p, row, 0x393939);
            }
            baseline_text(
                p,
                label,
                r(18.0, y, 158.0, 25.0),
                p.fonts.normal,
                if value == s.assignment.customized_only {
                    GREEN
                } else {
                    0xcccccc
                },
                DT_SINGLELINE | DT_VCENTER,
                Role::Options,
            );
            hit(p, row, Control::FilterChoice(value));
        }
    }
    RestoreDC(p.dc, clip);
    true
}
pub(super) unsafe fn paint_popup(s: &State, p: &mut Painter) {
    if !covered(s) {
        return;
    }
    let Some(d) = &s.assignment.draft else {
        return;
    };
    let Some(g) = shell(s) else {
        return;
    };
    hit(p, g.popup, Control::Block);
    hit(p, g.header, Control::Block);
    fill(p, g.popup, 0x111111);
    border(p, g.popup, 0x5d5d5d);
    fill(p, g.header, 0x222222);
    baseline_text(
        p,
        &d.input.label,
        r(g.header.x + 10.0, g.header.y, 235.0, 36.0),
        p.fonts.normal,
        0xcccccc,
        DT_SINGLELINE | DT_VCENTER,
        Role::Header,
    );
    hit(p, g.close, Control::Close);
    image(
        s,
        p,
        "assignment-close",
        r(g.close.x + 8.0, g.close.y + 8.0, 20.0, 20.0),
    );
    let f = geo::form(
        g,
        d.context.owner.pid,
        d.category,
        d.turbo,
        d.turbo_available(),
        d.form_scroll,
    );
    let saved = SaveDC(p.dc);
    let b = box_px(p, g.body);
    IntersectClipRect(p.dc, b.left, b.top, b.right, b.bottom);
    baseline_text(
        p,
        d.category
            .map(Category::title)
            .unwrap_or("ASSIGNMENT UNKNOWN"),
        f.heading,
        p.fonts.widget_heading,
        GREEN,
        DT_SINGLELINE,
        Role::Heading,
    );
    match d.category {
        Some(Category::Default) => {
            border(p, f.display, 0x5d5d5d);
            baseline_text(
                p,
                &d.input.default_label,
                f.display,
                p.fonts.normal,
                0xcccccc,
                DT_SINGLELINE | DT_VCENTER | DT_CENTER,
                Role::DefaultValue,
            );
        }
        Some(Category::Disable) => {}
        Some(Category::Mouse) => {
            border(p, f.dropdown, 0x515151);
            baseline_text(
                p,
                if d.mouse_choice_confirmed { model::MOUSE_ACTIONS[d.mouse_option].1 } else { "Retained choice unavailable" },
                r(f.dropdown.x + 10.0, f.dropdown.y, 180.0, 27.0),
                p.fonts.normal,
                0xcccccc,
                DT_VCENTER | DT_SINGLELINE,
                Role::Dropdown,
            );
            image(
                s,
                p,
                "expand",
                r(f.dropdown.x + 190.0, f.dropdown.y + 8.0, 10.0, 10.0),
            );
            hit(p, f.dropdown, Control::Options);
            if d.turbo_available() {
                checkbox(
                    p,
                    r(f.checkbox.x, f.checkbox.y, 20.0, 20.0),
                    d.turbo,
                    hover(s, f.checkbox),
                );
                text(
                    p,
                    "Enable Turbo",
                    r(f.checkbox.x + 30.0, f.checkbox.y, 180.0, 20.0),
                    p.fonts.normal,
                    0xcccccc,
                    DT_SINGLELINE | DT_VCENTER,
                );
                hit(p, f.checkbox, Control::Turbo);
                if d.turbo {
                    baseline_text(
                        p,
                        "Speed (press/sec)",
                        f.label,
                        p.fonts.normal,
                        0xcccccc,
                        DT_SINGLELINE,
                        Role::TurboLabel,
                    );
                    border(p, f.stepper, 0x515151);
                    image(
                        s,
                        p,
                        "dpi-stepper-up",
                        r(f.up.x + 5.0, f.up.y + 2.0, 8.0, 8.0),
                    );
                    image(
                        s,
                        p,
                        "dpi-stepper-down",
                        r(f.down.x + 5.0, f.down.y + 2.0, 8.0, 8.0),
                    );
                    hit(p, f.up, Control::Step(1));
                    hit(p, f.down, Control::Step(-1));
                    fill(p, f.track, 0x515151);
                    let x =
                        f.track.x + 8.0 + (d.speed as f64 - 1.0) / 19.0 * (f.track.width - 16.0);
                    rounded(
                        p,
                        r(x - 8.0, f.track.y - 5.0, 16.0, 16.0),
                        GREEN,
                        GREEN,
                        8.0,
                    );
                    text(
                        p,
                        "1",
                        r(f.slider.x, f.slider.y + 22.0, 30.0, 14.0),
                        p.fonts.small,
                        0x999999,
                        DT_SINGLELINE,
                    );
                    text(
                        p,
                        "20",
                        r(f.slider.x + 180.0, f.slider.y + 22.0, 30.0, 14.0),
                        p.fonts.small,
                        0x999999,
                        DT_SINGLELINE | DT_RIGHT,
                    );
                    hit(
                        p,
                        r(f.track.x, f.track.y - 5.0, f.track.width, 16.0),
                        Control::Slider(f.track),
                    );
                }
            }
        }
        _ => {
            text(
                p,
                if d.category.is_none() {
                    "Assignment is unknown. Select a category to preview; the original mapping is preserved."
                } else {
                    "This source category is not implemented. The original mapping is preserved."
                },
                r(f.display.x, f.display.y, 210.0, 65.0),
                p.fonts.normal,
                0x999999,
                DT_WORDBREAK,
            );
        }
    }
    button(p, "CANCEL", f.cancel, Some(Control::Cancel));
    button(p, "SAVE", f.save, (d.can_save() && !s.assignment.saving && s.pending == 0).then_some(Control::Save));
    text(
        p,
        d.save_reason(),
        r(f.cancel.x, f.cancel.y + 39.0, 210.0, 85.0),
        p.fonts.small,
        0x999999,
        DT_WORDBREAK,
    );
    RestoreDC(p.dc, saved);
    fill(p, g.rail, 0x222222);
    let saved = SaveDC(p.dc);
    let b = box_px(p, g.rail);
    IntersectClipRect(p.dc, b.left, b.top, b.right, b.bottom);
    let (rows, _) = shell::category_rows(g, d.categories.len(), d.rail_scroll);
    for (row, c) in rows.iter().zip(&d.categories) {
        let selected = d.category == Some(*c);
        let over = hover(s, *row);
        if selected || over {
            fill(p, *row, if selected { 0x111111 } else { 0x393939 });
        }
        let key = format!(
            "assignment-{}-{}",
            c.key(),
            if selected {
                "active"
            } else if over {
                "hover"
            } else {
                "normal"
            }
        );
        image(s, p, &key, r(row.x + 10.0, row.y + 10.0, 20.0, 20.0));
        if d.rail_expanded {
            baseline_text(
                p,
                c.title(),
                r(row.x + 50.0, row.y, row.width - 60.0, 40.0),
                p.fonts.small,
                if selected { GREEN } else { 0xcccccc },
                DT_SINGLELINE | DT_VCENTER,
                Role::Category,
            );
        }
        hit(p, *row, Control::Category(*c));
    }
    RestoreDC(p.dc, saved);
    if let Some(bar) = geo::scrollbar(g.rail, d.categories.len() as f64 * 40.0, d.rail_scroll, 8.0)
    {
        scroll_thumb(s, p, bar, 0x222222, s.assignment.rail_grab.is_some());
        hit(p, bar.thumb, Control::RailThumb(bar.thumb.y));
    }
    if let Some(bar) = geo::scrollbar(
        g.body,
        (f.bottom + d.form_scroll + 124.0 - g.body.y).max(g.body.height),
        d.form_scroll,
        8.0,
    ) {
        scroll_thumb(s, p, bar, 0x111111, s.assignment.form_grab.is_some());
        hit(p, bar.thumb, Control::FormThumb(bar.thumb.y));
    }
    if d.options_open {
        let panel = geo::options(f.dropdown, dimensions(s).1);
        fill(p, panel, 0);
        border(p, panel, 0x515151);
        let saved = SaveDC(p.dc);
        let b = box_px(p, panel);
        IntersectClipRect(p.dc, b.left + 1, b.top + 1, b.right - 1, b.bottom - 1);
        for (i, row) in geo::option_rows(panel, d.options_scroll)
            .into_iter()
            .enumerate()
        {
            if hover(s, row) {
                fill(p, row, 0x393939);
            }
            baseline_text(
                p,
                model::MOUSE_ACTIONS[i].1,
                r(row.x + 10.0, row.y, row.width - 20.0, row.height),
                p.fonts.normal,
                if i == d.mouse_option { GREEN } else { 0xcccccc },
                DT_SINGLELINE | DT_VCENTER,
                Role::Options,
            );
            hit(p, row, Control::Option(i));
        }
        RestoreDC(p.dc, saved);
        if let Some(bar) = geo::scrollbar(
            r(
                panel.x + 1.0,
                panel.y + 1.0,
                panel.width - 2.0,
                panel.height - 2.0,
            ),
            250.0,
            d.options_scroll,
            8.0,
        ) {
            scroll_thumb(s, p, bar, 0, s.assignment.options_grab.is_some());
            hit(p, bar.thumb, Control::OptionsThumb(bar.thumb.y));
        }
    }
    if s.assignment.pending.is_some() {
        p.hits.clear();
        let (w, h) = dimensions(s);
        let g = geo::alert(w, h);
        let backdrop_dc = CreateCompatibleDC(p.dc);
        let backdrop = CreateCompatibleBitmap(p.dc, 1, 1);
        if !backdrop_dc.is_null() && !backdrop.is_null() {
            let old = SelectObject(backdrop_dc, backdrop);
            SetPixel(backdrop_dc, 0, 0, 0);
            GdiAlphaBlend(
                p.dc,
                0,
                p.px(42),
                s.width,
                s.height - p.px(42),
                backdrop_dc,
                0,
                0,
                1,
                1,
                BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 128,
                    AlphaFormat: 0,
                },
            );
            SelectObject(backdrop_dc, old);
        }
        if !backdrop.is_null() {
            DeleteObject(backdrop);
        }
        if !backdrop_dc.is_null() {
            DeleteDC(backdrop_dc);
        }
        hit(p, r(0.0, 42.0, w, h - 42.0), Control::Block);
        rounded(p, g.panel, 0x111111, GREEN, 5.0);
        baseline_text(
            p,
            "SAVE REMAPPED BUTTON",
            g.title,
            p.fonts.widget_heading,
            GREEN,
            DT_SINGLELINE | DT_CENTER,
            Role::Heading,
        );
        text(
            p,
            "Your changes have not been saved yet. Would you like to save these changes?",
            g.first,
            p.fonts.normal,
            0xcccccc,
            DT_WORDBREAK | DT_CENTER,
        );
        text(
            p,
            "Note: Clicking DON’T SAVE will remove all changes made.",
            g.second,
            p.fonts.normal,
            0xcccccc,
            DT_WORDBREAK | DT_CENTER,
        );
        button(p, "DON'T SAVE", g.discard, Some(Control::Discard));
        button(p, "SAVE", g.save, s.assignment.draft.as_ref().is_some_and(|d|d.can_save() && !s.assignment.saving && s.pending == 0).then_some(Control::Save));
        image(s, p, "assignment-dialog-close", g.close);
        hit(p, g.close, Control::Dismiss);
    }
}
pub(super) unsafe fn open_at(s: &mut State, point: POINT) -> bool {
    if !covered(s) {
        return false;
    }
    if s.restricted_actions {
        return true;
    }
    if s.profile_open || s.profile_actions {
        return false;
    }
    if s.assignment.pending.is_some() {
        return true;
    }
    let (x, y) = (
        point.x as f64 / s.scale as f64,
        point.y as f64 / s.scale as f64,
    );
    let found = if s.device().is_some_and(|d| d["pid"] == "025D") {
        s.keyboard_layout
            .as_ref()
            .and_then(|l| l.hit_test(source_layout::Point { x, y }, viewport(s)))
            .map(|k| (input(k), Anchor::Key(k.input_id.clone())))
    } else {
        None
    };
    if let Some((i, a)) = found {
        action(s, Control::Open(i, a));
    } else if s.assignment.draft.is_some() {
        if shell(s).is_none_or(|g| !geo::contains(g.popup, x, y) && !geo::contains(g.header, x, y))
        {
            action(s, Control::Close);
        }
    }
    true
}
fn transition(s: &mut State, next: Transition) {
    match next {
        Transition::Close => s.assignment.cancel(),
        Transition::Open(i, a) => {
            if i.enabled {
                if let Some(c) = s.device().and_then(model::Context::from_device) {
                    s.assignment.draft = Some(Draft::new(Arc::new(c), i, a, s.assignment.hyper));
                }
            }
        }
        Transition::Drawer(v) => {
            s.assignment.cancel();
            s.assignment.drawer = v;
        }
        Transition::Layer(v) => {
            s.assignment.cancel();
            s.assignment.hyper = v;
        }
        Transition::External => s.assignment.cancel(),
    }
}
pub(super) unsafe fn action(s: &mut State, c: Control) {
    if s.restricted_actions || s.assignment.saving {
        return;
    }
    match c {
        Control::Block => {}
        Control::Open(i, a) => {
            if s.assignment
                .draft
                .as_ref()
                .is_some_and(|d| d.context.owner.pid != 605 && d.input.input_id == i.input_id)
            {
                return;
            }
            if let Some(t) = s.assignment.request(Transition::Open(i, a)) {
                transition(s, t);
            }
        }
        Control::Close => {
            if let Some(t) = s.assignment.request(Transition::Close) {
                transition(s, t);
            }
        }
        Control::Cancel => s.assignment.cancel(),
        Control::Save => {
            if s.pending == 0 {
                if let Some(d) = s.assignment.draft.as_mut().filter(|d| d.turbo && d.turbo_available()) { d.speed_blur(); }
            }
            if let Some(d) = s.assignment.draft.as_ref().filter(|d|d.can_save() && s.pending == 0) {
                let body = json!({"id":d.context.owner.device,"expected_profile":d.context.owner.profile,
                    "expected_payload":d.context.original_profile,"assignment":d.assignment().unwrap()});
                s.queue(Job::MouseAssignment(body));
                s.assignment.saving = s.pending > 0;
            }
        }
        Control::Drawer => {
            if let Some(t) = s
                .assignment
                .request(Transition::Drawer(!s.assignment.drawer))
            {
                transition(s, t);
            }
        }
        Control::Layer(v) => {
            if let Some(t) = s.assignment.request(Transition::Layer(v)) {
                transition(s, t);
            }
        }
        Control::Filter => {
            s.assignment.filter_open = !s.assignment.filter_open;
        }
        Control::FilterChoice(value) => {
            s.assignment.customized_only = value;
            s.assignment.filter_open = false;
            s.assignment.drawer_scroll = 0.0;
        }
        Control::Dismiss => {
            s.assignment.dismiss_prompt();
            s.assignment_next = None;
        }
        Control::Discard => {
            if let Some(t) = s.assignment.discard() {
                transition(s, t);
            }
            if let Some(next) = s.assignment_next.take() {
                s.action(*next);
            }
        }
        Control::Category(c) => {
            if let Some(d) = &mut s.assignment.draft {
                d.select(c);
            }
        }
        Control::Options => {
            if let Some(d) = &mut s.assignment.draft {
                d.options_open = !d.options_open;
            }
        }
        Control::Option(i) => {
            if let Some(d) = &mut s.assignment.draft {
                d.select_mouse(i);
            }
        }
        Control::Turbo => {
            if let Some(d) = &mut s.assignment.draft {
                d.toggle_turbo();
            }
        }
        Control::Step(delta) => {
            if let Some(d) = &mut s.assignment.draft {
                d.set_speed((d.speed as i16 + delta as i16).clamp(1, 20) as u8);
            }
        }
        Control::Slider(track) => {
            s.assignment.speed_drag = true;
            SetCapture(s.hwnd);
            if let Some((x, _)) = s.assignment.hover {
                set_slider(s, track, x);
            }
        }
        Control::RailThumb(top) => {
            s.assignment.rail_grab = Some(s.assignment.hover.map_or(0.0, |(_, y)| y - top));
            SetCapture(s.hwnd);
        }
        Control::DrawerThumb(top) => {
            s.assignment.drawer_grab = Some(s.assignment.hover.map_or(0.0, |(_, y)| y - top));
            SetCapture(s.hwnd);
        }
        Control::OptionsThumb(top) => {
            s.assignment.options_grab = Some(s.assignment.hover.map_or(0.0, |(_, y)| y - top));
            SetCapture(s.hwnd);
        }
        Control::FormThumb(top) => {
            s.assignment.form_grab = Some(s.assignment.hover.map_or(0.0, |(_, y)| y - top));
            SetCapture(s.hwnd);
        }
    }
    s.rebuild_fields();
    InvalidateRect(s.hwnd, null(), 0);
}
fn set_slider(s: &mut State, track: Rect, x: f64) {
    if let Some(d) = &mut s.assignment.draft {
        d.set_speed(
            (1.0 + ((x - track.x - 8.0) / (track.width - 16.0)).clamp(0.0, 1.0) * 19.0).round()
                as u8,
        );
    }
}
pub(super) unsafe fn pointer(s: &mut State, p: POINT) {
    let (x, y) = (p.x as f64 / s.scale as f64, p.y as f64 / s.scale as f64);
    s.assignment.hover = Some((x, y));
    if let Some(g) = shell(s) {
        if let Some(grab) = s.assignment.options_grab {
            let height = dimensions(s).1;
            let d = s.assignment.draft.as_mut().unwrap();
            let panel = geo::options(
                geo::form(
                    g,
                    d.context.owner.pid,
                    d.category,
                    d.turbo,
                    d.turbo_available(),
                    d.form_scroll,
                )
                .dropdown,
                height,
            );
            if let Some(bar) = geo::scrollbar(
                r(
                    panel.x + 1.0,
                    panel.y + 1.0,
                    panel.width - 2.0,
                    panel.height - 2.0,
                ),
                250.0,
                d.options_scroll,
                8.0,
            ) {
                d.options_scroll = bar.drag(y, grab);
            }
        } else if let Some(grab) = s.assignment.rail_grab {
            let d = s.assignment.draft.as_mut().unwrap();
            if let Some(bar) =
                geo::scrollbar(g.rail, d.categories.len() as f64 * 40.0, d.rail_scroll, 8.0)
            {
                d.rail_scroll = bar.drag(y, grab);
            }
        } else if s.assignment.speed_drag {
            let d = s.assignment.draft.as_ref().unwrap();
            let f = geo::form(
                g,
                d.context.owner.pid,
                d.category,
                d.turbo,
                d.turbo_available(),
                d.form_scroll,
            );
            set_slider(s, f.track, x);
            s.rebuild_fields();
        } else if let Some(grab) = s.assignment.form_grab {
            let d = s.assignment.draft.as_mut().unwrap();
            let f = geo::form(
                g,
                d.context.owner.pid,
                d.category,
                d.turbo,
                d.turbo_available(),
                0.0,
            );
            if let Some(bar) = geo::scrollbar(
                g.body,
                (f.bottom + 124.0 - g.body.y).max(g.body.height),
                d.form_scroll,
                8.0,
            ) {
                d.form_scroll = bar.drag(y, grab);
                s.rebuild_fields();
            }
        } else if let Some(d) = &mut s.assignment.draft {
            let expanded = geo::contains(g.rail, x, y);
            if d.rail_expanded != expanded {
                d.rail_expanded = expanded;
                s.rebuild_fields();
            }
        }
    }
    if let Some(grab) = s.assignment.drawer_grab {
        let (_, h) = dimensions(s);
        if let Some(bar) = geo::scrollbar(
            r(0.0, 195.0, 230.0, h - 207.0),
            visible_inputs(s).len() as f64 * 50.0,
            s.assignment.drawer_scroll,
            8.0,
        ) {
            s.assignment.drawer_scroll = bar.drag(y, grab);
        }
    }
    InvalidateRect(s.hwnd, null(), 0);
}
pub(super) unsafe fn release(s: &mut State) -> bool {
    let active = s.assignment.rail_grab.take().is_some()
        | s.assignment.drawer_grab.take().is_some()
        | s.assignment.options_grab.take().is_some()
        | s.assignment.form_grab.take().is_some()
        | std::mem::take(&mut s.assignment.speed_drag)
        | s.assignment.spinner.take().is_some();
    if active {
        KillTimer(s.hwnd, ASSIGNMENT_REPEAT);
        ReleaseCapture();
    }
    active
}
pub(super) unsafe fn wheel(s: &mut State, delta: f64) {
    let (x, y) = s.assignment.hover.unwrap_or((0.0, 0.0));
    let (_, h) = dimensions(s);
    if let Some(g) = shell(s) {
        let d = s.assignment.draft.as_mut().unwrap();
        if d.options_open
            && geo::contains(
                geo::options(
                    geo::form(
                        g,
                        d.context.owner.pid,
                        d.category,
                        d.turbo,
                        d.turbo_available(),
                        d.form_scroll,
                    )
                    .dropdown,
                    h,
                ),
                x,
                y,
            )
        {
            d.options_scroll = (d.options_scroll - delta * 25.0).clamp(0.0, 72.0);
        } else if geo::contains(g.rail, x, y) {
            d.rail_scroll = (d.rail_scroll - delta * 40.0).clamp(
                0.0,
                (d.categories.len() as f64 * 40.0 - g.rail.height).max(0.0),
            );
        } else if geo::contains(g.body, x, y) {
            let f = geo::form(
                g,
                d.context.owner.pid,
                d.category,
                d.turbo,
                d.turbo_available(),
                0.0,
            );
            d.form_scroll = (d.form_scroll - delta * 40.0).clamp(
                0.0,
                (f.bottom + 124.0 - (g.body.y + g.body.height)).max(0.0),
            );
        }
    } else if s.assignment.drawer && x < 230.0 {
        s.assignment.drawer_scroll = (s.assignment.drawer_scroll - delta * 50.0).clamp(
            0.0,
            (visible_inputs(s).len() as f64 * 50.0 - (h - 207.0)).max(0.0),
        );
    }
    s.rebuild_fields();
    InvalidateRect(s.hwnd, null(), 0);
}
pub(super) unsafe fn rebuild_fields(s: &mut State) -> bool {
    if !covered(s) {
        return false;
    }
    s.retained_fields.remove(&SPEED);
    if s.assignment.pending.is_some() || s.assignment.saving {
        return true;
    }
    if let Some(g) = shell(s) {
        if let Some(d) = &s.assignment.draft {
            if d.category == Some(Category::Mouse) && d.turbo && d.turbo_available() {
                let f = geo::form(
                    g,
                    d.context.owner.pid,
                    d.category,
                    d.turbo,
                    true,
                    d.form_scroll,
                );
                let value = d.speed_draft.clone();
                if f.input.y >= g.body.y && f.input.y + 24.0 < g.body.y + g.body.height {
                    s.field(
                        SPEED,
                        &value,
                        f.input.x.round() as i32,
                        f.input.y.round() as i32,
                        32,
                    );
                    if let Some(hwnd) = s.fields.get(&SPEED) {
                        SendMessageW(*hwnd, WM_SETFONT, s.fonts.input as usize, 1);
                        SendMessageW(*hwnd, EM_SETLIMITTEXT, 2, 0);
                        SetWindowPos(
                            *hwnd,
                            null_mut(),
                            0,
                            0,
                            s.px(32),
                            s.px(24),
                            SWP_NOMOVE | SWP_NOZORDER,
                        );
                    }
                }
            }
        }
    }
    true
}
pub(super) unsafe fn change(s: &mut State, blur: bool) {
    if s.assignment.saving { return; }
    let value = s.raw_text(SPEED);
    if let Some(d) = &mut s.assignment.draft {
        if blur {
            d.speed_blur();
        } else if !d.speed_change(&value) {
            if let Some(h) = s.fields.get(&SPEED) {
                SetWindowTextW(*h, wide(&d.speed_draft).as_ptr());
            }
        }
    }
    if blur {
        s.rebuild_fields();
    }
    InvalidateRect(s.hwnd, null(), 0);
}
pub(super) fn sync(s: &mut State) {
    if s.assignment
        .draft
        .as_ref()
        .is_some_and(|d| s.device().is_none_or(|v| !d.context.still_current(v)))
    {
        s.assignment.cancel();
        s.assignment_next = None;
    }
}

pub(super) unsafe fn begin_step(s: &mut State, delta: i8) {
    action(s, Control::Step(delta));
    s.assignment.spinner = Some(delta);
    SetCapture(s.hwnd);
    SetTimer(s.hwnd, ASSIGNMENT_REPEAT, 300, None);
}
pub(super) unsafe fn repeat(s: &mut State) {
    if let Some(delta) = s.assignment.spinner {
        action(s, Control::Step(delta));
    }
}

#[cfg(test)]
pub(super) fn diagnostics(s: &State) -> Value {
    json!({"can_save":s.assignment.draft.as_ref().is_some_and(|d|d.can_save()),"mouse_option":s.assignment.draft.as_ref().map(|d|d.mouse_option),"saving":s.assignment.saving,"background_ready":s.customize_background.borrow().ready(),"drawer":s.assignment.drawer,"drawer_scroll":s.assignment.drawer_scroll,"drawer_count":visible_inputs(s).len(),"customized_only":s.assignment.customized_only,"filter_open":s.assignment.filter_open,"rail_expanded":s.assignment.draft.as_ref().is_some_and(|d|d.rail_expanded),"rail_scroll":s.assignment.draft.as_ref().map(|d|d.rail_scroll),"source_icons":s.icons.keys().filter(|key|key.starts_with("assignment-")).count()})
}
#[cfg(test)]
pub(super) unsafe fn font_diagnostics(s: &State) -> Value {
    let dc = CreateCompatibleDC(null_mut());
    let bitmap = CreateCompatibleBitmap(dc, 800, 120);
    let old_bitmap = SelectObject(dc, bitmap);
    let mut rows = Vec::new();
    let painter = Painter {
        dc,
        scale: s.scale,
        fonts: &s.fonts,
        hits: Vec::new(),
    };
    let cases = [
        (Role::Header, s.fonts.normal, 36.0, 24.0),
        (Role::Category, s.fonts.small, 40.0, 24.0),
        (Role::Heading, s.fonts.widget_heading, 19.0, 15.0),
        (Role::DefaultValue, s.fonts.normal, 29.0, 19.0),
        (Role::Dropdown, s.fonts.normal, 27.0, 18.0),
        (Role::Options, s.fonts.normal, 25.0, 17.0),
        (Role::TurboLabel, s.fonts.normal, 17.0, 13.0),
        (Role::Button, s.fonts.small, 27.0, 18.0),
        (Role::Layer, s.fonts.normal, 24.0, 17.0),
        (Role::MouseLabel, s.fonts.normal, 30.0, 19.0),
    ];
    let min_y = |dc| -> (Option<i32>, i32, i32) {
        let count = GetPath(dc, null_mut(), null_mut(), 0);
        if count <= 0 {
            return (None, count, 0);
        }
        let mut points = vec![POINT { x: 0, y: 0 }; count as usize];
        let mut types = vec![0u8; count as usize];
        let read = GetPath(dc, points.as_mut_ptr(), types.as_mut_ptr(), count);
        if read != count {
            return (None, count, read);
        }
        (points.iter().map(|p| p.y).min(), count, read)
    };
    for (role, font, height, source_baseline) in cases {
        if matches!(role, Role::Heading) {
            // DirectWrite renders glyph pixels, not a GDI BeginPath outline.
            // The linked bridge's pixel/metric test validates that renderer;
            // here exercise the real assignment call site and its font state.
            let emitted = baseline_text(
                &painter, "A", r(0.0, 0.0, 200.0, height), font,
                0xcccccc, DT_SINGLELINE, role,
            );
            let metrics = s.fonts.source_text.metrics();
            let expected = (source_baseline * s.scale as f64).round() as i32;
            let actual = metrics.source_baseline_physical.round() as i32;
            rows.push(json!({
                "role":"Heading", "verification":"natural-layout/source-baseline",
                "actual_baseline_px":actual, "expected_baseline_px":expected,
                "source_baseline_css":source_baseline, "face":"authenticated RazerF5",
                "emitted":emitted, "fallback":s.fonts.source_text.diagnostic(),
                "matches":emitted && s.fonts.source_text.diagnostic().is_none()
                    && metrics.advance_css > 0.0 && actual == expected,
            }));
            continue;
        }
        let old_font = SelectObject(dc, font);
        let mut face = [0u16; 80];
        let face_len = GetTextFaceW(dc, face.len() as i32, face.as_mut_ptr());
        let face = String::from_utf16_lossy(&face[..face_len.max(0) as usize])
            .trim_end_matches('\0')
            .to_owned();
        let expected_face = if matches!(role, Role::Heading) {
            "RazerF5"
        } else {
            "Roboto"
        };
        let old_align = SetTextAlign(dc, TA_BASELINE | TA_LEFT);
        SetBkMode(dc, TRANSPARENT as i32);
        BeginPath(dc);
        TextOutW(dc, 0, 0, wide("A").as_ptr(), 1);
        EndPath(dc);
        let reference = min_y(dc);
        SelectObject(dc, old_font);
        SetTextAlign(dc, old_align);
        BeginPath(dc);
        let emitted = baseline_text(
            &painter,
            "A",
            r(0.0, 0.0, 200.0, height),
            font,
            0xcccccc,
            DT_SINGLELINE,
            role,
        );
        EndPath(dc);
        let drawn = min_y(dc);
        let actual = drawn
            .0
            .zip(reference.0)
            .map(|(actual, reference)| actual - reference);
        let expected = (source_baseline * s.scale as f64).round() as i32;
        rows.push(json!({"role":format!("{role:?}"),"actual_baseline_px":actual,"source_baseline_css":source_baseline,"expected_baseline_px":expected,"matches":actual==Some(expected)&&face==expected_face,"face":face,"reference_path":[reference.1,reference.2],"drawn_path":[drawn.1,drawn.2],"emitted":emitted}));
    }
    SelectObject(dc, old_bitmap);
    DeleteObject(bitmap);
    DeleteDC(dc);
    let mut outer: RECT = std::mem::zeroed();
    let mut client: RECT = std::mem::zeroed();
    let mut origin = POINT { x: 0, y: 0 };
    GetWindowRect(s.hwnd, &mut outer);
    GetClientRect(s.hwnd, &mut client);
    ClientToScreen(s.hwnd, &mut origin);
    json!({"scale":s.scale,"private_font_files":s.fonts.loaded.len(),"rows":rows,"window_geometry":{"outer_width_px":outer.right-outer.left,"outer_height_px":outer.bottom-outer.top,"client_width_px":client.right,"client_height_px":client.bottom,"client_inset_x_px":origin.x-outer.left,"client_inset_y_px":origin.y-outer.top}})
}

#[cfg(test)]
mod keyboard_selection_tests {
    use super::*;

    #[test]
    fn selected_path_matches_physical_key_pixels_at_each_display_scale() {
        // Synthetic key corners, spanning the keyboard, independent of private assets.
        let fixture = serde_json::json!({
            "product_id": 605, "edition_id": 0, "layout_id": 6,
            "view_box": [0, 0, 730, 387], "source_sha256": "0".repeat(64),
            "keys": [
                {"buttonKey":"esc", "inputID":"ESC", "isEnabled":true,
                 "d":"M68 95H93V120H68Z"},
                {"buttonKey":"a", "inputID":"A", "isEnabled":true,
                 "d":"M114 182H139V207H114Z"},
                {"buttonKey":"num", "inputID":"NUM", "isEnabled":true,
                 "d":"M637 209H662V260H637Z"}
            ]
        });
        let layout = source_layout::SourceLayout::from_slice(
            &serde_json::to_vec(&fixture).unwrap(),
        ).unwrap();
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            assert!(!dc.is_null());
            for scale in [1.0, 1.25, 1.5, 2.0] {
                for (width, drawer, scroll_x, scroll_y) in [
                    (1280.0, false, 0.0, 0.0),
                    (1000.0, true, 73.0, 41.0),
                ] {
                    let viewport = source_layout::Viewport {
                        left: geo::keyboard_center(width, drawer) - 365.0 - scroll_x,
                        top: 154.0 - scroll_y, width: 730.0, height: 387.0,
                    };
                    for (id, x, y) in [("ESC", 68.0, 95.0), ("A", 114.0, 182.0), ("NUM", 637.0, 209.0)] {
                        // The same point in the image and logical hitmap must be
                        // the first physical GDI point of the selection outline.
                        let hit = layout.hit_test(source_layout::Point {
                            x: viewport.left + x + 12.0, y: viewport.top + y + 12.0,
                        }, viewport).unwrap();
                        assert_eq!(hit.input_id, id);
                        assert!(trace_keyboard_selection(&layout, dc, id, viewport, scale));
                        let count = GetPath(dc, std::ptr::null_mut(), std::ptr::null_mut(), 0);
                        assert!(count > 0);
                        let mut points = vec![POINT { x: 0, y: 0 }; count as usize];
                        let mut types = vec![0u8; count as usize];
                        assert_eq!(GetPath(dc, points.as_mut_ptr(), types.as_mut_ptr(), count), count);
                        assert_eq!(points[0].x, ((viewport.left + x) * scale).round() as i32);
                        assert_eq!(points[0].y, ((viewport.top + y) * scale).round() as i32);
                        AbortPath(dc);
                    }
                }
            }
            DeleteDC(dc);
        }
    }
}

/// Save completion is the only point at which a queued transition may run.
pub(super) unsafe fn saved(s: &mut State, result: Result<Value, String>) {
    s.assignment.saving = false;
    match result {
        Err(error) => s.error(&error),
        Ok(state) => {
            let pending = s.assignment.pending.take();
            let external = s.assignment_next.take();
            s.assignment.cancel();
            s.state = state;
            s.status = "Assignment saved locally; hardware delivery unavailable".into();
            if let Some(next) = pending { transition(s, next); }
            if let Some(next) = external { s.action(*next); }
        }
    }
    s.rebuild_fields();
    InvalidateRect(s.hwnd, null(), 0);
}
