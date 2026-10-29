use super::*;
use crate::{
    host_static::test_support,
    host_uniform::Program,
    lighting_runtime::{Environment, Owner},
};
use secbird_core::lighting::SwitchOffLighting;
use std::{path::PathBuf, sync::atomic::AtomicU64};
static NEXT: AtomicU64 = AtomicU64::new(0);
mod brightness;

struct Fixture {
    engine: Engine,
    spy: test_support::Spy,
    key: String,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        HidApi::disable_device_discovery();
        let mut engine = Engine::from_api(HidApi::new().unwrap());
        let (device, spy) = test_support::fixture();
        let key = device.key();
        let path = std::env::temp_dir()
            .join(format!(
                "studio-worker-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ))
            .join("profiles.json");
        let mut profile = secbird_core::profile_compat::new_native_profile(605, "Default").unwrap();
        let mut settings = profile.lighting_settings.clone().unwrap();
        settings.switch_off = Some(SwitchOffLighting {
            display_off: true,
            ..Default::default()
        });
        settings.store_in_profile(&mut profile).unwrap();
        Program::Spectrum.opt_in(&mut profile).unwrap();
        let mut store = Store::load(&path).unwrap();
        store
            .for_device(&device)
            .profiles
            .insert("Default".into(), profile);
        store.save().unwrap();
        spy.sim.state().brightness.insert(5, 255);
        spy.sim.state().live_brightness.insert(5, 0);
        spy.sim.state().ornata_layout_edition = Some([6, 0]);
        engine.devices.push(device);
        engine.simulated_only = true;
        engine.runtime_store_path = Some(path.clone());
        engine.lighting_runtime.borrow_mut().test_environment = Some(Environment {
            now_ms: 1000,
            idle_ms: Ok(0),
            display_off: Some(false),
            owner: Owner::Clear,
        });
        engine.lighting_runtime.borrow_mut().start();
        engine.set_uniform_worker(
            true,
            None,
            Some(crate::uniform_worker::Signal::running_for_test()),
        );
        Self {
            engine,
            spy,
            key,
            path,
        }
    }
    fn select(&mut self) {
        let previous = self.engine.studio.session.target_key().map(str::to_owned);
        self.engine.studio.session = OutputSession::synthetic_for_test(
            &self.engine.devices[0],
            self.engine.studio.connection_epoch.unwrap(),
        );
        let key = self.key.clone();
        self.engine.studio_applied(previous, &key);
    }
    fn tick(&mut self) {
        self.engine.tick_uniform(&AtomicBool::new(false));
    }
    fn advance(&mut self) {
        self.engine.studio.epoch -= Duration::from_millis(50);
    }
    fn attempts(&self) -> usize {
        self.spy.state.lock().unwrap().attempts
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir(self.path.parent().unwrap());
    }
}

#[test]
fn shared_worker_hands_only_selected_target_to_studio_and_preserves_saved_quick() {
    let mut f = Fixture::new();
    let saved = std::fs::read(&f.path).unwrap();
    f.tick();
    assert_eq!(f.attempts(), 7);
    f.select();
    f.tick();
    assert_eq!(
        f.attempts(),
        14,
        "Studio and Quick must not both emit for this target"
    );
    assert_eq!(f.engine.studio_status()["accepted_frames"], 1);
    assert_eq!(f.engine.studio_status()["frame_prepared"], true);
    assert!(f.engine.studio_holds(&f.key));
    f.tick();
    assert_eq!(
        f.attempts(),
        14,
        "a second immediate tick must respect Studio's 40ms interval"
    );
    f.advance();
    f.tick();
    assert_eq!(f.attempts(), 21);
    assert_eq!(std::fs::read(&f.path).unwrap(), saved);
    let signal = f.engine.uniform.alive.as_ref().unwrap();
    assert_eq!(signal.metrics()["batches_completed"], 3);
    assert_eq!(signal.metrics()["batch_active"], false);
}

#[test]
fn stop_drains_before_saved_opted_quick_resumes_on_same_worker() {
    let mut f = Fixture::new();
    f.select();
    f.tick();
    let saved = std::fs::read(&f.path).unwrap();
    f.engine.studio_control().request_stop().unwrap();
    assert!(f.engine.studio_holds(&f.key));
    f.tick();
    assert_eq!(f.engine.studio_status()["state"], "stopped");
    assert!(!f.engine.studio_holds(&f.key));
    assert_eq!(
        f.attempts(),
        14,
        "the next 7 OUTs belong to retained Spectrum Quick"
    );
    assert_eq!(f.engine.uniform_status()[&f.key]["accepted_frames"], 1);
    assert_eq!(std::fs::read(&f.path).unwrap(), saved);
    assert_eq!(f.spy.state.lock().unwrap().mode, 3);
}

#[test]
fn periodic_refresh_and_profile_apply_do_not_paint_quick_over_studio() {
    let mut f = Fixture::new();
    f.select();
    f.tick();
    let saved = std::fs::read(&f.path).unwrap();
    f.engine.refresh_frames();
    let key = f.key.clone();
    f.engine.apply(&[key]);
    f.engine.request_lighting_refresh();
    assert_eq!(f.attempts(), 7);
    assert_eq!(std::fs::read(&f.path).unwrap(), saved);
    assert!(f.engine.studio_holds(&f.key));
}

#[test]
fn static_quick_refresh_is_held_and_reapplied_after_explicit_stop() {
    let mut f = Fixture::new();
    let mut store = Store::load(&f.path).unwrap();
    let profile = store.for_device(&f.engine.devices[0]).active_mut();
    Program::clear_opt_in(profile);
    secbird_core::profile_compat::set_source_quick_effect(
        profile,
        605,
        "Default",
        1,
        &json!({"color1":"#010203"}),
    )
    .unwrap();
    let zone = profile.lighting.get_mut("backlight").unwrap();
    zone.color = Some(secbird_core::Rgb(1, 2, 3));
    zone.effect = None;
    zone.off = false;
    store.save().unwrap();
    let saved = std::fs::read(&f.path).unwrap();
    f.select();
    f.tick();
    assert_eq!(f.attempts(), 7);
    f.engine.request_lighting_refresh();
    f.engine.refresh_frames();
    let key = f.key.clone();
    f.engine.apply(&[key]);
    assert_eq!(
        f.attempts(),
        7,
        "Static reconcile/profile/per-key refresh must all yield to Studio"
    );
    f.engine.stop_studio().unwrap();
    f.engine.request_lighting_refresh();
    assert_eq!(f.attempts(), 14);
    assert!(f
        .spy
        .sim
        .state()
        .matrix
        .as_ref()
        .unwrap()
        .2
        .iter()
        .all(|pixel| *pixel == [1, 2, 3]));
    assert_eq!(std::fs::read(&f.path).unwrap(), saved);
}

#[test]
fn apply_refuses_read_only_absent_worker_and_wrong_mode_before_any_frame() {
    let mut f = Fixture::new();
    let archive = Archive::parse(br#"{"version":1,"profiles":[]}"#).unwrap();
    let absent = Path::new("not-an-asset");
    f.engine.measurement_read_only = true;
    let intent = f.engine.studio_control().apply_intent().unwrap();
    assert!(f
        .engine
        .apply_studio_saved(intent, &f.key, &archive, 0, absent, 1)
        .is_err());
    f.engine.measurement_read_only = false;
    f.engine.set_uniform_worker(false, None, None);
    let intent = f.engine.studio_control().apply_intent().unwrap();
    assert!(f
        .engine
        .apply_studio_saved(intent, &f.key, &archive, 0, absent, 1)
        .is_err());
    f.engine.set_uniform_worker(
        true,
        None,
        Some(crate::uniform_worker::Signal::running_for_test()),
    );
    f.spy.state.lock().unwrap().mode = 0;
    let intent = f.engine.studio_control().apply_intent().unwrap();
    let error = f
        .engine
        .apply_studio_saved(intent, &f.key, &archive, 0, absent, 1)
        .unwrap_err();
    assert!(error.to_string().contains("Driver_Mode3"));
    assert!(!f.engine.studio_holds(&f.key));
    assert_eq!(f.attempts(), 0);
}

#[test]
fn another_keyboard_keeps_quick_while_selected_keyboard_plays_studio() {
    let mut f = Fixture::new();
    let (mut second, spy) = test_support::fixture();
    second.serial = "SECOND".into();
    second.path = std::ffi::CString::new("sim-other-keyboard").unwrap();
    let key = second.key();
    let mut store = Store::load(&f.path).unwrap();
    let profile = store.devices[&f.key].profiles["Default"].clone();
    store
        .for_device(&second)
        .profiles
        .insert("Default".into(), profile);
    store.save().unwrap();
    f.engine.devices.push(second);
    f.select();
    f.tick();
    assert_eq!(f.attempts(), 7);
    assert_eq!(spy.state.lock().unwrap().attempts, 7);
    assert!(!f.engine.studio_holds(&key));
    assert_eq!(f.engine.uniform_status()[&key]["accepted_frames"], 1);
}

#[test]
fn suppression_and_owner_conflict_stop_output_without_advancing_phase_or_losing_scene() {
    let mut f = Fixture::new();
    f.select();
    f.tick();
    f.engine
        .lighting_runtime
        .borrow_mut()
        .test_environment
        .as_mut()
        .unwrap()
        .display_off = Some(true);
    f.engine.request_lighting_refresh();
    f.advance();
    f.tick();
    assert_eq!(f.attempts(), 7);
    assert_eq!(f.engine.studio_status()["frame_index"], 1);
    assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
    {
        let mut runtime = f.engine.lighting_runtime.borrow_mut();
        let environment = runtime.test_environment.as_mut().unwrap();
        environment.display_off = Some(false);
        environment.owner = Owner::Competing(vec!["Synapse test owner".into()]);
    }
    f.advance();
    f.tick();
    assert_eq!(f.attempts(), 7);
    assert!(f.engine.studio_status()["runtime_error"]
        .as_str()
        .unwrap()
        .contains("owner"));
    f.engine
        .lighting_runtime
        .borrow_mut()
        .test_environment
        .as_mut()
        .unwrap()
        .owner = Owner::Clear;
    f.advance();
    f.tick();
    assert_eq!(f.attempts(), 14);
    assert_eq!(f.engine.studio_status()["frame_index"], 2);
    assert!(
        f.spy.sim.state().live_brightness[&5] > 0,
        "LIVE brightness resumes only after the fresh Studio frame"
    );
}

#[test]
fn partial_studio_failure_holds_target_and_never_falls_back_to_quick_automatically() {
    let mut f = Fixture::new();
    f.select();
    f.spy.state.lock().unwrap().fail_at = Some(3);
    f.tick();
    assert_eq!(f.attempts(), 4);
    assert_eq!(f.engine.studio_status()["state"], "faulted");
    assert_eq!(f.engine.studio_status()["failure"]["accepted_packets"], 3);
    for _ in 0..3 {
        f.advance();
        f.tick();
        f.engine.request_lighting_refresh();
        f.engine.refresh_frames();
    }
    assert_eq!(f.attempts(), 4);
    assert!(f.engine.studio_holds(&f.key));
    f.engine.stop_studio().unwrap();
    f.tick();
    assert_eq!(f.attempts(), 11);
}

#[test]
fn handle_and_worker_replacement_invalidate_session_and_never_replay_it() {
    let mut f = Fixture::new();
    f.select();
    f.tick();
    let epoch = f.engine.studio.connection_epoch;
    f.engine.invalidate_studio("test handle replacement", true);
    assert_eq!(
        f.engine.studio.connection_epoch,
        epoch.map(|epoch| epoch + 1)
    );
    assert_eq!(f.engine.studio_status()["state"], "stopped");
    f.select();
    f.engine.set_uniform_worker(
        true,
        None,
        Some(crate::uniform_worker::Signal::running_for_test()),
    );
    assert_eq!(f.engine.studio_status()["state"], "stopped");
    f.tick();
    assert_eq!(f.engine.studio_status()["state"], "stopped");
    f.select();
    f.engine.set_uniform_worker(false, None, None);
    f.tick();
    assert_eq!(f.engine.studio_status()["state"], "stopped");
}

#[test]
fn conflicting_quick_actions_are_refused_before_io_including_product_id_alias() {
    let mut f = Fixture::new();
    f.select();
    for target in [&f.key, "025D"] {
        for path in [
            "/api/source-effect",
            "/api/color",
            "/api/effect",
            "/api/off",
            "/api/keys",
            "/api/find-light",
            "/api/lighting-control",
            "/api/dimming",
        ] {
            assert!(f.engine.guard_studio_quick_action(target, path).is_err());
        }
    }
    assert!(f.engine.guard_studio_quick_action("", "/api/sync").is_err());
    assert!(f
        .engine
        .guard_studio_quick_action("other", "/api/source-effect")
        .is_ok());
    assert!(f
        .engine
        .guard_studio_quick_action(&f.key, "/api/lighting-policy")
        .is_ok());
    assert!(f.engine.guard_studio_quick_action(&f.key,"/api/lighting-settings").is_ok());
    assert!(f
        .engine
        .guard_studio_quick_action(&f.key, "/api/button")
        .is_ok());
    assert_eq!(f.attempts(), 0);
}

#[test]
#[ignore = "requires explicit pinned private manifest and actual exported composition; MOCK output only"]
fn explicit_actual_archive_apply_uses_existing_worker_and_does_not_enable_reconnect_replay() {
    let mut f = Fixture::new();
    let input = PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select actual export"),
    );
    let manifest = PathBuf::from(
        std::env::var_os("SECBIRD_ORNATA_MANIFEST_TEST_PATH").expect("select pinned manifest"),
    );
    let bytes = std::fs::read(&input).unwrap();
    let archive = Archive::parse(&bytes).unwrap();
    let saved = std::fs::read(&f.path).unwrap();
    let intent = f.engine.studio_control().apply_intent().unwrap();
    let status = f
        .engine
        .apply_studio_saved(intent, &f.key, &archive, 0, &manifest, 1)
        .unwrap();
    assert_eq!(status["state"], "applied");
    assert_eq!(status["automatic_reconnect_replay"], false);
    assert_eq!(f.attempts(), 0);
    f.tick();
    assert_eq!(f.attempts(), 7);
    assert_eq!(f.spy.sim.state().live_brightness[&5],255,"explicit accepted Studio output applies configured LIVE brightness from a prior LIVE0");
    assert_eq!(
        f.engine.lighting_runtime_status()[&f.key]["studio"]["accepted_frames"],
        1
    );
    assert_eq!(std::fs::read(&f.path).unwrap(), saved);
    assert_eq!(std::fs::read(&input).unwrap(), bytes);
    f.engine.stop_studio().unwrap();
    assert!(!f.engine.studio_holds(&f.key));
}
