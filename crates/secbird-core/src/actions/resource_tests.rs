//! Resource limits must fail before SET, including partial/recovery states.
use super::*;
use crate::{sim::Sim, transport::Transport};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const LIMIT: usize = 2 * 1024 * 1024;

struct Fixture {
    sim: Sim,
    dev: Device,
    store: Store,
    folder: PathBuf,
}
impl Fixture {
    fn mouse() -> Self {
        let sim = Sim::mouse();
        let dev = sim
            .open(crate::sim::MOUSE_PID, Sim::spec_for(crate::sim::MOUSE_PID))
            .unwrap();
        Self::new(sim, dev)
    }
    fn new(sim: Sim, dev: Device) -> Self {
        let folder = std::env::temp_dir().join(format!(
            "action-resource-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut store = Store::load(&folder.join("profiles.json")).unwrap();
        store
            .for_device(&dev)
            .active_mut()
            .extra
            .insert("future".into(), json!({"retain": [1, true]}));
        Self {
            sim,
            dev,
            store,
            folder,
        }
    }
    fn path(&self) -> PathBuf {
        self.folder.join("profiles.json")
    }
    fn prepare(&mut self) -> (Value, Vec<u8>) {
        self.store.save().unwrap();
        self.sim.state().log.clear();
        (
            serde_json::to_value(&self.store).unwrap(),
            std::fs::read(self.path()).unwrap(),
        )
    }
    fn refused_unchanged(&self, before: &(Value, Vec<u8>)) {
        assert_eq!(serde_json::to_value(&self.store).unwrap(), before.0);
        assert_eq!(std::fs::read(self.path()).unwrap(), before.1);
        assert!(
            self.sim.state().log.iter().all(|c| c.id & 0x80 != 0),
            "refusal sent SET: {:?}",
            self.sim.state().log
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.path());
        let _ = std::fs::remove_file(self.path().with_extension("json.tmp"));
        let _ = std::fs::remove_dir(&self.folder);
    }
}

fn fill_bytes(store: &mut Store) {
    store.extra.insert("padding".into(), json!(""));
    let size = serde_json::to_vec(store).unwrap().len();
    store
        .extra
        .insert("padding".into(), json!("x".repeat(LIMIT - size)));
    assert_eq!(store.native_bytes().unwrap().len(), LIMIT);
}
fn nodes(value: &Value) -> usize {
    1 + match value {
        Value::Object(map) => map.len() + map.values().map(nodes).sum::<usize>(),
        Value::Array(values) => values.iter().map(nodes).sum(),
        _ => 0,
    }
}
fn fill_nodes(store: &mut Store) {
    store.extra.insert("padding".into(), json!([]));
    let count = nodes(&serde_json::to_value(&*store).unwrap());
    store.extra.insert(
        "padding".into(),
        Value::Array(vec![Value::Null; 32768 - count]),
    );
    store.validate_save().unwrap();
}

#[test]
fn complete_candidates_refuse_byte_and_node_growth_before_set() {
    type Edit = fn(&Device, &mut Store) -> Result<()>;
    let edits: &[(&str, Edit)] = &[
        ("dpi", |d, s| set_dpi(d, s, 800, 800).map(|_| ())),
        ("stages", |d, s| {
            set_dpi_stages(
                d,
                s,
                &DpiStages {
                    active: 1,
                    stages: vec![(801, 901), (1601, 1701)],
                    ..Default::default()
                },
            )
            .map(|_| ())
        }),
        ("polling", |d, s| set_polling(d, s, 500)),
        ("color", |d, s| set_color(d, s, "logo", Rgb(1, 2, 3))),
        ("effect", |d, s| {
            set_effect(d, s, "logo", crate::effects::Effect::Spectrum)
        }),
        ("brightness", |d, s| set_brightness(d, s, "logo", 51)),
        ("off", |d, s| lights_off(d, s, "logo")),
        ("ownership", |d, s| set_lighting_control(d, s, false)),
        ("button", |d, s| {
            set_button(
                d,
                s,
                "side-forward",
                crate::buttons::Layer::Normal,
                Some(crate::buttons::Action::Mouse(2)),
            )
        }),
    ];
    for by_nodes in [false, true] {
        for (name, edit) in edits {
            let mut f = Fixture::mouse();
            if by_nodes {
                fill_nodes(&mut f.store);
            } else {
                fill_bytes(&mut f.store);
            }
            let before = f.prepare();
            let error = edit(&f.dev, &mut f.store).unwrap_err();
            assert!(
                error.to_string().contains("budget"),
                "{name}, nodes={by_nodes}: {error}"
            );
            f.refused_unchanged(&before);
        }
    }
}

#[test]
fn alias_resolution_is_discarded_on_resource_refusal() {
    let mut f = Fixture::mouse();
    let current = f.dev.key();
    let profile = f.store.devices.remove(&current).unwrap();
    f.store
        .devices
        .insert(format!("{:04X}", f.dev.pid), profile);
    fill_bytes(&mut f.store);
    let before = f.prepare();
    assert!(set_dpi(&f.dev, &mut f.store, 800, 800).is_err());
    f.refused_unchanged(&before);
}

#[test]
fn pure_stage_fit_matches_written_projection_and_retains_unknown_fields() {
    let mut f = Fixture::mouse();
    let stages = DpiStages {
        active: 2,
        stages: vec![(801, 901), (1601, 1701)],
        extra: std::collections::BTreeMap::from([("opaque".into(), json!([1, 2, 3]))]),
    };
    f.sim.state().log.clear();
    let fitted = f.dev.fitted_dpi_stages(&stages).unwrap();
    assert!(f.sim.state().log.is_empty(), "fit must be pure");
    assert_eq!(fitted.stages, vec![(800, 900), (1600, 1700)]);
    assert_eq!(
        set_dpi_stages(&f.dev, &mut f.store, &stages).unwrap(),
        fitted
    );
    assert_eq!(f.dev.dpi_stages().unwrap().stages, fitted.stages);
    let saved = f.store.for_device(&f.dev).active();
    assert_eq!(saved.dpi, Some((1600, 1700)));
    assert_eq!(saved.dpi_stages, Some(fitted));
    assert_eq!(saved.extra["future"], json!({"retain":[1,true]}));
}

struct FailSets {
    sim: Sim,
    path: PathBuf,
    calls: Arc<Mutex<usize>>,
    fail_at: usize,
    require_recovery: bool,
}
impl Transport for FailSets {
    fn send_feature_report(&self, data: &[u8]) -> Result<()> {
        if data[8] & 0x80 == 0 {
            let mut count = self.calls.lock().unwrap();
            *count += 1;
            let saved = Store::load(&self.path).expect("recovery must already be on disk");
            if self.require_recovery {
                assert!(
                    saved.devices.values().any(|p| p.onboard.is_some()),
                    "missing recovery before SET"
                );
            }
            if *count == self.fail_at {
                return Err(crate::Error::System("injected SET failure".into()));
            }
        }
        self.sim.send_feature_report(data)
    }
    fn get_feature_report(&self, bytes: &mut [u8]) -> Result<usize> {
        self.sim.get_feature_report(bytes)
    }
    fn read_delay(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }
}
fn failure_fixture(fail_at: usize) -> (Fixture, Arc<Mutex<usize>>) {
    let mut f = Fixture::mouse();
    let calls = Arc::new(Mutex::new(0));
    let transport = FailSets {
        sim: f.sim.clone(),
        path: f.path(),
        calls: calls.clone(),
        fail_at,
        require_recovery: true,
    };
    f.dev = Device::open(
        Box::new(transport),
        crate::sim::MOUSE_PID,
        0,
        std::ffi::CString::new("sim:recovery").unwrap(),
        &crate::device::TXID_CANDIDATES,
        Sim::spec_for(crate::sim::MOUSE_PID),
    )
    .unwrap();
    f.store
        .for_device(&f.dev)
        .active_mut()
        .buttons
        .insert("side-forward".into(), "right".into());
    (f, calls)
}

#[test]
fn onboard_failure_retains_durably_saved_originals_for_recovery() {
    let (mut f, calls) = failure_fixture(1);
    assert!(save_buttons_to_mouse(&f.dev, &mut f.store).is_err());
    assert_eq!(*calls.lock().unwrap(), 1);
    let memory = f.store.for_device(&f.dev).onboard.as_ref().unwrap().clone();
    assert!(memory.before.contains_key("side-forward"));
    assert!(
        memory.buttons.is_empty(),
        "final success metadata must not be fabricated"
    );
    let mut disk = Store::load(&f.path()).unwrap();
    assert_eq!(disk.for_device(&f.dev).onboard, Some(memory));
}

#[test]
fn onboard_recovery_growth_and_backup_io_failure_both_block_set() {
    let (mut f, calls) = failure_fixture(usize::MAX);
    fill_bytes(&mut f.store);
    let before = f.prepare();
    assert!(save_buttons_to_mouse(&f.dev, &mut f.store).is_err());
    assert_eq!(*calls.lock().unwrap(), 0);
    f.refused_unchanged(&before);

    let (mut f, calls) = failure_fixture(usize::MAX);
    // The prospective backup fits, but its parent is a file. SET must wait for
    // successful durable recovery persistence, not merely serialization.
    std::fs::write(&f.folder, b"not a directory").unwrap();
    let before = serde_json::to_value(&f.store).unwrap();
    assert!(save_buttons_to_mouse(&f.dev, &mut f.store).is_err());
    assert_eq!(*calls.lock().unwrap(), 0);
    assert_eq!(serde_json::to_value(&f.store).unwrap(), before);
    std::fs::remove_file(&f.folder).unwrap();
}

#[test]
fn sync_partial_state_growth_is_bounded_before_any_device_set() {
    for by_nodes in [false, true] {
        let mut f = Fixture::mouse();
        let other_sim = Sim::mouse();
        let mut other = other_sim
            .open(crate::sim::MOUSE_PID, Sim::spec_for(crate::sim::MOUSE_PID))
            .unwrap();
        other.serial = "second-synthetic".into();
        f.store.for_device(&other);
        // Full success frees the old frame, but a failure on this device and
        // successful color changes on the other would retain it plus growth.
        f.store.for_device(&f.dev).active_mut().keys = vec![Rgb(10, 20, 30); 1000];
        if by_nodes {
            fill_nodes(&mut f.store);
        } else {
            fill_bytes(&mut f.store);
        }
        let before = f.prepare();
        other_sim.state().log.clear();
        let first = f
            .sim
            .open(crate::sim::MOUSE_PID, Sim::spec_for(crate::sim::MOUSE_PID))
            .unwrap();
        let devices = vec![first, other];
        f.sim.state().log.clear();
        let results = sync_color(&devices, &mut f.store, Rgb(1, 2, 3));
        assert!(!results.is_empty());
        assert!(results.iter().all(|r| r.result.is_err()));
        assert!(f.sim.state().log.is_empty());
        assert!(other_sim.state().log.is_empty());
        assert_eq!(serde_json::to_value(&f.store).unwrap(), before.0);
        assert_eq!(std::fs::read(f.path()).unwrap(), before.1);
    }
}

#[test]
fn sync_commits_only_successful_zones_and_keeps_failed_device_alias() {
    let mut f = Fixture::mouse();
    let other_sim = Sim::mouse();
    let mut other = other_sim
        .open(crate::sim::MOUSE_PID, Sim::spec_for(crate::sim::MOUSE_PID))
        .unwrap();
    other.serial = "second-synthetic".into();
    let mut profiles = crate::profile::DeviceProfiles::default();
    profiles
        .active_mut()
        .extra
        .insert("future".into(), json!({"stay":true}));
    // A distinct legacy key is resolved for preflight, but must not migrate
    // merely because another device was successfully written.
    let legacy = format!("{:04X}", other.pid);
    f.store.devices.insert(legacy.clone(), profiles);
    other.enforce_query_only();
    f.prepare();
    let own_key = f.dev.key();
    let first = f
        .sim
        .open(crate::sim::MOUSE_PID, Sim::spec_for(crate::sim::MOUSE_PID))
        .unwrap();
    let devices = vec![first, other];
    f.sim.state().log.clear();
    let results = sync_color(&devices, &mut f.store, Rgb(4, 5, 6));
    assert_eq!(results.iter().filter(|r| r.result.is_ok()).count(), 3);
    assert_eq!(results.iter().filter(|r| r.result.is_err()).count(), 3);
    assert_eq!(
        f.store.devices[&legacy].active().extra["future"],
        json!({"stay":true})
    );
    assert!(f.store.devices[&legacy].active().lighting.is_empty());
    for zone in f.store.devices[&own_key].active().lighting.values() {
        assert_eq!(zone.color, Some(Rgb(4, 5, 6)));
    }
    let disk = Store::load(&f.path()).unwrap();
    assert_eq!(
        serde_json::to_value(&disk).unwrap(),
        serde_json::to_value(&f.store).unwrap()
    );
}

#[test]
fn partial_onboard_restore_keeps_entire_recovery_record() {
    let (mut f, calls) = failure_fixture(2);
    let direct = f
        .sim
        .open(crate::sim::MOUSE_PID, Sim::spec_for(crate::sim::MOUSE_PID))
        .unwrap();
    for id in [4, 5] {
        direct
            .set_slot_input(
                1,
                crate::buttons::Input::Button(id),
                crate::buttons::Layer::Normal,
                crate::buttons::Action::Mouse(2),
            )
            .unwrap();
    }
    let backup = crate::profile::OnboardSave {
        slot: 1,
        profile: "Default".into(),
        before: std::collections::BTreeMap::from([
            ("side-back".into(), "back".into()),
            ("side-forward".into(), "forward".into()),
        ]),
        extra: std::collections::BTreeMap::from([("opaque".into(), json!({"retain":true}))]),
        ..Default::default()
    };
    f.store.for_device(&f.dev).onboard = Some(backup.clone());
    assert!(put_back_onboard(&f.dev, &mut f.store).is_err());
    assert_eq!(*calls.lock().unwrap(), 2);
    assert_eq!(f.store.for_device(&f.dev).onboard, Some(backup.clone()));
    let mut disk = Store::load(&f.path()).unwrap();
    assert_eq!(disk.for_device(&f.dev).onboard, Some(backup));
}

#[test]
fn partial_live_restore_removes_only_successful_remaps() {
    let mut f = Fixture::mouse();
    for id in [4, 5] {
        f.dev
            .set_input(
                crate::buttons::Input::Button(id),
                crate::buttons::Layer::Normal,
                crate::buttons::Action::Mouse(2),
            )
            .unwrap();
    }
    f.store.for_device(&f.dev).active_mut().buttons = std::collections::BTreeMap::from([
        ("side-back".into(), "right".into()),
        ("side-forward".into(), "right".into()),
    ]);
    f.prepare();
    let calls = Arc::new(Mutex::new(0));
    let transport = FailSets {
        sim: f.sim.clone(),
        path: f.path(),
        calls: calls.clone(),
        fail_at: 2,
        require_recovery: false,
    };
    f.dev = Device::open(
        Box::new(transport),
        crate::sim::MOUSE_PID,
        0,
        std::ffi::CString::new("sim:partial").unwrap(),
        &crate::device::TXID_CANDIDATES,
        Sim::spec_for(crate::sim::MOUSE_PID),
    )
    .unwrap();
    assert_eq!(restore_all_buttons(&f.dev, &mut f.store).unwrap(), 1);
    assert_eq!(*calls.lock().unwrap(), 2);
    let profile = f.store.for_device(&f.dev).active();
    assert_eq!(
        profile.buttons,
        std::collections::BTreeMap::from([("side-forward".into(), "right".into())])
    );
    assert_eq!(profile.extra["future"], json!({"retain":[1,true]}));
    let disk = Store::load(&f.path()).unwrap();
    assert_eq!(
        serde_json::to_value(disk).unwrap(),
        serde_json::to_value(&f.store).unwrap()
    );
}

#[test]
fn power_keyboard_and_frame_candidates_also_preflight_the_complete_store() {
    type Edit = fn(&Device, &mut Store) -> Result<()>;
    let mouse_edits: &[Edit] = &[
        |d, s| set_sleep_minutes(d, s, 5),
        |d, s| set_auto_sleep(d, s, true),
        |d, s| set_dimming(d, s, "half".parse().unwrap()),
    ];
    for edit in mouse_edits {
        let mut f = Fixture::mouse();
        fill_bytes(&mut f.store);
        let before = f.prepare();
        assert!(edit(&f.dev, &mut f.store)
            .unwrap_err()
            .to_string()
            .contains("budget"));
        f.refused_unchanged(&before);
    }
    for frame in [false, true] {
        let sim = Sim::keyboard();
        let dev = sim
            .open(
                crate::sim::KEYBOARD_PID,
                Sim::spec_for(crate::sim::KEYBOARD_PID),
            )
            .unwrap();
        let mut f = Fixture::new(sim, dev);
        fill_nodes(&mut f.store);
        let before = f.prepare();
        let result = if frame {
            set_keys(&f.dev, &mut f.store, vec![Rgb(3, 4, 5); 132])
        } else {
            set_game_mode(&f.dev, &mut f.store, "win".parse().unwrap())
        };
        assert!(result.unwrap_err().to_string().contains("budget"));
        f.refused_unchanged(&before);
    }
}

#[test]
fn new_slot_backup_growth_is_refused_before_restoring_the_old_slot() {
    let (mut f, calls) = failure_fixture(usize::MAX);
    {
        let mut state = f.sim.state();
        let saved = state.buttons.clone();
        for ((slot, input, layer), value) in saved {
            if slot == 1 {
                state.buttons.insert((2, input, layer), value);
            }
        }
        state.active_slot = Some(2);
    }
    f.store.for_device(&f.dev).onboard = Some(crate::profile::OnboardSave {
        slot: 1,
        profile: "Old".into(),
        before: std::collections::BTreeMap::from([("side-back".into(), "back".into())]),
        ..Default::default()
    });
    fill_bytes(&mut f.store);
    let before = f.prepare();
    let error = save_buttons_to_mouse(&f.dev, &mut f.store).unwrap_err();
    assert!(error.to_string().contains("budget"), "{error}");
    assert_eq!(
        *calls.lock().unwrap(),
        0,
        "old-slot restoration must wait for every later candidate's bound"
    );
    f.refused_unchanged(&before);
}
