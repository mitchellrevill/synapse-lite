//! Shared AppEngine-style tab surface and local page lifecycle. Closing a tab
//! removes only its view, and follows the current assignment dirty-state gate.
use super::*;
use windows_sys::Win32::Graphics::GdiPlus as gp;

#[derive(Default)]
pub(super) struct View {
    pub session: host_tab_model::Session,
    pub pages: HashMap<String, Tab>,
    pub hover: Option<String>,
    pub pressed: Option<String>,
    close_hover: bool,
    close_pressed: bool,
    scroll: i32,
    max_scroll: i32,
    viewport: i32,
    viewport_left: i32,
    active: Option<String>,
    slots: Vec<(i32, i32)>,
}

impl View {
    pub fn load() -> Self {
        Self {
            session: host_tab_model::Session::load(secbird_core::config_dir().join("tabs.json")),
            ..Self::default()
        }
    }
}

fn entries(state:&State)->Vec<&Value> {
    static LINKED:std::sync::LazyLock<Value>=std::sync::LazyLock::new(||json!({"id":linked_games::ID,"name":"LINKED GAMES","module":"linked-games"}));
    static MACRO:std::sync::LazyLock<Value>=std::sync::LazyLock::new(||json!({"id":macro_view::ID,"name":"MACRO","module":"macro"}));
    static SHORTCUTS:std::sync::LazyLock<Value>=std::sync::LazyLock::new(||json!({"id":shortcuts::ID,"name":"GLOBAL SHORTCUTS","module":"global-shortcuts"}));
    let mut entries:Vec<_>=state.devices().iter().collect();
    if state.linked_games.used||state.host_tabs.borrow().session.known(linked_games::ID) {entries.push(&LINKED);}
    if state.macros.used||state.host_tabs.borrow().session.known(macro_view::ID) {entries.push(&MACRO);}
    if state.shortcuts.used||state.host_tabs.borrow().session.known(shortcuts::ID) {entries.push(&SHORTCUTS);}
    entries
}

pub(super) fn remember(state: &State) {
    if let Some(id) = &state.selected {
        state
            .host_tabs
            .borrow_mut()
            .pages
            .insert(id.clone(), state.tab);
    }
}

fn persist(state: &mut State) {
    if state.read_only_smoke || state.restricted_actions {
        return;
    }
    let result = state.host_tabs.borrow_mut().session.save();
    if let Err(error) = result {
        state.error(&format!("Tab layout was not saved: {error}"));
    }
}

pub(super) fn open(state: &mut State, id: &str) -> bool {
    let connected: Vec<_> = entries(state)
        .iter()
        .filter_map(|d| d["id"].as_str().map(str::to_owned))
        .collect();
    let opened = {
        let mut view = state.host_tabs.borrow_mut();
        view.session.reconcile(&connected);
        view.session.open(id)
    };
    if opened {
        persist(state);
    }
    opened
}

pub(super) unsafe fn close(state: &mut State, id: &str) {
    let previous = state.selected.clone();
    let next = state
        .host_tabs
        .borrow_mut()
        .session
        .close(id, previous.as_deref());
    state.host_tabs.borrow_mut().pages.remove(id);
    persist(state);
    if previous != next {
        state.selected = None;
        match next {
            Some(id) => state.action(Action::Device(id)),
            None => state.action(Action::Home),
        }
    }
    InvalidateRect(state.hwnd, null(), 0);
}

pub(super) unsafe fn move_to(state: &mut State, id: &str, target: usize) {
    if state.host_tabs.borrow_mut().session.move_to(id, target) {
        state.host_tabs.borrow_mut().active = None;
        persist(state);
        InvalidateRect(state.hwnd, null(), 0);
    }
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
    if device["module"]=="linked-games" {return "LINKED GAMES".into();}
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

unsafe fn text_advance(p: &Painter<'_>, text: &str) -> f32 {
    if let Some(metrics) = p
        .fonts
        .source_text
        .measure(text, source_text::Role::HostTab)
    {
        return metrics.advance_css;
    }
    let previous = SelectObject(p.dc, p.fonts.tab);
    let value = wide(text);
    let mut size: SIZE = std::mem::zeroed();
    GetTextExtentPoint32W(p.dc, value.as_ptr(), (value.len() - 1) as i32, &mut size);
    SelectObject(p.dc, previous);
    size.cx as f32 / p.scale
}

fn css_rect(p: &Painter<'_>, x: f32, y: f32, width: f32, height: f32) -> RECT {
    RECT {
        left: (x * p.scale).round() as i32,
        top: (y * p.scale).round() as i32,
        right: ((x + width) * p.scale).round() as i32,
        bottom: ((y + height) * p.scale).round() as i32,
    }
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
    width: f32,
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
    let width = width * scale;
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
    width: f32,
    active: bool,
) {
    let view = state.host_tabs.borrow();
    let hover = view.hover.as_deref() == Some(id);
    let pressed = view.pressed.as_deref() == Some(id) && hover;
    surface(p.dc, p.scale, x, width, active, hover, pressed);
    p.image(state.icons.get(icon), x + 13, 15, 20, 20);
    let old_clip = SaveDC(p.dc);
    let bounds = css_rect(p, x as f32 + 43.0, 18.0, width - 56.0, 14.0);
    IntersectClipRect(p.dc, bounds.left, bounds.top, bounds.right, bounds.bottom);
    // Natural layout and source baseline, with rounded physical clips debiased
    // by the bridge. Keep the existing visible GDI fallback on failure.
    let color = if active || pressed {
        0xcccccc
    } else {
        0x999999
    };
    if !p.source_text(
        text,
        source_text::Role::HostTab,
        source_text::SourceBox {
            left: x as f32 + 43.0,
            top: 18.0,
            width: width - 56.0,
        },
        bounds,
        color,
    ) {
        let previous = SelectObject(p.dc, p.fonts.tab);
        SetBkMode(p.dc, TRANSPARENT as i32);
        SetTextColor(p.dc, rgb(color));
        let mut bounds = bounds;
        DrawTextW(
            p.dc,
            wide(text).as_ptr(),
            -1,
            &mut bounds,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(p.dc, previous);
    }
    RestoreDC(p.dc, old_clip);
    if width > 186.0 && (!hover || active || pressed) {
        let mut graphics = null_mut();
        if gp::GdipCreateFromHDC(p.dc, &mut graphics) == gp::Ok {
            let left = (x as f32 + width - 46.0) * p.scale;
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
    let items=entries(state);
    let ids: Vec<_> = items
        .iter()
        .filter_map(|d| d["id"].as_str().map(str::to_owned))
        .collect();
    state.host_tabs.borrow_mut().session.reconcile(&ids);
    let order = state.host_tabs.borrow().session.visible();
    let devices: Vec<_> = order
        .iter()
        .filter_map(|id| items.iter().find(|d| d["id"] == *id).copied())
        .collect();
    let home_outer = (text_advance(p, "SYNAPSE") + 56.0).clamp(92.0, 242.0);
    // Browser clientWidth rounds the fractional CSS box for host positioning;
    // retain the fractional box itself for painting and clipping.
    let home = home_outer.round() as i32;
    let labels: Vec<_> = devices.iter().map(|d| title(d)).collect();
    let outer_widths: Vec<_> = labels
        .iter()
        .map(|s| (text_advance(p, s) + 56.0).clamp(92.0, 242.0))
        .collect();
    let widths: Vec<i32> = outer_widths
        .iter()
        .map(|width| width.round() as i32)
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
            if let Some(index) = order
                .iter()
                .position(|id| Some(id) == state.selected.as_ref())
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
        home_outer,
        state.selected.is_none() && !state.studio.open,
    );
    p.hits.push(Hit {
        rect: css_rect(p, 8.0, 8.0, home_outer, 34.0),
        action: Action::Home,
    });
    let clip = p.rect(g.start, 0, g.viewport, 42);
    let old_clip = SaveDC(p.dc);
    IntersectClipRect(p.dc, clip.left, clip.top, clip.right, clip.bottom);
    let mut x = g.start - state.host_tabs.borrow().scroll;
    for (index, device) in devices.iter().enumerate() {
        let width = widths[index];
        let id = device["id"].as_str().unwrap_or("");
        if x + width > g.start && x < g.start + g.viewport {
            let icon = if device["module"]=="linked-games" {"host-linked-games"} else if device["pid"] == "025D" {
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
                outer_widths[index],
                state.selected.as_deref() == Some(id),
            );
            let mut rect = css_rect(p, x as f32, 8.0, outer_widths[index], 34.0);
            rect.left = rect.left.max(clip.left);
            rect.right = rect.right.min(clip.right);
            p.hits.push(Hit {
                rect,
                action: Action::Device(id.into()),
            });
            let view = state.host_tabs.borrow();
            if state.selected.as_deref() == Some(id) && view.hover.as_deref() == Some(id) {
                let close = css_rect(p, x as f32 + outer_widths[index] - 30.0, 13.0, 24.0, 24.0);
                let icon = if view.close_pressed && view.close_hover {
                    "close-tab-pressed"
                } else if view.close_hover {
                    "close-tab-hover"
                } else {
                    "close-tab"
                };
                if let Some(image) = state.icons.get(icon) {
                    image_render::draw(p.dc, image.smooth, close, image.width, image.height);
                }
                let rect = RECT {
                    left: close.left.max(clip.left),
                    right: close.right.min(clip.right),
                    top: close.top,
                    bottom: close.bottom,
                };
                if rect.right > rect.left {
                    p.hits.push(Hit {
                        rect,
                        action: Action::HostClose(id.into()),
                    });
                }
            }
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
            let name = if direction < 0 {
                "@scroll-left"
            } else {
                "@scroll-right"
            };
            let hover = v.hover.as_deref() == Some(name);
            let pressed = v.pressed.as_deref() == Some(name) && hover;
            let icon = match (direction < 0, enabled, hover, pressed) {
                (true, false, _, _) => "host-scroll-left-disabled",
                (false, false, _, _) => "host-scroll-right-disabled",
                (true, true, _, true) => "host-scroll-left-active",
                (false, true, _, true) => "host-scroll-right-active",
                (true, true, true, _) => "host-scroll-left-hover",
                (false, true, true, _) => "host-scroll-right-hover",
                _ => icon,
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

/// Some(None) consumes a recognized shortcut with no eligible destination.
pub(super) fn shortcut_action(
    state: &State,
    key: usize,
    control: bool,
    shift: bool,
) -> Option<Option<Action>> {
    if !control || state.profile_modal.is_some() {
        return None;
    }
    let view = state.host_tabs.borrow();
    let order = view.session.visible();
    let index = state
        .selected
        .as_ref()
        .and_then(|id| order.iter().position(|key| key == id))
        .map(|i| i + 1)
        .unwrap_or(0);
    let select = |position: usize| {
        if position == 0 {
            Some(Action::Home)
        } else {
            order.get(position - 1).cloned().map(Action::Device)
        }
    };
    Some(match key {
        9 => select(if shift {
            (index + order.len()) % (order.len() + 1)
        } else {
            (index + 1) % (order.len() + 1)
        }),
        49..=57 if !shift => select(key - 49),
        87 if !shift => state.selected.clone().map(Action::HostClose),
        84 if shift => view.session.last_closed().map(Action::Device),
        33 | 34 if shift => state.selected.as_ref().and_then(|id| {
            let current = order.iter().position(|key| key == id)?;
            let target = if key == 33 {
                current.checked_sub(1)?
            } else {
                (current + 1 < order.len()).then_some(current + 1)?
            };
            Some(Action::HostMove(id.clone(), target))
        }),
        _ => return None,
    })
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
        "natural_text_available":state.fonts.source_text.diagnostic().is_none(),
        "order":v.session.visible(),"recent_closed":v.session.last_closed(),
        "device_count":state.devices().len(),"page":state.tab.title(),
        "cards":state.hits.iter().filter_map(|h| match &h.action {
            Action::Device(id) if h.rect.top >= state.px(42) => Some(json!({"id":id,"x":h.rect.left,"y":h.rect.top,"width":h.rect.right-h.rect.left,"height":h.rect.bottom-h.rect.top})),
            _ => None,
        }).collect::<Vec<_>>(),
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
    let action = if message == WM_MOUSELEAVE {
        None
    } else {
        state
            .hits
            .iter()
            .rev()
            .find(|hit| point.y < state.px(42) && PtInRect(&hit.rect, point) != 0)
            .map(|hit| hit.action.clone())
    };
    let hover = action.as_ref().and_then(|action| match action {
        Action::Home => Some(String::new()),
        Action::Device(id) | Action::HostClose(id) => Some(id.clone()),
        Action::HostScroll(direction) => Some(
            if *direction < 0 {
                "@scroll-left"
            } else {
                "@scroll-right"
            }
            .into(),
        ),
        _ => None,
    });
    let mut view = state.host_tabs.borrow_mut();
    let close_hover = matches!(action, Some(Action::HostClose(_)));
    let changed = view.hover != hover || view.close_hover != close_hover || message != WM_MOUSEMOVE;
    view.hover = hover;
    view.close_hover = close_hover;
    if message == WM_LBUTTONDOWN {
        view.pressed = view.hover.clone();
        view.close_pressed = close_hover;
    }
    if matches!(message, WM_LBUTTONUP | WM_CANCELMODE | WM_MOUSELEAVE) {
        view.pressed = None;
        view.close_pressed = false;
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
            surface(dc, 1.25, 8, 150.0, true, false, false);
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
            surface(dc, 1.25, 8, 150.0, false, true, false);
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
