//! Retained local Chroma dashboard. All writes use the existing ownership-bound
//! Quick endpoint; the dashboard never grants itself lighting ownership.
use super::*;

pub(super) const ID: &str = "module:chroma";
#[cfg(test)]
pub(super) const TEST: u32 = WM_APP + 180;
const MAX_TARGETS: usize = 64;
const MAX_SELECTION_BYTES: usize = 2 * 1024 * 1024;
const EFFECTS: &[(u8, &str)] = &[
    (3, "SPECTRUM CYCLING"),
    (1, "STATIC"),
    (2, "BREATHING"),
    (4, "WAVE"),
    (7, "STARLIGHT"),
];

#[derive(Clone)]
pub(super) enum Control {
    Open,
    Device(String),
    Dismiss,
    Effect(u8),
    Apply,
    Lighting(String),
}
#[derive(Default)]
pub(super) struct View {
    pub used: bool,
    pub reader: Option<crate::lighting_status::Reader>,
    modal: Option<String>,
    effect: Option<u8>,
    pub busy: bool,
    pub report: Value,
    scroll: usize,
}
#[derive(Clone, Debug)]
pub(super) struct Target {
    id: String,
    profile: String,
    setting: Value,
}
pub(super) struct Request {
    effect: u8,
    targets: Vec<Target>,
}
pub(super) struct Response {
    state: Value,
    report: Value,
}
pub(super) fn active(state: &State) -> bool {
    state.selected.as_deref() == Some(ID)
}

// A known product and a usable snapshot are necessary, not sufficient: the
// endpoint still checks fresh transport, mode, profile and competing ownership.
fn refusal(device: &Value) -> Option<&'static str> {
    if device["known"] != true {
        return Some("Unknown device; lighting controls unavailable");
    }
    if device["pid"] != "025D" {
        return Some("No verified Chroma lighting route for this device");
    }
    if device["lighting_control"] != true {
        return Some("Lighting control is disabled in the device profile");
    }
    if device["active"].as_str().is_none() || device["id"].as_str().is_none() {
        return Some("Device/profile identity unavailable");
    }
    if device["lighting_readback"]["available"] != true
        || device["lighting_readback"]["selectors_match"] != true
    {
        return Some("Lighting readback unavailable or selectors differ");
    }
    if device["chroma_quick"]["source_effects"]
        .as_array()
        .is_none_or(|effects| effects.is_empty())
    {
        return Some("Device mode or worker has no admitted Quick lighting route");
    }
    None
}
fn supports(device: &Value, effect: u8) -> bool {
    refusal(device).is_none()
        && device["chroma_quick"]["source_effects"]
            .as_array()
            .is_some_and(|effects| effects.iter().any(|id| id == effect))
}
fn setting(effect: u8) -> Option<Value> {
    Some(match effect {
        1 => json!({"color1":"#00ff00"}),
        2 => json!({"color1":"#00ff00","color2":"no-color","isRandom":false}),
        3 => json!({}),
        4 => json!({"direction":2}),
        7 => json!({"color1":"#00ff00","color2":"no-color","isRandom":false,"duration":2}),
        _ => return None,
    })
}
fn effect_name(device: &Value) -> String {
    match device["source_quick_effects"]["selectedEffectId"].as_u64() {
        Some(id) => EFFECTS
            .iter()
            .find(|(effect, _)| u64::from(*effect) == id)
            .map(|(_, label)| (*label).to_string())
            .unwrap_or_else(|| format!("Retained effect {id}")),
        None => "Not selected".into(),
    }
}
fn saved_selection(devices: &[Value]) -> String {
    let eligible: Vec<_> = devices
        .iter()
        .filter(|device| refusal(device).is_none())
        .collect();
    let Some(first) = eligible.first() else {
        return "Unavailable".into();
    };
    let quick = &first["source_quick_effects"];
    if eligible.iter().all(|device| {
        device["source_quick_effects"]["selectedEffectId"] == quick["selectedEffectId"]
            && device["source_quick_effects"]["selectedEffectSetting"]
                == quick["selectedEffectSetting"]
    }) {
        effect_name(first)
    } else {
        "Mixed device settings".into()
    }
}
fn plan(devices: &[Value], modal: Option<&str>, effect: u8) -> Result<Request, String> {
    let defaults = setting(effect).ok_or("This dashboard effect has no supported durable route")?;
    let mut targets = Vec::new();
    let mut bytes = 0usize;
    for device in devices {
        if modal.is_some_and(|id| device["id"].as_str() != Some(id)) {
            continue;
        }
        if let Some(reason) = refusal(device) {
            if modal.is_some() {
                return Err(reason.into());
            }
            continue;
        }
        if !supports(device, effect) {
            if modal.is_some() {
                return Err("This effect is unavailable in the current device mode/worker".into());
            }
            continue;
        }
        let mut selected = defaults.clone();
        // Reselecting the current effect preserves its complete opaque setting.
        if device["source_quick_effects"]["selectedEffectId"] == effect {
            if let Some(value) = device["source_quick_effects"]
                .get("selectedEffectSetting")
                .filter(|v| v.is_object())
            {
                selected = value.clone();
            }
        }
        bytes = bytes.saturating_add(
            serde_json::to_vec(&selected)
                .map_err(|error| error.to_string())?
                .len(),
        );
        if bytes > MAX_SELECTION_BYTES {
            return Err("Captured Chroma settings exceed the 2 MiB selection budget".into());
        }
        targets.push(Target {
            id: device["id"].as_str().unwrap().into(),
            profile: device["active"].as_str().unwrap().into(),
            setting: selected,
        });
    }
    if targets.is_empty() {
        return Err("No eligible connected lighting devices in this selection".into());
    }
    if targets.len() > MAX_TARGETS {
        return Err("Selection exceeds the 64-device dashboard budget".into());
    }
    Ok(Request { effect, targets })
}

pub(super) fn work(engine: &mut Engine, request: Request) -> Result<Response, String> {
    if engine.is_measurement_read_only() {
        return Err("Read-only mode refuses Chroma changes".into());
    }
    if setting(request.effect).is_none()
        || request.targets.is_empty()
        || request.targets.len() > MAX_TARGETS
    {
        return Err("Invalid Chroma selection".into());
    }
    let current = server::dispatch(engine, "GET", "/api/state", &[]).map_err(|e| e.to_string())?;
    let devices = current["devices"]
        .as_array()
        .ok_or("Device snapshot unavailable")?;
    let mut seen = std::collections::HashSet::new();
    // Preflight the whole captured membership before the first write. Never
    // substitute a newly connected device or a newly active profile.
    for target in &request.targets {
        if !seen.insert(target.id.clone()) {
            return Err("Duplicate Chroma target".into());
        }
        let device = devices
            .iter()
            .find(|d| d["id"] == target.id)
            .ok_or("A selected device disconnected; refresh Chroma")?;
        if let Some(reason) = refusal(device) {
            return Err(reason.into());
        }
        if !supports(device, request.effect) {
            return Err("A selected device mode/worker changed; refresh Chroma".into());
        }
        if device["active"] != target.profile {
            return Err("A selected profile changed; refresh Chroma".into());
        }
        engine
            .guard_studio_quick_action(&target.id, "/api/source-effect")
            .map_err(|e| e.to_string())?;
    }
    let mut results = Vec::new();
    for target in request.targets {
        let body = json!({"id":target.id,"expected_device":target.id,
            "expected_profile":target.profile,"source_effect_id":request.effect,
            "source_effect_setting":target.setting});
        let answer = server::dispatch(
            engine,
            "POST",
            "/api/source-effect",
            &serde_json::to_vec(&body).map_err(|e| e.to_string())?,
        );
        let row = match answer {
            Ok(answer) => {
                json!({"id":target.id,"profile":target.profile,"ok":true,"answer":answer})
            }
            Err(error) => {
                json!({"id":target.id,"profile":target.profile,"ok":false,"error":error.to_string()})
            }
        };
        let failed = row["ok"] == false;
        results.push(row);
        // A failed transfer is not replayed or rolled back with more HID writes.
        if failed {
            break;
        }
    }
    let state = server::dispatch(engine, "GET", "/api/state", &[]).map_err(|e| e.to_string())?;
    Ok(Response {
        state,
        report: json!({"effect":request.effect,"results":results,
        "requested":seen.len(),"automatic_retry":false}),
    })
}

pub(super) unsafe fn receive(state: &mut State, response: Result<Response, String>) {
    state.chroma.busy = false;
    match response {
        Ok(response) => {
            state.state = response.state;
            state.failed = response.report["results"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|row| row["ok"] == false));
            state.status = if let Some(row) = response.report["results"]
                .as_array()
                .and_then(|rows| rows.iter().find(|row| row["ok"] == false))
            {
                format!(
                    "{}: {} · stopped without retry; earlier targets may have changed",
                    row["id"].as_str().unwrap_or("Device"),
                    row["error"].as_str().unwrap_or("Failed")
                )
            } else if let Some(reason) = response.report["results"]
                .as_array()
                .and_then(|rows| rows.iter().find_map(|row| row["answer"]["reason"].as_str()))
            {
                format!("Quick selection saved; output deferred: {reason}")
            } else {
                "Chroma selection saved; output status is reported separately".into()
            };
            state.chroma.report = response.report;
        }
        Err(error) => {
            state.failed = true;
            state.status = error.clone();
            state.chroma.report = json!({"error":error});
        }
    }
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn action(state: &mut State, control: Control) {
    match control {
        Control::Open => {
            if !studio_view::can_leave_to_chroma(state) {
                return;
            }
            state.chroma.used = true;
            state.chroma.modal = None;
            state.chroma.effect = None;
            state.action(Action::Device(ID.into()));
        }
        Control::Device(id) => {
            state.chroma.modal = Some(id);
            state.chroma.effect = None;
        }
        Control::Dismiss => {
            state.chroma.modal = None;
            state.chroma.effect = None;
        }
        Control::Effect(effect) => {
            if setting(effect).is_some() {
                state.chroma.effect = Some(effect);
            }
        }
        Control::Lighting(id) => {
            if state
                .devices()
                .iter()
                .any(|d| d["id"] == id && d["pid"] == "025D")
            {
                state.chroma.modal = None;
                state.action(Action::Device(id));
                state.action(Action::Tab(Tab::Lighting));
            }
        }
        Control::Apply => {
            if state.chroma.busy
                || state.pending != 0
                || state.read_only_smoke
                || state.restricted_actions
            {
                return;
            }
            let request = state
                .chroma
                .effect
                .ok_or_else(|| "Choose an effect first".to_string())
                .and_then(|effect| plan(state.devices(), state.chroma.modal.as_deref(), effect));
            match request {
                Ok(request) => {
                    let before = (state.pending, state.pending_jobs.len());
                    state.queue(Job::Chroma(request));
                    state.chroma.busy = before != (state.pending, state.pending_jobs.len());
                    if state.chroma.busy {
                        state.status = "Applying captured Chroma selection…".into();
                    }
                }
                Err(error) => {
                    state.failed = true;
                    state.status = error;
                }
            }
        }
    }
    state.rebuild_fields();
    InvalidateRect(state.hwnd, null(), 0);
}
fn runtime(state: &State, device: &Value) -> String {
    let snapshot = device["id"]
        .as_str()
        .and_then(|id| state.chroma.reader.as_ref()?.read_device(id));
    let Some(snapshot) = snapshot else {
        return "Output status unavailable · refresh to read device state".into();
    };
    if snapshot.observed_at.elapsed() > Duration::from_secs(3) {
        return "Output status is old · Stop remains available".into();
    }
    let value = snapshot.value.as_ref();
    if let Some(studio) = value.get("studio") {
        if let Some(error) = studio["runtime_error"].as_str() {
            return format!("Studio: {error}");
        }
        return format!(
            "Studio owns lighting · {}",
            studio["state"].as_str().unwrap_or("unknown")
        );
    }
    if let Some(error) = value["error"].as_str() {
        return error.into();
    }
    if value["suppressed"] == true {
        return format!(
            "Lighting suppressed · {}",
            value["blocked_reason"]
                .as_str()
                .unwrap_or("policy or competing owner")
        );
    }
    for key in ["uniform_host", "host_static"] {
        if let Some(host) = value.get(key).filter(|v| v.is_object()) {
            if let Some(error) = host["error"].as_str() {
                return error.into();
            }
            if let Some(reason) = host["reason"].as_str() {
                return reason.into();
            }
            if host["worker_enabled"] == false {
                return "Lighting worker stopped".into();
            }
            return if host["streaming"] == true {
                "Quick lighting · shared worker streaming".into()
            } else {
                format!(
                    "Quick lighting · {}",
                    host["blocked_reason"]
                        .as_str()
                        .or_else(|| host["state"].as_str())
                        .unwrap_or("status available")
                )
            };
        }
    }
    format!(
        "Quick lighting · {}",
        value["blocked_reason"]
            .as_str()
            .unwrap_or("no active host session reported")
    )
}
// Keep the source-matching drawing parameters explicit; grouping them would churn geometry call sites.
#[allow(clippy::too_many_arguments)]
unsafe fn button(
    p: &mut Painter,
    label: &str,
    x: i32,
    y: i32,
    w: i32,
    control: Control,
    enabled: bool,
    selected: bool,
) {
    if enabled {
        p.button(label, x, y, w, Action::Chroma(control), selected);
    } else {
        p.text(label, x, y, w, 28, p.fonts.small, MUTED, DT_CENTER);
    }
}
// Keep the source-matching drawing parameters explicit; grouping them would churn geometry call sites.
#[allow(clippy::too_many_arguments)]
unsafe fn wrapped(
    p: &Painter,
    text: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    color: u32,
    align: u32,
) {
    let previous = SelectObject(p.dc, p.fonts.small);
    SetBkMode(p.dc, TRANSPARENT as i32);
    SetTextColor(p.dc, rgb(color));
    let mut rect = p.rect(x, y, width, height);
    DrawTextW(
        p.dc,
        wide(text).as_ptr(),
        -1,
        &mut rect,
        align | DT_WORDBREAK | DT_END_ELLIPSIS | DT_NOPREFIX,
    );
    SelectObject(p.dc, previous);
}
unsafe fn effects(state: &State, p: &mut Painter, x: i32, y: i32, width: i32, enabled: bool) {
    let w = ((width - 40) / 5).min(175);
    for (index, (id, label)) in EFFECTS.iter().enumerate() {
        button(
            p,
            label,
            x + index as i32 * (w + 8),
            y,
            w,
            Control::Effect(*id),
            enabled
                && state.devices().iter().any(|device| {
                    state
                        .chroma
                        .modal
                        .as_deref()
                        .is_none_or(|target| device["id"].as_str() == Some(target))
                        && supports(device, *id)
                }),
            state.chroma.effect == Some(*id),
        );
    }
}
pub(super) unsafe fn paint(state: &State, p: &mut Painter, width: i32, height: i32) {
    let left = ((width - 1120) / 2).max(30);
    let room = (width - 2 * left).min(1120);
    p.text(
        "DASHBOARD",
        left,
        94,
        200,
        28,
        p.fonts.normal,
        GREEN,
        DT_LEFT,
    );
    p.button(
        "STUDIO",
        width - left - 290,
        94,
        120,
        Action::Studio(studio_view::Control::Enter),
        false,
    );
    p.button(
        "STOP STUDIO",
        width - left - 160,
        94,
        160,
        Action::Studio(studio_view::Control::StopDevice),
        false,
    );
    p.text(
        "APPLY LIGHTING EFFECTS",
        left,
        150,
        room,
        26,
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    let enabled = !state.chroma.busy
        && state.pending == 0
        && !state.read_only_smoke
        && !state.restricted_actions;
    effects(state, p, left, 188, room, enabled);
    let eligible = state
        .devices()
        .iter()
        .filter(|d| {
            state
                .chroma
                .effect
                .map_or_else(|| refusal(d).is_none(), |effect| supports(d, effect))
        })
        .count();
    p.text(
        &format!(
            "{eligible} eligible device(s) · Saved: {} · applies to active profiles",
            saved_selection(state.devices())
        ),
        left,
        226,
        room - 160,
        28,
        p.fonts.small,
        MUTED,
        DT_LEFT,
    );
    button(
        p,
        "APPLY TO ALL",
        left + room - 150,
        226,
        150,
        Control::Apply,
        enabled && eligible > 0 && state.chroma.effect.is_some(),
        false,
    );
    p.text(
        "RAZER CHROMA DEVICES",
        left,
        276,
        room,
        26,
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    let columns = (room / 285).max(1);
    let visible_rows = ((height - 390) / 250).max(1);
    let page = (columns * visible_rows) as usize;
    for (index, device) in state
        .devices()
        .iter()
        .skip(state.chroma.scroll)
        .take(page)
        .enumerate()
    {
        let x = left + (index as i32 % columns) * 285;
        let y = 312 + (index as i32 / columns) * 250;
        p.fill(x, y, 270, 235, PANEL);
        if !state
            .device_artwork
            .borrow_mut()
            .draw(p, device, x + 20, y + 10, 230, 105)
            && device
                .get("edition_id")
                .is_none_or(|id| id.is_null() || id.as_u64() == Some(0))
        {
            let image = match device["pid"].as_str() {
                Some("025D") => state.keyboard.as_ref(),
                Some("00C4" | "00C5") => state.mouse.as_ref(),
                _ => None,
            };
            p.image(image, x + 20, y + 10, 230, 105);
        }
        p.text(
            device["name"].as_str().unwrap_or("Unknown device"),
            x + 12,
            y + 120,
            246,
            30,
            p.fonts.normal,
            TEXT,
            DT_CENTER,
        );
        let label = refusal(device).map(str::to_owned).unwrap_or_else(|| {
            format!(
                "Profile: {}\nQuick effect: {}",
                device["active"].as_str().unwrap_or("?"),
                effect_name(device)
            )
        });
        wrapped(p, &label, x + 12, y + 155, 246, 38, MUTED, DT_CENTER);
        if let Some(id) = device["id"].as_str() {
            button(
                p,
                "LIGHTING DETAILS",
                x + 50,
                y + 197,
                170,
                Control::Device(id.into()),
                true,
                false,
            );
        }
    }
    if state.devices().is_empty() {
        p.text(
            "No connected devices · catalogue entries are not lighting targets",
            left,
            320,
            room,
            50,
            p.fonts.normal,
            MUTED,
            DT_LEFT,
        );
    }
    let bottom = (height - 90).max(570);
    let unavailable = state.state["unsupported"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|d| {
                    format!(
                        "{}: {}",
                        d["name"].as_str().unwrap_or("Device"),
                        d["reason"].as_str().unwrap_or("Unavailable")
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .unwrap_or_default();
    wrapped(p, &unavailable, left, bottom, room, 45, MUTED, DT_LEFT);
    if let Some(id) = &state.chroma.modal {
        // Discard background hit targets; keyboard and pointer stay in the modal.
        p.hits.retain(|hit| {
            matches!(
                hit.action,
                Action::Close | Action::Minimize | Action::Maximize
            )
        });
        let x = (width - 900) / 2;
        let y = 145;
        p.fill(x, y, 900, 400, 0x111111);
        p.line(x, y, 900, BORDER);
        p.text(
            "DEVICE LIGHTING",
            x + 24,
            y + 16,
            700,
            28,
            p.fonts.heading,
            GREEN,
            DT_LEFT,
        );
        button(
            p,
            "CLOSE",
            x + 790,
            y + 16,
            85,
            Control::Dismiss,
            true,
            false,
        );
        let device = state.devices().iter().find(|d| d["id"] == *id);
        if let Some(device) = device {
            p.text(
                device["name"].as_str().unwrap_or("Device"),
                x + 24,
                y + 60,
                850,
                30,
                p.fonts.normal,
                TEXT,
                DT_LEFT,
            );
            let label = format!(
                "Active profile: {} · Saved Quick effect: {}",
                device["active"].as_str().unwrap_or("Unavailable"),
                effect_name(device)
            );
            p.text(
                &label,
                x + 24,
                y + 98,
                850,
                28,
                p.fonts.small,
                TEXT,
                DT_LEFT,
            );
            wrapped(
                p,
                &runtime(state, device),
                x + 24,
                y + 140,
                850,
                52,
                MUTED,
                DT_LEFT,
            );
            if let Some(reason) = refusal(device) {
                p.text(
                    reason,
                    x + 24,
                    y + 208,
                    850,
                    45,
                    p.fonts.normal,
                    MUTED,
                    DT_LEFT,
                );
            } else {
                effects(state, p, x + 24, y + 210, 850, enabled);
                button(
                    p,
                    "APPLY TO DEVICE",
                    x + 24,
                    y + 260,
                    190,
                    Control::Apply,
                    enabled && state.chroma.effect.is_some(),
                    false,
                );
                button(
                    p,
                    "QUICK SETTINGS",
                    x + 230,
                    y + 260,
                    190,
                    Control::Lighting(id.clone()),
                    true,
                    false,
                );
                p.button(
                    "STUDIO",
                    x + 436,
                    y + 260,
                    160,
                    Action::Studio(studio_view::Control::Enter),
                    false,
                );
                p.button(
                    "STOP STUDIO",
                    x + 612,
                    y + 260,
                    200,
                    Action::Studio(studio_view::Control::StopDevice),
                    false,
                );
            }
        } else {
            p.text(
                "Device disconnected · close and refresh Chroma",
                x + 24,
                y + 80,
                850,
                50,
                p.fonts.normal,
                MUTED,
                DT_LEFT,
            );
        }
        wrapped(
            p,
            &state.status,
            x + 24,
            y + 320,
            850,
            60,
            if state.failed { 0xfd4949 } else { MUTED },
            DT_LEFT,
        );
    }
}
pub(super) unsafe fn wheel(state: &mut State, delta: i16) {
    if state.chroma.modal.is_some() {
        return;
    }
    let max = state.devices().len().saturating_sub(1);
    state.chroma.scroll = if delta < 0 {
        (state.chroma.scroll + 3).min(max)
    } else {
        state.chroma.scroll.saturating_sub(3)
    };
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn escape(state: &mut State) -> bool {
    if active(state) && state.chroma.modal.is_some() {
        action(state, Control::Dismiss);
        return true;
    }
    false
}

#[cfg(test)]
pub(super) fn diagnostics(state: &State) -> Value {
    json!({"open":active(state),"modal":state.chroma.modal,"effect":state.chroma.effect,
        "busy":state.chroma.busy,"pending":state.pending,"report":state.chroma.report,
        "status":state.status,"failed":state.failed})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn device(id: &str) -> Value {
        json!({"id":id,"pid":"025D","known":true,"active":"Work","lighting_control":true,"lighting_readback":{"available":true,"selectors_match":true},"chroma_quick":{"source_effects":[1,2,3,4,7],"mode":0}})
    }
    #[test]
    fn selection_filters_unknown_disabled_nonlighting_and_binds_exact_profiles() {
        let mut disabled = device("disabled");
        disabled["lighting_control"] = json!(false);
        let mut unknown = device("unknown");
        unknown["known"] = json!(false);
        let mut mouse = device("mouse");
        mouse["pid"] = json!("00C4");
        let mut offline = device("offline");
        offline["lighting_readback"]["available"] = json!(false);
        let rows = vec![device("ornata"), disabled, unknown, mouse, offline];
        let request = plan(&rows, None, 1).unwrap();
        assert_eq!(request.targets.len(), 1);
        assert_eq!(request.targets[0].id, "ornata");
        assert_eq!(request.targets[0].profile, "Work");
        for id in ["disabled", "unknown", "mouse", "offline", "missing"] {
            assert!(plan(&rows, Some(id), 1).is_err());
        }
        assert!(plan(&rows, None, 6).is_err());
    }
    #[test]
    fn current_effect_retains_opaque_settings_and_target_budget_is_bounded() {
        let mut row = device("ornata");
        let opaque = json!({"color1":"#123456","foreign":{"keep":true}});
        row["source_quick_effects"] = json!({"selectedEffectId":1,"selectedEffectSetting":opaque});
        assert_eq!(plan(&[row], None, 1).unwrap().targets[0].setting, opaque);
        assert!(plan(
            &(0..65).map(|i| device(&i.to_string())).collect::<Vec<_>>(),
            None,
            3
        )
        .is_err());
    }
    #[test]
    fn global_membership_uses_each_mode_and_bounds_aggregate_opaque_settings() {
        let mut normal = device("normal");
        normal["chroma_quick"] = json!({"mode":0,"source_effects":[1,2,3,7]});
        let mut driver = device("driver");
        driver["chroma_quick"] = json!({"mode":3,"source_effects":[1,2,3,4]});
        let rows = vec![normal, driver];
        assert!(plan(&rows, Some("normal"), 4).is_err());
        assert!(plan(&rows, Some("driver"), 7).is_err());
        assert_eq!(plan(&rows, None, 4).unwrap().targets[0].id, "driver");
        assert_eq!(plan(&rows, None, 7).unwrap().targets[0].id, "normal");
        let mut large = device("large");
        large["source_quick_effects"] = json!({"selectedEffectId":1,"selectedEffectSetting":{"color1":"#123456","opaque":"x".repeat(MAX_SELECTION_BYTES)}});
        assert!(plan(&[large], None, 1).is_err());
    }
}
