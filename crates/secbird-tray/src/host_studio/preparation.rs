//! Fresh identity GET plus bounded source authentication. Never device output.
use super::PreparedStudio;
use secbird_core::{
    ornata_identity::OrnataEditionReadback, ornata_manifest::load_manifest,
    studio::document::ReadSnapshot, Device, Error, Result,
};
use serde_json::{json, Value};
use std::{path::Path, time::Instant};

pub(crate) struct ReadyStudio {
    prepared: PreparedStudio,
    target_key: String,
    target_path: Vec<u8>,
    observed: OrnataEditionReadback,
    started: Instant,
    finished: Instant,
    digest: [u8; 32],
    source_byte_len: usize,
}
impl ReadyStudio {
    pub(crate) fn prepared(&self) -> &PreparedStudio {
        &self.prepared
    }
    pub(crate) fn prepared_mut(&mut self) -> &mut PreparedStudio {
        &mut self.prepared
    }
    /// Identity binding only; not a fresh control/owner/connection-generation check.
    pub(crate) fn matches_target(&self, device: &Device) -> bool {
        device.pid == 605
            && device.interface == 2
            && device.key() == self.target_key
            && device.path.as_bytes() == self.target_path
    }
    pub(crate) fn observation_started_at(&self) -> Instant {
        self.started
    }
    pub(crate) fn metadata_at(&self, now: Instant) -> Value {
        json!({"layout_id":self.observed.layout_id(),"edition_id":self.observed.edition_id(),
            "firmware_id":self.observed.firmware_id(),"identity_declared_size":self.observed.declared_size(),
            "identity_query_span_us":self.finished.duration_since(self.started).as_micros(),
            "identity_observation_age_us":now.checked_duration_since(self.started).map(|age|age.as_micros()),
            "identity_query_completed_by_observation":now>=self.finished,
            "authenticated_manifest_sha256":self.digest.iter().map(|b|format!("{b:02x}")).collect::<String>(),
            "source_byte_len":self.source_byte_len,
            "physical_rows":6,"physical_cols":22,"logical_rows":7,"logical_cols":23,
            "preparation_performed_output":false,"identity_is_ownership_lease":false,
            "identity_is_connection_generation":false,"frame_interval_ms":40})
    }
}

pub(crate) fn prepare(
    device: &Device,
    snapshot: ReadSnapshot,
    manifest: &Path,
    random_state: u32,
) -> Result<ReadyStudio> {
    if device.pid != 605
        || device.interface != 2
        || !device
            .spec
            .as_ref()
            .and_then(|spec| spec.matrix)
            .is_some_and(|matrix| matrix.rows == 6 && matrix.cols == 22)
    {
        return Err(Error::Unsupported(
            "Studio physical preparation requires Ornata025D/interface2 physical6x22".into(),
        ));
    }
    let started = Instant::now();
    let observed = device.ornata_edition_readback()?;
    let finished = Instant::now();
    let layout = load_manifest(manifest, observed.layout_identity()).map_err(|error| {
        Error::InvalidArg(format!("Studio layout authentication failed: {error}"))
    })?;
    let digest = layout.authenticated_sha256();
    let source_byte_len = layout.source_byte_len();
    let prepared = PreparedStudio::new(snapshot, &layout, random_state)?;
    Ok(ReadyStudio {
        prepared,
        target_key: device.key(),
        target_path: device.path.as_bytes().to_vec(),
        observed,
        started,
        finished,
        digest,
        source_byte_len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use secbird_core::{
        proto::Command,
        sim::Sim,
        studio::{document::Draft, Archive},
        Db,
    };
    use std::{ffi::CString, path::PathBuf};
    const PROFILE: &str =
        include_str!("../../../secbird-core/tests/fixtures/studio-preview-profile.json");
    fn device() -> (Device, Sim) {
        let sim = Sim::keyboard();
        sim.state().ornata_layout_edition = Some([6, 0]);
        let mut device = sim
            .open(605, Db::builtin().unwrap().get(605).cloned())
            .unwrap();
        device.interface = 2;
        device.enforce_query_only();
        sim.state().log.clear();
        (device, sim)
    }
    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "studio-output-asset-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[test]
    fn wrong_scope_is_refused_before_get_or_asset_loading() {
        let draft = Draft::open(PROFILE).unwrap();
        let absent = path();
        let (mut target, sim) = device();
        target.interface = 1;
        assert!(prepare(&target, draft.read_snapshot(), &absent, 1).is_err());
        target.interface = 2;
        target.pid = 196;
        assert!(prepare(&target, draft.read_snapshot(), &absent, 1).is_err());
        target.pid = 605;
        target.spec = None;
        assert!(prepare(&target, draft.read_snapshot(), &absent, 1).is_err());
        assert!(sim.state().log.is_empty());
    }
    #[test]
    fn every_preparation_queries_identity_and_refuses_missing_or_changed_assets_without_output() {
        let draft = Draft::open(PROFILE).unwrap();
        let asset = path();
        let (target, sim) = device();
        assert!(prepare(&target, draft.read_snapshot(), &asset, 1).is_err());
        std::fs::write(&asset, b"{}").unwrap();
        let result = prepare(&target, draft.read_snapshot(), &asset, 1);
        assert!(result.err().unwrap().to_string().contains("SHA256"));
        assert_eq!(
            sim.state().log,
            [Command::new(0, 0x86), Command::new(0, 0x86)]
        );
        assert_eq!(sim.state().device_mode, Some(0));
        assert!(sim
            .state()
            .matrix
            .as_ref()
            .unwrap()
            .2
            .iter()
            .all(|pixel| *pixel == [0, 0, 0]));
        std::fs::remove_file(asset).unwrap();
    }
    #[test]
    #[ignore = "requires explicit pinned private manifest and actual exported composition; MOCK identity only"]
    fn actual_export_and_authenticated_asset_prepare_frames_with_mock_identity_only() {
        let manifest = PathBuf::from(
            std::env::var_os("SECBIRD_ORNATA_MANIFEST_TEST_PATH").expect("select pinned asset"),
        );
        let input = PathBuf::from(
            std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select actual export"),
        );
        let bytes = std::fs::read(input).unwrap();
        let archive = Archive::parse(&bytes).unwrap();
        let draft = Draft::open(archive.profile_json(0).unwrap()).unwrap();
        let (mut target, sim) = device();
        let start = Instant::now();
        let mut ready = prepare(&target, draft.read_snapshot(), &manifest, 1).unwrap();
        assert!(ready.observation_started_at() >= start);
        assert!(ready.matches_target(&target));
        let metadata = ready.metadata_at(Instant::now());
        assert_eq!(metadata["layout_id"], 6);
        assert_eq!(metadata["edition_id"], 0);
        assert_eq!(metadata["identity_declared_size"], 2);
        assert!(metadata["firmware_id"].is_null());
        assert_eq!(metadata["preparation_performed_output"], false);
        assert_eq!(metadata["identity_is_ownership_lease"], false);
        assert_eq!(
            metadata["authenticated_manifest_sha256"],
            super::super::SOURCE_MANIFEST_SHA256
        );
        assert_eq!(metadata["source_byte_len"], 2_895_966);
        let packets = ready
            .prepared()
            .current_frame()
            .unwrap()
            .packets(target.txid);
        ready.prepared_mut().advance().unwrap();
        assert_ne!(
            packets,
            ready
                .prepared()
                .current_frame()
                .unwrap()
                .packets(target.txid)
        );
        target.serial.push('x');
        assert!(!ready.matches_target(&target));
        target.serial.pop();
        target.path = CString::new("sim:another-device").unwrap();
        assert!(!ready.matches_target(&target));
        assert_eq!(sim.state().log, [Command::new(0, 0x86)]);
        assert!(!draft.is_dirty());
        eprintln!("Actual Studio physical preparation: {metadata}");
    }
}
