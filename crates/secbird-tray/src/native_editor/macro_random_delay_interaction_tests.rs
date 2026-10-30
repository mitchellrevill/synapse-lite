use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use secbird_core::profile_compat::{macro_library as library, synapse_hash};

#[test]
#[ignore = "isolated simulated macro editor; temporary source definitions, no playback or physical input"]
fn native_random_delay_edit_save_history_invalid_readonly_and_conflict() {
    let folder = std::env::temp_dir().join(format!(
        "native-random-delay-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&folder).unwrap();
    let frame = folder.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &folder),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("dashboard")),
    ]);
    let sim = Sim::mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let path = folder.join("profiles.json");
    let source = r#"{"guid":"range","macroEvents":[{"Type":"actionBar","recordProfile":{}},{"Type":0,"Number":{"min":"0.100","max":"0.900","future":9.9900e+100},"future":1.2300e-7},{"Type":0,"Number":"0.050"}],"future":9007199254740993123456789}"#;
    let value: Value = serde_json::from_str(source).unwrap();
    let record = json!({"name":"Range fixture","payload":STANDARD.encode(source),"hash":synapse_hash(&value).unwrap()});
    let mut retained = value.clone();
    retained["guid"] = json!("retained");
    retained["macroEvents"][1]["Number"]["min"] = json!(0.0001);
    let retained = json!({"name":"Retained sub-millisecond","payload":STANDARD.encode(retained.to_string()),"hash":synapse_hash(&retained).unwrap()});
    let mut store = Store::load(&path).unwrap();
    store.for_device(&engine.devices()[0]).active_mut().dpi = Some((800, 800));
    store = library::edit(
        &store,
        library::Change::Import(serde_json::to_vec(&json!({"macros":[record,retained]})).unwrap()),
    )
    .unwrap();
    store.save().unwrap();
    let original = std::fs::read(&path).unwrap();
    let profiles = serde_json::to_value(&store.devices).unwrap();
    let monitor = sim.clone();
    let output = folder.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                let start = Instant::now();
                while !frame.exists() && start.elapsed() < Duration::from_secs(15) {
                    std::thread::sleep(Duration::from_millis(20));
                }
                if !frame.exists() {
                    return Err("Random-delay editor did not start".into());
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
                wait_for(
                    || query()["open"] == true && query()["busy"] == false,
                    "macro library",
                )?;
                monitor.state().log.clear();
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);
                let width = f64::from(rect.right) / scale;
                let left = (width - 900.).max(10.) / 2.;
                let timeline = left + 210.;
                let click = |x, y| lighting_click(hwnd, x, y, scale);
                let edit = |id, text: &str| {
                    let handle = GetDlgItem(hwnd, id);
                    assert!(!handle.is_null(), "missing random-delay field {id}");
                    SetWindowTextW(handle, wide(text).as_ptr());
                    SendMessageW(
                        hwnd,
                        WM_COMMAND,
                        ((EN_CHANGE as usize) << 16) | id as usize,
                        handle as isize,
                    );
                };
                redraw(hwnd);
                click(timeline + 100., 258.);
                if query()["range_min"] != "100"
                    || query()["range_max"] != "900"
                    || query()["dirty"] != false
                {
                    return Err(format!("Range selection altered source: {}", query()));
                }
                edit(2320, "250");
                edit(2321, "1250");
                if query()["dirty"] != true || std::fs::read(&path).unwrap() != original {
                    return Err("Range draft was not local".into());
                }
                click(left + 90., 404.);
                let applied = query()["events"].clone();
                if applied[1]["Number"]["min"] != "0.250" || applied[1]["Number"]["max"] != "1.250"
                {
                    return Err(format!("Range Apply failed: {}", query()));
                }
                click(timeline + 40., 174.);
                if query()["range_min"] != "100" {
                    return Err("Undo did not restore the range".into());
                }
                click(timeline + 135., 174.);
                if query()["events"] != applied || query()["range_max"] != "1250" {
                    return Err("Redo did not restore the range".into());
                }
                edit(2320, "2000");
                edit(2321, "1000");
                click(left + 90., 404.);
                if query()["failed"] != true
                    || query()["events"] != applied
                    || std::fs::read(&path).unwrap() != original
                {
                    return Err("Invalid range changed data".into());
                }
                edit(2320, "250");
                edit(2321, "1250");
                click(left + 90., 404.);
                redraw(hwnd);
                click(timeline + 630., 174.);
                wait_for(
                    || query()["busy"] == false && query()["dirty"] == false,
                    "range Save",
                )?;
                let saved = std::fs::read(&path).unwrap();
                let current = Store::load(&path).unwrap();
                let entry = library::read(&current).unwrap().entries.remove(0);
                let raw = String::from_utf8(
                    STANDARD
                        .decode(entry.encoded_record["payload"].as_str().unwrap())
                        .unwrap(),
                )
                .unwrap();
                if !raw.contains("9.9900e+100")
                    || !raw.contains("1.2300e-7")
                    || !raw.contains("9007199254740993123456789")
                    || serde_json::to_value(&current.devices).unwrap() != profiles
                {
                    return Err("Save lost opaque data or changed device profiles".into());
                }
                SendMessageW(hwnd, super::super::macro_view::TEST, 3, 0);
                SendMessageW(hwnd, super::super::macro_view::TEST, 1, 0);
                wait_for(|| query()["busy"] == false, "range reopen")?;
                redraw(hwnd);
                click(timeline + 100., 258.);
                if query()["range_min"] != "250" || query()["range_max"] != "1250" {
                    return Err("Reopened range differs".into());
                }
                for (id, expected) in [(2320, "250"), (2321, "1250")] {
                    let mut text = [0u16; 32];
                    let length =
                        GetWindowTextW(GetDlgItem(hwnd, id), text.as_mut_ptr(), text.len() as i32);
                    if String::from_utf16(&text[..length as usize]).unwrap() != expected {
                        return Err("Native range field text differs from saved data".into());
                    }
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                redraw(hwnd);
                edit(2320, "400");
                click(left + 90., 404.);
                if query()["dirty"] != false || std::fs::read(&path).unwrap() != saved {
                    return Err("Read-only range changed data".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                redraw(hwnd);
                // Refresh actual fields after the test-only read-only HWND mutation.
                click(timeline + 100., 296.);
                click(timeline + 100., 258.);
                let retained = json!("retained");
                SendMessageW(
                    hwnd,
                    super::super::macro_view::TEST,
                    7,
                    &retained as *const Value as isize,
                );
                redraw(hwnd);
                click(timeline + 100., 258.);
                if query()["range_min"] != ""
                    || GetWindowLongPtrW(GetDlgItem(hwnd, 2320), GWL_STYLE) & ES_READONLY as isize
                        == 0
                {
                    return Err("Sub-millisecond source was rounded or made editable".into());
                }
                edit(2320, "100");
                click(left + 90., 404.);
                if query()["dirty"] != false || std::fs::read(&path).unwrap() != saved {
                    return Err("Unsupported source range changed".into());
                }
                let supported = json!("range");
                SendMessageW(
                    hwnd,
                    super::super::macro_view::TEST,
                    7,
                    &supported as *const Value as isize,
                );
                redraw(hwnd);
                click(timeline + 100., 258.);
                edit(2320, "300");
                click(left + 90., 404.);
                let current = Store::load(&path).unwrap();
                let changed = library::edit(
                    &current,
                    library::Change::Rename {
                        guid: "range".into(),
                        name: "External range".into(),
                    },
                )
                .unwrap();
                changed.save().unwrap();
                let external = std::fs::read(&path).unwrap();
                redraw(hwnd);
                click(timeline + 630., 174.);
                wait_for(
                    || query()["busy"] == false && query()["failed"] == true,
                    "stale range refusal",
                )?;
                if std::fs::read(&path).unwrap() != external {
                    return Err("Stale range save overwrote external change".into());
                }
                let capture = json!(output.join("random-delay.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                SendMessageW(hwnd, super::super::macro_view::TEST, 5, 0);
                wait_for(
                    || query()["busy"] == false && !query()["dirty"].as_bool().unwrap_or(true),
                    "discard conflicted range",
                )?;
                if !monitor.state().log.is_empty() {
                    return Err("Range editing submitted device commands".into());
                }
                SendMessageW(hwnd, WM_CLOSE, 0, 0);
                Ok(
                    json!({"native_min_max_fields":true,"save_reopen":true,"undo_redo":true,"invalid_readonly_stale_refused":true,"opaque_source_preserved":true,"profiles_unchanged":true,"device_commands":0,"physical_discovery":false,"playback":false}),
                )
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            if IsWindow(hwnd) != 0 {
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
    eprintln!("Random delay native evidence: {}", folder.display());
}
