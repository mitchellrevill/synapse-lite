use super::super::shortcuts::{self as page, Control, Request};
use super::*;
use secbird_core::global_shortcuts as model;

#[test]
#[ignore = "opens an isolated simulated Global Shortcuts editor; no recording, registrations or personal input"]
fn native_shortcuts_save_discard_conflict_import_export_and_recovery() {
    let folder = std::env::temp_dir().join(format!(
        "native-shortcuts-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    let frame = folder.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &folder),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("dashboard")),
    ]);
    let sim = Sim::mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let profiles = folder.join("profiles.json");
    let mut store = Store::load(&profiles).unwrap();
    store.for_device(&engine.devices()[0]).active_mut().dpi = Some((800, 800));
    store
        .extra
        .insert("foreign".into(), json!({"preserve":true}));
    store.save().unwrap();
    let original = std::fs::read(&profiles).unwrap();
    let output = folder.clone();
    let monitor = sim.clone();
    let controller = std::thread::spawn(move || {
        let result = std::panic::catch_unwind(|| (|| -> Result<Value, String> {
            unsafe {
                wait_for(|| frame.exists(), "shortcut editor HWND")?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let path = output.join("global-shortcuts.json");
                let query = || {
                    let mut result = Value::Null;
                    SendMessageW(hwnd, page::TEST, 1, &mut result as *mut Value as isize);
                    result
                };
                let control = |c: Control| {
                    SendMessageW(hwnd, page::TEST, 2, &c as *const Control as isize);
                };
                let request = |r: Request| {
                    SendMessageW(hwnd, page::TEST, 3, &r as *const Request as isize);
                };
                let idle = || query()["busy"] == false;
                SendMessageW(hwnd, page::TEST, 0, 0);
                wait_for(
                    || query()["open"] == true && idle(),
                    "shortcut library load",
                )?;
                monitor.state().log.clear();
                control(Control::Add);
                if query()["dirty"] != true || path.exists() {
                    return Err("Add escaped the local draft".into());
                }
                control(Control::Key("KEY_F23".into()));
                control(Control::Modifier("KEY_LEFT_ALT"));
                control(Control::Enabled);
                if query()["records"][0][model::HOST]["enabled"] != true {
                    return Err(format!("Supported enable refused: {}", query()));
                }
                control(Control::Save);
                wait_for(
                    || idle() && query()["dirty"] == false,
                    "explicit shortcut save",
                )?;
                let first = std::fs::read(&path).unwrap();
                control(Control::Duplicate);
                let duplicated = query();
                if duplicated["records"].as_array().map(Vec::len) != Some(2)
                    || duplicated["records"][1]["inputID"] != Value::Null
                    || duplicated["records"][0]["guid"] == duplicated["records"][1]["guid"]
                {
                    return Err(format!("Duplicate reused trigger/identity: {duplicated}"));
                }
                control(Control::Enabled);
                if query()["records"][1][model::HOST]["enabled"] != false {
                    return Err("Unassigned duplicate enabled".into());
                }
                control(Control::Key("KEY_F23".into()));
                for modifier in ["KEY_LEFT_CTRL", "KEY_LEFT_SHIFT", "KEY_LEFT_ALT"] {
                    control(Control::Modifier(modifier));
                }
                control(Control::Enabled);
                control(Control::Save);
                wait_for(
                    || idle() && query()["failed"] == true,
                    "conflicting save refusal",
                )?;
                if query()["dirty"] != true || std::fs::read(&path).unwrap() != first {
                    return Err("Conflict overwrote persisted library".into());
                }
                control(Control::Enabled);
                control(Control::Save);
                wait_for(
                    || idle() && query()["dirty"] == false,
                    "disabled duplicate save",
                )?;
                let baseline = std::fs::read(&path).unwrap();
                control(Control::Delete);
                control(Control::Cancel);
                if query()["dirty"] != false {
                    return Err("Cancelled delete changed definition".into());
                }
                control(Control::Delete);
                control(Control::ConfirmDelete);
                if query()["records"].as_array().map(Vec::len) != Some(1)
                    || query()["dirty"] != true
                {
                    return Err("Delete did not remain a draft".into());
                }
                SendMessageW(hwnd, WM_CLOSE, 0, 0);
                if query()["pending"] != true || IsWindow(hwnd) == 0 {
                    return Err("Dirty close skipped Save/Don't Save".into());
                }
                control(Control::Cancel);
                if query()["pending"] != false {
                    return Err("Cancel left a pending transition".into());
                }
                control(Control::Discard);
                wait_for(|| idle() && query()["dirty"] == false, "discard reload")?;
                if std::fs::read(&path).unwrap() != baseline {
                    return Err("Discard rewrote shortcuts".into());
                }
                control(Control::Select(0));
                let unchanged = query()["records"].clone();
                control(Control::Menu(false));
                wait_for(|| query()["popup"] == true, "explicit trigger choices")?;
                SendMessageW(hwnd, STUDIO_TEST, 9, 0);
                wait_for(|| query()["popup"] == false, "cancel trigger choices")?;
                if query()["records"] != unchanged {
                    return Err("Cancelled trigger chooser changed mappings".into());
                }
                control(Control::Category(4));
                let field = GetDlgItem(hwnd, 2400);
                if field.is_null() {
                    return Err("Missing native text assignment editor".into());
                }
                SetWindowTextW(field, wide("A local text assignment").as_ptr());
                if query()["records"][0]["textBlockGroup"]["text"] != "A local text assignment" {
                    return Err(format!("Native assignment field not bound: {}", query()));
                }
                let unsupported = query()["records"].clone();
                control(Control::Enabled);
                if query()["records"] != unsupported || query()["failed"] != true {
                    return Err("Unproved text dispatch was enabled".into());
                }
                let exports=output.join("exports");std::fs::create_dir_all(&exports).map_err(|e|e.to_string())?;
                let export = exports.join("draft-export.json");
                request(Request::Export(
                    export.clone(),
                    model::Library {
                        envelope: json!({"mappings":query()["records"]}),
                    },
                ));
                wait_for(idle, "draft export")?;
                if query()["failed"]==true{return Err(format!("Draft export failed: {}",query()));}
                let exported = model::load(&export).map_err(|e| e.to_string())?;
                if exported.records()[0]["textBlockGroup"]["text"] != "A local text assignment" {
                    return Err("Draft export missed unsaved assignment".into());
                }
                control(Control::Discard);
                wait_for(|| idle() && query()["dirty"] == false, "discard assignment")?;
                request(Request::Import(export));
                wait_for(|| idle() && query()["dirty"] == true, "import local draft")?;
                if std::fs::read(&path).unwrap() != baseline {
                    return Err("Import wrote without Save".into());
                }
                control(Control::Save);
                wait_for(|| idle() && query()["dirty"] == false, "import save")?;
                // External edits are retained. The draft is exportable after refusal.
                control(Control::Key("KEY_F24".into()));
                let mut external = model::load(&path).map_err(|e| e.to_string())?;
                let before = external.clone();
                external.envelope["foreign"] = json!({"external":true});
                model::save(&path, &before, &external).map_err(|e| e.to_string())?;
                let bytes = std::fs::read(&path).unwrap();
                control(Control::Save);
                wait_for(|| idle() && query()["failed"] == true, "stale save refusal")?;
                if std::fs::read(&path).unwrap() != bytes || query()["dirty"] != true {
                    return Err("Stale save lost external edit or draft".into());
                }
                control(Control::Discard);
                wait_for(
                    || idle() && query()["dirty"] == false,
                    "stale recovery reload",
                )?;
                let current = query()["records"].clone();
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                control(Control::Add);
                control(Control::Enabled);
                control(Control::Save);
                if query()["records"] != current || std::fs::read(&path).unwrap() != bytes {
                    return Err("Read-only editor mutated shortcuts".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                control(Control::Hyper);
                control(Control::Enabled);
                if query()["records"][0][model::HOST]["enabled"] != false {
                    return Err("Hypershift dispatch enabled".into());
                }
                SendMessageW(hwnd, WM_CLOSE, 0, 0);
                if query()["pending"] != true {
                    return Err("Missing second dirty close prompt".into());
                }
                let capture = json!(output.join("shortcuts-editor.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                control(Control::Discard);
                wait_for(|| IsWindow(hwnd) == 0, "Don't Save close")?;
                if std::fs::read(&profiles).unwrap() != original {
                    return Err("Shortcut workflow changed profiles or recovery state".into());
                }
                if !monitor.state().log.is_empty() {
                    return Err("Shortcut editor accessed simulated device transport".into());
                }
                Ok(
                    json!({"draft_save_discard":true,"duplicate_clears_trigger":true,"enabled_conflict_refused":true,"stale_save_preserves_both":true,"native_text_field":true,"trigger_menu_cancel":true,"delete_confirmation":true,"unsupported_hypershift_text_refused":true,"import_export_disabled":true,"read_only":true,"profile_bytes_unchanged":true,"device_commands":0,"personal_input_recording":false}),
                )
            }
        })()).unwrap_or_else(|_|Err("Shortcut test controller panicked; owned window cleanup requested".into()));
        if result.is_err() {
            unsafe {
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let c = Control::Discard;
                SendMessageW(hwnd, page::TEST, 2, &c as *const Control as isize);
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
        if let Ok(evidence) = &result {
            std::fs::write(
                output.join("validation.json"),
                serde_json::to_vec_pretty(evidence).unwrap(),
            )
            .unwrap();
        }
        result
    });
    let run = run_window(
        Arc::new(Mutex::new(engine)),
        Some(Duration::from_secs(45)),
        None,
    );
    let result = controller.join().unwrap();
    assert!(run.is_ok(), "{run:?}");
    assert!(result.is_ok(), "{result:?}");
    eprintln!("Global Shortcuts native evidence: {}", folder.display());
}
