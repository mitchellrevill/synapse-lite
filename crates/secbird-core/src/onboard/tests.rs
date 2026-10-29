use super::*;
use crate::{
    proto::{checksum, Report, REPORT_LEN},
    transport::Transport,
};
use std::{
    ffi::CString,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone)]
struct Mock(Arc<Mutex<MockState>>);
struct MockState {
    settings: Settings,
    reply: Option<Report>,
    packets: Vec<Report>,
    fail_set: usize,
    sets: usize,
    ignore_set: bool,
    corrupt_slot: bool,
    block_save: Option<PathBuf>,
}
impl Mock {
    fn new() -> Self {
        let settings: Settings = serde_json::from_str(include_str!(
            "../../tests/fixtures/onboard-196-registers.json"
        ))
        .unwrap();
        Self(Arc::new(Mutex::new(MockState {
            settings,
            reply: None,
            packets: vec![],
            fail_set: 0,
            sets: 0,
            ignore_set: false,
            corrupt_slot: false,
            block_save: None,
        })))
    }
    fn open(&self, pid: u16) -> Device {
        Device::open(
            Box::new(self.clone()),
            pid,
            0,
            CString::new("onboard-simulated-only").unwrap(),
            &[0x1f],
            crate::Db::builtin().unwrap().get(pid).cloned(),
        )
        .unwrap()
    }
    fn sets(&self) -> usize {
        self.0.lock().unwrap().sets
    }
}
impl Transport for Mock {
    fn read_delay(&self) -> Duration {
        Duration::ZERO
    }
    fn send_feature_report(&self, data: &[u8]) -> Result<()> {
        assert_eq!(data.len(), 91);
        let mut bytes = [0; REPORT_LEN];
        bytes.copy_from_slice(&data[1..]);
        let req = Report(bytes);
        let mut m = self.0.lock().unwrap();
        m.packets.push(req.clone());
        let command = req.command();
        let size = req.0[5] as usize;
        let a = req.args();
        let mut payload;
        let status = 2;
        if command == cmd::SERIAL.get() {
            payload = b"OBMSIM196001".to_vec();
        } else if command.id & 0x80 != 0 {
            assert_eq!(a[0], 1, "no fallback to live/other slot");
            payload = match command {
                c if c == cmd::DPI.get() => {
                    assert_eq!(size, 7);
                    m.settings.dpi.clone()
                }
                c if c == cmd::DPI_STAGES.get() => {
                    assert_eq!(size, 80);
                    m.settings.stages.clone()
                }
                c if c == Command::new(0, 0x8e) => {
                    assert_eq!(size, 2);
                    m.settings.polling.clone()
                }
                _ => panic!("unexpected GET {command}"),
            };
            if m.corrupt_slot {
                payload[0] = 0;
            }
        } else {
            m.sets += 1;
            if m.sets == m.fail_set {
                return Err(Error::Unsupported("simulated disconnect during SET".into()));
            }
            assert_eq!(a[0], 1);
            if !m.ignore_set {
                match command {
                    cmd::DPI => {
                        assert_eq!(size, 7);
                        m.settings.dpi = a[..7].to_vec();
                    }
                    cmd::DPI_STAGES => {
                        let n = a[2] as usize;
                        assert_eq!(size, 3 + 7 * n);
                        for i in 0..n {
                            assert_eq!(a[3 + 7 * i], i as u8);
                        }
                        m.settings.stages[..size].copy_from_slice(&a[..size]);
                        for i in 0..n {
                            m.settings.stages[3 + 7 * i] += 1;
                        }
                        let index = 3 + 7 * usize::from(a[1] - 1);
                        let pair = a[index + 1..index + 5].to_vec();
                        m.settings.dpi[1..5].copy_from_slice(&pair);
                    }
                    c if c == Command::new(0, 0x0e) => {
                        assert_eq!(size, 2);
                        m.settings.polling = a[..2].to_vec();
                    }
                    _ => panic!("unverified SET {command}"),
                }
            }
            if let Some(path) = m.block_save.as_ref().filter(|_| m.sets % 3 == 0) {
                std::fs::create_dir(path).unwrap();
            }
            payload = a[..size].to_vec();
        }
        let mut reply = Report::request(req.txid(), command, req.0[5], &payload);
        reply.0[0] = status;
        reply.0[88] = checksum(&reply.0);
        m.reply = Some(reply);
        Ok(())
    }
    fn get_feature_report(&self, buf: &mut [u8]) -> Result<usize> {
        let m = self.0.lock().unwrap();
        buf[0] = 0;
        buf[1..91].copy_from_slice(&m.reply.as_ref().unwrap().0);
        Ok(91)
    }
}
struct Fixture {
    mock: Mock,
    dev: Device,
    store: Store,
    dir: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "onboard196-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mock = Mock::new();
        let dev = mock.open(196);
        let mut store = Store::load(&dir.join("profiles.json")).unwrap();
        let p = store.for_device(&dev).active_mut();
        p.dpi = Some((1600, 1800));
        p.polling = Some(500);
        p.extra
            .insert("sourceOpaque".into(), serde_json::json!({"keep":true}));
        store.save().unwrap();
        Self {
            mock,
            dev,
            store,
            dir,
        }
    }
    fn save(&mut self) -> Result<Settings> {
        let id = self.dev.key();
        save(&self.dev, &mut self.store, &id, "Default")
    }
    fn restore(&mut self) -> Result<Settings> {
        let id = self.dev.key();
        restore(&self.dev, &mut self.store, &id, "Default")
    }
    fn disk(&self) -> Store {
        Store::load(&self.dir.join("profiles.json")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
#[test]
fn source_vectors_roundtrip_and_preserve_raw_originals() {
    let mut f = Fixture::new();
    let original = read(&f.dev).unwrap();
    let before = serde_json::to_value(f.store.for_device(&f.dev).active()).unwrap();
    let saved = f.save().unwrap();
    assert_eq!(saved.dpi_pair(), (1600, 1800));
    assert_eq!(saved.polling, [1, 2]);
    assert_eq!(
        f.store
            .for_device(&f.dev)
            .onboard_settings
            .as_ref()
            .unwrap()
            .phase,
        Phase::Applied
    );
    assert_eq!(f.restore().unwrap(), original);
    assert_eq!(read(&f.dev).unwrap(), original);
    assert!(f.store.for_device(&f.dev).onboard_settings.is_none());
    assert_eq!(
        serde_json::to_value(f.store.for_device(&f.dev).active()).unwrap(),
        before
    );
}
#[test]
fn each_partial_save_and_restore_retains_complete_durable_recovery() {
    for fail in 1..=3 {
        let mut f = Fixture::new();
        let original = read(&f.dev).unwrap();
        f.mock.0.lock().unwrap().fail_set = fail;
        assert!(f.save().is_err());
        let mut disk = f.disk();
        assert_eq!(
            disk.for_device(&f.dev)
                .onboard_settings
                .as_ref()
                .unwrap()
                .before,
            original
        );
        assert_eq!(
            disk.for_device(&f.dev)
                .onboard_settings
                .as_ref()
                .unwrap()
                .phase,
            Phase::Saving
        );
        f.store = disk;
        let count = f.mock.sets();
        assert!(f.save().is_err());
        assert_eq!(f.mock.sets(), count);
        f.mock.0.lock().unwrap().fail_set = count + fail;
        assert!(f.restore().is_err());
        assert_eq!(
            f.disk()
                .for_device(&f.dev)
                .onboard_settings
                .as_ref()
                .unwrap()
                .before,
            original
        );
        f.mock.0.lock().unwrap().fail_set = 0;
        f.store = f.disk();
        f.restore().unwrap();
        assert_eq!(read(&f.dev).unwrap(), original);
    }
}
#[test]
fn reconnect_wired_to_receiver_uses_same_serial_and_originals() {
    let mut f = Fixture::new();
    let original = read(&f.dev).unwrap();
    f.save().unwrap();
    let receiver = f.mock.open(197);
    let mut disk = f.disk();
    restore(&receiver, &mut disk, &receiver.key(), "Default").unwrap();
    assert_eq!(read(&receiver).unwrap(), original);
    assert_eq!(disk.devices.len(), 1);
    assert!(disk.devices.contains_key(&receiver.key()));
}
#[test]
fn repeat_save_never_replaces_first_originals() {
    let mut f = Fixture::new();
    let original = read(&f.dev).unwrap();
    f.save().unwrap();
    f.store.for_device(&f.dev).active_mut().dpi = Some((2000, 2200));
    f.save().unwrap();
    assert_eq!(
        f.store
            .for_device(&f.dev)
            .onboard_settings
            .as_ref()
            .unwrap()
            .before,
        original
    );
    f.restore().unwrap();
    assert_eq!(read(&f.dev).unwrap(), original);
}
#[test]
fn acknowledgement_without_readback_is_failure_and_keeps_journal() {
    let mut f = Fixture::new();
    f.mock.0.lock().unwrap().ignore_set = true;
    assert!(f.save().is_err());
    assert_eq!(
        f.disk()
            .for_device(&f.dev)
            .onboard_settings
            .as_ref()
            .unwrap()
            .phase,
        Phase::Saving
    );
}
#[test]
fn refuse_wrong_device_profile_selector_and_unverified_families_before_set() {
    let mut f = Fixture::new();
    let key = f.dev.key();
    assert!(save(&f.dev, &mut f.store, "another device", "Default").is_err());
    assert!(save(&f.dev, &mut f.store, &key, "Changed").is_err());
    f.mock.0.lock().unwrap().corrupt_slot = true;
    assert!(f.save().is_err());
    for pid in [605, 153, 0xb3] {
        let device = f.mock.open(pid);
        assert_eq!(capabilities(&device).slots, 0);
        assert!(read(&device).is_err());
    }
    assert_eq!(f.mock.sets(), 0);
}
#[test]
fn budget_and_io_errors_precede_first_set() {
    let mut f = Fixture::new();
    f.store
        .extra
        .insert("padding".into(), serde_json::json!(""));
    let overhead = f.store.native_bytes().unwrap().len();
    f.store.extra.insert(
        "padding".into(),
        serde_json::json!("x".repeat(2 * 1024 * 1024 - overhead)),
    );
    f.store.validate_save().unwrap();
    assert!(f.save().is_err());
    assert_eq!(f.mock.sets(), 0);
    let mut f = Fixture::new();
    std::fs::remove_file(f.dir.join("profiles.json")).unwrap();
    std::fs::create_dir(f.dir.join("profiles.json")).unwrap();
    assert!(f.save().is_err());
    assert_eq!(f.mock.sets(), 0);
}
#[test]
fn malformed_imported_recovery_never_replays_and_unknown_fields_survive() {
    let mut f = Fixture::new();
    f.save().unwrap();
    f.store
        .for_device(&f.dev)
        .onboard_settings
        .as_mut()
        .unwrap()
        .extra
        .insert("future".into(), serde_json::json!({"opaque":[1,2]}));
    f.store.save().unwrap();
    f.store = f.disk();
    assert!(f
        .store
        .for_device(&f.dev)
        .onboard_settings
        .as_ref()
        .unwrap()
        .extra
        .contains_key("future"));
    let count = f.mock.sets();
    f.store
        .for_device(&f.dev)
        .onboard_settings
        .as_mut()
        .unwrap()
        .before
        .stages
        .truncate(3);
    assert!(f.restore().is_err());
    assert_eq!(f.mock.sets(), count);
}
#[test]
fn physical_serial_mismatch_blocks_imported_originals() {
    let mut f = Fixture::new();
    f.save().unwrap();
    let count = f.mock.sets();
    f.store
        .for_device(&f.dev)
        .onboard_settings
        .as_mut()
        .unwrap()
        .serial = "ANOTHERUNIT".into();
    assert!(f.restore().is_err());
    assert_eq!(f.mock.sets(), count);
}
#[test]
fn empty_or_unsupported_intent_is_not_a_save() {
    let mut f = Fixture::new();
    f.store.for_device(&f.dev).active_mut().dpi = None;
    f.store.for_device(&f.dev).active_mut().polling = None;
    assert!(f.save().is_err());
    assert_eq!(f.mock.sets(), 0);
    f.store.for_device(&f.dev).active_mut().polling = Some(8000);
    assert!(f.save().is_err());
    assert_eq!(f.mock.sets(), 0);
}

#[test]
fn completion_file_failure_keeps_pending_originals_after_hardware_success() {
    let mut f = Fixture::new();
    let original = read(&f.dev).unwrap();
    let temporary = f.dir.join("profiles.json.tmp");
    f.mock.0.lock().unwrap().block_save = Some(temporary.clone());
    assert!(f.save().is_err());
    assert_eq!(f.mock.sets(), 3);
    assert_eq!(
        f.disk()
            .for_device(&f.dev)
            .onboard_settings
            .as_ref()
            .unwrap()
            .phase,
        Phase::Saving
    );
    assert_eq!(
        f.disk()
            .for_device(&f.dev)
            .onboard_settings
            .as_ref()
            .unwrap()
            .before,
        original
    );
    std::fs::remove_dir(&temporary).unwrap();
    f.mock.0.lock().unwrap().block_save = None;
    f.restore().unwrap();
    assert_eq!(read(&f.dev).unwrap(), original);
}
#[test]
fn imported_source_profile_preserves_opaque_payload_and_active_selection() {
    let mut f = Fixture::new();
    let imported = crate::profile_compat::import_document(include_bytes!(
        "../../tests/fixtures/synthetic.synapse4"
    ))
    .unwrap();
    let key = f.dev.key();
    f.store.import_for(imported.store, &[key.clone()]);
    assert_eq!(f.store.for_device(&f.dev).active, "Default");
    let before = f.store.for_device(&f.dev).profiles["Fixture Mouse"].clone();
    f.store.for_device(&f.dev).switch("Fixture Mouse").unwrap();
    let saved = save(&f.dev, &mut f.store, &key, "Fixture Mouse").unwrap();
    assert_eq!(saved.dpi_pair(), (800, 1600));
    assert_eq!(f.store.for_device(&f.dev).profiles["Fixture Mouse"], before);
    let backup = crate::profile_compat::import_document(&f.store.native_bytes().unwrap()).unwrap();
    assert_eq!(
        backup.store.devices[&key].onboard_settings,
        f.store.for_device(&f.dev).onboard_settings
    );
    let mut incoming = backup.store;
    incoming
        .devices
        .get_mut(&key)
        .unwrap()
        .onboard_settings
        .as_mut()
        .unwrap()
        .serial = "DIFFERENT".into();
    let originals = f.store.for_device(&f.dev).onboard_settings.clone();
    f.store.import(incoming);
    assert_eq!(f.store.for_device(&f.dev).onboard_settings, originals);
    assert!(f
        .store
        .for_device(&f.dev)
        .extra
        .contains_key("_synapseLiteImported:onboard_settings"));
}
#[test]
fn query_only_handle_refuses_before_storage_or_set() {
    let mut f = Fixture::new();
    f.dev.enforce_query_only();
    let before = std::fs::read(f.dir.join("profiles.json")).unwrap();
    assert!(f.save().is_err());
    assert_eq!(f.mock.sets(), 0);
    assert_eq!(std::fs::read(f.dir.join("profiles.json")).unwrap(), before);
    assert!(read(&f.dev).is_ok());
}

#[test]
fn polling_only_save_preserves_independent_active_dpi_and_stage_table() {
    let mut f = Fixture::new();
    f.store.for_device(&f.dev).active_mut().dpi = None;
    f.mock.0.lock().unwrap().settings.dpi[1..5].copy_from_slice(&[6, 64, 7, 8]);
    let original = read(&f.dev).unwrap();
    let desired = f.save().unwrap();
    assert_eq!(desired.dpi, original.dpi);
    assert_eq!(desired.stages, original.stages);
    assert_eq!(f.mock.sets(), 1);
    assert_eq!(read(&f.dev).unwrap().dpi, original.dpi);
    f.restore().unwrap();
    assert_eq!(read(&f.dev).unwrap(), original);
}
