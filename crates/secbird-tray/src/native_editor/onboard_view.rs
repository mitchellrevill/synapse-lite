//! Native onboard settings dialog. Deliberately labels settings scope; source
//! slot metadata and installed-family button assignments remain unverified.
use super::*;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Operation {
    Read,
    Save,
    Restore,
}
#[derive(Clone)]
pub(super) struct Request {
    pub(super) id: String,
    pub(super) profile: String,
    pub(super) operation: Operation,
}
#[derive(Clone, Copy)]
pub(super) enum Control {
    Open,
    Close,
    Read,
    Save,
    Restore,
}
#[derive(Default)]
pub(super) struct View {
    pub(super) open: bool,
    owner: Option<(String, String)>,
    snapshot: Option<Value>,
    loading: bool,
    error: Option<String>,
}
pub(super) fn work(engine: &mut Engine, request: &Request) -> Result<Value, String> {
    let mut error = None;
    if request.operation != Operation::Read {
        let body = json!({"id":request.id,"expected_device":request.id,"expected_profile":request.profile,
            "action":if request.operation==Operation::Save {"save"} else {"restore"}});
        if let Err(failure) = server::dispatch(
            engine,
            "POST",
            "/api/onboard-settings",
            &serde_json::to_vec(&body).map_err(|e| e.to_string())?,
        ) {
            error = Some(failure.to_string());
        }
    }
    let mut snapshot = server::onboard_snapshot(engine, &request.id, &request.profile)
        .map_err(|e| e.to_string())?;
    snapshot["operation_error"] = json!(error);
    Ok(snapshot)
}
fn selected(state: &State) -> Option<(String, String)> {
    let d = state.device()?;
    (d["onboard_capabilities"]["slots"] == 1).then(|| {
        (
            d["id"].as_str().unwrap_or_default().into(),
            d["active"].as_str().unwrap_or_default().into(),
        )
    })
}
fn can_write(state: &State) -> bool {
    state.onboard.open
        && !state.restricted_actions
        && !state.read_only_smoke
        && state.pending == 0
        && !state.onboard.loading
        && state.onboard.owner == selected(state)
        && state
            .onboard
            .snapshot
            .as_ref()
            .is_some_and(|s| s["readback"]["available"] == true)
}
pub(super) unsafe fn action(state: &mut State, control: Control) {
    match control {
        Control::Close => {
            state.onboard.open = false;
            state.onboard.owner = None;
            state.onboard.snapshot = None;
            state.rebuild_fields();
            InvalidateRect(state.hwnd, null(), 0);
            return;
        }
        Control::Open => {
            let Some(owner) = selected(state) else { return };
            state.profile_actions = false;
            state.profile_open = false;
            state.onboard = View {
                open: true,
                owner: Some(owner),
                ..Default::default()
            };
            state.rebuild_fields();
        }
        Control::Save | Control::Restore if !can_write(state) => return,
        _ => {}
    }
    let Some((id, profile)) = state.onboard.owner.clone() else {
        return;
    };
    if selected(state) != Some((id.clone(), profile.clone())) {
        return;
    }
    if state.onboard.loading {
        return;
    }
    let operation = match control {
        Control::Save => Operation::Save,
        Control::Restore => Operation::Restore,
        _ => Operation::Read,
    };
    if operation == Operation::Save
        && state
            .onboard
            .snapshot
            .as_ref()
            .is_some_and(|s| s["recovery"].is_object() && s["recovery"]["phase"] != "applied")
    {
        return;
    }
    if operation == Operation::Restore
        && state
            .onboard
            .snapshot
            .as_ref()
            .is_none_or(|s| !s["recovery"].is_object())
    {
        return;
    }
    state.onboard.loading = true;
    state.onboard.error = None;
    state.submit(
        Job::Onboard(Request {
            id,
            profile,
            operation,
        }),
        false,
    );
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn receive(state: &mut State, request: Request, result: Result<Value, String>) {
    let owner = (request.id, request.profile);
    if !state.onboard.open || state.onboard.owner.as_ref() != Some(&owner) {
        return;
    }
    state.onboard.loading = false;
    if selected(state).as_ref() != Some(&owner) {
        state.onboard.snapshot = None;
        state.onboard.error = Some("Selected device or profile changed; close and reopen".into());
    } else {
        match result {
            Ok(value) => {
                state.onboard.error = value["operation_error"].as_str().map(str::to_string);
                state.onboard.snapshot = Some(value);
            }
            Err(error) => {
                state.onboard.snapshot = None;
                state.onboard.error = Some(error);
            }
        }
    }
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn pointer(
    state: &mut State,
    message: u32,
    lparam: LPARAM,
    wparam: WPARAM,
) -> bool {
    if !state.onboard.open {
        return false;
    }
    match message {
        WM_KEYDOWN if wparam == 9 => {
            state.focus = state.focus % state.hits.len().max(1) + 1;
            InvalidateRect(state.hwnd, null(), 0);
            true
        }
        WM_KEYDOWN if wparam == 13 => {
            let picked = state
                .hits
                .get(state.focus.wrapping_sub(1))
                .map(|h| h.action.clone());
            if let Some(Action::Onboard(control)) = picked {
                action(state, control);
            }
            true
        }
        WM_KEYDOWN if wparam == 0x1b => {
            action(state, Control::Close);
            true
        }
        WM_LBUTTONUP => {
            let point = POINT {
                x: (lparam as u32 & 0xffff) as i16 as i32,
                y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
            };
            let picked = state
                .hits
                .iter()
                .rev()
                .find(|hit| PtInRect(&hit.rect, point) != 0)
                .map(|h| h.action.clone());
            if let Some(Action::Onboard(control)) = picked {
                action(state, control);
            }
            true
        }
        WM_LBUTTONDOWN | WM_MOUSEMOVE | WM_RBUTTONUP | WM_MOUSEWHEEL | WM_COMMAND | WM_KEYDOWN => {
            true
        }
        _ => false,
    }
}
pub(super) unsafe fn paint(state: &State, p: &mut Painter) {
    if !state.onboard.open {
        return;
    }
    p.hits.clear();
    let width = (state.width as f32 / state.scale) as i32;
    let height = (state.height as f32 / state.scale) as i32;
    p.fill(0, 128, width, (height - 128).max(0), BG);
    let x = ((width - 690) / 2).max(20);
    let y = 156;
    p.fill(x, y, 650, 420, PANEL);
    p.text(
        "ONBOARD MEMORY",
        x + 24,
        y + 18,
        600,
        30,
        p.fonts.heading,
        GREEN,
        DT_LEFT,
    );
    let profile = state
        .onboard
        .owner
        .as_ref()
        .map(|o| o.1.as_str())
        .unwrap_or("");
    p.text(
        &format!("Slot 1 · {profile}"),
        x + 24,
        y + 56,
        600,
        26,
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    p.text(
        "Save DPI stages and polling from the selected profile.",
        x + 24,
        y + 90,
        600,
        24,
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    p.text(
        "Buttons, macros and lighting are not included.",
        x + 24,
        y + 116,
        600,
        24,
        p.fonts.normal,
        MUTED,
        DT_LEFT,
    );
    let snapshot = state.onboard.snapshot.as_ref();
    let mut line = "Read the device to inspect its saved settings.".to_string();
    if state.onboard.loading {
        line = "Reading onboard settings…".into();
    } else if let Some(value) = snapshot {
        let read = &value["readback"];
        if read["available"] == true {
            let stages = read["stages"]["stages"].as_array().map_or(0, Vec::len);
            line = format!(
                "{} × {} DPI · {} stages · {} Hz",
                read["dpi"][0], read["dpi"][1], stages, read["polling_hz"]
            );
        } else {
            line = read["error"]
                .as_str()
                .unwrap_or("Onboard readback unavailable")
                .into();
        }
    }
    p.text(
        &line,
        x + 24,
        y + 160,
        600,
        48,
        p.fonts.normal,
        TEXT,
        DT_LEFT | DT_WORDBREAK,
    );
    let recovery = snapshot.map(|v| &v["recovery"]);
    let journal = if let Some(r) = recovery.filter(|r| r.is_object()) {
        if r["phase"] == "applied" {
            "Original settings are saved. Restore originals to undo onboard changes."
        } else {
            "An incomplete operation needs recovery. Original settings remain saved."
        }
    } else {
        "Original settings will be saved before the first device write."
    };
    p.text(
        journal,
        x + 24,
        y + 216,
        600,
        46,
        p.fonts.normal,
        MUTED,
        DT_LEFT | DT_WORDBREAK,
    );
    if let Some(error) = &state.onboard.error {
        p.text(
            error,
            x + 24,
            y + 268,
            600,
            62,
            p.fonts.small,
            0xfd4949,
            DT_LEFT | DT_WORDBREAK,
        );
    }
    let enabled = can_write(state);
    if !state.onboard.loading && state.pending == 0 {
        p.button(
            "Read again",
            x + 24,
            y + 344,
            120,
            Action::Onboard(Control::Read),
            false,
        );
    }
    if enabled && recovery.is_none_or(|r| !r.is_object() || r["phase"] == "applied") {
        p.button(
            "Save profile settings",
            x + 154,
            y + 344,
            180,
            Action::Onboard(Control::Save),
            false,
        );
    }
    if enabled && recovery.is_some_and(|r| r.is_object()) {
        p.button(
            "Restore originals",
            x + 344,
            y + 344,
            165,
            Action::Onboard(Control::Restore),
            false,
        );
    }
    p.button(
        "Close",
        x + 526,
        y + 344,
        100,
        Action::Onboard(Control::Close),
        false,
    );
    p.text(
        "Hardware acceptance remains unverified. Device writes require an attended check.",
        x + 24,
        y + 386,
        600,
        24,
        p.fonts.tiny,
        MUTED,
        DT_LEFT,
    );
}

#[cfg(test)]
pub(super) fn diagnostics(state: &State) -> Value {
    json!({"open":state.onboard.open,"snapshot":state.onboard.snapshot,"error":state.onboard.error,"loading":state.onboard.loading})
}
