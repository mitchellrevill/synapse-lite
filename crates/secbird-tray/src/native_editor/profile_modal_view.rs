//! Source ImportExportModal GDI view. Source specification remains the geometry authority.
use super::*;
use performance_view::{faded, hit, rounded, text};
use profile_modal_layout::{self as layout, Rect};
use profile_modal_model::{Control, Mode, View};
fn pr(r: Rect) -> performance_layout::Rect {
    performance_layout::Rect {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}
unsafe fn tx(p: &Painter<'_>, s: &str, r: Rect, font: HFONT, color: u32, format: u32) {
    text(p, s, pr(r), font, color, format)
}
unsafe fn box_(p: &Painter<'_>, r: Rect, fill: u32, border: u32, radius: f64) {
    rounded(p, pr(r), fill, border, radius)
}
unsafe fn target(p: &mut Painter<'_>, r: Rect, c: Control) {
    hit(p, pr(r), Action::ProfileModal(c))
}
unsafe fn hovered(state: &State, r: Rect) -> bool {
    let mut q = POINT { x: 0, y: 0 };
    GetCursorPos(&mut q);
    ScreenToClient(state.hwnd, &mut q);
    let x = f64::from(q.x) / f64::from(state.scale);
    let y = f64::from(q.y) / f64::from(state.scale);
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}
unsafe fn backdrop(state: &State, p: &Painter<'_>) {
    let dc = CreateCompatibleDC(p.dc);
    let bitmap = CreateCompatibleBitmap(p.dc, 1, 1);
    if dc.is_null() || bitmap.is_null() {
        if !dc.is_null() {
            DeleteDC(dc);
        }
        if !bitmap.is_null() {
            DeleteObject(bitmap);
        }
        return;
    }
    let old = SelectObject(dc, bitmap);
    SetPixel(dc, 0, 0, 0);
    GdiAlphaBlend(
        p.dc,
        0,
        state.px(42),
        state.width,
        state.height - state.px(42),
        dc,
        0,
        0,
        1,
        1,
        BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 179,
            AlphaFormat: 0,
        },
    );
    SelectObject(dc, old);
    DeleteObject(bitmap);
    DeleteDC(dc);
}
// Keep the source-matching drawing parameters explicit; grouping them would churn geometry call sites.
#[allow(clippy::too_many_arguments)]
unsafe fn checkbox(
    state: &State,
    p: &mut Painter<'_>,
    r: Rect,
    label: &str,
    on: bool,
    enabled: bool,
    c: Control,
    width: f64,
) {
    let a = if enabled { 1.0 } else { 0.3 };
    let border = if on || enabled && hovered(state, r) {
        GREEN
    } else {
        0x737373
    };
    box_(
        p,
        r,
        if on { faded(GREEN, PANEL, a) } else { PANEL },
        faded(border, PANEL, a),
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
                    x: ((origin.0 + x * angle.cos() - y * angle.sin()) * f64::from(p.scale)).round()
                        as i32,
                    y: ((origin.1 + x * angle.sin() + y * angle.cos()) * f64::from(p.scale)).round()
                        as i32,
                })
                .collect();
            let brush = CreateSolidBrush(rgb(PANEL));
            let old = SelectObject(p.dc, brush);
            let pen = SelectObject(p.dc, GetStockObject(NULL_PEN));
            Polygon(p.dc, points.as_ptr(), 4);
            SelectObject(p.dc, pen);
            SelectObject(p.dc, old);
            DeleteObject(brush);
        }
    }
    tx(
        p,
        label,
        layout::rect(r.x + 31.0, r.y + 3.0, (width - 31.0).max(0.0), 17.0),
        p.fonts.normal,
        faded(TEXT, PANEL, a),
        DT_LEFT | DT_END_ELLIPSIS,
    );
    if enabled {
        target(p, layout::rect(r.x, r.y, width, 20.0), c)
    }
}
unsafe fn button(
    state: &State,
    p: &mut Painter<'_>,
    r: Rect,
    label: &str,
    primary: bool,
    enabled: bool,
    c: Control,
) {
    let fill = if primary { GREEN } else { 0x707070 };
    let a = if !enabled {
        0.3
    } else if hovered(state, r) {
        0.8
    } else {
        1.0
    };
    box_(p, r, faded(fill, BG, a), faded(0x000000, BG, 0.3 * a), 3.0);
    tx(
        p,
        &label.to_uppercase(),
        r,
        p.fonts.small,
        faded(if primary { 0 } else { 0xffffff }, BG, a),
        DT_CENTER,
    );
    if enabled {
        target(p, r, c)
    }
}
unsafe fn wrapped(p: &Painter<'_>, value: &str, r: Rect, color: u32) {
    let old = SelectObject(p.dc, p.fonts.normal);
    let mut line = String::new();
    let mut y = r.y;
    for word in value.split_whitespace() {
        let next = if line.is_empty() {
            word.into()
        } else {
            format!("{line} {word}")
        };
        let mut size = SIZE { cx: 0, cy: 0 };
        let wide: Vec<_> = next.encode_utf16().collect();
        GetTextExtentPoint32W(p.dc, wide.as_ptr(), wide.len() as i32, &mut size);
        if !line.is_empty() && f64::from(size.cx) > r.width * f64::from(p.scale) {
            tx(
                p,
                &line,
                layout::rect(r.x, y, r.width, 17.0),
                p.fonts.normal,
                color,
                DT_LEFT,
            );
            line = word.into();
            y += 17.0;
        } else {
            line = next
        }
        if y + 17.0 > r.y + r.height {
            break;
        }
    }
    tx(
        p,
        &line,
        layout::rect(r.x, y, r.width, 17.0),
        p.fonts.normal,
        color,
        DT_LEFT,
    );
    SelectObject(p.dc, old);
}
pub(super) fn geometry(state: &State, v: &View) -> layout::Geometry {
    layout::geometry(
        f64::from(state.width) / f64::from(state.scale),
        v.mode == Mode::Import,
        v.native(),
        v.warning,
    )
}
pub(super) unsafe fn paint(state: &State, p: &mut Painter<'_>) {
    let Some(v) = state.profile_modal.as_ref() else {
        return;
    };
    let g = geometry(state, v);
    backdrop(state, p);
    p.hits.clear();
    box_(p, g.modal, PANEL, 0x515151, 5.0);
    box_(p, g.header, BG, BG, 0.0);
    tx(
        p,
        if v.mode == Mode::Import {
            "IMPORT PROFILES"
        } else {
            "EXPORT PROFILES"
        },
        g.header,
        p.fonts.normal,
        0x999999,
        DT_CENTER,
    );
    p.image(
        state.icons.get("profile-close"),
        g.close.x.round() as i32,
        g.close.y.round() as i32,
        20,
        20,
    );
    target(p, g.close, Control::Close);
    if v.mode == Mode::Import {
        box_(
            p,
            layout::rect(g.modal.x + 1.0, g.modal.y + 37.0, 600.0, 49.0),
            BG,
            BG,
            0.0,
        );
        box_(
            p,
            layout::rect(g.local.x - 6.0, g.local.y - 6.0, 126.96875, 36.0),
            PANEL,
            BORDER,
            18.0,
        );
        for (r, label, selected, c) in [
            (g.local, "Local", !v.cloud, false),
            (g.cloud, "Cloud", v.cloud, true),
        ] {
            if selected {
                box_(p, r, GREEN, GREEN, 12.0)
            }
            tx(
                p,
                label,
                r,
                p.fonts.normal,
                if selected { 0x212121 } else { TEXT },
                DT_CENTER,
            );
            target(p, r, Control::Cloud(c));
        }
        box_(
            p,
            layout::rect(g.modal.x + 1.0, g.modal.y + 86.0, 600.0, 47.0),
            BG,
            BG,
            0.0,
        );
        let browse = if v.cloud {
            layout::rect(
                g.browse.x - 0.265625,
                g.browse.y,
                g.browse.width,
                g.browse.height,
            )
        } else {
            g.browse
        };
        tx(
            p,
            if v.cloud {
                "Compatible Cloud Profiles"
            } else {
                "BROWSE FOR PROFILES"
            },
            if v.cloud {
                layout::rect(g.modal.x + 90.0, g.browse.y + 5.0, 161.71875, 17.0)
            } else {
                layout::rect(g.modal.x + 101.0, g.browse.y + 5.0, 150.984375, 17.0)
            },
            p.fonts.normal,
            TEXT,
            DT_RIGHT,
        );
        box_(
            p,
            browse,
            if v.cloud {
                faded(PANEL, BG, 0.3)
            } else {
                PANEL
            },
            if v.cloud {
                faded(BORDER, BG, 0.3)
            } else if !v.cloud && hovered(state, g.browse) {
                0x44c62d
            } else {
                BORDER
            },
            0.0,
        );
        tx(
            p,
            if v.cloud {
                "Compatible device"
            } else {
                &v.filename
            },
            layout::rect(
                browse.x + if v.cloud { 6.0 } else { 7.0 },
                browse.y + 5.0,
                218.0,
                17.0,
            ),
            p.fonts.normal,
            if v.cloud { faded(TEXT, BG, 0.3) } else { TEXT },
            DT_LEFT | DT_END_ELLIPSIS,
        );
        if v.cloud {
            if let Some(icon) = state.icons.get("expand") {
                let dc = CreateCompatibleDC(p.dc);
                if !dc.is_null() {
                    let old = SelectObject(dc, icon.handle);
                    GdiAlphaBlend(
                        p.dc,
                        ((browse.x + 220.5) * f64::from(p.scale)).round() as i32,
                        ((browse.y + 8.5) * f64::from(p.scale)).round() as i32,
                        p.px(10),
                        p.px(10),
                        dc,
                        0,
                        0,
                        icon.width,
                        icon.height,
                        BLENDFUNCTION {
                            BlendOp: AC_SRC_OVER as u8,
                            BlendFlags: 0,
                            SourceConstantAlpha: 77,
                            AlphaFormat: 0,
                        },
                    );
                    SelectObject(dc, old);
                    DeleteDC(dc);
                }
            }
        } else {
            p.image(
                state.icons.get("profile-folder"),
                g.browse.x.round() as i32 + 225,
                g.browse.y.round() as i32 + 3,
                20,
                20,
            );
            target(p, g.browse, Control::Browse);
        }
    }
    let has_rows = v.data.as_ref().is_some_and(|d| !d.rows.is_empty()) && !v.cloud;
    if has_rows {
        tx(
            p,
            if v.native() {
                "Full native backup: all devices, profiles and metadata"
            } else if v.deselect_all {
                "Select all profiles"
            } else {
                "Deselect all profiles"
            },
            layout::rect(g.all.x + 20.0, g.all.y + 12.0, 558.0, 19.0),
            p.fonts.product,
            if v.native() { 0xfd8611 } else { GREEN },
            DT_LEFT,
        );
        if !v.native() {
            target(p, g.all, Control::All)
        }
        let saved = SaveDC(p.dc);
        IntersectClipRect(
            p.dc,
            (g.content.x * f64::from(p.scale)).round() as i32,
            (g.content.y * f64::from(p.scale)).round() as i32,
            ((g.content.x + g.content.width) * f64::from(p.scale)).round() as i32,
            ((g.content.y + g.content.height) * f64::from(p.scale)).round() as i32,
        );
        tx(
            p,
            if v.native() {
                "Native backup - imports remain inactive"
            } else {
                &v.device_name
            },
            layout::rect(g.content.x + 20.0, g.content.y - v.scroll, 551.0, 17.0),
            p.fonts.normal,
            TEXT,
            DT_LEFT,
        );
        let (rows, _) = layout::rows(&g, &v.expanded(), v.scroll);
        let hit_start = p.hits.len();
        let data = v.data.as_ref().unwrap();
        for shape in rows {
            let row = &data.rows[shape.index];
            let choice = &v.choices[shape.index];
            let name = if v.native() {
                format!("{} / {}", row.device_key, row.name)
            } else {
                row.name.clone()
            };
            checkbox(
                state,
                p,
                shape.check,
                &name,
                choice.selected,
                !v.native(),
                Control::Profile(shape.index),
                551.0,
            );
            for (m, r) in shape.macros.iter().enumerate() {
                let item = &row.macros[m];
                let line = layout::rect(shape.bounds.x + 32.0, r.y - 5.5, 3.0, 31.0);
                box_(p, line, 0x707070, 0x707070, 0.0);
                if m == 0 {
                    p.image(
                        state.icons.get("profile-macro"),
                        shape.bounds.x.round() as i32 + 45,
                        r.y.round() as i32,
                        20,
                        20,
                    );
                }
                checkbox(
                    state,
                    p,
                    *r,
                    &item.name,
                    v.macro_marks
                        .get(shape.index)
                        .is_some_and(|marks| marks.contains(&item.guid)),
                    item.available && !v.native(),
                    Control::Macro(shape.index, m),
                    476.0,
                );
            }
        }
        for h in p.hits.iter_mut().skip(hit_start) {
            h.rect.top = h
                .rect
                .top
                .max((g.content.y * f64::from(p.scale)).round() as i32);
            h.rect.bottom = h
                .rect
                .bottom
                .min(((g.content.y + g.content.height) * f64::from(p.scale)).round() as i32);
        }
        let mut n = 0;
        p.hits.retain(|h| {
            let yes = n < hit_start || h.rect.bottom > h.rect.top;
            n += 1;
            yes
        });
        RestoreDC(p.dc, saved);
        if let Some(bar) = layout::scrollbar(&g, &v.expanded(), v.scroll) {
            box_(p, bar.track, BG, BG, 0.0);
            let color = if v.scroll_grab.is_some() {
                0x2f941e
            } else if hovered(state, bar.thumb) {
                GREEN
            } else {
                faded(0xffffff, BG, 77.0 / 255.0)
            };
            box_(
                p,
                layout::rect(
                    bar.thumb.x + 1.0,
                    bar.thumb.y + 1.0,
                    6.0,
                    (bar.thumb.height - 2.0).max(0.0),
                ),
                color,
                color,
                3.0,
            );
        }
    } else if v.cloud {
        let r = layout::cloud_message(&g);
        box_(p, r, PANEL, 0xfd8611, 3.0);
        p.image(
            state.icons.get("profile-cone"),
            r.x.round() as i32 + 21,
            r.y.round() as i32 + 20,
            40,
            40,
        );
        tx(
            p,
            "This feature is currently in the works.",
            layout::rect(r.x + 71.0, r.y + 30.5, 260.953125, 19.0),
            p.fonts.product,
            TEXT,
            DT_LEFT,
        );
    } else if !v.error.is_empty() {
        wrapped(p,&format!("Unable to import profile as the file might be empty, corrupted, and/or not compatible. Please try again. {}",v.error),layout::rect(g.modal.x+51.0,g.modal.y+153.0,500.0,180.0),0xfd4949)
    } else if v.loading {
        tx(
            p,
            "Reading profiles...",
            layout::rect(g.modal.x + 51.0, g.modal.y + 153.0, 500.0, 34.0),
            p.fonts.normal,
            MUTED,
            DT_CENTER,
        );
    }
    box_(p, g.footer, BG, BG, 0.0);
    let pen = CreatePen(PS_SOLID, 1, rgb(0x515151));
    let old = SelectObject(p.dc, pen);
    MoveToEx(
        p.dc,
        (g.footer.x * f64::from(p.scale)).round() as i32,
        (g.footer.y * f64::from(p.scale)).round() as i32,
        null_mut(),
    );
    LineTo(
        p.dc,
        ((g.footer.x + g.footer.width) * f64::from(p.scale)).round() as i32,
        (g.footer.y * f64::from(p.scale)).round() as i32,
    );
    SelectObject(p.dc, old);
    DeleteObject(pen);
    if v.mode == Mode::Export {
        tx(
            p,
            "Linked Games and Chroma Effects will not be exported.",
            layout::rect(g.footer.x, g.footer.y + 17.0, 600.0, 16.0),
            p.fonts.normal,
            TEXT,
            DT_CENTER,
        );
    }
    button(state, p, g.cancel, "Cancel", false, true, Control::Close);
    button(
        state,
        p,
        g.commit,
        if v.native() {
            "Import native backup"
        } else if v.mode == Mode::Import {
            "Import"
        } else {
            "Export"
        },
        true,
        v.enabled(),
        Control::Commit,
    );
    if v.warning {
        p.image(
            state.icons.get("profile-warning"),
            g.warning.x.round() as i32,
            g.warning.y.round() as i32 + 3,
            20,
            20,
        );
        if hovered(state, g.warning) {
            let text = if v.native() {
                "Full native backup imports all devices, profiles and opaque settings. No hardware profile is activated."
            } else {
                "Not all mappings from this profile can be carried over to this device."
            };
            let r = layout::rect(g.warning.x - 280.0, g.warning.y + 32.0, 310.0, 72.0);
            box_(p, r, 0, BORDER, 0.0);
            wrapped(
                p,
                text,
                layout::rect(r.x + 10.0, r.y + 8.0, r.width - 20.0, r.height - 16.0),
                TEXT,
            );
        }
    }
}
