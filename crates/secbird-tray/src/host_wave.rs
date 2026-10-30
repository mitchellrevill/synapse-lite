//! Pure source Quick Wave selection and authenticated-layout frame preparation.
//!
//! Selection/rendering has no clock, scheduling, device access, file loading,
//! or profile activation. The preparation child separately reads identity/assets.
//! The caller establishes fresh identity and output ownership. See
//! docs/HOST-WAVE.md for direction/projection and source-derived limits.

use secbird_core::{
    effects::Effect,
    ornata_frame::PreparedOrnataFrame,
    ornata_manifest::AuthenticatedLayout,
    ornata_spatial::{OrnataSpatialMap, LOGICAL_PIXELS, PHYSICAL_PIXELS},
    wave_lighting::{CardinalDirection, WaveCycle, WaveRegion},
    Error, Profile, Result, Rgb,
};
use serde_json::{json, Value};
#[cfg(test)]
use secbird_core::ornata_spatial::LayoutIdentity;
#[cfg(test)]
use std::time::Duration;

pub(crate) mod preparation;

const SOURCE: &str = "_synapseLiteSynapse4";
const OPT_IN: &str = "waveHostIntent";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Program {
    Left,
    Right,
}
impl Program {
    pub(crate) fn from_setting(setting: &Value) -> Result<Self> {
        let object = setting
            .as_object()
            .ok_or_else(|| Error::InvalidArg("Source Wave setting must be an object".into()))?;
        // Either boolean counterclockwise selects a rotational source path.
        // Reject semantic overrides that this fixed Quick constructor cannot
        // represent, while preserving unrelated opaque members without edits.
        if object.contains_key("counterclockwise") || object.contains_key("angle") {
            return Err(Error::Unsupported(
                "Host Wave supports only the source Quick left/right linear preset".into(),
            ));
        }
        for (field, default) in [
            ("speed", json!(25)),
            ("width", json!(100)),
            ("pause", json!(0)),
            ("cycles", json!(-1)),
            ("split", json!(false)),
        ] {
            if object.get(field).is_some_and(|value| *value != default) {
                return Err(Error::Unsupported(format!(
                    "Host Wave does not support a nondefault {field}"
                )));
            }
        }
        match object.get("direction").and_then(Value::as_u64) {
            Some(1) => Ok(Self::Left),
            Some(2) => Ok(Self::Right),
            _ => Err(Error::Unsupported("Host Wave requires source direction1(left) or2(right); split/rotational directions are unsupported".into())),
        }
    }
    pub(crate) fn source_direction(self) -> u8 {
        match self {
            Self::Left => 1,
            Self::Right => 2,
        }
    }
    fn cardinal(self) -> CardinalDirection {
        match self {
            Self::Left => CardinalDirection::Left,
            Self::Right => CardinalDirection::Right,
        }
    }
    /// The existing interchange codec binds direction1 to reverse=false and
    /// direction2 to reverse=true. Its legacy Display labels are not the source
    /// Wave direction authority; rendering follows the retained source setting.
    pub(crate) fn native_projection(self) -> Effect {
        Effect::Wave {
            reverse: self == Self::Right,
            speed: 3,
        }
    }
    pub(crate) fn from_profile(profile: &Profile, pid: u16) -> Result<Option<Self>> {
        if pid != 605 || !profile.keys.is_empty() {
            return Ok(None);
        }
        let Some(quick) = secbird_core::profile_compat::source_quick_effects(profile) else {
            return Ok(None);
        };
        if quick.get("selectedEffectId").and_then(Value::as_u64) != Some(4) {
            return Ok(None);
        }
        let Some(zone) = profile.lighting.get("backlight") else {
            return Err(Error::InvalidArg(
                "Source Wave has no native backlight projection".into(),
            ));
        };
        if zone.off {
            return Ok(None);
        }
        let setting = quick.get("selectedEffectSetting").ok_or_else(|| {
            Error::InvalidArg("Source Wave has no retained direction setting".into())
        })?;
        let program = Self::from_setting(setting)?;
        if zone.effect.as_ref() != Some(&program.native_projection()) {
            return Err(Error::Unsupported(
                "Retained source Wave and native direction/default speed projection disagree"
                    .into(),
            ));
        }
        Ok(Some(program))
    }
    fn marker(self) -> Value {
        json!({"version":1,"source_id":4,"direction":self.source_direction()})
    }
    /// Import/default selection never calls this. The explicit effect action
    /// must validate its current source/native projection before opting in.
    pub(crate) fn opt_in(self, profile: &mut Profile) -> Result<()> {
        if Self::from_profile(profile, 605)? != Some(self) {
            return Err(Error::InvalidArg(
                "Wave opt-in requires the current matching profile choice".into(),
            ));
        }
        let metadata = profile
            .extra
            .get_mut(SOURCE)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| {
                Error::InvalidArg("Source metadata is required for Wave opt-in".into())
            })?;
        metadata.insert(OPT_IN.into(), self.marker());
        Ok(())
    }
    pub(crate) fn opted_in(self, profile: &Profile) -> bool {
        matches!(Self::from_profile(profile,605), Ok(Some(current)) if current == self)
            && profile
                .extra
                .get(SOURCE)
                .and_then(|value| value.get(OPT_IN))
                == Some(&self.marker())
    }
    pub(crate) fn clear_opt_in(profile: &mut Profile) {
        if let Some(metadata) = profile.extra.get_mut(SOURCE).and_then(Value::as_object_mut) {
            metadata.remove(OPT_IN);
        }
    }
}

/// Prepared immutable frame-index function. The production constructor requires
/// the manifest loader's opaque authenticated selection, never a declared SHA.
#[derive(Debug, Clone)]
pub(crate) struct PreparedWave {
    program: Program,
    cycle: WaveCycle,
    map: OrnataSpatialMap,
}
impl PreparedWave {
    pub(crate) fn new(program: Program, layout: &AuthenticatedLayout) -> Result<Self> {
        Self::build(program, layout.map().clone())
    }
    fn build(program: Program, map: OrnataSpatialMap) -> Result<Self> {
        let region = WaveRegion::new(7, 23, 0, 0, 23, 7)?;
        let cycle = WaveCycle::source_quick(region, program.cardinal())?;
        Ok(Self {
            program,
            cycle,
            map,
        })
    }
    pub(crate) fn program(&self) -> Program {
        self.program
    }
    #[cfg(test)]
    pub(crate) fn identity(&self) -> LayoutIdentity {
        self.map.provenance().identity
    }
    pub(crate) fn frame_count(&self) -> usize {
        self.cycle.frame_count()
    }
    /// Source sample interval, not a promise about achieved USB cadence.
    #[cfg(test)]
    pub(crate) fn frame_interval(&self) -> Duration {
        self.cycle.frame_interval()
    }
    pub(crate) fn render_frame(&self, index: usize) -> Result<PreparedOrnataFrame> {
        let mut logical = [Rgb(0, 0, 0); LOGICAL_PIXELS];
        self.cycle.render_frame(index, &mut logical)?;
        let mut physical = [Rgb(0, 0, 0); PHYSICAL_PIXELS];
        self.map
            .map_frame_into(&logical, &mut physical)
            .map_err(|e| Error::InvalidArg(e.to_string()))?;
        PreparedOrnataFrame::new(&physical)
    }
    #[cfg(test)]
    fn from_numeric_test_map(program: Program, map: OrnataSpatialMap) -> Result<Self> {
        Self::build(program, map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secbird_core::{
        ornata_spatial::{MappingEntry, SourceLayoutRecord, SOURCE_MANIFEST_SHA256, UNMAPPED},
        profile::DeviceProfiles,
        Store,
    };

    fn profile(program: Program) -> Profile {
        let mut profile =
            secbird_core::profile_compat::new_native_profile(605, "Synthetic").unwrap();
        secbird_core::profile_compat::set_source_quick_effect(
            &mut profile,
            605,
            "Synthetic",
            4,
            &json!({"direction":program.source_direction(),"future":{"retain":[1,true]}}),
        )
        .unwrap();
        profile.lighting.get_mut("backlight").unwrap().effect = Some(program.native_projection());
        profile
    }
    fn map() -> OrnataSpatialMap {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../secbird-core/tests/fixtures/ornata_spatial.json"
        ))
        .unwrap();
        let record = fixture["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["layout"] == 6)
            .unwrap();
        let entries: Vec<MappingEntry> = record["entries"]
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
        let id = LayoutIdentity {
            vendor_id: 0x1532,
            product_id: 605,
            edition_id: 0,
            layout_id: 6,
        };
        OrnataSpatialMap::from_record(
            id,
            SourceLayoutRecord {
                identity: id,
                logical_dimensions: [7, 23],
                physical_dimensions: [6, 22],
                entries: &entries,
                manifest_sha256: SOURCE_MANIFEST_SHA256,
            },
        )
        .unwrap()
    }

    #[test]
    fn source_choices_require_matching_native_projection_and_default_speed() {
        for program in [Program::Left, Program::Right] {
            let mut profile = profile(program);
            assert_eq!(Program::from_profile(&profile, 605).unwrap(), Some(program));
            assert!(Program::from_profile(&profile, 196).unwrap().is_none());
            profile.lighting.get_mut("backlight").unwrap().effect = Some(Effect::Wave {
                reverse: program == Program::Left,
                speed: 3,
            });
            assert!(Program::from_profile(&profile, 605).is_err());
            profile.lighting.get_mut("backlight").unwrap().effect = Some(Effect::Wave {
                reverse: program == Program::Right,
                speed: 2,
            });
            assert!(Program::from_profile(&profile, 605).is_err());
            profile.lighting.get_mut("backlight").unwrap().off = true;
            assert!(Program::from_profile(&profile, 605).unwrap().is_none());
        }
        assert!(Program::from_profile(&Profile::default(), 605)
            .unwrap()
            .is_none());
        let mut profile = profile(Program::Left);
        profile.keys = vec![Rgb(1, 2, 3); 132];
        assert!(Program::from_profile(&profile, 605).unwrap().is_none());
    }
    #[test]
    fn unsupported_variants_never_become_linear_quick_wave() {
        for setting in [
            json!({}),
            json!({"direction":5}),
            json!({"direction":11}),
            json!({"direction":3}),
            json!({"direction":1,"counterclockwise":false}),
            json!({"direction":2,"counterclockwise":true}),
            json!({"direction":1,"split":true}),
            json!({"direction":1,"speed":26}),
            json!({"direction":1,"angle":270}),
            json!({"direction":1,"width":50}),
            json!({"direction":1,"cycles":1}),
            json!({"direction":1,"pause":1}),
        ] {
            assert!(Program::from_setting(&setting).is_err(), "{setting}");
        }
    }
    #[test]
    fn opt_in_is_explicit_choice_bound_and_preserves_foreign_payload_and_opaque_members() {
        let mut profile = profile(Program::Left);
        let decoded = profile.extra[SOURCE]["decoded"].clone();
        assert!(!Program::Left.opted_in(&profile));
        Program::Left.opt_in(&mut profile).unwrap();
        assert!(Program::Left.opted_in(&profile));
        assert_eq!(profile.extra[SOURCE]["decoded"], decoded);
        let mut device = DeviceProfiles::default();
        device.profiles.insert("Default".into(), profile.clone());
        let mut store = Store::default();
        store.devices.insert("025D:synthetic".into(), device);
        let opted = secbird_core::profile_compat::export_synapse4(&store, "025D:synthetic")
            .unwrap()
            .bytes;
        Program::clear_opt_in(
            store
                .devices
                .get_mut("025D:synthetic")
                .unwrap()
                .active_mut(),
        );
        assert_eq!(
            secbird_core::profile_compat::export_synapse4(&store, "025D:synthetic")
                .unwrap()
                .bytes,
            opted
        );
        profile.extra.get_mut(SOURCE).unwrap()["decoded"]["quickEffects"]
            ["selectedEffectSetting"]["direction"] = json!(2);
        profile.lighting.get_mut("backlight").unwrap().effect =
            Some(Program::Right.native_projection());
        assert!(!Program::Left.opted_in(&profile));
        assert!(!Program::Right.opted_in(&profile));
        assert!(Program::Left.opt_in(&mut profile).is_err());
        Program::Right.opt_in(&mut profile).unwrap();
        assert!(Program::Right.opted_in(&profile));
        assert_eq!(
            profile.extra[SOURCE]["decoded"]["quickEffects"]["selectedEffectSetting"]["future"],
            json!({"retain":[1,true]})
        );
        Program::clear_opt_in(&mut profile);
        assert!(!Program::Right.opted_in(&profile));
    }
    #[test]
    fn complete_physical_cycles_match_independent_goldens_with_black_holes_and_no_advance() {
        // Production callers cannot supply an unauthenticated numeric map.
        // Numeric source fixtures enter only through the private cfg(test) helper.
        let _constructor: fn(Program, &AuthenticatedLayout) -> Result<PreparedWave> =
            PreparedWave::new;
        let raw = include_bytes!("../../secbird-core/tests/fixtures/ornata_spatial_wave.rgb");
        for (program, offset) in [(Program::Left, 0), (Program::Right, 9900)] {
            let map = map();
            let holes: Vec<_> = map
                .physical_to_logical()
                .iter()
                .enumerate()
                .filter_map(|(index, source)| (*source == UNMAPPED).then_some(index))
                .collect();
            let wave = PreparedWave::from_numeric_test_map(program, map).unwrap();
            assert_eq!(wave.program(), program);
            assert_eq!(wave.identity().layout_id, 6);
            assert_eq!(wave.frame_count(), 25);
            assert_eq!(wave.frame_interval(), Duration::from_millis(40));
            let initial = wave.render_frame(0).unwrap().packets(0x1f);
            for frame in 0..25 {
                let packets = wave.render_frame(frame).unwrap().packets(0x1f);
                let bytes: Vec<u8> = packets[..6]
                    .iter()
                    .flat_map(|packet| packet[14..80].iter().copied())
                    .collect();
                let start = offset + frame * 396;
                assert_eq!(bytes, &raw[start..start + 396], "{program:?} frame{frame}");
                for hole in &holes {
                    assert_eq!(&bytes[hole * 3..hole * 3 + 3], &[0, 0, 0]);
                }
                assert_eq!(&packets[6][9..15], &[0, 0, 8, 0, 0, 0]);
            }
            assert_eq!(wave.render_frame(25).unwrap().packets(0x1f), initial);
            assert_eq!(wave.render_frame(0).unwrap().packets(0x1f), initial);
            assert_eq!(
                wave.render_frame(usize::MAX).unwrap().packets(0x1f),
                wave.render_frame(usize::MAX % 25).unwrap().packets(0x1f)
            );
        }
    }
}
