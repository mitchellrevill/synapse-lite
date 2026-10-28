//! Routed196 Smart Tracking form, with device-wide verified worker actions.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum Control {
    Distance(u8),
    Retry,
    DismissWelcome,
}

pub(super) struct View {
    pub(super) owner: Option<(String, String)>,
    pub(super) snapshot: Option<Value>,
    pub(super) loading: bool,
    welcome: bool,
    prefs: Value,
    prefs_valid: bool,
    error: Option<String>,
}
impl Default for View {
    fn default() -> Self {
        use std::io::Read;
        let read = || -> Result<Value, String> {
            let file = match std::fs::File::open(secbird_core::config_dir().join("sensor-ui.json"))
            {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(json!({"schema":1}))
                }
                Err(e) => return Err(e.to_string()),
            };
            let mut bytes = Vec::new();
            file.take(16385)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 16384 {
                return Err("Sensor UI preferences are too large".into());
            }
            let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if !value.is_object() || value["schema"] != 1 {
                return Err("Unsupported sensor UI preference format".into());
            }
            Ok(value)
        };
        let loaded = read();
        let valid = loaded.is_ok();
        let prefs = loaded.unwrap_or_else(|_| json!({"schema":1}));
        Self {
            owner: None,
            snapshot: None,
            loading: false,
            welcome: prefs["calibration_welcome_dismissed"] != true,
            prefs,
            prefs_valid: valid,
            error: None,
        }
    }
}

fn owner(state: &State) -> Option<(String, String)> {
    let device = state.device()?;
    if !matches!(state.tab, Tab::Calibration | Tab::Advanced)
        || !matches!(device["pid"].as_str(), Some("00C4" | "00C5"))
    {
        return None;
    }
    Some((
        device["id"].as_str()?.into(),
        device["active"].as_str()?.into(),
    ))
}

pub(super) unsafe fn request(state: &mut State, force: bool) {
    let Some(owner) = owner(state) else { return };
    if state.calibration.loading && state.calibration.owner.as_ref() == Some(&owner) {
        return;
    }
    if !force && state.calibration.owner.as_ref() == Some(&owner) {
        return;
    }
    let before = (state.pending, state.pending_jobs.len());
    state.submit(Job::SensorRead(owner.0.clone(), owner.1.clone()), false);
    if before == (state.pending, state.pending_jobs.len()) {
        return;
    }
    state.calibration.owner = Some(owner);
    state.calibration.loading = true;
    state.calibration.error = None;
    if force {
        state.calibration.snapshot = None;
    }
}

pub(super) unsafe fn receive(
    state: &mut State,
    received: (String, String),
    result: Result<Value, String>,
) {
    if state.calibration.owner.as_ref() != Some(&received) {
        return;
    }
    state.calibration.loading = false;
    if owner(state).as_ref() != Some(&received) {
        return;
    }
    match result {
        Ok(value) => {
            state.calibration.snapshot = Some(value);
            state.calibration.error = None;
            advanced_view::sync(state);
        }
        Err(error) => {
            state.calibration.snapshot = None;
            state.calibration.error = Some(error);
        }
    }
    InvalidateRect(state.hwnd, null(), 0);
}

pub(super) unsafe fn action(state: &mut State, control: Control) {
    match control {
        Control::Retry => request(state, true),
        Control::Distance(mm) => {
            let Some(current) = owner(state) else { return };
            if state.read_only_smoke
                || state.restricted_actions
                || state.pending > 0
                || state.calibration.loading
                || state.calibration.owner.as_ref() != Some(&current)
                || state
                    .calibration
                    .snapshot
                    .as_ref()
                    .is_none_or(|s| s["calibration"]["available"] != true)
            {
                return;
            }
            state.queue(Job::Action(
                "/api/calibration-tracking",
                json!({"id":current.0,"expected_device":current.0,"tracking_distance_mm":mm}),
            ));
            request(state, true);
        }
        Control::DismissWelcome => {
            state.calibration.welcome = false;
            if !state.read_only_smoke && !state.restricted_actions {
                if !state.calibration.prefs_valid {
                    state.error("Sensor UI preferences could not be saved; original file retained");
                } else {
                    state.calibration.prefs["calibration_welcome_dismissed"] = json!(true);
                    let result = (|| -> Result<(), String> {
                        let folder = secbird_core::config_dir();
                        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                        let bytes = serde_json::to_vec_pretty(&state.calibration.prefs)
                            .map_err(|e| e.to_string())?;
                        profile_files::write(&folder.join("sensor-ui.json"), &bytes)
                    })();
                    if let Err(error) = result {
                        state.error(&format!(
                            "Calibration welcome preference was not saved: {error}"
                        ));
                    }
                }
            }
        }
    }
    InvalidateRect(state.hwnd, null(), 0);
}

unsafe fn paragraph(p: &Painter<'_>, text: &str, x: i32, y: i32, width: i32, height: i32) {
    let previous = SelectObject(p.dc, p.fonts.normal);
    SetBkMode(p.dc, TRANSPARENT as i32);
    SetTextColor(p.dc, rgb(TEXT));
    let mut r = p.rect(x, y, width, height);
    DrawTextW(
        p.dc,
        wide(text).as_ptr(),
        -1,
        &mut r,
        DT_LEFT | DT_WORDBREAK | DT_NOPREFIX,
    );
    SelectObject(p.dc, previous);
}

pub(super) unsafe fn paint(state: &State, p: &mut Painter<'_>) {
    if owner(state).is_none() {
        return;
    }
    let width = (state.width as f32 / state.scale).round() as i32;
    let x = (width - 600) / 2;
    let y = if state.calibration.welcome { 276 } else { 148 };
    if state.calibration.welcome {
        let banner_x = (width - 744) / 2;
        performance_view::rounded(
            p,
            performance_layout::Rect {
                x: banner_x as f64,
                y: 148.0,
                width: 744.0,
                height: 98.0,
            },
            0x2d2d2d,
            BORDER,
            5.0,
        );
        p.text(
            "MOUSE MAT SURFACE CALIBRATION",
            banner_x + 60,
            165,
            624,
            38,
            p.fonts.calibration_welcome,
            TEXT,
            DT_CENTER,
        );
        p.text(
            "Your Razer gaming mouse delivers immensely accurate tracking straight out of the box.",
            banner_x + 50,
            207,
            644,
            20,
            p.fonts.normal,
            TEXT,
            DT_CENTER,
        );
        p.image(state.icons.get("close"), banner_x + 714, 160, 13, 13);
        p.hits.push(Hit {
            rect: p.rect(banner_x + 704, 150, 36, 32),
            action: Action::Calibration(Control::DismissWelcome),
        });
    }
    performance_view::rounded(
        p,
        performance_layout::Rect {
            x: x as f64,
            y: y as f64,
            width: 600.0,
            height: 213.0,
        },
        PANEL,
        PANEL,
        5.0,
    );
    p.text(
        "SMART TRACKING",
        x + 40,
        y + 30,
        520,
        22,
        p.fonts.widget_heading,
        GREEN,
        DT_LEFT,
    );
    paragraph(p,"Set a single cut off point to your preferred distance in millimeters (mm) regardless of the surface it is placed on.",x+40,y+72,520,34);
    p.text(
        "TRACKING DISTANCE",
        x + 40,
        y + 126,
        320,
        17,
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    let data = state
        .calibration
        .snapshot
        .as_ref()
        .map(|s| &s["calibration"]);
    let selected = data.and_then(|d| d["tracking_distance_mm"].as_u64());
    let available = data.is_some_and(|d| d["available"] == true);
    let enabled = available
        && !state.calibration.loading
        && state.pending == 0
        && !state.read_only_smoke
        && !state.restricted_actions;
    for (offset, mm, label, hit_width) in [(40, 1, "Low", 57), (136, 2, "High", 60)] {
        let r = p.rect(x + offset, y + 163, 20, 20);
        let brush = CreateSolidBrush(rgb(PANEL));
        let pen = CreatePen(
            PS_SOLID,
            p.px(1).max(1),
            rgb(if enabled { 0x737373 } else { 0x454545 }),
        );
        let old_b = SelectObject(p.dc, brush);
        let old_p = SelectObject(p.dc, pen);
        Ellipse(p.dc, r.left, r.top, r.right, r.bottom);
        SelectObject(p.dc, old_b);
        SelectObject(p.dc, old_p);
        DeleteObject(brush);
        DeleteObject(pen);
        if selected == Some(mm as u64) {
            let dot = p.rect(x + offset + 5, y + 168, 10, 10);
            let brush = CreateSolidBrush(rgb(if enabled { GREEN } else { 0x256e1a }));
            let old_b = SelectObject(p.dc, brush);
            let old_p = SelectObject(p.dc, GetStockObject(NULL_PEN));
            Ellipse(p.dc, dot.left, dot.top, dot.right, dot.bottom);
            SelectObject(p.dc, old_b);
            SelectObject(p.dc, old_p);
            DeleteObject(brush);
        }
        p.text(
            label,
            x + offset + 30,
            y + 163,
            70,
            20,
            p.fonts.normal,
            if enabled { TEXT } else { MUTED },
            DT_LEFT,
        );
        if enabled {
            p.hits.push(Hit {
                rect: p.rect(x + offset, y + 163, hit_width, 20),
                action: Action::Calibration(Control::Distance(mm)),
            });
        }
    }
    if state.calibration.loading {
        p.text(
            "Reading mouse settings…",
            x + 40,
            y + 225,
            520,
            22,
            p.fonts.normal,
            MUTED,
            DT_LEFT,
        );
    } else if !available {
        p.text(
            "Unable to read tracking distance from the mouse.",
            x + 40,
            y + 225,
            430,
            22,
            p.fonts.normal,
            MUTED,
            DT_LEFT,
        );
        if state.pending == 0 {
            p.button(
                "RETRY",
                x + 460,
                y + 225,
                100,
                Action::Calibration(Control::Retry),
                false,
            );
        }
    }
}

#[cfg(test)]
pub(super) fn diagnostics(state: &State) -> Value {
    json!({"loading":state.calibration.loading,"snapshot":state.calibration.snapshot,"welcome":state.calibration.welcome,"error":state.calibration.error})
}
