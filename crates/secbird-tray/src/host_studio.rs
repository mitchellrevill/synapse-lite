//! Compile Studio RGB into an authenticated Ornata physical frame.
//!
//! No output, ownership acquisition, scheduling, mode change or persistence.
//! The live session must establish those separately before submitting a frame.
use secbird_core::{
    ornata_frame::PreparedOrnataFrame,
    ornata_manifest::AuthenticatedLayout,
    ornata_spatial::{
        LayoutIdentity, MappingEntry, OrnataSpatialMap, SourceLayoutRecord, MAX_SOURCE_ENTRIES,
        PHYSICAL_PIXELS, SOURCE_MANIFEST_SHA256,
    },
    studio::document::{Draft, Preview, ReadSnapshot},
    Error, Result, Rgb,
};
use serde::Deserialize;
use std::time::Duration;

pub(crate) mod preparation;

#[derive(Deserialize)]
struct Composition {
    devices: Vec<Placement>,
}
#[derive(Deserialize)]
struct Placement {
    id: String,
    #[serde(rename = "productId")]
    product: u16,
    #[serde(rename = "editionId")]
    edition: u16,
    layout: u16,
    config: Config,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Config {
    #[serde(rename = "VID")]
    vid: u16,
    #[serde(rename = "PID")]
    pid: u16,
    #[serde(rename = "EID")]
    eid: u16,
    layout: u16,
    matrix_max_row: u16,
    matrix_max_col: u16,
    device_max_row: u16,
    device_max_col: u16,
    led_matrix: Vec<Mapping>,
}
#[derive(Deserialize)]
struct Mapping {
    #[serde(rename = "MatrixPos")]
    logical: [u16; 2],
    #[serde(rename = "DevicePos")]
    physical: [u16; 2],
}

pub(crate) struct PreparedStudio {
    snapshot: ReadSnapshot,
    preview: Preview,
    map: OrnataSpatialMap,
}
impl PreparedStudio {
    /// Production admission requires the loader's opaque authenticated selection.
    /// A document's declared hash or USB product name cannot substitute for it.
    pub(crate) fn new(
        snapshot: ReadSnapshot,
        layout: &AuthenticatedLayout,
        random_state: u32,
    ) -> Result<Self> {
        Self::build(snapshot, layout.map().clone(), random_state)
    }

    fn build(
        snapshot: ReadSnapshot,
        authenticated: OrnataSpatialMap,
        random_state: u32,
    ) -> Result<Self> {
        let composition: Composition = serde_json::from_str(snapshot.json())
            .map_err(|error| Error::InvalidArg(format!("Studio frame placement: {error}")))?;
        let [placement]: [Placement; 1] = composition.devices.try_into().map_err(|_| {
            Error::Unsupported(
                "Studio physical frames currently require one Ornata placement".into(),
            )
        })?;
        let config = &placement.config;
        let expected = authenticated.provenance().identity;
        if (placement.product, placement.edition, placement.layout)
            != (expected.product_id, expected.edition_id, expected.layout_id)
            || (config.vid, config.pid, config.eid, config.layout)
                != (
                    expected.vendor_id,
                    expected.product_id,
                    expected.edition_id,
                    expected.layout_id,
                )
        {
            return Err(Error::Unsupported(
                "Studio placement and authenticated physical layout disagree".into(),
            ));
        }
        if config.led_matrix.is_empty() || config.led_matrix.len() > MAX_SOURCE_ENTRIES {
            return Err(Error::InvalidArg(
                "Studio frame mapping exceeds source entry bounds".into(),
            ));
        }
        let entries: Vec<_> = config
            .led_matrix
            .iter()
            .map(|entry| MappingEntry {
                logical: entry.logical,
                physical: entry.physical,
            })
            .collect();
        let declared = OrnataSpatialMap::from_record(
            expected,
            SourceLayoutRecord {
                identity: expected,
                logical_dimensions: [config.matrix_max_row, config.matrix_max_col],
                physical_dimensions: [config.device_max_row, config.device_max_col],
                entries: &entries,
                manifest_sha256: SOURCE_MANIFEST_SHA256,
            },
        )
        .map_err(|error| Error::InvalidArg(error.to_string()))?;
        // A structurally valid but different mapping is still a different layout.
        // Keep the authenticated table, not the document's declared projection.
        if declared.physical_to_logical() != authenticated.physical_to_logical()
            || declared.provenance().source_entry_count
                != authenticated.provenance().source_entry_count
        {
            return Err(Error::Unsupported(
                "Studio physical mapping differs from authenticated source layout".into(),
            ));
        }
        let preview = Preview::from_snapshot(&snapshot, random_state)?;
        if preview.device_id() != placement.id {
            return Err(Error::InvalidArg(
                "Studio frame placement identity changed during compilation".into(),
            ));
        }
        Ok(Self {
            snapshot,
            preview,
            map: authenticated,
        })
    }

    pub(crate) fn identity(&self) -> LayoutIdentity {
        self.map.provenance().identity
    }
    pub(crate) fn placement_id(&self) -> &str {
        self.preview.device_id()
    }
    pub(crate) fn matches_draft(&self, draft: &Draft) -> bool {
        draft.matches_snapshot(&self.snapshot)
    }
    pub(crate) fn frame_index(&self) -> u64 {
        self.preview.rendered_frames() - 1
    }
    pub(crate) fn frame_interval(&self) -> Duration {
        Duration::from_millis(40)
    }
    pub(crate) fn final_random_state(&self) -> u32 {
        self.preview.final_random_state()
    }
    pub(crate) fn current_frame(&self) -> Result<PreparedOrnataFrame> {
        let mut physical = [Rgb(0, 0, 0); PHYSICAL_PIXELS];
        self.map
            .map_frame_into(self.preview.frame(), &mut physical)
            .map_err(|error| Error::InvalidArg(error.to_string()))?;
        PreparedOrnataFrame::new(&physical)
    }
    /// Advancement is explicit; polling the same prepared frame never consumes phase.
    pub(crate) fn advance(&mut self) -> Result<()> {
        self.preview.advance()
    }
    pub(crate) fn storage_bytes(&self) -> usize {
        self.preview.storage_bytes()
            + self.snapshot.json().len()
            + std::mem::size_of::<OrnataSpatialMap>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{host_static::test_support, host_uniform::io::StreamState};
    use serde_json::{json, Value};
    use std::time::Instant;
    const PROFILE: &str =
        include_str!("../../secbird-core/tests/fixtures/studio-preview-profile.json");

    fn map() -> OrnataSpatialMap {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../secbird-core/tests/fixtures/ornata_spatial.json"
        ))
        .unwrap();
        let layout = fixture["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["layout"] == 6)
            .unwrap();
        let entries: Vec<_> = layout["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| MappingEntry {
                logical: [
                    entry[0].as_u64().unwrap() as u16,
                    entry[1].as_u64().unwrap() as u16,
                ],
                physical: [
                    entry[2].as_u64().unwrap() as u16,
                    entry[3].as_u64().unwrap() as u16,
                ],
            })
            .collect();
        let identity = LayoutIdentity {
            vendor_id: 5426,
            product_id: 605,
            edition_id: 0,
            layout_id: 6,
        };
        OrnataSpatialMap::from_record(
            identity,
            SourceLayoutRecord {
                identity,
                logical_dimensions: [7, 23],
                physical_dimensions: [6, 22],
                entries: &entries,
                manifest_sha256: SOURCE_MANIFEST_SHA256,
            },
        )
        .unwrap()
    }
    fn program(draft: &Draft) -> PreparedStudio {
        PreparedStudio::build(draft.read_snapshot(), map(), 1).unwrap()
    }
    fn rgb(frame: &PreparedOrnataFrame) -> Vec<u8> {
        let packets = frame.packets(0x1f);
        let mut rgb = Vec::with_capacity(396);
        for (row, packet) in packets[..6].iter().enumerate() {
            assert_eq!(&packet[9..14], &[0, 0, row as u8, 0, 21]);
            rgb.extend_from_slice(&packet[14..80]);
        }
        assert_eq!(&packets[6][9..15], &[0, 0, 8, 0, 0, 0]);
        rgb
    }

    #[test]
    fn five_hundred_physical_frames_match_independent_reference_and_existing_transfer_path() {
        let reference: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/studio-physical-reference.json"
        ))
        .unwrap();
        let draft = Draft::open(PROFILE).unwrap();
        let before = draft.json().to_string();
        let mut program = program(&draft);
        let mut digest = md5::Context::new();
        let (mut device, spy) = test_support::fixture();
        let mut stream = StreamState::default();
        let baseline = spy.sim.state().clone();
        assert_eq!(program.identity().layout_id, 6);
        assert_eq!(program.placement_id(), "keyboard");
        assert_eq!(program.frame_interval(), Duration::from_millis(40));
        assert!(program.storage_bytes() < 150000);
        assert_eq!(program.final_random_state(), 2609502873);
        for index in 0..500 {
            assert_eq!(program.frame_index(), index);
            let frame = program.current_frame().unwrap();
            let pixels = rgb(&frame);
            digest.consume(&pixels);
            // Repeated frame reads have no clock/phase side effect.
            assert_eq!(pixels, rgb(&program.current_frame().unwrap()));
            let submitted = stream.submit_frame(
                &mut device,
                &frame,
                Instant::now() + Duration::from_secs(2),
                || false,
            );
            assert!(
                submitted.completion.error.is_none(),
                "{:?}",
                submitted.completion
            );
            assert_eq!(
                (
                    submitted.completion.accepted_packets,
                    submitted.completion.attempted_packets
                ),
                (7, 7)
            );
            assert_eq!(
                spy.sim.state().matrix.as_ref().unwrap().2,
                pixels
                    .chunks_exact(3)
                    .map(|p| [p[0], p[1], p[2]])
                    .collect::<Vec<_>>()
            );
            program.advance().unwrap();
        }
        assert_eq!(
            format!("{:x}", digest.compute()),
            reference["physical_rgb_md5"].as_str().unwrap()
        );
        assert_eq!(spy.state.lock().unwrap().attempts, 3500);
        assert_eq!(spy.state.lock().unwrap().mode, 3);
        let after = spy.sim.state();
        assert_eq!(after.game_mode, baseline.game_mode);
        assert_eq!(after.brightness, baseline.brightness);
        assert_eq!(after.live_brightness, baseline.live_brightness);
        assert_eq!(after.macros, baseline.macros);
        assert_eq!(after.effects, baseline.effects);
        assert_eq!(draft.json(), before);
        assert!(!draft.is_dirty());
        assert!(program.matches_draft(&draft));
        assert!(spy
            .state
            .lock()
            .unwrap()
            .commands
            .iter()
            .all(
                |command| *command == secbird_core::proto::Command::new(0, 0x84)
                    || command.class == 15 && matches!(command.id, 2 | 3 | 0x82)
            ));
    }

    #[test]
    fn snapshot_binding_never_acknowledges_save_or_follows_new_draft_edits() {
        let mut draft = Draft::open(PROFILE).unwrap();
        let prepared = program(&draft);
        let before = rgb(&prepared.current_frame().unwrap());
        assert!(prepared.matches_draft(&draft));
        let other = Draft::open(PROFILE).unwrap();
        assert!(!prepared.matches_draft(&other));
        draft.rename_layer("fire", "Changed").unwrap();
        assert!(!prepared.matches_draft(&draft));
        assert_eq!(before, rgb(&prepared.current_frame().unwrap()));
        assert!(draft.is_dirty());
        assert_eq!(prepared.frame_index(), 0);
    }

    #[test]
    fn conflicting_physical_mapping_and_geometry_fail_before_frame_preparation() {
        let base: Value = serde_json::from_str(PROFILE).unwrap();
        let mut cases = Vec::new();
        for (field, value) in [
            ("DeviceMaxRow", json!(7)),
            ("DeviceMaxCol", json!(23)),
            ("Layout", json!(2)),
            ("EID", json!(128)),
        ] {
            let mut bad = base.clone();
            bad["devices"][0]["config"][field] = value;
            cases.push(bad);
        }
        let mut bad = base.clone();
        bad["devices"][0]["config"]["LedMatrix"][0]["DevicePos"] = json!([0, 0]);
        cases.push(bad);
        let mut bad = base.clone();
        bad["devices"][0]["config"]["LedMatrix"][0]
            .as_object_mut()
            .unwrap()
            .remove("DevicePos");
        cases.push(bad);
        let mut bad = base.clone();
        let duplicate = bad["devices"][0]["config"]["LedMatrix"][0].clone();
        bad["devices"][0]["config"]["LedMatrix"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        cases.push(bad);
        let mut bad = base.clone();
        let extra = bad["devices"][0].clone();
        bad["devices"].as_array_mut().unwrap().push(extra);
        cases.push(bad);
        for case in cases {
            let draft = Draft::open(&case.to_string()).unwrap();
            assert!(PreparedStudio::build(draft.read_snapshot(), map(), 1).is_err());
            assert!(!draft.is_dirty());
        }
    }

    #[test]
    fn cancelled_wrong_mode_and_partial_outs_do_not_advance_or_retry_the_frame() {
        let draft = Draft::open(PROFILE).unwrap();
        for fail in [0, 3, 6] {
            let prepared = program(&draft);
            let frame = prepared.current_frame().unwrap();
            let (mut device, spy) = test_support::fixture();
            spy.state.lock().unwrap().fail_at = Some(fail);
            let result = StreamState::default().submit_frame(
                &mut device,
                &frame,
                Instant::now() + Duration::from_secs(2),
                || false,
            );
            assert!(result.completion.error.is_some());
            assert_eq!(spy.state.lock().unwrap().attempts, fail + 1);
            assert_eq!(result.completion.accepted_packets, fail);
            assert_eq!(prepared.frame_index(), 0);
        }
        for cancel in [true, false] {
            let prepared = program(&draft);
            let frame = prepared.current_frame().unwrap();
            let (mut device, spy) = test_support::fixture();
            if !cancel {
                spy.state.lock().unwrap().mode = 0;
            }
            let result = StreamState::default().submit_frame(
                &mut device,
                &frame,
                Instant::now() + Duration::from_secs(2),
                || cancel,
            );
            assert!(result.completion.error.is_some());
            assert_eq!(spy.state.lock().unwrap().attempts, 0);
            assert_eq!(prepared.frame_index(), 0);
        }
    }
}
