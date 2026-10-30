//! Bounded native library check with simulated transport and private files only.
use super::*;

#[test]
#[ignore = "opens an isolated native Studio window using simulated hardware"]
fn native_studio_composition_library_create_rename_duplicate_delete_and_dirty_guards() {
    let folder = std::env::temp_dir().join(format!(
        "native-studio-library-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    let source = folder.join("source.ChromaEffects");
    let original=br#"{"version":1,"opaque":9.9900e+100,"profiles":[{"id":"foreign","title":"Desk","devices":[{"id":"keyboard","productId":605,"editionId":0,"layout":1,"x":0,"y":0,"width":960,"height":360,"leds":[{"row":0,"col":1}]}],"layers":[],"future":9007199254740993123}]}"#;
    std::fs::write(&source, original).unwrap();
    let frame = folder.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &folder),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("studio")),
    ]);
    let sim = Sim::mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let monitor = sim.clone();
    let output = folder.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for_studio_window(|| frame.exists(), "Studio window")?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut value = Value::Null;
                    SendMessageW(hwnd, STUDIO_TEST, 2, &mut value as *mut Value as isize);
                    value
                };
                wait_for(|| query()["pending"] == 0, "initial reads")?;
                monitor.state().log.clear();
                SendMessageW(hwnd, STUDIO_TEST, 0, 0);
                wait_for(
                    || query()["pending"] == 0 && !query()["busy"].as_bool().unwrap(),
                    "session restore",
                )?;
                let input = json!(source.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 1, &input as *const Value as isize);
                wait_for(
                    || query()["profiles"] == 1 && query()["pending"] == 0,
                    "source open",
                )?;
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                if query()["canvas_devices"]!=1 {return Err("Library test requires a populated canvas".into());}
                // Quick-selection dispatch must not alias library Delete or Remap.
                lighting_click(hwnd, 335., 148., scale);
                wait_for(||query()["dialog"]==true,"quick-selection menu")?;
                SendMessageW(hwnd,STUDIO_TEST,9,0);
                wait_for(||query()["dialog"]==false,"quick-selection cancel")?;
                if query()["dirty"]!=false || query()["profiles"]!=1 || query()["selected_leds"]!=0 {
                    return Err("Quick-selection cancel changed the library or selection".into());
                }
                // Separate ellipsis action menu, as in the stock composition header.
                let menu = |row: usize| {
                    lighting_click(hwnd, 215., 100., scale);
                    lighting_click(hwnd, 350., 129. + row as f64 * 28., scale);
                };
                menu(0);
                if query()["profiles"] != 2 || query()["dirty"] != true || query()["index"] != 1 {
                    return Err(format!("Create failed: {}", query()));
                }
                let created = query()["profile_id"].as_str().unwrap().to_owned();
                if created.len() != 12 {
                    return Err("New identity was not stock-shaped".into());
                }
                let saved = output.join("saved.ChromaEffects");
                let path = json!(saved.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(
                    || query()["dirty"] == false && query()["pending"] == 0,
                    "library save",
                )?;
                menu(2);
                let field = GetDlgItem(hwnd, 2100);
                if field.is_null() {
                    return Err("Composition rename field missing".into());
                }
                SendMessageW(
                    field,
                    WM_SETTEXT,
                    0,
                    wide("Renamed composition").as_ptr() as isize,
                );
                lighting_click(hwnd, 215., 100., scale); // Commit text before opening the menu.
                if query()["dirty"] != true {
                    return Err(format!("Rename did not dirty library: {}", query()));
                }
                lighting_click(hwnd, 215., 100., scale);
                // Switching compositions keeps the entire unsaved library.
                lighting_click(hwnd, 140., 100., scale);
                lighting_click(hwnd, 140., 130., scale);
                wait_for(|| query()["index"] == 0, "switch with unsaved rename")?;
                lighting_click(hwnd, 140., 100., scale);
                lighting_click(hwnd, 140., 159., scale);
                wait_for(
                    || query()["index"] == 1 && query()["dirty"] == true,
                    "return to unsaved composition",
                )?;
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(|| query()["dirty"] == false && query()["pending"] == 0, "save rename")?;
                lighting_click(hwnd, 140., 100., scale);
                lighting_click(hwnd, 140., 130., scale);
                menu(3);
                if query()["profiles"] != 3 || query()["dirty"] != true {
                    return Err(format!("Duplicate failed: {}", query()));
                }
                let bad = json!(output.join("missing/bad.ChromaEffects").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 3, &bad as *const Value as isize);
                wait_for(|| query()["busy"] == false, "failed save")?;
                if query()["dirty"] != true || query()["failed"] != true {
                    return Err("Failed save accepted dirty draft".into());
                }
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(|| query()["dirty"] == false && query()["pending"] == 0, "retry save")?;
                lighting_click(hwnd,140.,100.,scale);
                lighting_click(hwnd,140.,186.,scale);
                wait_for(
                    || query()["dirty"] == false && query()["pending"] == 0,
                    "reselect saved duplicate",
                )?;
                SendMessageW(hwnd, STUDIO_TEST, 4, IDNO as isize);
                menu(5);
                wait_for(|| query()["dialog"] == false, "decline delete")?;
                if query()["profiles"] != 3 {
                    return Err("Delete ignored No".into());
                }
                SendMessageW(hwnd, STUDIO_TEST, 4, IDYES as isize);
                menu(5);
                wait_for(
                    || query()["profiles"] == 2 && query()["dirty"] == true,
                    "confirmed delete",
                )?;
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(
                    || query()["dirty"] == false && query()["pending"] == 0,
                    "delete save",
                )?;
                lighting_click(hwnd, 215., 100., scale);
                let capture = json!(output.join("composition-menu.png").to_string_lossy());
                if SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize) == 0 {
                    return Err("Menu screenshot failed".into());
                }
                let bytes = std::fs::read(saved).map_err(|e| e.to_string())?;
                let archive =
                    secbird_core::studio::Archive::parse(&bytes).map_err(|e| e.to_string())?;
                if archive
                    .profile_header(1)
                    .map_err(|e| e.to_string())?
                    .id
                    .as_deref()
                    != Some(&created)
                {
                    return Err("Stable saved composition ID changed".into());
                }
                if !String::from_utf8_lossy(&bytes).contains("9007199254740993123")
                    || !String::from_utf8_lossy(&bytes).contains("9.9900e+100")
                {
                    return Err("Opaque numeric metadata changed".into());
                }
                if !monitor.state().log.is_empty() {
                    return Err("Library actions issued device requests".into());
                }
                Ok(
                    json!({"create":true,"rename":true,"duplicate":true,"delete_confirmation":true,"switch_preserves_unsaved_library":true,"failed_save_retains_draft":true,"source_unchanged":true,"opaque_values_preserved":true,"simulated_device_requests":0}),
                )
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            SendMessageW(hwnd, STUDIO_TEST, 5, 0);
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
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
    let run = run_standalone(Arc::new(Mutex::new(engine)), Some(Duration::from_secs(35)));
    let result = controller.join().unwrap();
    assert!(run.is_ok(), "{run:?}");
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        std::fs::read(folder.join("source.ChromaEffects")).unwrap(),
        original
    );
    eprintln!("Studio composition library evidence: {}", folder.display());
}
