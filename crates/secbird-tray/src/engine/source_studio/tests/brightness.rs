//! Brightness commits must never repaint the saved Quick effect over Studio.
use super::*;
use secbird_core::lighting::LightingSettings;

fn settings(f: &Fixture, value: u8) -> LightingSettings {
    let mut store = Store::load(&f.path).unwrap();
    let mut settings = store
        .for_device(&f.engine.devices[0])
        .active()
        .lighting_settings
        .clone()
        .unwrap();
    settings
        .brightness
        .as_mut()
        .unwrap()
        .set_value(value)
        .unwrap();
    settings
}
fn send(f: &mut Fixture, want: &LightingSettings) -> Result<Value> {
    let body=serde_json::to_vec(&json!({"id":f.key,"expected_device":f.key,"expected_profile":"Default","lighting_settings":want})).unwrap();
    crate::server::dispatch(&mut f.engine, "POST", "/api/lighting-settings", &body)
}

#[test]
fn live_brightness_commits_without_a_quick_frame_or_scene_restart() {
    let mut f = Fixture::new();
    f.select();
    f.tick();
    let before = f.engine.studio_status();
    let store_before = Store::load(&f.path).unwrap();
    let profile_before = store_before.devices[&f.key].profiles["Default"].clone();
    let hardware_before = {
        let sim = f.spy.sim.state();
        (
            sim.brightness.clone(),
            sim.effects.clone(),
            sim.game_mode,
            sim.macros.clone(),
        )
    };
    let want = settings(&f, 37);
    let response = send(&mut f, &want).unwrap();
    assert_eq!(response["execution"], "host-studio-brightness");
    assert_eq!(response["saved"], true);
    assert_eq!(response["accepted_packets"], 0);
    assert_eq!(
        f.attempts(),
        7,
        "saving brightness must not render the Quick program"
    );
    f.advance();
    f.tick();
    assert_eq!(f.attempts(), 14);
    let after = f.engine.studio_status();
    assert_eq!(after["composition_sha256"], before["composition_sha256"]);
    assert_eq!(after["generation"], before["generation"]);
    assert_eq!(after["frame_index"], 2);
    let mut store = Store::load(&f.path).unwrap();
    let profile = store.for_device(&f.engine.devices[0]).active();
    assert_eq!(profile.lighting_settings.as_ref().unwrap(), &want);
    assert_eq!(
        profile.lighting["backlight"].effect,
        profile_before.lighting["backlight"].effect
    );
    assert_eq!(
        profile.lighting["backlight"].color,
        profile_before.lighting["backlight"].color
    );
    assert_eq!(profile.keys, profile_before.keys);
    assert_eq!(
        secbird_core::profile_compat::source_quick_effects(profile),
        secbird_core::profile_compat::source_quick_effects(&profile_before)
    );
    let sim = f.spy.sim.state();
    assert_eq!(
        sim.live_brightness[&5], 94,
        "37 percent maps through the existing LIVE bridge"
    );
    assert_eq!(
        (
            sim.brightness.clone(),
            sim.effects.clone(),
            sim.game_mode,
            sim.macros.clone()
        ),
        hardware_before
    );
    assert_eq!(f.spy.state.lock().unwrap().mode, 3);
}

#[test]
fn brightness_switch_retains_value_and_freezes_phase_until_a_fresh_resume_frame() {
    let mut f = Fixture::new();
    f.select();
    f.tick();
    let mut want = settings(&f, 61);
    want.brightness.as_mut().unwrap().set_enabled(false);
    send(&mut f, &want).unwrap();
    assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
    for _ in 0..3 {
        f.advance();
        f.tick();
    }
    assert_eq!(f.attempts(), 7);
    assert_eq!(f.engine.studio_status()["frame_index"], 1);
    let saved = Store::load(&f.path).unwrap();
    let brightness = saved.devices[&f.key].profiles["Default"]
        .lighting_settings
        .as_ref()
        .unwrap()
        .brightness
        .as_ref()
        .unwrap();
    assert_eq!(brightness.value, 61);
    assert!(!brightness.enabled);
    want.brightness.as_mut().unwrap().set_enabled(true);
    send(&mut f, &want).unwrap();
    assert_eq!(
        f.spy.sim.state().live_brightness[&5],
        0,
        "positive LIVE waits for the worker frame"
    );
    f.advance();
    f.tick();
    assert_eq!(f.attempts(), 14);
    assert_eq!(f.engine.studio_status()["frame_index"], 2);
    assert_eq!(f.spy.sim.state().live_brightness[&5], 155);
}

#[test]
fn display_suppression_does_not_allow_a_brightness_edit_to_light_an_old_frame() {
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
    let before = f.engine.studio_status();
    let want = settings(&f, 37);
    send(&mut f, &want).unwrap();
    f.advance();
    f.tick();
    assert_eq!(f.attempts(), 7);
    assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
    assert_eq!(
        f.engine.studio_status()["frame_index"],
        before["frame_index"]
    );
    f.engine
        .lighting_runtime
        .borrow_mut()
        .test_environment
        .as_mut()
        .unwrap()
        .display_off = Some(false);
    f.advance();
    f.tick();
    assert_eq!(f.attempts(), 14);
    assert_eq!(f.spy.sim.state().live_brightness[&5], 94);
    assert_eq!(f.engine.studio_status()["generation"], before["generation"]);
}

#[test]
fn brightness_is_independent_of_unsupported_saved_quick_frame_program() {
    let mut f = Fixture::new();
    let mut store = Store::load(&f.path).unwrap();
    store.for_device(&f.engine.devices[0]).active_mut().keys =
        vec![secbird_core::Rgb(9, 8, 7); 132];
    store.save().unwrap();
    f.select();
    f.tick();
    let want = settings(&f, 37);
    send(&mut f, &want).unwrap();
    f.advance();
    f.tick();
    assert_eq!(f.attempts(), 14);
    assert_eq!(f.spy.sim.state().live_brightness[&5], 94);
    assert_eq!(
        Store::load(&f.path).unwrap().devices[&f.key].profiles["Default"].keys,
        vec![secbird_core::Rgb(9, 8, 7); 132]
    );
}

#[test]
fn invalid_identity_policy_and_owner_requests_leave_store_and_hardware_unchanged() {
    let mut f = Fixture::new();
    f.select();
    f.tick();
    let original = std::fs::read(&f.path).unwrap();
    let good = settings(&f, 37);
    let mut policy = good.clone();
    policy.switch_off.as_mut().unwrap().display_off = false;
    let mut invalid = good.clone();
    invalid.brightness.as_mut().unwrap().value = 101;
    let mut missing = good.clone();
    missing.brightness = None;
    for want in [policy, invalid, missing] {
        assert!(send(&mut f, &want).is_err());
    }
    let stale=serde_json::to_vec(&json!({"id":f.key,"expected_device":"different","expected_profile":"Default","lighting_settings":good})).unwrap();
    assert!(
        crate::server::dispatch(&mut f.engine, "POST", "/api/lighting-settings", &stale).is_err()
    );
    assert!(f
        .engine
        .set_studio_brightness(&f.key, "stale-profile", &good)
        .is_err());
    f.engine
        .lighting_runtime
        .borrow_mut()
        .test_environment
        .as_mut()
        .unwrap()
        .owner = Owner::Unknown("mock owner query failed".into());
    assert!(send(&mut f, &good).is_err());
    assert_eq!(std::fs::read(&f.path).unwrap(), original);
    assert_eq!(f.attempts(), 7);
    assert_eq!(f.spy.sim.state().live_brightness[&5], 255);
    assert_eq!(f.engine.studio_status()["frame_prepared"], true);
}

#[test]
fn failed_storage_does_not_commit_brightness_or_invalidate_a_prepared_scene() {
    let mut f = Fixture::new();
    f.select();
    f.tick();
    let original = std::fs::read(&f.path).unwrap();
    let want = settings(&f, 37);
    let mut permissions = std::fs::metadata(&f.path).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&f.path, permissions.clone()).unwrap();
    let result = send(&mut f, &want);
    permissions.set_readonly(false);
    std::fs::set_permissions(&f.path, permissions).unwrap();
    assert!(
        result.is_err(),
        "read-only profile destination must refuse persistence"
    );
    assert_eq!(std::fs::read(&f.path).unwrap(), original);
    assert_eq!(f.spy.sim.state().live_brightness[&5], 255);
    assert_eq!(f.attempts(), 7);
    assert_eq!(f.engine.studio_status()["frame_prepared"], true);
}

#[test]
fn a_faulted_session_can_be_darkened_but_never_automatically_retried_or_brightened() {
    let mut f = Fixture::new();
    f.select();
    f.spy.state.lock().unwrap().fail_at = Some(3);
    f.tick();
    let before = std::fs::read(&f.path).unwrap();
    let mut want = settings(&f, 37);
    assert!(send(&mut f, &want).is_err());
    assert_eq!(std::fs::read(&f.path).unwrap(), before);
    want.brightness.as_mut().unwrap().set_enabled(false);
    send(&mut f, &want).unwrap();
    assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
    f.advance();
    f.tick();
    assert_eq!(f.attempts(), 4);
    assert_eq!(f.engine.studio_status()["state"], "faulted");
    assert_eq!(f.engine.studio_status()["frame_index"], 0);
}

#[test]
fn stopped_worker_mode_change_and_stop_request_refuse_before_persistence() {
    for case in [0, 1, 2, 3] {
        let mut f = Fixture::new();
        f.select();
        f.tick();
        let original = std::fs::read(&f.path).unwrap();
        let want = settings(&f, 37);
        match case {
            0 => f.engine.measurement_read_only = true,
            1 => f.engine.set_uniform_worker(false, None, None),
            2 => f.spy.state.lock().unwrap().mode = 0,
            _ => f.engine.studio_control().request_stop().unwrap(),
        }
        assert!(f
            .engine
            .set_studio_brightness(&f.key, "Default", &want)
            .is_err());
        assert_eq!(std::fs::read(&f.path).unwrap(), original);
        assert_eq!(f.attempts(), 7);
    }
}
