//! Real window routing and asynchronous file saves, with simulated hardware only.
use super::*;

#[test]
#[ignore = "opens a bounded native Studio window; run alone"]
fn native_studio_edits_save_failure_and_exit_recovery() {
    let directory = std::env::temp_dir().join(format!(
        "native-studio-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("studio")),
    ]);
    let sim = Sim::wireless_mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    store.for_device(&engine.devices()[0]);
    store.save().unwrap();
    let original_store = std::fs::read(directory.join("profiles.json")).unwrap();
    let profile = r#"{"id":"p0","title":"Desk","devices":[{"id":"keyboard","productId":605,"editionId":0,"layout":6,"x":4950,"y":4980,"width":960,"height":360,"orientation":0,"ledRef":[{"row":0,"col":1},{"row":0,"col":4}]}],"unknown":9007199254740993123456789,"layers":[{"id":"g","type":"group","title":"Group","visible":true,"opened":true,"layers":[{"id":"a","groupId":"g","type":"layer","title":"STATIC","name":"static","visible":false,"regions":[],"future":1.2300e-7}]}]}"#;
    let other = r#"{"id":"p1","title":"Other","devices":[],"layers":[{"id":"b","type":"layer","title":"SECOND","name":"static","visible":true,"regions":[]}]}"#;
    let source = directory.join("input.ChromaEffects");
    let output = directory.join("saved.ChromaEffects");
    std::fs::write(
        &source,
        format!("{{\"version\":1,\"profiles\":[{profile},{other}]}}"),
    )
    .unwrap();
    let controller_sim = sim.clone();
    let folder = directory.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for(
                    || {
                        std::fs::read(&frame)
                            .ok()
                            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                            .is_some_and(|v| v["presented"] == true)
                    },
                    "Studio presented",
                )?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut value = Value::Null;
                    SendMessageW(hwnd, STUDIO_TEST, 2, &mut value as *mut Value as isize);
                    value
                };
                let file = |kind, path: &Path| {
                    let value = json!(path.to_string_lossy());
                    SendMessageW(hwnd, STUDIO_TEST, kind, &value as *const Value as isize);
                };
                wait_for(|| query()["pending"] == 0, "initial device reads")?;
                controller_sim.state().log.clear();
                file(1, &source);
                wait_for(
                    || query()["profiles"] == 2 && query()["busy"] == false,
                    "composition load",
                )?;
                redraw(hwnd);
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let right = f64::from(client.right) / scale - 250.0;
                let click = |x, y| lighting_click(hwnd, x, y, scale);
                let check = |ok: bool, message: &str| {
                    if ok {
                        Ok(())
                    } else {
                        Err(format!("{message}: {}", query()))
                    }
                };
                check(
                    query()["canvas_notes"].as_array().unwrap().is_empty(),
                    "source geometry unavailable",
                )?;
                let width = f64::from(client.right) / scale;
                let height = f64::from(client.bottom) / scale;
                let fit = ((width - 540.0) / 960.0)
                    .min((height - 250.0) / 360.0)
                    .clamp(0.05, 4.0);
                // Independent coordinates from the pinned UK SVG: Esc center67,76;
                // F1 center145,76. The imported ledRef admits exactly those two.
                let screen = |x: f64, y: f64| {
                    position(
                        width / 2.0 + (x - 480.0) * fit,
                        (168.0 + height) / 2.0 + (y - 180.0) * fit,
                        scale,
                    )
                };
                let esc = screen(67.0, 76.0);
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, esc);
                SendMessageW(hwnd, WM_LBUTTONUP, 0, esc);
                check(
                    query()["selected_leds"] == 1 && query()["dirty"] == false,
                    "LED hit selection changed document or missed",
                )?;
                let from = screen(45.0, 54.0);
                let to = screen(165.0, 98.0);
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, from);
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, to);
                SendMessageW(hwnd, WM_CANCELMODE, 0, 0);
                check(
                    query()["selected_leds"] == 1,
                    "canceled marquee lost prior selection",
                )?;
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, from);
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, to);
                SendMessageW(hwnd, WM_LBUTTONUP, 0, to);
                check(
                    query()["selected_leds"] == 2 && query()["dirty"] == false,
                    "marquee did not select admitted LED paths",
                )?;
                click(right - 40.0, 148.0);
                check(
                    query()["selected_leds"] == 2 && query()["dirty"] == false,
                    "zoom changed selection or draft",
                )?;
                click(right - 100.0, 148.0);
                redraw(hwnd);
                click(215.0, 188.0);
                check(
                    query()["rows"][0]["visible"] == false
                        && query()["rows"][1]["visible"] == false,
                    "group visibility did not cascade",
                )?;
                click(440.0, 100.0);
                check(
                    query()["dirty"] == false && query()["rows"][0]["visible"] == true,
                    "undo did not restore clean baseline",
                )?;
                click(515.0, 100.0);
                check(query()["dirty"] == true, "redo did not restore edit")?;
                click(440.0, 100.0);
                click(60.0, 228.0);
                check(query()["selected"] == "a", "child selection failed")?;
                let color_edit = || GetDlgItem(hwnd, i32::from(studio_view::COLOR));
                if color_edit().is_null() {
                    return Err("Static RGB field missing".into());
                }
                SendMessageW(
                    color_edit(),
                    WM_SETTEXT,
                    0,
                    wide("GG0000").as_ptr() as isize,
                );
                click(right + 100.0, 343.0);
                check(
                    query()["failed"] == true && query()["dirty"] == false,
                    "invalid RGB mutated composition",
                )?;
                SendMessageW(
                    color_edit(),
                    WM_SETTEXT,
                    0,
                    wide("FF0000").as_ptr() as isize,
                );
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                click(right + 100.0, 343.0);
                check(
                    query()["dirty"] == false,
                    "read-only Static edit changed composition",
                )?;
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                click(right + 100.0, 343.0);
                check(
                    query()["dirty"] == true && query()["color"] == "FF0000",
                    "native RGB did not update selected memberships",
                )?;
                click(440.0, 100.0);
                check(
                    query()["dirty"] == false,
                    "region-only undo did not return clean",
                )?;
                click(515.0, 100.0);
                check(
                    query()["dirty"] == true && query()["color"] == "FF0000",
                    "region-only redo failed",
                )?;
                click(right + 100.0, 381.0);
                check(
                    query()["dirty"] == false,
                    "erase did not return to unassigned baseline",
                )?;
                click(440.0, 100.0);
                check(
                    query()["dirty"] == true && query()["color"] == "FF0000",
                    "erase undo failed",
                )?;
                let rename = |name: &str| -> Result<(), String> {
                    click(right + 60.0, 193.0);
                    let edit = GetDlgItem(hwnd, i32::from(studio_view::NAME));
                    if edit.is_null() {
                        return Err("Rename edit missing".into());
                    }
                    SendMessageW(edit, WM_SETTEXT, 0, wide(name).as_ptr() as isize);
                    click(right + 60.0, 193.0);
                    check(query()["renaming"] == false, "rename did not commit")
                };
                rename("Desk lighting")?;
                check(
                    query()["rows"][1]["title"] == "Desk lighting",
                    "rename lost native edit text",
                )?;
                file(3, &output);
                wait_for(|| query()["busy"] == false, "first save")?;
                check(
                    query()["dirty"] == false && query()["failed"] == false,
                    "save not acknowledged",
                )?;
                let bytes = std::fs::read(&output).map_err(|e| e.to_string())?;
                let capture = json!(folder.join("studio-frame.png").to_string_lossy());
                check(
                    SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize) == 1,
                    "native frame capture failed",
                )?;
                let archive =
                    secbird_core::studio::Archive::parse(&bytes).map_err(|e| e.to_string())?;
                let saved_draft =
                    secbird_core::studio::document::Draft::open(archive.profile_json(0).unwrap())
                        .map_err(|e| e.to_string())?;
                let selected = [1, 4].map(|col| secbird_core::studio::document::LedSelection {
                    device: "keyboard".into(),
                    row: 0,
                    col,
                });
                check(
                    saved_draft
                        .static_selection_color("a", &selected)
                        .map_err(|e| e.to_string())?
                        == secbird_core::studio::document::SelectedColor::Uniform(Some(0xff0000)),
                    "saved RGB assignments lost",
                )?;
                check(
                    archive
                        .profile_json(0)
                        .unwrap()
                        .contains("9007199254740993123456789")
                        && archive.profile_json(0).unwrap().contains("1.2300e-7")
                        && archive.profile_json(1) == Some(other),
                    "unknown data or other composition changed",
                )?;
                click(70.0, 100.0);
                click(70.0, 158.0);
                check(query()["index"] == 1, "composition menu did not switch")?;
                rename("Second edited")?;
                file(3, &output);
                wait_for(|| query()["busy"] == false, "second save")?;
                let saved = std::fs::read(&output).map_err(|e| e.to_string())?;
                let archive =
                    secbird_core::studio::Archive::parse(&saved).map_err(|e| e.to_string())?;
                check(
                    archive.profile_json(0).unwrap().contains("Desk lighting")
                        && archive.profile_json(1).unwrap().contains("Second edited"),
                    "later save lost earlier composition edits",
                )?;
                rename("Unsaved work")?;
                file(3, &folder.join("missing").join("cannot-save.ChromaEffects"));
                wait_for(|| query()["busy"] == false, "failed save")?;
                check(
                    query()["dirty"] == true && query()["failed"] == true,
                    "failed save discarded draft",
                )?;
                check(
                    std::fs::read(&output).unwrap() == saved,
                    "failed save damaged prior file",
                )?;
                // The existing Home tab may be used while an unsaved draft remains open.
                click(65.0, 20.0);
                check(query()["open"] == false, "Home did not hide Studio")?;
                SendMessageW(hwnd, STUDIO_TEST, 0, 0);
                check(
                    query()["dirty"] == true && query()["rows"][0]["title"] == "Unsaved work",
                    "navigation lost Studio draft",
                )?;
                // An invalid in-progress rename must be recoverable even if hidden.
                click(right + 60.0, 193.0);
                let edit = GetDlgItem(hwnd, i32::from(studio_view::NAME));
                SendMessageW(edit, WM_SETTEXT, 0, wide("").as_ptr() as isize);
                click(65.0, 20.0);
                SendMessageW(hwnd, WM_CLOSE, 0, 0);
                check(
                    IsWindow(hwnd) != 0
                        && query()["open"] == true
                        && query()["renaming"] == true
                        && query()["failed"] == true,
                    "invalid hidden rename was not revealed on close",
                )?;
                click(right + 60.0, 233.0);
                check(
                    query()["renaming"] == false && query()["rows"][0]["title"] == "Unsaved work",
                    "cancel rename lost previously committed draft",
                )?;
                // Exercise the real tray-exit guard against our own message-only sentinel.
                let tray = CreateWindowExW(
                    0,
                    wide("STATIC").as_ptr(),
                    wide("Studio test tray").as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    null_mut(),
                    GetModuleHandleW(null()),
                    null(),
                );
                if tray.is_null() {
                    return Err("Tray sentinel creation failed".into());
                }
                SendMessageW(hwnd, STUDIO_TEST, 4, IDCANCEL as isize);
                check(defer_process_exit(tray), "tray exit did not defer")?;
                wait_for(|| query()["dialog"] == false, "cancel close")?;
                // A round trip to the UI thread follows the posted close/prompt messages.
                std::thread::sleep(Duration::from_millis(100));
                check(
                    IsWindow(hwnd) != 0 && query()["dirty"] == true,
                    "cancel close lost window or draft",
                )?;
                let mut message: MSG = std::mem::zeroed();
                if PeekMessageW(&mut message, tray, WM_CLOSE, WM_CLOSE, PM_REMOVE) != 0 {
                    DestroyWindow(tray);
                    return Err("Canceled close acknowledged process exit".into());
                }
                SendMessageW(hwnd, STUDIO_TEST, 4, IDNO as isize);
                check(defer_process_exit(tray), "discard exit did not defer")?;
                wait_for(
                    || PeekMessageW(&mut message, tray, WM_CLOSE, WM_CLOSE, PM_REMOVE) != 0,
                    "exit acknowledgement",
                )?;
                let ack = message.wParam == 1 && WINDOW.load(Ordering::Acquire) == 0;
                DestroyWindow(tray);
                if !ack {
                    return Err("Exit acknowledged before editor destruction".into());
                }
                if std::fs::read(folder.join("profiles.json")).unwrap() != original_store {
                    return Err("Studio file work changed device profiles".into());
                }
                if controller_sim
                    .state()
                    .log
                    .iter()
                    .any(|command| command.id & 0x80 == 0)
                {
                    return Err("Studio file work wrote device commands".into());
                }
                Ok(
                    json!({"actual_hwnd":true,"simulated_only":true,"group_visibility_undo_redo":true,
                "native_rename":true,"two_composition_saves_preserved":true,"unknown_literals_preserved":true,
                "save_failure_keeps_draft_and_file":true,"navigation_keeps_draft":true,
                "invalid_hidden_rename_recovery":true,
                "source_led_hit_marquee_cancel_zoom":true,
                "static_region_native_rgb_erase_history_save":true,"invalid_and_readonly_rgb_inert":true,
                "tray_exit_cancel_and_discard":true,"device_profiles_unchanged":true,"no_device_sets":true}),
                )
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            if !hwnd.is_null() {
                SendMessageW(hwnd, STUDIO_TEST, 5, 0);
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
        result
    });
    run_window(
        Arc::new(Mutex::new(engine)),
        Some(Duration::from_secs(25)),
        None,
    )
    .unwrap();
    let evidence = controller.join().unwrap().unwrap();
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    eprintln!(
        "Studio native interaction evidence: {}",
        directory.display()
    );
}
