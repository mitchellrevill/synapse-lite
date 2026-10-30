use super::*;
use secbird_core::profile_compat::macro_library as library;

#[test]
#[ignore = "opens an isolated simulated editor and its owned export file dialog"]
fn native_macro_xml_export_import_refusals_and_dialog_cancel() {
    let folder = std::env::temp_dir().join(format!(
        "native-macro-xml-{}-{}",
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
    let path = folder.join("profiles.json");
    let mut store = Store::load(&path).unwrap();
    store.for_device(&engine.devices()[0]).active_mut().dpi = Some((800, 800));
    store = library::edit(
        &store,
        library::Change::Create {
            guid: "portable-native".into(),
            name: "Portable native".into(),
        },
    )
    .unwrap();
    store = library::edit(
        &store,
        library::Change::InsertDelay {
            guid: "portable-native".into(),
            index: 1,
            milliseconds: 1250,
        },
    )
    .unwrap();
    store.save().unwrap();
    let original = std::fs::read(&path).unwrap();
    let original_devices = serde_json::to_value(&store.devices).unwrap();
    let output = folder.clone();
    let monitor = sim.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                let start = Instant::now();
                while !frame.exists() && start.elapsed() < Duration::from_secs(15) {
                    std::thread::sleep(Duration::from_millis(20));
                }
                if !frame.exists() {
                    return Err("Macro XML window did not start".into());
                }
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut value = Value::Null;
                    SendMessageW(
                        hwnd,
                        super::super::macro_view::TEST,
                        2,
                        &mut value as *mut Value as isize,
                    );
                    value
                };
                SendMessageW(hwnd, super::super::macro_view::TEST, 1, 0);
                SendMessageW(hwnd, super::super::macro_view::TEST, 13, 0);
                wait_for(
                    || query()["busy"] == false && query()["selected"] == "portable-native",
                    "macro XML library",
                )?;
                monitor.state().log.clear();
                redraw(hwnd);
                macro_command(hwnd,super::super::macro_view::Control::Export);
                let started = Instant::now();
                let mut dialog = null_mut();
                while started.elapsed() < Duration::from_secs(5) {
                    let popup = GetLastActivePopup(hwnd);
                    if popup != hwnd && !popup.is_null() && IsWindowVisible(popup) != 0 {
                        let mut pid = 0;
                        GetWindowThreadProcessId(popup, &mut pid);
                        if pid == std::process::id() {
                            dialog = popup;
                            break;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                if dialog.is_null() {
                    return Err("Export button did not open its owned file dialog".into());
                }
                PostMessageW(dialog, WM_CLOSE, 0, 0);
                wait_for(|| IsWindow(dialog) == 0, "owned export dialog cancel")?;
                if std::fs::read(&path).unwrap() != original || query()["dirty"] != false {
                    return Err("Cancelled export changed profiles/draft".into());
                }
                let destination = output.join("portable.xml");
                let target = json!(destination.to_string_lossy());
                SendMessageW(
                    hwnd,
                    super::super::macro_view::TEST,
                    10,
                    &target as *const Value as isize,
                );
                wait_for(
                    || query()["busy"] == false && destination.exists(),
                    "macro XML export",
                )?;
                if query()["failed"] == true {
                    return Err(format!("Export failed: {}", query()));
                }
                let xml = std::fs::read(&destination).unwrap();
                let encoded = library::xml::import(&xml).map_err(|e| e.to_string())?;
                if library::describe(&encoded).unwrap().guid != "portable-native"
                    || std::fs::read(&path).unwrap() != original
                {
                    return Err("Export changed identity/profiles".into());
                }
                SendMessageW(
                    hwnd,
                    super::super::macro_view::TEST,
                    10,
                    &target as *const Value as isize,
                );
                wait_for(
                    || query()["busy"] == false && query()["failed"] == true,
                    "existing destination refusal",
                )?;
                if std::fs::read(&destination).unwrap() != xml {
                    return Err("Existing XML was overwritten".into());
                }
                macro_command(hwnd,super::super::macro_view::Control::Rename);
                let name=GetDlgItem(hwnd,2300);assert!(!name.is_null(),"inline Rename field missing");
                SetWindowTextW(name, wide("Unsaved name").as_ptr());
                let dirty = output.join("dirty.xml");
                let target = json!(dirty.to_string_lossy());
                SendMessageW(
                    hwnd,
                    super::super::macro_view::TEST,
                    10,
                    &target as *const Value as isize,
                );
                if dirty.exists() || query()["dirty"] != true || query()["failed"] != true {
                    return Err("Dirty export was not refused".into());
                }
                SendMessageW(hwnd, super::super::macro_view::TEST, 5, 0);
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                let read_only = output.join("read-only.xml");
                let target = json!(read_only.to_string_lossy());
                SendMessageW(
                    hwnd,
                    super::super::macro_view::TEST,
                    10,
                    &target as *const Value as isize,
                );
                if read_only.exists()
                    || query()["busy"] == true
                    || std::fs::read(&path).unwrap() != original
                {
                    return Err("Read-only export changed files".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                let fresh = Store::load(&path).unwrap();
                let entry = library::read(&fresh).unwrap().entries.remove(0);
                let stale_path = output.join("stale.xml");
                let stale = super::super::macro_view::work(
                    super::super::macro_view::Request::Export {
                        guid: entry.guid,
                        revision: "stale".into(),
                        path: stale_path.clone(),
                    },
                    false,
                );
                if stale.is_ok() || stale_path.exists() {
                    return Err("Stale export created a file".into());
                }
                let xml = String::from_utf8(xml).unwrap().replace(
                    "<Guid>portable-native</Guid>",
                    "<Guid>imported-native</Guid>",
                );
                let incoming = output.join("incoming.xml");
                std::fs::write(&incoming, xml).unwrap();
                let target = json!(incoming.to_string_lossy());
                SendMessageW(
                    hwnd,
                    super::super::macro_view::TEST,
                    4,
                    &target as *const Value as isize,
                );
                wait_for(
                    || {
                        query()["busy"] == false
                            && query()["entries"].as_array().is_some_and(|e| e.len() == 2)
                    },
                    "standalone XML import",
                )?;
                let current = Store::load(&path).unwrap();
                if serde_json::to_value(&current.devices).unwrap() != original_devices {
                    return Err("XML import changed device profiles".into());
                }
                let id = json!("imported-native");
                SendMessageW(
                    hwnd,
                    super::super::macro_view::TEST,
                    7,
                    &id as *const Value as isize,
                );
                redraw(hwnd);
                let capture = json!(output.join("macro-xml.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                if !monitor.state().log.is_empty() {
                    return Err("XML workflow submitted device commands".into());
                }
                Ok(
                    json!({"owned_dialog_open_cancel":true,"export_reimports":true,"guid_preserved":true,"profiles_unchanged":true,"existing_dirty_read_only_stale_refused":true,"standalone_import":true,"simulated_device_commands":0,"physical_discovery":false}),
                )
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            if IsWindow(hwnd) != 0 {
                let dialog = GetLastActivePopup(hwnd);
                if dialog != hwnd && !dialog.is_null() {
                    PostMessageW(dialog, WM_CLOSE, 0, 0);
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                SendMessageW(hwnd, super::super::macro_view::TEST, 5, 0);
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
        std::fs::write(
            output.join("interaction.json"),
            serde_json::to_vec_pretty(&match &result {
                Ok(v) => v.clone(),
                Err(e) => json!({"error":e}),
            })
            .unwrap(),
        )
        .unwrap();
        result
    });
    let run = run_standalone(Arc::new(Mutex::new(engine)), Some(Duration::from_secs(45)));
    let result = controller.join().unwrap();
    assert!(run.is_ok(), "{run:?}");
    assert!(result.is_ok(), "{result:?}");
    eprintln!("Standalone macro XML native evidence: {}", folder.display());
}
