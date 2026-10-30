//! Local Settings. Source720's SYNAPSE/GENERAL sections; no vendor code executes.
use super::*;
pub(super) const ID: &str = "module:settings";
#[cfg(test)]
pub(super) const TEST: u32 = WM_APP + 184;
#[cfg(test)]
pub(super) fn diagnostics(state: &State) -> Value {
    json!({"active":active(state),"general":state.settings.general,"busy":state.settings.busy,"data":state.settings.data,"message":state.settings.message,"scroll":state.settings.scroll})
}
#[derive(Clone)]
pub enum Control {
    Open,
    Page(bool),
    Scroll(i32),
    Toggle(&'static str, bool),
    Away(&'static str),
    Battery(u8),
    Lighting(String, String, bool),
    Help,
    License,
    Backup,
    Restore,
}
#[derive(Default)]
pub(super) struct View {
    pub used: bool,
    general: bool,
    busy: bool,
    data: Option<Value>,
    scroll: i32,
    maximum: std::cell::Cell<i32>,
    message: String,
    failed: bool,
}
pub(super) fn active(state: &State) -> bool {
    state.selected.as_deref() == Some(ID)
}
pub(super) unsafe fn request(state: &mut State, body: Option<Value>) {
    if state.settings.busy {
        return;
    }
    if body.is_some()
        && (state.read_only_smoke
            || state.restricted_actions
            || state.state["measurement_read_only"] == true)
    {
        state.settings.message = "Read-only mode refuses settings changes".into();
        state.settings.failed = true;
        return;
    }
    let before = (state.pending, state.pending_jobs.len());
    state.settings.busy = true;
    state.queue(Job::Settings(body));
    if before == (state.pending, state.pending_jobs.len()) {
        state.settings.busy = false;
        state.settings.failed = true;
        state.settings.message =
            "Settings operation was not accepted by the worker; refresh before retrying".into();
    }
}
pub(super) unsafe fn receive(state: &mut State, result: Result<Value, String>) {
    state.settings.busy = false;
    match result {
        Ok(data) => {
            if data["backup_preview"] == true {
                let text=format!("Restore {} devices and {} profiles?\n\nThis replaces the whole local store, including active selections and opaque metadata. Current profiles with conflicting names are replaced. Nothing is merged or renamed. A recovery copy is retained beside the store. Cancel keeps the current store. No device commands are sent by this operation.",data["devices"],data["profiles"]);
                if MessageBoxW(state.hwnd,wide(&text).as_ptr(),wide("Preview whole-store restore").as_ptr(),MB_YESNO|MB_DEFBUTTON2)==IDYES {
                    let mut body=data;body["key"]=json!("store_restore");request(state,Some(body));
                }
                return;
            }
            if let Some(message)=data["backup_message"].as_str() {
                state.settings.message=message.into();state.settings.failed=false;
                InvalidateRect(state.hwnd,null(),0);return;
            }
            state.settings.data = Some(data);
            state.settings.failed = false;
            state.settings.message.clear();
        }
        Err(error) => {
            state.settings.failed = true;
            state.settings.message = error;
        }
    }
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn action(state: &mut State, control: Control) {
    if matches!(control, Control::Open) {
        state.settings.used = true;
        state.action(Action::Device(ID.into()));
        return;
    }
    if !active(state) {
        return;
    }
    match control {
        Control::Page(general) => {
            state.settings.general = general;
            state.settings.scroll = 0;
            request(state, None);
        }
        Control::Scroll(delta) => {
            state.settings.scroll =
                (state.settings.scroll + delta).clamp(0, state.settings.maximum.get());
        }
        Control::Help => {
            MessageBoxW(state.hwnd,wide("Auto Launch registers this copy at sign-in. Minimize to system tray affects sign-in launches; opening Synapse Lite normally shows its window. Closing the editor leaves the tray running. Exit in the tray stops it.\n\nProfile notifications, low battery warnings and automatic game profiles are independent. Away lighting applies at lock/screen-off and restores saved lighting on return.\n\nDevice Lighting: in Settings, uncheck a device to leave its RGB to another application. Sensor and button settings still work.\n\nProfile import/export is available from each device's profile menu. Additional languages and Windows Dynamic Lighting handoff are not yet available.").as_ptr(),wide("Local Help").as_ptr(),MB_OK);
        }
        Control::Backup | Control::Restore => {
            if state.read_only_smoke || state.restricted_actions || state.state["measurement_read_only"] == true { return; }
            let restore=matches!(control,Control::Restore);
            let choice=if restore { profile_files::choose_import(state.hwnd) } else { profile_files::choose_export(state.hwnd).map(|v|v.map(|(p,_)|p)) };
            match choice {
                Ok(Some(path))=>request(state,Some(json!({"key":if restore {"store_preview"} else {"store_backup"},"path":path}))),
                Ok(None)=>{},
                Err(e)=>{state.settings.message=e;state.settings.failed=true;}
            }
        }
        Control::License => {
            MessageBoxW(state.hwnd,wide(concat!("Synapse Lite is MIT OR Apache-2.0.\n\n",include_str!("../../../../LICENSE-MIT"),"\nApache-2.0 alternative and third-party licenses: see the distribution's LICENSE-APACHE and dependency notices.\nComplete offline dependency notices are not yet packaged.")).as_ptr(),wide("License and notices").as_ptr(),MB_OK);
        }
        change => {
            if state.read_only_smoke
                || state.restricted_actions
                || state.state["measurement_read_only"] == true
            {
                state.settings.message = "Read-only mode refuses settings changes".into();
                state.settings.failed = true;
                return;
            }
            let body = match change {
                Control::Toggle(key, on) => json!({"key":key,"on":on}),
                Control::Away(data) => json!({"key":"away_lights","data":data}),
                Control::Lighting(id, profile, enabled) => {
                    json!({"key":"lighting_control","id":id,"expected_profile":profile,"enabled":enabled})
                }
                Control::Battery(value) => json!({"key":"battery_warning_at","value":value}),
                _ => return,
            };
            request(state, Some(body));
        }
    }
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn wheel(state: &mut State, delta: i16) {
    state.settings.scroll = (state.settings.scroll + if delta < 0 { 70 } else { -70 })
        .clamp(0, state.settings.maximum.get());
    InvalidateRect(state.hwnd, null(), 0);
}
unsafe fn card(p: &Painter<'_>, x: i32, y: i32, w: i32, h: i32, title: &str) {
    p.fill(x, y, w, h, PANEL);
    p.text(
        title,
        x + 30,
        y + 22,
        w - 60,
        28,
        p.fonts.widget_heading,
        GREEN,
        DT_LEFT,
    );
}
unsafe fn note(p: &Painter<'_>, x: i32, y: i32, w: i32, text: &str) {
    let previous = SelectObject(p.dc, p.fonts.small);
    SetBkMode(p.dc, TRANSPARENT as i32);
    SetTextColor(p.dc, rgb(MUTED));
    let mut rect = p.rect(x, y, w, 80);
    DrawTextW(
        p.dc,
        wide(text).as_ptr(),
        -1,
        &mut rect,
        DT_LEFT | DT_WORDBREAK | DT_NOPREFIX,
    );
    SelectObject(p.dc, previous);
}
unsafe fn check(
    p: &mut Painter<'_>,
    geometry: (i32, i32, i32),
    label: &str,
    on: bool,
    enabled: bool,
    control: Control,
) {
    let (x, y, w) = geometry;
    p.fill(x, y + 5, 18, 18, if enabled && on { GREEN } else { BORDER });
    p.fill(
        x + 1,
        y + 6,
        16,
        16,
        if on && enabled { GREEN } else { PANEL },
    );
    if on {
        p.text(
            "✓",
            x,
            y + 1,
            18,
            25,
            p.fonts.normal,
            if enabled { PANEL } else { MUTED },
            DT_CENTER,
        );
    }
    p.text(
        label,
        x + 29,
        y,
        w - 29,
        28,
        p.fonts.normal,
        if enabled { TEXT } else { MUTED },
        DT_LEFT,
    );
    if enabled {
        p.hits.push(Hit {
            rect: p.rect(x, y, w, 29),
            action: Action::Settings(control),
        });
    }
}
pub(super) unsafe fn paint(state: &State, p: &mut Painter<'_>, width: i32, height: i32) {
    let v = &state.settings;
    for (general, label, x) in [
        (false, "SYNAPSE", width / 2 - 90),
        (true, "GENERAL", width / 2 + 10),
    ] {
        p.button(
            label,
            x,
            92,
            80,
            Action::Settings(Control::Page(general)),
            v.general == general,
        );
    }
    let Some(a) = &v.data else {
        p.text(
            if v.failed {
                &v.message
            } else {
                "Reading settings…"
            },
            40,
            160,
            width - 80,
            60,
            p.fonts.normal,
            TEXT,
            DT_LEFT,
        );
        return;
    };
    let read_only =
        state.read_only_smoke || state.restricted_actions || a["measurement_read_only"] == true;
    let enabled = !read_only && !v.busy && a["settings_error"].is_null();
    let devices = a["lighting_devices"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let lighting_height = 250 + devices.len() as i32 * 34;
    let columns = width >= 1100;
    let bottom = if v.general {
        if columns {
            622
        } else {
            964
        }
    } else if columns {
        150 + 529.max(lighting_height + 339)
    } else {
        1038 + lighting_height
    };
    v.maximum.set((bottom - (height - 45)).max(0));
    let scroll = v.scroll.min(v.maximum.get());
    let top = 150 - scroll;
    let w = if columns {
        ((width - 80) / 2).min(600)
    } else {
        (width - 60).min(600)
    };
    let x = if columns {
        (width - w * 2 - 20) / 2
    } else {
        (width - w) / 2
    };
    let right = if columns { x + w + 20 } else { x };
    let second = if columns { top } else { top + 549 };
    let clip = SaveDC(p.dc);
    let r = p.rect(0, 137, width, height - 182);
    IntersectClipRect(p.dc, r.left, r.top, r.right, r.bottom);
    if !v.general {
        card(p, x, top, w, 170, "AUTO-LAUNCH");
        let auto = a["autostart"].as_bool();
        check(
            p,
            (x + 30, top + 64, w - 60),
            "Start Synapse Lite when Windows starts",
            auto == Some(true),
            enabled && auto.is_some(),
            Control::Toggle("autostart", auto != Some(true)),
        );
        check(
            p,
            (x + 60, top + 102, w - 90),
            "Minimize to system tray on startup",
            a["start_minimized"] == true,
            enabled && auto == Some(true),
            Control::Toggle("start_minimized", a["start_minimized"] != true),
        );
        note(
            p,
            x + 30,
            top + 139,
            w - 60,
            "Registration is local to this copy; Windows Startup settings can also disable it.",
        );
        card(p, x, top + 190, w, 184, "NOTIFICATIONS");
        check(
            p,
            (x + 30, top + 248, w - 60),
            "Display profile and DPI notifications",
            a["popups"] == true,
            enabled,
            Control::Toggle("popups", a["popups"] != true),
        );
        check(
            p,
            (x + 30, top + 286, w - 60),
            "Warn when a device battery is low",
            a["battery_warning"] == true,
            enabled,
            Control::Toggle("battery_warning", a["battery_warning"] != true),
        );
        let battery = a["battery_warning_at"].as_u64().unwrap_or(15) as u8;
        p.text(
            &format!("Low battery at {battery}%"),
            x + 30,
            top + 329,
            w - 150,
            28,
            p.fonts.normal,
            TEXT,
            DT_LEFT,
        );
        if enabled && a["battery_warning"] == true {
            p.button(
                "−",
                x + w - 112,
                top + 327,
                30,
                Action::Settings(Control::Battery(battery.saturating_sub(1).max(1))),
                false,
            );
            p.button(
                "+",
                x + w - 72,
                top + 327,
                30,
                Action::Settings(Control::Battery((battery + 1).min(95))),
                false,
            );
        }
        card(p, x, top + 394, w, 135, "AUTOMATIC PROFILES");
        check(
            p,
            (x + 30, top + 452, w - 60),
            "Switch profiles for linked foreground programs",
            a["auto_switch"] == true,
            enabled,
            Control::Toggle("auto_switch", a["auto_switch"] != true),
        );
        card(p, right, second, w, lighting_height, "DEVICE LIGHTING");
        note(p,right+30,second+63,w-60,"Allow Synapse Lite to control lighting for checked devices. Uncheck a device to leave its RGB to other applications.");
        for (i, device) in devices.iter().enumerate() {
            let name = device["name"].as_str().unwrap_or("Device");
            check(
                p,
                (right + 30, second + 132 + i as i32 * 34, w - 60),
                name,
                device["lighting_control"] == true,
                enabled,
                Control::Lighting(
                    device["id"].as_str().unwrap_or_default().into(),
                    device["active"].as_str().unwrap_or_default().into(),
                    device["lighting_control"] != true,
                ),
            );
        }
        if devices.is_empty() {
            note(
                p,
                right + 30,
                second + 132,
                w - 60,
                "No connected devices. Open a device to inspect its lighting capability.",
            );
        }
        note(p,right+30,second+176+devices.len() as i32*34,w-60,"Windows Dynamic Lighting handoff is not available in this build. These controls apply only to Synapse Lite's lighting policy.");
        card(
            p,
            right,
            second + lighting_height + 20,
            w,
            210,
            "LIGHTING WHILE AWAY",
        );
        note(p,right+30,second+lighting_height+79,w-60,"On lock or screen-off; restore saved lighting on return. Changes take effect at the next lock/display event.");
        for (i, (key, label)) in [("", "Keep"), ("dim", "Dim to 25%"), ("off", "Off")]
            .iter()
            .enumerate()
        {
            let bx = right + 30 + i as i32 * ((w - 60) / 3);
            if enabled {
                p.button(
                    label,
                    bx,
                    second + lighting_height + 145,
                    (w - 80) / 3,
                    Action::Settings(Control::Away(key)),
                    a["away_lights"] == *key,
                );
            } else {
                p.text(
                    label,
                    bx,
                    second + lighting_height + 145,
                    (w - 80) / 3,
                    28,
                    p.fonts.small,
                    MUTED,
                    DT_LEFT,
                );
            }
        }
        note(
            p,
            right + 30,
            second + lighting_height + 190,
            w - 60,
            "DPI and button mappings keep working when lighting is left to another app.",
        );
        card(
            p,
            right,
            second + lighting_height + 250,
            w,
            89,
            "LOCAL HELP",
        );
        p.button(
            "Help",
            right + w - 110,
            second + lighting_height + 277,
            80,
            Action::Settings(Control::Help),
            false,
        );
    } else {
        card(p, x, top, w, 172, "LANGUAGE");
        p.text(
            "English",
            x + 30,
            top + 63,
            w - 60,
            28,
            p.fonts.normal,
            TEXT,
            DT_LEFT,
        );
        note(p,x+30,top+104,w-60,"English is available in this build. Additional languages and automatic restart after a language change are not yet supported.");
        card(p, x, top + 192, w, 130, "LOCAL HELP");
        note(
            p,
            x + 30,
            top + 251,
            w - 160,
            "Startup, tray, profiles and lighting",
        );
        p.button(
            "Help",
            x + w - 110,
            top + 251,
            80,
            Action::Settings(Control::Help),
            false,
        );
        card(p,x,top+342,w,130,"LOCAL PROFILE STORE");
        if enabled {
            p.button("Back up",x+30,top+406,110,Action::Settings(Control::Backup),false);
            p.button("Restore…",x+155,top+406,110,Action::Settings(Control::Restore),false);
        }
        let about = if columns { top } else { top + 492 };
        card(p, right, about, w, 322, "ABOUT");
        p.text(
            "SYNAPSE LITE",
            right + 30,
            about + 70,
            w - 60,
            42,
            p.fonts.heading,
            TEXT,
            DT_CENTER,
        );
        p.text(
            &format!("Version {}", env!("CARGO_PKG_VERSION")),
            right + 30,
            about + 125,
            w - 60,
            28,
            p.fonts.normal,
            TEXT,
            DT_CENTER,
        );
        note(p,right+30,about+174,w-60,"Based on Secretarybird. MIT OR Apache-2.0. Device names and trademarks belong to their respective owners.");
        p.button(
            "License and notices",
            right + 30,
            about + 250,
            w - 60,
            Action::Settings(Control::License),
            false,
        );
    }
    RestoreDC(p.dc, clip);
    // Clip hit targets as well as pixels, including scrolled-off controls.
    for hit in &mut p.hits {
        if matches!(
            hit.action,
            Action::Settings(
                Control::Toggle(..)
                    | Control::Away(..)
                    | Control::Battery(..)
                    | Control::Lighting(..)
                    | Control::Help
                    | Control::License
                    | Control::Backup
                    | Control::Restore
            )
        ) {
            hit.rect.top = hit.rect.top.max(r.top);
            hit.rect.bottom = hit.rect.bottom.min(r.bottom);
        }
    }
    p.hits.retain(|hit| hit.rect.bottom > hit.rect.top);
    if v.maximum.get() > 0 {
        let track = (height - 182).max(1);
        let thumb = (track * track / (track + v.maximum.get())).max(25);
        let offset = (track - thumb) * scroll / v.maximum.get();
        p.fill(width - 8, 137, 5, track, PANEL);
        p.fill(width - 8, 137 + offset, 5, thumb, BORDER);
        for (y, h, delta) in [
            (137, offset, -track),
            (137 + offset + thumb, track - offset - thumb, track),
        ] {
            if h > 0 {
                p.hits.push(Hit {
                    rect: p.rect(width - 16, y, 16, h),
                    action: Action::Settings(Control::Scroll(delta)),
                });
            }
        }
    }
    let message = if !v.message.is_empty() {
        v.message.as_str()
    } else if let Some(error) = a["settings_error"].as_str() {
        error
    } else if let Some(error) = a["autostart_error"].as_str() {
        error
    } else if read_only {
        "Read-only mode: settings cannot be changed"
    } else if v.busy {
        "Applying and reading back…"
    } else {
        "Changes are shared with the tray. Refresh reads changes from other editors."
    };
    p.fill(0, height - 44, width, 44, BG);
    p.text(
        message,
        30,
        height - 43,
        width - 60,
        30,
        p.fonts.small,
        if v.failed || a["settings_error"].is_string() || a["autostart_error"].is_string() {
            0xfd4949
        } else {
            MUTED
        },
        DT_LEFT,
    );
}

/// Runs under the engine worker lock; no apply/refresh or device transport calls.
pub(super) fn store_work(engine: &Engine, body: &Value) -> Result<Value,String> {
    use secbird_core::profile::backup;
    let run=|| -> Result<Value,String> {
        if engine.is_measurement_read_only() {return Err("Read-only mode refuses store recovery".into());}
        let path=engine.profile_store_path();
        let current=backup::read(&path).map_err(|e|e.to_string())?;
        let input=body["path"].as_str().map(PathBuf::from);
        match body["key"].as_str().unwrap_or("") {
            "store_backup"=>{
                backup::create(input.as_deref().ok_or("Missing destination")?,&current).map_err(|e|e.to_string())?;
                Ok(json!({"backup_message":"Backup saved. Existing files are never overwritten."}))
            },
            "store_preview"=>{
                let incoming=backup::read(input.as_deref().ok_or("Missing backup")?).map_err(|e|e.to_string())?;
                let store=backup::validate(&incoming).map_err(|e|e.to_string())?;
                Ok(json!({"backup_preview":true,"devices":store.devices.len(),"profiles":store.devices.values().map(|d|d.profiles.len()).sum::<usize>(),"expected":String::from_utf8(current).map_err(|e|e.to_string())?,"incoming":String::from_utf8(incoming).map_err(|e|e.to_string())?}))
            },
            "store_restore"=>{
                let expected=body["expected"].as_str().ok_or("Missing preview")?.as_bytes();
                let incoming=body["incoming"].as_str().ok_or("Missing candidate")?.as_bytes();
                let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();
                let recovery=path.with_extension(format!("recovery-{stamp}.json"));
                backup::restore(&path,expected,incoming,&recovery).map_err(|e|e.to_string())?;
                Ok(json!({"backup_message":format!("Store restored. Recovery: {}. Reopen device pages to see restored selections.",recovery.display())}))
            },
            _=>Err("Unknown backup operation".into())
        }
    };run()
}

#[cfg(test)] mod backup_tests {
 use super::*;
 #[test] fn worker_preview_restore_and_backup_preserve_opaque_data_without_device_commands() {
 let dir=std::env::temp_dir().join(format!("backup-worker-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();
 let path=dir.join("profiles.json");let input=dir.join("incoming.json");let output=dir.join("backup.json");
 let old=br#"{"devices":{},"future":{"id":"old"}}"#;let new=br#"{"devices":{},"future":{"id":"new","guid":"opaque"}}"#;
 std::fs::write(&path,old).unwrap();std::fs::write(&input,new).unwrap();
 let sim=secbird_core::sim::Sim::keyboard();
 let engine=Engine::with_simulated_keyboard(&sim,path.clone()).unwrap();sim.state().log.clear();
 store_work(&engine,&json!({"key":"store_backup","path":output})).unwrap();assert_eq!(std::fs::read(&output).unwrap(),old);
 let mut preview=store_work(&engine,&json!({"key":"store_preview","path":input})).unwrap();assert_eq!(std::fs::read(&path).unwrap(),old);
 preview["key"]=json!("store_restore");store_work(&engine,&preview).unwrap();assert_eq!(std::fs::read(&path).unwrap(),new);
 assert!(store_work(&engine,&preview).is_err());assert!(sim.state().log.is_empty());
 for entry in std::fs::read_dir(&dir).unwrap() {std::fs::remove_file(entry.unwrap().path()).unwrap();}std::fs::remove_dir(dir).unwrap();
 }
}
