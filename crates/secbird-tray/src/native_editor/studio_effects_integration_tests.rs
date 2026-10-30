//! One real window switches between consolidated effects; all transport is simulated.
use super::*;
#[test]
#[ignore = "opens isolated consolidated Studio window; requires recovered geometry assets"]
fn native_studio_integrated_effect_fields_routes_and_persistence() {
    let folder = std::env::temp_dir().join(format!("native-studio-effects-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let source = folder.join("input.ChromaEffects");
    let frame = folder.join("frame.json");
    let mut profile: Value = serde_json::from_str(include_str!(
        "../../../secbird-core/tests/fixtures/studio-preview-profile.json"
    ))
    .unwrap();
    profile["devices"][0]["width"] = json!(960);
    profile["devices"][0]["height"] = json!(360);
    profile["layers"]=json!([("wheel",13),("tidal",19),("audio",12),("spectrum",4),("breathing",5),("fire",8),("ambient",11)].map(|(name,id)|json!({"id":name,"name":name,"value":id,"title":name,"type":"layer","visible":true,"params":if matches!(name,"wheel"|"tidal"){json!({"centerPointX":125,"centerPointY":123,"centerPointX2":5000,"centerPointY2":5000,"counterclockwise":false})}else{json!({})},"regions":[]})));
    let input = serde_json::to_vec(&json!({"version":1,"profiles":[profile]})).unwrap();
    std::fs::write(&source, &input).unwrap();
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &folder),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("studio")),
    ]);
    let sim = Sim::wireless_mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let mut store = Store::load(&folder.join("profiles.json")).unwrap();
    store.for_device(&engine.devices()[0]);
    store.save().unwrap();
    let baseline = std::fs::read(folder.join("profiles.json")).unwrap();
    let report = folder.clone();
    let monitor = sim.clone();
    let path = source.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for(|| frame.exists(), "integrated Studio window")?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut v = Value::Null;
                    SendMessageW(hwnd, STUDIO_TEST, 2, &mut v as *mut Value as isize);
                    v
                };
                wait_for(|| query()["pending"] == 0, "initial reads")?;
                monitor.state().log.clear();
                let path = json!(path.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 1, &path as *const Value as isize);
                wait_for(
                    || query()["profiles"] == 1 && query()["busy"] == false,
                    "integrated composition load",
                )?;
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let width = f64::from(client.right) / scale;
                let height = f64::from(client.bottom) / scale;
                let right = width - 250.;
                let fit = ((width - 540.) / 960.)
                    .min((height - 250.) / 360.)
                    .clamp(0.05, 4.);
                let select = |index: usize| {
                    lighting_click(hwnd, 100., 188. + 40. * index as f64, scale);
                    lighting_click(
                        hwnd,
                        width / 2. + (67. - 480.) * fit,
                        (168. + height) / 2. + (76. - 180.) * fit,
                        scale,
                    );
                };
                let text = |id: i32, value: &str| -> Result<(), String> {
                    let field = GetDlgItem(hwnd, id);
                    if field.is_null() || IsWindowVisible(field) == 0 {
                        return Err(format!("Active field{id} missing: {}", query()));
                    }
                    SendMessageW(field, WM_SETTEXT, 0, wide(value).as_ptr() as isize);
                    Ok(())
                };
                let capture = |name: &str| {
                    let path = json!(report.join(name).to_string_lossy());
                    SendMessageW(hwnd, STUDIO_TEST, 6, &path as *const Value as isize);
                };
                select(0);
                text(2130, "120")?;
                lighting_click(hwnd, right + 170., 298., scale);
                capture("integrated-wheel.png");
                select(1);
                text(2142, "22")?;
                lighting_click(hwnd, right + 160., 411., scale);
                capture("integrated-tidal.png");
                if IsWindowVisible(GetDlgItem(hwnd, 2130)) != 0
                    || IsWindowVisible(GetDlgItem(hwnd, 2120)) != 0
                {
                    return Err("Tidal retained another effect's active fields".into());
                }
                select(2);
                text(2120, "2")?;
                lighting_click(hwnd, right + 100., 327., scale);
                capture("integrated-audio.png");
                let undo = query()["undo"].clone();
                SendMessageW(hwnd, WM_COMMAND, ((EN_CHANGE as usize) << 16) | 2130, 0);
                SendMessageW(hwnd, WM_COMMAND, ((EN_CHANGE as usize) << 16) | 2142, 0);
                if query()["undo"] != undo {
                    return Err("Stale effect field message edited Audio".into());
                }
                select(0);
                if edit_text(hwnd, 2130).as_deref() != Some("120") {
                    return Err("Wheel speed was lost after Tidal/Audio edits".into());
                }
                select(3);
                lighting_click(hwnd,right+30.,360.,scale);
                lighting_click(hwnd, right + 210., 290., scale);
                text(2103, "0")?;
                text(2104, "abcdef")?;
                lighting_click(hwnd, right + 70., 493., scale);
                capture("integrated-spectrum.png");
                select(4);
                lighting_click(hwnd, right + 175., 232., scale);
                lighting_click(hwnd, right + 175., 290., scale);
                lighting_click(hwnd, right + 115., 462., scale);
                capture("integrated-breathing.png");
                select(5);
                text(2101, "bb3300")?;
                lighting_click(hwnd, right + 60., 333., scale);
                capture("integrated-fire.png");
                select(6);
                if IsWindowVisible(GetDlgItem(hwnd, 2130)) != 0
                    || IsWindowVisible(GetDlgItem(hwnd, 2142)) != 0
                    || IsWindowVisible(GetDlgItem(hwnd, 2120)) != 0
                {
                    return Err("Ambient retained Wheel/Tidal/Audio controls".into());
                }
                let output = report.join("effects.ChromaEffects");
                let path = json!(output.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(
                    || query()["busy"] == false && query()["dirty"] == false,
                    "integrated save",
                )?;
                let bytes = std::fs::read(output).map_err(|e| e.to_string())?;
                let archive =
                    secbird_core::studio::Archive::parse(&bytes).map_err(|e| e.to_string())?;
                let saved: Value = serde_json::from_str(archive.profile_json(0).unwrap()).unwrap();
                let params = |index: usize| &saved["layers"][index]["regions"][0]["params"];
                if params(0)["speed"] != 120
                    || params(1)["speed"] != 22
                    || params(2)["boost"].as_f64() != Some(2.0)
                    || params(3)["duration"] != 18870
                    || params(3)["colorStops"][0]["Color"] != 0xabcdef
                    || params(4)["randomColor"] != true
                    || params(4)["cycles"] != 1
                    || params(5)["color"] != 0xbb3300
                {
                    return Err(format!(
                        "Consolidated field dispatch or persistence differs: {}",
                        saved["layers"]
                    ));
                }
                if !monitor.state().log.is_empty() {
                    return Err("Consolidated offline authoring issued transport requests".into());
                }
                Ok(
                    json!({"effect_switching":["wheel","tidal","audio","spectrum","breathing","fire","ambient"],"wheel_fields":[2130,2131,2132],"tidal_fields":[2140,2141,2142,2143],"audio_fields":[2120,2121],"stale_field_messages_inert":true,"saved_independent_values":true,"device_requests":0}),
                )
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            SendMessageW(hwnd, STUDIO_TEST, 5, 0);
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        std::fs::write(
            report.join("interaction.json"),
            serde_json::to_vec_pretty(&match &result {
                Ok(v) => v.clone(),
                Err(e) => json!({"error":e}),
            })
            .unwrap(),
        )
        .unwrap();
        result
    });
    let run = run_standalone(Arc::new(Mutex::new(engine)), Some(Duration::from_secs(40)));
    let result = controller.join().unwrap();
    assert!(run.is_ok(), "{run:?}");
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(std::fs::read(source).unwrap(), input);
    assert_eq!(
        std::fs::read(folder.join("profiles.json")).unwrap(),
        baseline
    );
    eprintln!("Integrated effects native evidence: {}", folder.display());
}
