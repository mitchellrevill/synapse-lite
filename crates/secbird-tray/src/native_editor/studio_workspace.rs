//! Recovered Studio effect palette geometry and mask presentation.
use super::*;
use secbird_core::studio::document::NewEffect;

#[derive(Clone, Copy, Default, PartialEq)]
pub enum Tool {
    #[default]
    Select,
    Pen,
    Bucket,
    Move,
}

pub(super) unsafe fn paint_host(state: &State, p: &mut Painter, _width: i32) {
    p.image(state.icons.get("host-chroma"), 20, 15, 20, 20);
    p.text("CHROMA", 50, 8, 65, 34, p.fonts.small, MUTED, DT_LEFT);
    p.hits.push(Hit {
        rect: p.rect(0, 0, 118, 42),
        action: Action::Device(chroma_view::ID.into()),
    });
    host_tabs::surface(p.dc, p.scale, 118, 96., true, false, false);
    p.image(state.icons.get("host-chroma"), 130, 15, 20, 20);
    p.text("STUDIO", 160, 8, 54, 34, p.fonts.small, TEXT, DT_LEFT);
    p.hits.push(Hit {
        rect: p.rect(118, 0, 96, 42),
        action: Action::Studio(Control::Enter),
    });
}

unsafe fn icon_button(
    state: &State,
    p: &mut Painter,
    role: &str,
    (x, y): (i32, i32),
    control: Control,
    enabled: bool,
    active: bool,
) {
    icon(
        state,
        p,
        role,
        if !enabled {
            "muted"
        } else if active {
            "active"
        } else {
            "normal"
        },
        x + 3,
        y + 3,
        20,
    );
    if enabled {
        p.hits.push(Hit {
            rect: p.rect(x, y, 27, 27),
            action: Action::Studio(control),
        });
    }
}

pub(super) unsafe fn paint_chrome(state: &State, p: &mut Painter, width: i32, height: i32) {
    let view = &state.studio;
    let right = width - 250;
    let enabled = view.editable();
    performance_view::rounded(
        p,
        performance_layout::Rect {
            x: (width / 2 - 57) as f64,
            y: 90.,
            width: 114.,
            height: 28.,
        },
        GREEN,
        GREEN,
        14.,
    );
    p.text(
        "CHROMA STUDIO",
        width / 2 - 57,
        90,
        114,
        28,
        p.fonts.small,
        0,
        DT_CENTER,
    );
    p.fill(10, 90, 180, 27, BG);
    border(p, 10, 90, 180, 27, 0x444444);
    p.text(
        &format!(
            "{}. {}{}",
            view.index + 1,
            view.title(),
            if view.dirty() { " *" } else { "" }
        ),
        16,
        90,
        150,
        27,
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    p.text("▾", 168, 90, 20, 27, p.fonts.small, MUTED, DT_CENTER);
    p.hits.push(Hit {
        rect: p.rect(10, 90, 180, 27),
        action: Action::Studio(Control::Profiles),
    });
    p.text("…", 200, 88, 30, 27, p.fonts.normal, TEXT, DT_CENTER);
    p.hits.push(Hit {
        rect: p.rect(200, 88, 30, 27),
        action: Action::Studio(Control::Library(library::Control::Menu)),
    });
    p.text(
        "EFFECT LAYERS",
        10,
        128,
        200,
        40,
        p.fonts.small,
        MUTED,
        DT_LEFT,
    );
    icon_button(
        state,
        p,
        "folder-add",
        (216, 135),
        Control::Layers(layer_tools::Control::Group),
        enabled,
        false,
    );
    p.fill(260, 135, 150, 27, BG);
    border(p, 260, 135, 150, 27, 0x5d5d5d);
    p.text(
        "Quick Selections",
        267,
        135,
        120,
        27,
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    p.text("▾", 387, 135, 20, 27, p.fonts.small, MUTED, DT_CENTER);
    if enabled {
        p.hits.push(Hit {
            rect: p.rect(260, 135, 150, 27),
            action: Action::Studio(Control::QuickSelection),
        });
    }
    let center = width / 2 - 35;
    for (i, (role, tool)) in [
        ("cursor", Tool::Select),
        ("pen", Tool::Pen),
        ("bucket", Tool::Bucket),
        ("move", Tool::Move),
    ]
    .into_iter()
    .enumerate()
    {
        icon_button(
            state,
            p,
            role,
            (center - 75 + i as i32 * 41, 135),
            Control::Tool(tool),
            enabled,
            view.tool == tool,
        );
    }
    p.fill(center + 85, 138, 1, 20, 0x5d5d5d);
    icon_button(
        state,
        p,
        "trash",
        (center + 99, 135),
        Control::Erase,
        enabled && view.canvas.count() > 0,
        false,
    );
    icon_button(
        state,
        p,
        "undo",
        (right - 70, 135),
        Control::Undo,
        enabled && view.draft.as_ref().is_some_and(|d| d.undo_len() > 0),
        false,
    );
    icon_button(
        state,
        p,
        "redo",
        (right - 30, 135),
        Control::Redo,
        enabled && view.draft.as_ref().is_some_and(|d| d.redo_len() > 0),
        false,
    );
    let x = right - 242;
    for (offset, role, control, active) in [
        (0, "preview", Control::Play, output::active(&view.output)),
        (32, "settings", Control::WorkspaceMenu, view.workspace_menu),
        (64, "keyboard", Control::KeyboardHelp, view.keyboard_help),
    ] {
        p.fill(x + offset, height - 40, 30, 30, BG);
        icon_button(
            state,
            p,
            role,
            (x + offset + 1, height - 39),
            control,
            !view.busy && !view.dialog,
            active,
        );
    }
    for (offset, label, step) in [(96, "−", -1), (124, "+", 1)] {
        p.fill(x + offset, height - 40, 27, 30, BG);
        p.text(
            label,
            x + offset,
            height - 40,
            27,
            30,
            p.fonts.normal,
            TEXT,
            DT_CENTER,
        );
        p.hits.push(Hit {
            rect: p.rect(x + offset, height - 40, 27, 30),
            action: Action::Studio(Control::Zoom(step)),
        });
    }
    p.fill(x + 154, height - 40, 84, 30, BG);
    p.text(
        &format!("{:.0}% ▾", view.canvas.percent(width as f64, height as f64)),
        x + 158,
        height - 40,
        76,
        30,
        p.fonts.small,
        TEXT,
        DT_CENTER,
    );
    p.hits.push(Hit {
        rect: p.rect(x + 154, height - 40, 84, 30),
        action: Action::Studio(Control::WorkspaceMenu),
    });
}

pub(super) unsafe fn paint_property_header(state: &State, p: &mut Painter, right: i32) {
    let view = &state.studio;
    let selected = view.draft.as_ref().and_then(|draft| {
        draft
            .layers()
            .iter()
            .find(|row| Some(&row.id) == view.selected.as_ref())
    });
    if let Some(row) = selected {
        icon(
            state,
            p,
            row.effect.as_deref().unwrap_or("folder"),
            "normal",
            right + 10,
            138,
            20,
        );
        p.text(
            layer_tools::display_title(row),
            right + 40,
            128,
            170,
            40,
            p.fonts.normal,
            TEXT,
            DT_LEFT,
        );
        icon_button(
            state,
            p,
            "reset",
            (right + 217, 135),
            Control::Properties(property_tools::Control::Reset),
            view.editable(),
            false,
        );
    }
    p.fill(right, 168, 250, 1, 0x333333);
}

pub(super) unsafe fn paint_save(state: &State, p: &mut Painter, right: i32, height: i32) {
    let view = &state.studio;
    p.fill(right, height - 38, 250, 38, BG);
    let enabled = view.archive.is_some()
        && !view.busy
        && !view.dialog
        && (view.dirty() || view.renaming || view.composition_renaming);
    p.fill(
        right + 75,
        height - 28,
        100,
        27,
        if enabled { GREEN } else { 0x285c22 },
    );
    p.text(
        "SAVE",
        right + 75,
        height - 28,
        100,
        27,
        p.fonts.small,
        if enabled { 0 } else { MUTED },
        DT_CENTER,
    );
    if enabled {
        p.hits.push(Hit {
            rect: p.rect(right + 75, height - 28, 100, 27),
            action: Action::Studio(Control::Save),
        });
    }
}

pub(super) unsafe fn paint_menu(state: &State, p: &mut Painter, right: i32, height: i32) {
    if state.studio.keyboard_help {
        p.fill(right - 365, height - 320, 351, 264, 0x111111);
        p.text(
            "KEYBOARD SHORTCUTS",
            right - 350,
            height - 310,
            320,
            28,
            p.fonts.small,
            TEXT,
            DT_LEFT,
        );
        for (index, (title, key)) in [
            ("Save", "Ctrl + S"),
            ("Open composition", "Ctrl + O"),
            ("Undo / Redo", "Ctrl + Z / Y"),
            ("Duplicate layer", "Ctrl + D"),
            ("Rename layer", "Ctrl + E"),
            ("Fit all / 100%", "Ctrl + Shift + F / Ctrl + F"),
            ("Pan canvas", "Space + drag"),
            ("Zoom canvas", "Ctrl + wheel"),
        ]
        .into_iter()
        .enumerate()
        {
            p.text(
                title,
                right - 350,
                height - 274 + index as i32 * 26,
                170,
                26,
                p.fonts.small,
                MUTED,
                DT_LEFT,
            );
            p.text(
                key,
                right - 180,
                height - 274 + index as i32 * 26,
                158,
                26,
                p.fonts.small,
                TEXT,
                DT_LEFT,
            );
        }
    }
    if !state.studio.workspace_menu {
        return;
    }
    let x = right - 242;
    let y = height - 40 - 10 * 28;
    p.fill(x, y, 240, 10 * 28, PANEL);
    for (i, (title, control)) in [
        ("Open composition…", Control::Open),
        ("Add device…", Control::AddPreset),
        ("Remap device…", Control::Remap),
        ("Fit all", Control::Zoom(0)),
        ("Fit selected", Control::FitSelected),
        ("100%", Control::ZoomPreset(100.)),
        (if state.studio.playback.active() {"Stop canvas preview"} else {"Canvas preview only"}, Control::Preview),
        ("Apply saved to device", Control::ApplyDevice),
        ("Stop device playback", Control::StopDevice),
        ("Rename layer", Control::Rename),
    ]
    .into_iter()
    .enumerate()
    {
        button(
            p,
            title,
            x + 2,
            y + i as i32 * 28,
            236,
            control,
            !state.studio.busy && !state.studio.dialog,
        );
    }
}

pub(super) unsafe fn pointer(state: &mut State, message: u32, lparam: isize) -> bool {
    if !state.studio.open
        || (!state.studio.workspace_menu
            && state.studio.color_popup.is_none()
            && !state.studio.keyboard_help)
    {
        return false;
    }
    if message == WM_LBUTTONDOWN {
        return true;
    }
    if message == WM_LBUTTONUP {
        let point = POINT {
            x: (lparam as u32 & 0xffff) as i16 as i32,
            y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
        };
        let action = state
            .hits
            .iter()
            .rev()
            .find(|hit| PtInRect(&hit.rect, point) != 0)
            .map(|hit| hit.action.clone());
        state.studio.workspace_menu = false;
        state.studio.keyboard_help = false;
        if !matches!(
            action,
            Some(Action::Studio(
                Control::ColorPopup(_)
                    | Control::ColorSwatch(_, _)
                    | Control::ApplyColor
                    | Control::ApplyCold
                    | Control::ClearColor(_)
            ))
        ) {
            state.studio.color_popup = None;
        }
        if let Some(action) = action {
            state.action(action);
        }
        redraw(state);
        return true;
    }
    false
}

pub(super) unsafe fn paint_colors(state: &State, p: &mut Painter, right: i32) {
    let view = &state.studio;
    p.text(
        "COLOR",
        right + 10,
        174,
        220,
        26,
        p.fonts.small,
        MUTED,
        DT_LEFT,
    );
    for (cold, x, title, text) in [
        (
            false,
            right + 10,
            if view.is_fire() {
                "Hot"
            } else if view.has_two_colors() {
                "Color 1"
            } else {
                ""
            },
            &view.color,
        ),
        (
            true,
            right + 74,
            if view.is_fire() { "Cold" } else { "Color 2" },
            &view.cold,
        ),
    ] {
        if cold && !view.has_two_colors() {
            continue;
        }
        if !title.is_empty() {
            p.text(title, x, 200, 60, 26, p.fonts.small, MUTED, DT_LEFT);
        }
        p.fill(x, 226, 53, 27, BG);
        border(p, x, 226, 53, 27, 0x444444);
        for row in 0..4 {
            for col in 0..4 {
                p.fill(
                    x + 3 + col * 5,
                    230 + row * 5,
                    5,
                    5,
                    if (row + col) % 2 == 0 {
                        0x555555
                    } else {
                        0x333333
                    },
                );
            }
        }
        if let Ok(color) = u32::from_str_radix(text.trim().trim_start_matches('#'), 16) {
            p.fill(x + 3, 230, 20, 20, color);
        }
        p.text("▾", x + 29, 226, 20, 27, p.fonts.small, MUTED, DT_CENTER);
        if view.editable() {
            p.hits.push(Hit {
                rect: p.rect(x, 226, 53, 27),
                action: Action::Studio(Control::ColorPopup(cold)),
            });
        }
    }
}

pub(super) unsafe fn paint_color_popup(state: &State, p: &mut Painter, right: i32) {
    let Some(cold) = state.studio.color_popup else {
        return;
    };
    let enabled = state.studio.editable() && state.studio.canvas.count() > 0;
    p.fill(right + 10, 260, 230, 190, 0x111111);
    p.text(
        "HEX",
        right + 20,
        270,
        190,
        25,
        p.fonts.small,
        MUTED,
        DT_LEFT,
    );
    for (index, color) in [
        0x000000, 0xff0000, 0xff8000, 0xffff00, 0x44d62c, 0x00ffff, 0x0000ff, 0xff00ff,
    ]
    .into_iter()
    .enumerate()
    {
        let x = right + 20 + index as i32 * 26;
        p.fill(x, 347, 22, 22, color);
        if enabled {
            p.hits.push(Hit {
                rect: p.rect(x, 347, 22, 22),
                action: Action::Studio(Control::ColorSwatch(cold, color)),
            });
        }
    }
    button(
        p,
        "APPLY",
        right + 20,
        389,
        100,
        if cold {
            Control::ApplyCold
        } else {
            Control::ApplyColor
        },
        enabled,
    );
    button(
        p,
        "NONE",
        right + 130,
        389,
        100,
        Control::ClearColor(cold),
        enabled,
    );
}

const EFFECTS: [(&str, &str, NewEffect); 13] = [
    ("Ambient\nAwareness", "ambient", NewEffect::Ambient),
    ("Audio\nMeter", "audio", NewEffect::Audio),
    ("Breathing", "breathing", NewEffect::Breathing),
    (
        "Chroma\nGenerate",
        "chroma-generate",
        NewEffect::ChromaGenerate,
    ),
    ("Fire", "fire", NewEffect::Fire),
    ("Reactive", "reactive", NewEffect::Reactive),
    ("Ripple", "ripple", NewEffect::Ripple),
    ("Spectrum\nCycling", "spectrum", NewEffect::Spectrum),
    ("Starlight", "starlight", NewEffect::Starlight),
    ("Static", "static", NewEffect::Static),
    ("Tidal", "tidal", NewEffect::Tidal),
    ("Wave", "wave", NewEffect::Wave),
    ("Wheel", "wheel", NewEffect::Wheel),
];

pub(super) fn palette_top(height: i32) -> i32 {
    (height - 410).max(208)
}
pub(super) fn max_scroll(height: i32) -> i32 {
    (360 - (height - palette_top(height) - 40)).max(0)
}

pub(super) unsafe fn hover(state: &mut State, message: u32, lparam: isize) {
    if !state.studio.open || !matches!(message, WM_MOUSEMOVE | WM_MOUSELEAVE) {
        return;
    }
    let point = (message != WM_MOUSELEAVE)
        .then(|| {
            (
                ((lparam as u32 & 0xffff) as i16 as f32 / state.scale) as i32,
                (((lparam as u32 >> 16) & 0xffff) as i16 as f32 / state.scale) as i32,
            )
        })
        .filter(|(x, y)| (0..250).contains(x) && *y >= 128);
    if state.studio.workspace_hover != point {
        state.studio.workspace_hover = point;
        // Only the left panel depends on this pointer state.
        let rect = RECT {
            left: 0,
            top: (128. * state.scale) as i32,
            right: (250. * state.scale) as i32,
            bottom: state.height,
        };
        InvalidateRect(state.hwnd, &rect, 0);
    }
}

pub(super) unsafe fn icon(
    state: &State,
    p: &Painter,
    role: &str,
    tint: &str,
    x: i32,
    y: i32,
    size: i32,
) {
    p.image(
        state.icons.get(format!("studio-{role}-{tint}").as_str()),
        x,
        y,
        size,
        size,
    );
}

pub(super) unsafe fn paint_palette(state: &State, p: &mut Painter, height: i32) {
    let top = palette_top(height);
    p.fill(0, top, 250, height - top, BG);
    p.fill(0, top, 250, 1, 0x383838);
    p.text(
        "ADD EFFECT",
        10,
        top,
        230,
        40,
        p.fonts.small,
        0x707070,
        DT_LEFT,
    );
    let clip = SaveDC(p.dc);
    IntersectClipRect(p.dc, 0, p.px(top + 40), p.px(250), p.px(height));
    let enabled = state.studio.editable() && !state.read_only_smoke && !state.restricted_actions;
    for (index, (title, role, effect)) in EFFECTS.iter().enumerate() {
        let x = (index % 3) as i32 * 76;
        let y = top + 40 + (index / 3) as i32 * 72 - state.studio.palette_scroll;
        if y + 72 <= top + 40 || y >= height {
            continue;
        }
        let hover = state.studio.workspace_hover.is_some_and(|(px, py)| {
            px >= x && px < x + 76 && py >= y.max(top + 40) && py < (y + 72).min(height)
        });
        if hover && enabled {
            p.fill(x, y, 76, 72, 0x181818);
        }
        icon(
            state,
            p,
            role,
            if hover && enabled { "normal" } else { "muted" },
            x + 23,
            y + 8,
            30,
        );
        for (line, text) in title.split('\n').enumerate() {
            p.text(
                text,
                x + 2,
                y + 40 + line as i32 * 12,
                72,
                12,
                p.fonts.tiny,
                if hover && enabled { TEXT } else { 0x707070 },
                DT_CENTER,
            );
        }
        if enabled {
            p.hits.push(Hit {
                rect: p.rect(
                    x,
                    y.max(top + 40),
                    76,
                    (y + 72).min(height) - y.max(top + 40),
                ),
                action: Action::Studio(super::Control::Layers(layer_tools::Control::Add(*effect))),
            });
        }
    }
    RestoreDC(p.dc, clip);
}

unsafe fn border(p: &Painter, x: i32, y: i32, width: i32, height: i32, color: u32) {
    p.fill(x, y, width, 1, color);
    p.fill(x, y + height - 1, width, 1, color);
    p.fill(x, y, 1, height, color);
    p.fill(x + width - 1, y, 1, height, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palette_reserves_five_rows_and_scrolls_on_small_windows() {
        assert_eq!(palette_top(1104), 694);
        assert_eq!(max_scroll(1104), 0);
        assert_eq!(palette_top(600), 208);
        assert_eq!(max_scroll(600), 8);
        assert_eq!(EFFECTS.len(), 13);
        for (index, (_, role, effect)) in EFFECTS.iter().enumerate() {
            assert!(EFFECTS[..index]
                .iter()
                .all(|(_, other_role, other)| role != other_role && effect != other));
        }
    }
}
