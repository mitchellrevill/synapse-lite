use super::*;
use secbird_core::profile_compat::macro_library::{
    self as library,
    host_actions::Action,
    input_events::{Gesture, Input},
    Change,
};

#[test]
#[ignore = "Isolated simulated editor; executes only explicit SECBIRD_HOST_LAUNCH_FIXTURE, no physical input or vendor apps"]
fn native_macro_host_play_stop_refusals_revision_and_close_drain() {
    let fixture = PathBuf::from(
        std::env::var("SECBIRD_HOST_LAUNCH_FIXTURE")
            .expect("build and select the owned host-launch-fixture.exe"),
    )
    .canonicalize()
    .unwrap();
    assert_eq!(fixture.file_name().unwrap(), "host-launch-fixture.exe");
    assert!(fixture.is_file());
    let folder = std::env::temp_dir().join(format!(
        "native-macro-playback-{}-{}",
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
    let mut store = Store::load(&path).unwrap();
    store.for_device(&engine.devices()[0]).active_mut().dpi = Some((800, 800));
    for (guid, name) in [
        ("launch", "Owned launch"),
        ("pause", "Delay stop"),
        ("mixed", "Unsupported mixed"),
        ("failure", "Partial runtime failure"),
    ] {
        store = library::edit(
            &store,
            Change::Create {
                guid: guid.into(),
                name: name.into(),
            },
        )
        .unwrap();
        store = library::edit(
            &store,
            if guid == "pause" {
                Change::InsertDelay {
                    guid: guid.into(),
                    index: 1,
                    milliseconds: 10000,
                }
            } else {
                Change::InsertAction {
                    guid: guid.into(),
                    index: 1,
                    action: Action::Launch {
                        website: false,
                        file: fixture.to_str().unwrap().into(),
                        url: String::new(),
                    },
                }
            },
        )
        .unwrap();
    }
    store = library::edit(
        &store,
        Change::InsertInput {
            guid: "mixed".into(),
            index: 2,
            input: Input::Keyboard("KEY_A".into()),
            gesture: Gesture::Tap,
        },
    )
    .unwrap();
    store = library::edit(
        &store,
        Change::InsertAction {
            guid: "failure".into(),
            index: 2,
            action: Action::Launch {
                website: false,
                file: folder
                    .join("missing-owned-fixture.exe")
                    .to_str()
                    .unwrap()
                    .into(),
                url: String::new(),
            },
        },
    )
    .unwrap();
    store.save().unwrap();
    let original = std::fs::read(&path).unwrap();
    let devices = serde_json::to_value(&store.devices).unwrap();
    let output = folder.clone();
    let monitor = sim.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for(|| frame.exists(), "macro playback window")?;
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
                let select = |guid: &str| {
                    let guid = json!(guid);
                    SendMessageW(
                        hwnd,
                        super::super::macro_view::TEST,
                        7,
                        &guid as *const Value as isize,
                    );
                    redraw(hwnd);
                };
                SendMessageW(hwnd, super::super::macro_view::TEST, 1, 0);
                wait_for(
                    || {
                        query()["busy"] == false
                            && query()["entries"].as_array().is_some_and(|e| e.len() == 4)
                    },
                    "macro library",
                )?;
                monitor.state().log.clear();
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                let width = f64::from(client.right) / scale;
                let timeline = (width - 900.).max(10.) / 2. + 210.;
                select("launch");
                lighting_click(hwnd, timeline + 260., 174., scale);
                wait_for(
                    || {
                        query()["playback"]["report"]["outcome"] == "finished"
                            && query()["playback"]["active"] == false
                    },
                    "owned program playback",
                )?;
                let launch_report = query()["playback"]["report"].clone();
                if launch_report["launches"] != 1 || launch_report["completed"] != 2 {
                    return Err(format!("Wrong launch report: {launch_report}"));
                }
                if std::fs::read(&path).unwrap() != original {
                    return Err("Playback changed the saved document".into());
                }
                select("failure");
                SendMessageW(hwnd, super::super::macro_view::TEST, 11, 0);
                wait_for(
                    || {
                        query()["playback"]["report"]["outcome"] == "failed"
                            && query()["playback"]["active"] == false
                    },
                    "partial runtime failure",
                )?;
                if query()["playback"]["report"]["launches"] != 1
                    || !query()["status"]
                        .as_str()
                        .is_some_and(|s| s.contains("1 earlier launch"))
                    || std::fs::read(&path).unwrap() != original
                {
                    return Err(
                        "Runtime failure concealed an earlier accepted launch or changed source"
                            .into(),
                    );
                }
                select("pause");
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                SendMessageW(hwnd, super::super::macro_view::TEST, 11, 0);
                if query()["playback"]["active"] != false {
                    return Err("Read-only mode admitted playback".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                let name = GetDlgItem(hwnd, 2300);
                SetWindowTextW(name, wide("Unsaved name").as_ptr());
                SendMessageW(
                    hwnd,
                    WM_COMMAND,
                    ((EN_CHANGE as usize) << 16) | 2300,
                    name as isize,
                );
                SendMessageW(hwnd, super::super::macro_view::TEST, 11, 0);
                if query()["dirty"] != true || query()["playback"]["active"] != false {
                    return Err("Dirty draft admitted playback".into());
                }
                SendMessageW(hwnd, super::super::macro_view::TEST, 5, 0);
                select("mixed");
                SendMessageW(hwnd, super::super::macro_view::TEST, 11, 0);
                wait_for(
                    || {
                        query()["playback"]["report"]["outcome"] == "failed"
                            && !query()["playback"]["active"].as_bool().unwrap_or(true)
                    },
                    "whole mixed macro refusal",
                )?;
                if query()["playback"]["report"]["launches"] != 0
                    || std::fs::read(&path).unwrap() != original
                {
                    return Err("Mixed refusal executed or changed source".into());
                }
                select("pause");
                let current = Store::load(&path).map_err(|e| e.to_string())?;
                let changed = library::edit(
                    &current,
                    Change::Rename {
                        guid: "pause".into(),
                        name: "External delay".into(),
                    },
                )
                .map_err(|e| e.to_string())?;
                changed.save().map_err(|e| e.to_string())?;
                let expected = std::fs::read(&path).unwrap();
                SendMessageW(hwnd, super::super::macro_view::TEST, 11, 0);
                wait_for(
                    || {
                        query()["playback"]["report"]["outcome"] == "failed"
                            && query()["playback"]["active"] == false
                    },
                    "stale saved revision refusal",
                )?;
                if query()["playback"]["report"]["launches"] != 0 {
                    return Err("Stale selection launched a program".into());
                }
                SendMessageW(hwnd, super::super::macro_view::TEST, 3, 0);
                SendMessageW(hwnd, super::super::macro_view::TEST, 1, 0);
                wait_for(|| query()["busy"] == false, "external revision reload")?;
                select("pause");
                redraw(hwnd);
                lighting_click(hwnd, timeline + 260., 174., scale);
                wait_for(
                    || {
                        query()["playback"]["active"] == true
                            && query()["playback"]["snapshot"]["total"]
                                .as_u64()
                                .unwrap_or(0)
                                > 0
                    },
                    "delay playback",
                )?;
                let capture = json!(output.join("macro-playing.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                lighting_click(hwnd, timeline + 370., 174., scale);
                wait_for(
                    || {
                        query()["playback"]["report"]["outcome"] == "stopped"
                            && query()["playback"]["active"] == false
                    },
                    "Stop and drain",
                )?;
                if query()["playback"]["report"]["launches"] != 0 {
                    return Err("Delay Stop reported a launch".into());
                }
                SendMessageW(hwnd, super::super::macro_view::TEST, 11, 0);
                wait_for(
                    || query()["playback"]["active"] == true,
                    "close during playback",
                )?;
                SendMessageW(hwnd, WM_CLOSE, 0, 0);
                wait_for(|| IsWindow(hwnd) == 0, "close after playback drain")?;
                let current = Store::load(&path).map_err(|e| e.to_string())?;
                let entry = library::read(&current)
                    .map_err(|e| e.to_string())?
                    .entries
                    .into_iter()
                    .find(|e| e.guid == "pause")
                    .unwrap();
                let session = crate::host_macro_worker::start(crate::host_macro_worker::Request {
                    path: path.clone(),
                    guid: entry.guid,
                    revision: entry.revision,
                    read_only: false,
                })?;
                session.stop();
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    if matches!(
                        crate::host_macro_worker::poll(&session),
                        crate::host_macro_worker::Poll::Finished(_)
                    ) {
                        break;
                    }
                    if Instant::now() > deadline {
                        return Err("Post-close macro worker did not drain".into());
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                if std::fs::read(&path).unwrap() != expected
                    || serde_json::to_value(&Store::load(&path).unwrap().devices).unwrap()
                        != devices
                {
                    return Err("Playback changed profiles or source definitions".into());
                }
                if !monitor.state().log.is_empty() {
                    return Err("Macro playback submitted simulated device commands".into());
                }
                Ok(
                    json!({"owned_fixture_launch":launch_report,"partial_runtime_failure_preserves_accepted_launch_count":true,"actual_play_stop_buttons":true,"readonly_dirty_mixed_stale_refused":true,"stop_wakes_delay":true,"close_waits_for_drain":true,"post_close_admission_reused":true,"playback_source_writes":0,"controlled_external_rename_detected":true,"device_profiles_unchanged":true,"device_commands":0,"physical_discovery":false}),
                )
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            if IsWindow(hwnd) != 0 {
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                SendMessageW(hwnd, super::super::macro_view::TEST, 12, 0);
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
        std::fs::write(
            output.join("interaction.json"),
            serde_json::to_vec_pretty(&match &result {
                Ok(value) => value.clone(),
                Err(error) => json!({"error":error}),
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
    eprintln!("Macro playback native evidence: {}", folder.display());
}
