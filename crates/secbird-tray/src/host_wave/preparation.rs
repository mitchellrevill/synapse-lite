//! One fresh identity GET, then authentication of an explicit local manifest.
//! No ownership lease, mode acquisition, output, brightness, profile or Store I/O.
use super::{PreparedWave, Program};
use secbird_core::{
    ornata_identity::OrnataEditionReadback, ornata_manifest::load_manifest, Device, Error, Result,
};
use serde_json::{json, Value};
use std::{path::Path, time::Instant};

#[derive(Debug, Clone)]
pub(crate) struct ReadyWave {
    prepared: PreparedWave,
    observed: OrnataEditionReadback,
    observation_started: Instant,
    observation_finished: Instant,
    authenticated_sha256: [u8; 32],
    source_byte_len: usize,
}
impl ReadyWave {
    pub(crate) fn prepared(&self) -> &PreparedWave {
        &self.prepared
    }
    /// Conservative monotonic timestamp: before the identity query starts,
    /// not after asset authentication finishes. It is not a portable wall time.
    pub(crate) fn observation_started_at(&self) -> Instant {
        self.observation_started
    }
    /// Cached observation metadata only; this method performs no GET/file I/O.
    /// The caller supplies its observation time rather than creating a new
    /// hardware-freshness or ownership claim by reading this value.
    pub(crate) fn metadata_at(&self, now: Instant) -> Value {
        let digest: String = self
            .authenticated_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        json!({
            "source":"scoped GET00:86 followed by authenticated exact source manifest selection",
            "layout_id":self.observed.layout_id(),"edition_id":self.observed.edition_id(),
            "firmware_id":self.observed.firmware_id(),"identity_declared_size":self.observed.declared_size(),
            "identity_observation_age_us":now.checked_duration_since(self.observation_started).map(|age|age.as_micros()),
            "identity_query_span_us":self.observation_finished.duration_since(self.observation_started).as_micros(),
            "identity_query_completed_by_observation":now>=self.observation_finished,
            "authenticated_manifest_sha256":digest,"source_byte_len":self.source_byte_len,
            "identity_is_ownership_lease":false,"preparation_performed_device_output":false,
            "source_direction":self.prepared.program().source_direction(),
            "physical_rows":6,"physical_cols":22,"logical_rows":7,"logical_cols":23,
        })
    }
}

/// Each call queries the selected handle anew. No fallback/cache/locale lookup
/// exists. The path is used once by the bounded authenticated loader and is not
/// retained. Caller separately enforces DriverMode3, owner and lighting policies.
pub(crate) fn prepare(
    device: &Device,
    program: Program,
    manifest_path: &Path,
) -> Result<ReadyWave> {
    let observation_started = Instant::now();
    let observed = device.ornata_edition_readback()?;
    let observation_finished = Instant::now();
    let layout = load_manifest(manifest_path, observed.layout_identity())
        .map_err(|error| Error::InvalidArg(format!("Wave manifest validation failed: {error}")))?;
    let prepared = PreparedWave::new(program, &layout)?;
    Ok(ReadyWave {
        prepared,
        observed,
        observation_started,
        observation_finished,
        authenticated_sha256: layout.authenticated_sha256(),
        source_byte_len: layout.source_byte_len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use secbird_core::{
        proto::{Command, Report, Status},
        sim::Sim,
        Transport,
    };
    use std::{
        ffi::CString,
        path::PathBuf,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::Duration,
    };

    const EDITION: Command = Command::new(0, 0x86);
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct State {
        reply: Report,
        active: bool,
        commands: Vec<Command>,
        reads: usize,
    }
    struct Mock {
        sim: Sim,
        state: Arc<Mutex<State>>,
    }
    impl Transport for Mock {
        fn send_feature_report(&self, data: &[u8]) -> Result<()> {
            let command = Command::new(data[7], data[8]);
            assert_ne!(command.id & 0x80, 0, "preparation must never send SET");
            let mut state = self.state.lock().unwrap();
            state.commands.push(command);
            if command == EDITION {
                let mut expected = vec![0];
                expected.extend(Report::request(data[2], EDITION, 3, &[]).0);
                assert_eq!(
                    data, expected,
                    "source request must stay size3 with zero selectors"
                );
                state.active = true;
                Ok(())
            } else {
                drop(state);
                self.sim.send_feature_report(data)
            }
        }
        fn get_feature_report(&self, out: &mut [u8]) -> Result<usize> {
            let mut state = self.state.lock().unwrap();
            if state.active {
                state.reads += 1;
                out[0] = 0;
                out[1..91].copy_from_slice(&state.reply.0);
                Ok(91)
            } else {
                drop(state);
                self.sim.get_feature_report(out)
            }
        }
        fn read_delay(&self) -> Duration {
            Duration::ZERO
        }
    }
    fn fixture(size: u8, status: u8) -> (Device, Arc<Mutex<State>>) {
        let mut reply = Report::request(0x1f, EDITION, size, &[6, 0, 9]);
        reply.0[0] = status;
        let state = Arc::new(Mutex::new(State {
            reply,
            active: false,
            commands: Vec::new(),
            reads: 0,
        }));
        let sim = Sim::keyboard();
        sim.state().txid = 0x1f;
        let mut device = Device::open(
            Box::new(Mock {
                sim,
                state: state.clone(),
            }),
            605,
            2,
            CString::new("sim:private-wave").unwrap(),
            &[0x1f],
            None,
        )
        .unwrap();
        device.enforce_query_only();
        state.lock().unwrap().commands.clear();
        (device, state)
    }
    struct Asset(PathBuf);
    impl Asset {
        fn missing() -> Self {
            Self(std::env::temp_dir().join(format!(
                "wave-preparation-{}-{}.json",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Asset {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn wrong_scope_unsupported_and_truncated_identity_never_load_or_output() {
        let path = Asset::missing();
        for (pid, interface) in [(196, 2), (605, 0), (605, 1), (605, 3)] {
            let (mut device, state) = fixture(2, 2);
            device.pid = pid;
            device.interface = interface;
            assert!(matches!(
                prepare(&device, Program::Left, &path.0),
                Err(Error::Unsupported(_))
            ));
            assert!(state.lock().unwrap().commands.is_empty());
        }
        let (device, state) = fixture(0, 5);
        assert!(matches!(
            prepare(&device, Program::Left, &path.0),
            Err(Error::Device {
                status: Status::NotSupported,
                ..
            })
        ));
        assert_eq!(state.lock().unwrap().commands, vec![EDITION]);
        for size in [0, 1] {
            let (device, state) = fixture(size, 2);
            assert!(matches!(
                prepare(&device, Program::Right, &path.0),
                Err(Error::NoReply(_))
            ));
            assert_eq!(state.lock().unwrap().commands, vec![EDITION]);
            assert_eq!(state.lock().unwrap().reads, 30);
        }
    }
    #[test]
    fn missing_or_untrusted_asset_is_rejected_after_fresh_get_on_every_call() {
        let path = Asset::missing();
        let (device, state) = fixture(2, 2);
        assert!(matches!(
            prepare(&device, Program::Left, &path.0),
            Err(Error::InvalidArg(_))
        ));
        std::fs::write(&path.0, b"{}").unwrap();
        let error = prepare(&device, Program::Right, &path.0)
            .err()
            .expect("untrusted asset rejected");
        assert!(error.to_string().contains("SHA256"));
        assert_eq!(state.lock().unwrap().commands, vec![EDITION, EDITION]);
        assert_eq!(state.lock().unwrap().reads, 2);
    }

    #[test]
    #[ignore = "requires SECBIRD_ORNATA_MANIFEST_TEST_PATH selecting the exact pinned private asset; uses MOCK device GET only"]
    fn explicit_private_asset_and_mock_two_byte_get_prepare_the_exact_map() {
        let path = std::env::var_os("SECBIRD_ORNATA_MANIFEST_TEST_PATH")
            .expect("explicit pinned manifest path");
        let (device, state) = fixture(2, 2);
        let before = Instant::now();
        let ready = prepare(&device, Program::Left, Path::new(&path)).unwrap();
        let after = Instant::now();
        assert!(
            ready.observation_started_at() >= before && ready.observation_started_at() <= after
        );
        let metadata = ready.metadata_at(after);
        assert_eq!(metadata["layout_id"], 6);
        assert_eq!(metadata["edition_id"], 0);
        assert!(
            metadata["firmware_id"].is_null(),
            "padding9 is not a declared firmware ID"
        );
        assert_eq!(metadata["identity_declared_size"], 2);
        assert_eq!(metadata["source_byte_len"], 2_895_966);
        assert_eq!(
            metadata["authenticated_manifest_sha256"],
            secbird_core::ornata_spatial::SOURCE_MANIFEST_SHA256
        );
        assert_eq!(metadata["identity_is_ownership_lease"], false);
        assert_eq!(metadata["preparation_performed_device_output"], false);
        assert_eq!(metadata["identity_query_completed_by_observation"], true);
        assert!(
            metadata["identity_observation_age_us"].as_u64().unwrap()
                >= metadata["identity_query_span_us"].as_u64().unwrap()
        );
        assert!(!metadata
            .to_string()
            .contains(&Path::new(&path).to_string_lossy().to_string()));
        let frame = ready.prepared().render_frame(0).unwrap().packets(0x1f);
        let pixels: Vec<_> = frame[..6]
            .iter()
            .flat_map(|row| row[14..80].chunks_exact(3))
            .collect();
        assert_eq!(pixels.iter().filter(|rgb| ***rgb == [0, 0, 0]).count(), 18);
        assert_eq!(ready.prepared().identity().layout_id, 6);
        assert_eq!(state.lock().unwrap().commands, vec![EDITION]);
        assert_eq!(state.lock().unwrap().reads, 1);
        let shared = ready.clone();
        assert_eq!(
            shared.observation_started_at(),
            ready.observation_started_at()
        );
        assert_eq!(shared.metadata_at(after), metadata);
        assert_eq!(
            shared.prepared().render_frame(0).unwrap().packets(0x1f),
            frame
        );
        assert_eq!(
            state.lock().unwrap().commands,
            vec![EDITION],
            "clone/render/metadata must not requery"
        );
        let earlier = ready
            .observation_started_at()
            .checked_sub(Duration::from_secs(1))
            .unwrap();
        let invalid_time = ready.metadata_at(earlier);
        assert!(invalid_time["identity_observation_age_us"].is_null());
        assert_eq!(
            invalid_time["identity_query_completed_by_observation"],
            false
        );
    }
}
