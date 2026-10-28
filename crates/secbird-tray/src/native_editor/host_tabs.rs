//! Shared AppEngine-style tab surface. Close/reorder eligibility remains a
//! separate recovery item; this component does not remove connected devices.
use super::*;
use windows_sys::Win32::Graphics::GdiPlus as gp;

#[derive(Default)]
pub(super) struct View {
    pub hover: Option<String>,
    pub pressed: Option<String>,
    scroll: i32,
    max_scroll: i32,
    viewport: i32,
    viewport_left: i32,
    active: Option<String>,
    slots: Vec<(i32, i32)>,
}

#[derive(Debug, PartialEq)]
struct Layout {
    home: i32,
    start: i32,
    viewport: i32,
    total: i32,
    overflow: bool,
}

fn layout(window: i32, home: i32, widths: &[i32]) -> Layout {
    let total = widths.iter().sum::<i32>() + (widths.len().saturating_sub(1) as i32) * 4;
    // TabUI.calculateTabWrapperWidth: retain the source's empty draggable area
    // as well as the 144px window controls. Scroll buttons consume20px each.
    let spare = (((window + 1 - 600) as f64 / 16.5 + 10.0).floor() as i32).min(100);
    let available = (window - 144 - home - 10 - spare).max(0);
    let overflow = total > available;
    let start = 8 + home + 4 + if overflow { 20 } else { 0 };
    Layout {
        home,
        start,
        viewport: (available - if overflow { 40 } else { 0 }).max(0),
        total,
        overflow,
    }
}

fn title(device: &Value) -> String {
    // Product UI's transport suffix belongs in the product view, not its host
    // tab. Keep unknown product names rather than guessing their family.
    match device["pid"]
        .as_str()
        .unwrap_or("")
        .to_ascii_uppercase()
        .as_str()
    {
        "00C4" | "00C5" => "DEATHADDER V3 HYPERSPEED".into(),
        "025D" => "ORNATA V2".into(),
        _ => device["name"]
            .as_str()
            .unwrap_or("RAZER DEVICE")
            .trim_start_matches("Razer ")
            .trim_end_matches(" (Wired)")
            .trim_end_matches(" (Wireless)")
            .to_uppercase(),
    }
}

unsafe fn text_width(p: &Painter<'_>, text: &str) -> i32 {
    let previous = SelectObject(p.dc, p.fonts.tab);
    let value = wide(text);
    let mut size: SIZE = std::mem::zeroed();
    GetTextExtentPoint32W(p.dc, value.as_ptr(), (value.len() - 1) as i32, &mut size);
    SelectObject(p.dc, previous);
    (size.cx as f32 / p.scale).ceil() as i32
}

unsafe fn rounded_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> *mut gp::GpPath {
    let mut path = null_mut();
    if gp::GdipCreatePath(gp::FillModeWinding, &mut path) != gp::Ok {
        return path;
    }
    gp::GdipAddPathLine(path, x, y + h, x, y + r);
    gp::GdipAddPathArc(path, x, y, r * 2.0, r * 2.0, 180.0, 90.0);
    gp::GdipAddPathLine(path, x + r, y, x + w - r, y);
    gp::GdipAddPathArc(path, x + w - r * 2.0, y, r * 2.0, r * 2.0, 270.0, 90.0);
    gp::GdipAddPathLine(path, x + w, y + r, x + w, y + h);
    gp::GdipClosePathFigure(path);
    path
}

unsafe fn surface(
    dc: HDC,
    scale: f32,
    x: i32,
    width: i32,
    active: bool,
    hover: bool,
    pressed: bool,
) {
    let mut graphics = null_mut();
    if gp::GdipCreateFromHDC(dc, &mut graphics) != gp::Ok {
        return;
    }
    gp::GdipSetSmoothingMode(graphics, gp::SmoothingModeAntiAlias);
    let x = x as f32 * scale;
    let y = 8.0 * scale;
    let width = width as f32 * scale;
    let height = 34.0 * scale;
    let selected = active || pressed;
    for inset in [0.0, 1.0] {
        let top = if selected && inset == 0.0 {
            0xff44d62c
        } else if hover && !selected {
            0xff444444
        } else if selected {
            0xff222222
        } else {
            0xff000000
        };
        let bottom = if selected || hover {
            0xff222222
        } else {
            0xff000000
        };
        let mut brush = null_mut();
        // Extend past the first coverage sample: GDI+ samples a gradient at
        // half-pixel offsets and otherwise wraps the first antialiased row.
        let a = gp::PointF { X: 0.0, Y: y - 1.0 };
        let b = gp::PointF {
            X: 0.0,
            Y: y + height,
        };
        if gp::GdipCreateLineBrush(&a, &b, top, bottom, gp::WrapModeTileFlipY, &mut brush) == gp::Ok
        {
            let colors = [top, bottom, bottom];
            let stops = [0.0, 0.8, 1.0];
            let blend_status =
                gp::GdipSetLinePresetBlend(brush, colors.as_ptr(), stops.as_ptr(), 3);
            debug_assert_eq!(blend_status, gp::Ok);
            let path = rounded_path(
                x + inset * scale,
                y + inset * scale,
                width - inset * 2.0 * scale,
                height - inset * scale,
                5.0 * scale,
            );
            if !path.is_null() {
                let fill_status = gp::GdipFillPath(graphics, brush.cast(), path);
                debug_assert_eq!(fill_status, gp::Ok);
                gp::GdipDeletePath(path);
            }
            gp::GdipDeleteBrush(brush.cast());
        }
    }
    gp::GdipDeleteGraphics(graphics);
}

unsafe fn label(
    p: &mut Painter<'_>,
    state: &State,
    id: &str,
    text: &str,
    icon: &str,
    x: i32,
    width: i32,
    active: bool,
) {
    let view = state.host_tabs.borrow();
    let hover = view.hover.as_deref() == Some(id);
    let pressed = view.pressed.as_deref() == Some(id) && hover;
    surface(p.dc, p.scale, x, width, active, hover, pressed);
    p.image(state.icons.get(icon), x + 13, 15, 20, 20);
    let old_clip = SaveDC(p.dc);
    let bounds = p.rect(x + 43, 9, width - 56, 32);
    IntersectClipRect(p.dc, bounds.left, bounds.top, bounds.right, bounds.bottom);
    // CSS clips the title instead of substituting an ellipsis. The GDI light
    // font is an explicit remaining typography acceptance item.
    let previous = SelectObject(p.dc, p.fonts.tab);
    SetBkMode(p.dc, TRANSPARENT as i32);
    SetTextColor(
        p.dc,
        rgb(if active || pressed {
            0xcccccc
        } else {
            0x999999
        }),
    );
    let mut bounds = bounds;
    DrawTextW(
        p.dc,
        wide(text).as_ptr(),
        -1,
        &mut bounds,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );
    SelectObject(p.dc, previous);
    RestoreDC(p.dc, old_clip);
    if width > 186 && (!hover || active || pressed) {
        let mut graphics = null_mut();
        if gp::GdipCreateFromHDC(p.dc, &mut graphics) == gp::Ok {
            let left = (x + width - 46) as f32 * p.scale;
            let a = gp::PointF { X: left, Y: 0.0 };
            let b = gp::PointF {
                X: left + 45.0 * p.scale,
                Y: 0.0,
            };
            let color = if active || pressed { 0x222222 } else { 0 };
            let colors = [color, 0xff000000 | color, 0xff000000 | color];
            let stops = [0.0, 0.75, 1.0];
            let mut brush = null_mut();
            if gp::GdipCreateLineBrush(
                &a,
                &b,
                colors[0],
                colors[1],
                gp::WrapModeTileFlipX,
                &mut brush,
            ) == gp::Ok
            {
                gp::GdipSetLinePresetBlend(brush, colors.as_ptr(), stops.as_ptr(), 3);
                gp::GdipFillRectangle(
                    graphics,
                    brush.cast(),
                    left,
                    9.0 * p.scale,
                    45.0 * p.scale,
                    33.0 * p.scale,
                );
                gp::GdipDeleteBrush(brush.cast());
            }
            gp::GdipDeleteGraphics(graphics);
        }
    }
}

pub(super) unsafe fn paint(state: &State, p: &mut Painter<'_>, window: i32) {
    let home = (text_width(p, "SYNAPSE") + 56).clamp(92, 242);
    let labels: Vec<_> = state.devices().iter().map(title).collect();
    let widths: Vec<_> = labels
        .iter()
        .map(|s| (text_width(p, s) + 56).clamp(92, 242))
        .collect();
    let g = layout(window, home, &widths);
    {
        let mut view = state.host_tabs.borrow_mut();
        let resized = view.viewport != g.viewport;
        view.max_scroll = (g.total - g.viewport).max(0);
        view.viewport = g.viewport;
        view.viewport_left = g.start;
        view.scroll = view.scroll.clamp(0, view.max_scroll);
        view.slots.clear();
        let mut left = 0;
        for width in &widths {
            view.slots.push((left, left + width));
            left += width + 4;
        }
        if resized || view.active != state.selected {
            if let Some(index) = state
                .devices()
                .iter()
                .position(|d| d["id"].as_str() == state.selected.as_deref())
            {
                let left: i32 = widths[..index].iter().sum::<i32>() + index as i32 * 4;
                let right = left + widths[index];
                if right > view.scroll + g.viewport {
                    view.scroll = (right - g.viewport + 6).min(view.max_scroll);
                }
                if left < view.scroll {
                    view.scroll = left;
                }
            }
            view.active = state.selected.clone();
        }
    }
    label(
        p,
        state,
        "",
        "SYNAPSE",
        "host-synapse",
        8,
        home,
        state.selected.is_none(),
    );
    p.hits.push(Hit {
        rect: p.rect(8, 8, home, 34),
        action: Action::Home,
    });
    let clip = p.rect(g.start, 0, g.viewport, 42);
    let old_clip = SaveDC(p.dc);
    IntersectClipRect(p.dc, clip.left, clip.top, clip.right, clip.bottom);
    let mut x = g.start - state.host_tabs.borrow().scroll;
    for (index, device) in state.devices().iter().enumerate() {
        let width = widths[index];
        let id = device["id"].as_str().unwrap_or("");
        if x + width > g.start && x < g.start + g.viewport {
            let icon = if device["pid"] == "025D" {
                "host-keyboard"
            } else {
                "host-mouse"
            };
            label(
                p,
                state,
                id,
                &labels[index],
                icon,
                x,
                width,
                state.selected.as_deref() == Some(id),
            );
            let mut rect = p.rect(x, 8, width, 34);
            rect.left = rect.left.max(clip.left);
            rect.right = rect.right.min(clip.right);
            p.hits.push(Hit {
                rect,
                action: Action::Device(id.into()),
            });
        }
        x += width + 4;
    }
    RestoreDC(p.dc, old_clip);
    if g.overflow {
        for (x, direction, icon) in [
            (g.start - 20, -1, "host-scroll-left"),
            (g.start + g.viewport, 1, "host-scroll-right"),
        ] {
            let v = state.host_tabs.borrow();
            let enabled = if direction < 0 {
                v.scroll > 0
            } else {
                v.scroll < v.max_scroll
            };
            p.image(state.icons.get(icon), x, 15, 20, 20);
            if enabled {
                p.hits.push(Hit {
                    rect: p.rect(x, 8, 20, 34),
                    action: Action::HostScroll(direction),
                });
            }
        }
    }
}

pub(super) fn scroll(state: &State, direction: i32) {
    let mut view = state.host_tabs.borrow_mut();
    let target = if direction < 0 {
        view.slots
            .iter()
            .rev()
            .find(|(left, _)| *left < view.scroll)
            .map(|(left, _)| *left)
            .unwrap_or(0)
    } else {
        view.slots
            .iter()
            .find(|(_, right)| *right > view.scroll + view.viewport)
            .map(|(_, right)| *right - view.viewport + 6)
            .unwrap_or(view.max_scroll)
    };
    view.scroll = target.clamp(0, view.max_scroll);
}

pub(super) fn wheel(state: &State, delta: i32) {
    let mut view = state.host_tabs.borrow_mut();
    view.scroll = (view.scroll - delta).clamp(0, view.max_scroll);
}

pub(super) fn contains(state: &State, point: POINT) -> bool {
    let view = state.host_tabs.borrow();
    point.y >= state.px(8)
        && point.y < state.px(42)
        && point.x >= state.px(view.viewport_left)
        && point.x < state.px(view.viewport_left + view.viewport)
}

#[cfg(test)]
pub(super) fn diagnostics(state: &State) -> Value {
    let v = state.host_tabs.borrow();
    let icons = [
        "host-synapse",
        "host-mouse",
        "host-keyboard",
        "host-scroll-left",
        "host-scroll-right",
    ];
    json!({"hover":v.hover,"pressed":v.pressed,"scroll":v.scroll,"maximum":v.max_scroll,
        "icons_loaded":icons.iter().filter(|icon| state.icons.contains_key(**icon)).count(),
        "selected":state.selected,"tabs":state.hits.iter().filter_map(|h| {
            let id = match &h.action { Action::Home if h.rect.top < state.px(42) => "", Action::Device(id) if h.rect.top < state.px(42) => id, _ => return None };
            Some(json!({"id":id,"x":h.rect.left,"y":h.rect.top,"width":h.rect.right-h.rect.left,"height":h.rect.bottom-h.rect.top}))
        }).collect::<Vec<_>>()})
}

/// Runs before device-specific mouse routing so every page shares hover state.
pub(super) unsafe fn pointer(state: &State, message: u32, lparam: isize) {
    let point = POINT {
        x: (lparam as u32 & 0xffff) as i16 as i32,
        y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
    };
    let hover = if message == WM_MOUSELEAVE {
        None
    } else {
        state
            .hits
            .iter()
            .find(|hit| point.y < state.px(42) && PtInRect(&hit.rect, point) != 0)
            .and_then(|hit| match &hit.action {
                Action::Home => Some(String::new()),
                Action::Device(id) => Some(id.clone()),
                _ => None,
            })
    };
    let mut view = state.host_tabs.borrow_mut();
    let changed = view.hover != hover || message != WM_MOUSEMOVE;
    view.hover = hover;
    if message == WM_LBUTTONDOWN {
        view.pressed = view.hover.clone();
    }
    if matches!(message, WM_LBUTTONUP | WM_CANCELMODE | WM_MOUSELEAVE) {
        view.pressed = None;
    }
    if changed {
        InvalidateRect(
            state.hwnd,
            &RECT {
                left: 0,
                top: 0,
                right: state.width,
                bottom: state.px(42),
            },
            0,
        );
    }
    if message == WM_MOUSEMOVE {
        let mut tracking = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: state.hwnd,
            dwHoverTime: 0,
        };
        TrackMouseEvent(&mut tracking);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overflow_preserves_natural_tab_widths_and_window_controls() {
        let g = layout(1000, 105, &[242, 116, 242, 180]);
        assert!(g.overflow);
        assert_eq!(g.total, 792);
        assert!(g.start + g.viewport + 20 < 1000 - 144);
        let wide = layout(2000, 105, &[242, 116]);
        assert!(!wide.overflow);
        assert_eq!(wide.start, 117);
    }
    #[test]
    fn host_titles_do_not_leak_connection_suffixes() {
        assert_eq!(
            title(&json!({"pid":"025D", "name":"Razer Ornata V2"})),
            "ORNATA V2"
        );
        assert_eq!(
            title(&json!({"name":"DeathAdder V3 HyperSpeed (Wired)"})),
            "DEATHADDER V3 HYPERSPEED"
        );
    }

    #[test]
    fn real_renderer_draws_selected_gradient_and_preserves_rounded_corner() {
        unsafe {
            let _runtime = image_render::Runtime::start().unwrap();
            let dc = CreateCompatibleDC(null_mut());
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = 260;
            info.bmiHeader.biHeight = -60;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            let mut pixels = null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
            assert!(!bitmap.is_null());
            let previous = SelectObject(dc, bitmap);
            PatBlt(dc, 0, 0, 260, 60, BLACKNESS);
            surface(dc, 1.25, 8, 150, true, false, false);
            let border = GetPixel(dc, 50, 10);
            let green = (border >> 8) & 255;
            assert!(
                green > (border & 255) * 2,
                "selected border not green: {border:x}"
            );
            assert_eq!(GetPixel(dc, 50, 30), rgb(0x222222));
            assert_eq!(
                GetPixel(dc, 10, 10),
                0,
                "rounded corner must retain black host"
            );
            PatBlt(dc, 0, 0, 260, 60, BLACKNESS);
            surface(dc, 1.25, 8, 150, false, true, false);
            assert!(GetPixel(dc, 50, 14) & 255 > GetPixel(dc, 50, 45) & 255);
            assert_eq!(
                GetPixel(dc, 50, 30) & 255,
                (GetPixel(dc, 50, 30) >> 8) & 255
            );
            SelectObject(dc, previous);
            DeleteObject(bitmap);
            DeleteDC(dc);
        }
    }
}
