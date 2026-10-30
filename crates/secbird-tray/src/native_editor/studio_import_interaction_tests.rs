//! Real Import menu/worker flow; only the Windows file-picker result is supplied.
use super::*;

#[test]
#[ignore = "opens an isolated Studio window and reads an explicitly selected import file"]
fn native_studio_import_cancel_failure_success_and_profile_switch_preserve_drafts() {
    let input = PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select import file"),
    );
    let original = std::fs::read(&input).unwrap();
    let source = secbird_core::studio::Archive::parse(&original).unwrap();
    assert_eq!(
        source.profile_count(),
        1,
        "single-composition import fixture"
    );
    let folder = std::env::temp_dir().join(format!("native-studio-import-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let frame = folder.join("frame.json");
    let bad = folder.join("invalid.ChromaEffects");
    std::fs::write(&bad, b"not a composition").unwrap();
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &folder),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("keyboard-lighting")),
    ]);
    let sim = Sim::keyboard();
    let engine = Engine::with_simulated_keyboard(&sim, folder.join("profiles.json")).unwrap();
    let output = folder.clone();
    let source_path = input.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            unsafe {
                wait_for(|| frame.exists(), "import window")?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut value = Value::Null;
                    SendMessageW(hwnd, STUDIO_TEST, 2, &mut value as *mut Value as isize);
                    value
                };
                let snapshot = || {
                    let mut value = Value::Null;
                    SendMessageW(
                        hwnd,
                        STUDIO_TEST,
                        studio_view::TEST_DRAFT_SNAPSHOT,
                        &mut value as *mut Value as isize,
                    );
                    value
                };
                SendMessageW(hwnd, STUDIO_TEST, 0, 0);
                wait_for(
                    || query()["busy"] == false && query()["profiles"] == 1,
                    "editable initial composition",
                )?;
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                studio_effect_click(hwnd, "Fire", scale)?;
                let before = snapshot();
                let before_state = query();
                let import = |choice: Value| -> Result<(), String> {
                    SendMessageW(
                        hwnd,
                        STUDIO_TEST,
                        studio_view::TEST_IMPORT_FILE,
                        &choice as *const Value as isize,
                    );
                    studio_click(hwnd, "composition-menu", scale)?;
                    studio_click(hwnd, "import", scale)?;
                    wait_for(
                        || {
                            query()["import_picker_pending"] == false
                                && query()["busy"] == false
                                && query()["dialog"] == false
                        },
                        "Import completes or cancels",
                    )
                };
                import(Value::Null)?;
                if snapshot() != before
                    || query()["undo"] != before_state["undo"]
                    || query()["profiles"] != 1
                {
                    return Err("Cancelling Import discarded the active draft".into());
                }
                import(json!(bad.to_string_lossy()))?;
                if snapshot() != before || query()["failed"] != true || query()["profiles"] != 1 {
                    return Err("Failed Import replaced or disabled the active draft".into());
                }
                studio_effect_click(hwnd, "Static", scale)?;
                studio_click(hwnd, "undo", scale)?;
                if snapshot() != before {
                    return Err("Effect controls stopped working after a failed Import".into());
                }
                import(json!(source_path.to_string_lossy()))?;
                if query()["profiles"] != 2 || query()["index"] != 1 || query()["failed"] == true {
                    return Err(format!(
                        "Single-profile Import did not finish automatically: {}",
                        query()
                    ));
                }
                if query()["canvas_devices"] != 1 || query()["rows"].as_array().map(Vec::len) != Some(2) {
                    return Err("Imported composition lost its keyboard geometry or effect layers".into());
                }
                studio_click(hwnd, "profiles", scale)?;
                lighting_click(hwnd, 100., 131., scale);
                if query()["index"] != 0 || snapshot() != before || query()["profiles"] != 2 {
                    return Err("Switching profiles discarded unsaved imported compositions or the original draft".into());
                }
                studio_click(hwnd, "profiles", scale)?;
                lighting_click(hwnd, 100., 159., scale);
                if query()["index"] != 1 || query()["profiles"] != 2 {
                    return Err("Imported profile could not be selected again".into());
                }
                let saved = output.join("merged.ChromaEffects");
                let path = json!(saved.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(
                    || query()["busy"] == false && query()["dirty"] == false,
                    "merged library save",
                )?;
                let archive = secbird_core::studio::Archive::parse(
                    &std::fs::read(saved).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if archive.profile_count() != 2 || archive.profile_json(0) != before.as_str() {
                    return Err("Saved import lost the original unsaved draft".into());
                }
                Ok(())
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            SendMessageW(hwnd, STUDIO_TEST, 9, 0);
            SendMessageW(hwnd, STUDIO_TEST, 5, 0);
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        result
    });
    let run = run_standalone(Arc::new(Mutex::new(engine)), Some(Duration::from_secs(45)));
    let result = controller.join().unwrap();
    assert!(run.is_ok(), "{run:?}");
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(std::fs::read(input).unwrap(), original);
    eprintln!("Native Import workflow verified: {}", folder.display());
}
