//! Normal macro surface recovered from MacroContent and MacroMenu source CSS.
//! The existing detailed event tools remain reachable through the action menu.
use super::*;

unsafe fn icon(state: &State, p: &Painter, role: &str, x: i32, y: i32) {
    p.image(state.icons.get(role), x, y, 20, 20);
}
unsafe fn hit(p: &mut Painter, x: i32, y: i32, w: i32, h: i32, control: Control) {
    p.hits.push(Hit {
        rect: p.rect(x, y, w, h),
        action: Action::MacroLibrary(control),
    });
}
pub(super) fn editor_left(width: i32) -> i32 {
    ((width + 20 - 600) / 2).max(290)
}
pub(super) fn selected_row_y(view: &View, height: i32) -> Option<i32> {
    view.draft
        .as_ref()?
        .entry
        .events
        .iter()
        .enumerate()
        .filter(|(_, event)| event["Type"] != "actionBar")
        .skip(view.scroll)
        .take(((height - 255).max(0) / 42) as usize)
        .position(|(index, _)| Some(index) == view.event)
        .map(|offset| 202 + offset as i32 * 42)
}
pub(super) unsafe fn paint(state: &State, p: &mut Painter, width: i32, height: i32) {
    let view = &state.macros;
    let draft = view.draft.as_ref();
    let can = view.recording.is_none()
        && !view.busy
        && !view.playing()
        && !state.read_only_smoke
        && !state.restricted_actions;
    p.fill(0, 80, width, height - 80, BG);
    p.line(0, 127, width, 0);
    icon(state, p, "profile-default", 12, 91);
    p.fill(46, 89, 230, 28, 0x5d5d5d);
    p.fill(47, 90, 228, 26, BG);
    p.text(
        draft.map_or("Select a macro", |d| d.entry.name.as_str()),
        52,
        89,
        200,
        28,
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    p.text("▾", 252, 89, 18, 28, p.fonts.normal, MUTED, DT_CENTER);
    hit(p, 46, 89, 230, 28, Control::List);
    icon(state, p, "more", 288, 93);
    hit(p, 284, 89, 28, 28, Control::Actions);
    p.fill(322, 93, 1, 20, 0x5d5d5d);
    p.text("New Macro", 332, 89, 100, 28, p.fonts.normal, TEXT, DT_LEFT);
    p.line(332, 110, 72, TEXT);
    if can {
        hit(p, 330, 89, 104, 28, Control::New);
    }
    p.navigation(
        "MY MACROS",
        width / 2 - 90,
        89,
        90,
        Action::MacroLibrary(Control::Open),
        true,
    );
    p.text(
        "KEY BINDS",
        width / 2 + 20,
        89,
        90,
        28,
        p.fonts.small,
        0x555555,
        DT_CENTER,
    );
    // Source: left:20px/right:0/margin:auto, min/max-width:600px;
    // the 250px add menu sits at margin-left:-260px.
    let editor = editor_left(width);
    let menu = editor - 260;
    let panel = |x, y, w, h| performance_layout::Rect {
        x: x as f64,
        y: y as f64,
        width: w as f64,
        height: h as f64,
    };
    performance_view::rounded(p, panel(menu, 148, 250, 370), PANEL, PANEL, 5.);
    p.text(
        "ADD",
        menu + 10,
        158,
        230,
        30,
        p.fonts.product,
        TEXT,
        DT_LEFT,
    );
    let items = [
        ("DELAY", "host-macro-delay", Control::AddDelay),
        (
            "KEYBOARD FUNCTION",
            "host-macro-keyboard",
            Control::InputMenu,
        ),
        ("MOUSE FUNCTION", "host-macro-mouse", Control::InputMenu),
        ("MACRO", "host-macro", Control::InputMenu),
        (
            "LAUNCH",
            "host-macro-launch",
            Control::OpenHost(host_actions::Kind::Program),
        ),
        (
            "RUN COMMAND",
            "host-macro-command",
            Control::OpenHost(host_actions::Kind::Command),
        ),
        (
            "TEXT FUNCTION",
            "host-macro-text",
            Control::OpenHost(host_actions::Kind::Text),
        ),
        ("LOOP", "host-macro-loop", Control::WrapLoop),
    ];
    for (index, (text, role, control)) in items.into_iter().enumerate() {
        let y = 188 + index as i32 * 40;
        icon(state, p, role, menu + 10, y + 10);
        p.text(
            text,
            menu + 42,
            y,
            198,
            40,
            p.fonts.small,
            if can && draft.is_some() {
                TEXT
            } else {
                0x555555
            },
            DT_LEFT,
        );
        if can && draft.is_some() {
            hit(p, menu, y, 250, 40, control);
        }
    }
    performance_view::rounded(
        p,
        panel(editor, 148, 600, (height - 160).max(420)),
        PANEL,
        PANEL,
        5.,
    );
    icon(state, p, "host-macro-delay", editor + 52, 164);
    let elapsed = draft.map_or(0., |d| {
        d.entry
            .events
            .iter()
            .filter(|e| e["Type"] == 0)
            .filter_map(|e| {
                e["Number"]
                    .as_f64()
                    .or_else(|| e["Number"].as_str()?.parse::<f64>().ok())
            })
            .sum::<f64>()
    });
    p.text(
        &format!("{:.3}s", elapsed + 0.0),
        editor + 82,
        160,
        120,
        28,
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    performance_view::rounded(p, panel(editor + 240, 160, 122, 28), 0x777777, 0x777777, 3.);
    icon(state, p, "host-macro-record", editor + 250, 164);
    p.text(
        if view.recording.is_some() || view.playing() {
            "STOP"
        } else {
            "RECORD"
        },
        editor + 273,
        160,
        66,
        28,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    p.text(
        "▾",
        editor + 340,
        160,
        20,
        28,
        p.fonts.small,
        PANEL,
        DT_CENTER,
    );
    if !view.busy && !state.read_only_smoke && !state.restricted_actions && draft.is_some() {
        hit(
            p,
            editor + 240,
            160,
            122,
            28,
            if view.playing() {
                Control::Stop
            } else if view.recording.is_some() {
                Control::StopRecording
            } else {
                Control::Record
            },
        );
    }
    for (x, role, control, enabled) in [
        (
            editor + 398,
            "host-macro-undo",
            Control::Undo,
            can && draft.is_some_and(|d| !d.undo.is_empty()),
        ),
        (
            editor + 434,
            "host-macro-redo",
            Control::Redo,
            can && draft.is_some_and(|d| !d.redo.is_empty()),
        ),
    ] {
        let enabled_role = format!("{role}-enabled");
        icon(state, p, if enabled { &enabled_role } else { role }, x, 164);
        if enabled {
            hit(p, x - 4, 158, 28, 32, control);
        }
    }
    p.fill(
        editor + 480,
        160,
        100,
        28,
        if can && view.dirty() { GREEN } else { 0x224f1c },
    );
    p.text(
        "SAVE",
        editor + 480,
        160,
        100,
        28,
        p.fonts.small,
        0x111111,
        DT_CENTER,
    );
    if can && view.dirty() {
        hit(p, editor + 480, 160, 100, 28, Control::Save);
    }
    p.line(editor, 201, 600, 0);
    if let Some(draft) = draft {
        for (offset, (index, event)) in draft
            .entry
            .events
            .iter()
            .enumerate()
            .filter(|(_, e)| e["Type"] != "actionBar")
            .skip(view.scroll)
            .take(((height - 255).max(0) / 42) as usize)
            .enumerate()
        {
            let y = 202 + offset as i32 * 42;
            if view.event == Some(index) {
                p.fill(editor + 1, y, 598, 41, 0x1b3816);
            }
            p.line(editor + 1, y + 41, 598, 0x222222);
            let role = match event["Type"].as_u64() {
                Some(0) => "host-macro-delay",
                Some(1) => "host-macro-keyboard",
                Some(2) => "host-macro-mouse",
                Some(6) => "host-macro-loop",
                Some(7) => "host-macro",
                _ => "host-macro-launch",
            };
            icon(state, p, role, editor + 52, y + 11);
            hit(p, editor + 1, y, 598, 42, Control::Event(index));
            let fixed = view.event == Some(index) && fixed_delay(&draft.entry, index).is_some();
            let ranged = view.event == Some(index) && random_delay::selected(view);
            let loop_row = view.event == Some(index) && loop_pair(view).is_some();
            if fixed {
                p.text(
                    "seconds",
                    editor + 182,
                    y,
                    90,
                    42,
                    p.fonts.normal,
                    TEXT,
                    DT_LEFT,
                );
            } else if ranged {
                p.text("ms", editor + 345, y, 35, 42, p.fonts.normal, TEXT, DT_LEFT);
            } else {
                p.text(
                    &event_label(event, view.data.as_ref().map(|data| &data.library)),
                    editor + 82,
                    y,
                    if loop_row { 150 } else { 430 },
                    42,
                    p.fonts.normal,
                    TEXT,
                    DT_LEFT,
                );
            }
            if view.event == Some(index) {
                if can && (event["Type"] == 1 || event["Type"] == 2 || event["Type"] == 7) {
                    hit(p, editor + 82, y + 3, 360, 36, Control::EditInputMenu);
                }
                if can {
                    icon(state, p, "host-macro-delete", editor + 530, y + 11);
                    hit(p, editor + 526, y + 4, 28, 34, Control::DeleteEvent);
                }
                icon(state, p, "more", editor + 564, y + 11);
                hit(p, editor + 558, y + 4, 32, 34, Control::RowMenu);
            }
        }
    }
    if !view.status.is_empty() {
        p.text(
            &view.status,
            30,
            height - 40,
            width - 60,
            36,
            p.fonts.small,
            if view.failed { 0xfd4949 } else { MUTED },
            DT_LEFT | DT_WORDBREAK,
        );
    }
}
pub(super) unsafe fn actions(state: &State, p: &mut Painter, can: bool) {
    let draft = state.macros.draft.as_ref();
    p.fill(285, 118, 180, 276, 0);
    for (index, (label, control, enabled)) in [
        ("Add", Control::New, can),
        ("Import", Control::Import, can),
        ("Rename", Control::Rename, draft.is_some()),
        ("Duplicate", Control::Duplicate, can && draft.is_some()),
        ("Export", Control::Export, can && draft.is_some()),
        (
            "Delete",
            Control::Delete,
            can && draft.is_some_and(|d| !d.entry.referenced),
        ),
        (
            "Discard changes",
            Control::Discard,
            can && state.macros.dirty(),
        ),
        (
            "Play once",
            Control::Play,
            can && !state.macros.dirty() && draft.is_some(),
        ),
        (
            "Cancel recording",
            Control::CancelRecording,
            state.macros.recording.is_some(),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let y = 120 + index as i32 * 30;
        p.text(
            label,
            292,
            y,
            166,
            30,
            p.fonts.normal,
            if enabled { TEXT } else { 0x555555 },
            DT_LEFT,
        );
        if enabled {
            hit(p, 286, y, 178, 30, control);
        }
    }
}
pub(super) unsafe fn row_actions(
    state: &State,
    p: &mut Painter,
    can: bool,
    width: i32,
    height: i32,
) {
    let Some(y) = selected_row_y(&state.macros, height) else {
        return;
    };
    let x = editor_left(width) + 360;
    let y = (y + 42).min(height - 310);
    p.fill(x, y, 240, 300, 0);
    for (index, (label, control)) in [
        ("Edit action / input", Control::RebindInput),
        ("Input / macro", Control::EditInputMenu),
        ("Press / release", Control::GestureMenu),
        ("Apply value", Control::ApplyDelay),
        ("Move up", Control::Move(-1)),
        ("Move down", Control::Move(1)),
        ("Start loop here", Control::MarkLoopStart),
        ("Wrap selection in loop", Control::WrapLoop),
        ("Apply loop repeat", Control::ApplyLoop),
        ("Delete", Control::DeleteEvent),
    ]
    .into_iter()
    .enumerate()
    {
        let top = y + index as i32 * 30;
        p.text(
            label,
            x + 10,
            top,
            220,
            30,
            p.fonts.normal,
            if can { TEXT } else { MUTED },
            DT_LEFT,
        );
        if can {
            hit(p, x, top, 240, 30, control);
        }
    }
}
