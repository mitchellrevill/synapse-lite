//! Source sensitivity forms, saved locally until installed mapping delivery is verified.
//! Pending intent is deliberately separate from executable profile buttons/source records.
use crate::{db::DpiRange, Error, Profile, Result, Store};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DRAFTS: &str = "_synapseLiteSensitivityDrafts";
pub const OPTIONS: [(&str, &str); 6] = [
    ("DPI_Clutch", "Sensitivity Clutch"),
    ("DPI_Up", "Sensitivity Stage Up"),
    ("DPI_Down", "Sensitivity Stage Down"),
    ("DPI_OnTheFly", "On-The-Fly Sensitivity"),
    ("DPI_CycleUp", "Cycle Up Sensitivity Stages"),
    ("DPI_CycleDown", "Cycle Down Sensitivity Stages"),
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub input: String,
    pub hyper: bool,
    #[serde(rename = "sensitivityAssignment")]
    pub assignment: String,
    pub x: u16,
    pub y: u16,
    pub independent: bool,
}
impl Intent {
    pub fn allowed(&self, pid: u16) -> bool {
        // Installed mouse input lists exclude the primary click and wheel rotations.
        matches!(pid, 196 | 197)
            && matches!(
                self.input.as_str(),
                "RightClick" | "ScrollButton" | "Button4" | "Button5" | "DKM_SB_03"
            )
            && OPTIONS.iter().any(|(id, _)| *id == self.assignment)
            && !(self.hyper && self.assignment == "DPI_OnTheFly")
            && !(self.input == "DKM_SB_03"
                && matches!(self.assignment.as_str(), "DPI_Clutch" | "DPI_OnTheFly"))
    }
    pub fn validate(&self, pid: u16, range: &DpiRange) -> Result<()> {
        if !self.allowed(pid) || range.min == 0 || range.min > range.max || range.step == 0 {
            return Err(Error::InvalidArg(
                "Sensitivity assignment is unavailable for this input/device/layer".into(),
            ));
        }
        if self.assignment == "DPI_Clutch"
            && (!(range.min..=range.max).contains(&self.x)
                || !(range.min..=range.max).contains(&self.y)
                || (!self.independent && self.x != self.y))
        {
            return Err(Error::InvalidArg(
                "Clutch DPI exceeds device bounds or linked axes differ".into(),
            ));
        }
        Ok(())
    }
}

/// All-or-nothing persistence with exact profile comparison; never applies hardware.
pub fn save_draft(
    store: &mut Store,
    device: &str,
    profile: &str,
    pid: u16,
    range: &DpiRange,
    expected: &Value,
    intent: &Intent,
) -> Result<()> {
    intent.validate(pid, range)?;
    let mut candidate = store.clone();
    let profiles = candidate
        .devices
        .get_mut(device)
        .ok_or_else(|| Error::InvalidArg("Selected profile store is missing".into()))?;
    if profiles.active != profile {
        return Err(Error::InvalidArg("Selected profile changed".into()));
    }
    let target = profiles
        .profiles
        .get_mut(profile)
        .ok_or_else(|| Error::InvalidArg("Selected profile is missing".into()))?;
    if serde_json::to_value(&*target).map_err(|e| Error::InvalidArg(e.to_string()))? != *expected {
        return Err(Error::InvalidArg(
            "Profile changed; reopen the sensitivity form".into(),
        ));
    }
    // Reject unknown schema rather than erasing a future version or foreign data.
    let mut drafts = match target.extra.get(DRAFTS) {
        None => Vec::new(),
        Some(v) => serde_json::from_value::<Vec<Intent>>(v.clone()).map_err(|_| {
            Error::InvalidArg("Unrecognized saved sensitivity drafts; preserved unchanged".into())
        })?,
    };
    crate::profile_compat::assignment_state::clear_for_sensitivity(target,pid,profile,&intent.input,intent.hyper)?;
    drafts.retain(|old| old.input != intent.input || old.hyper != intent.hyper);
    drafts.push(intent.clone());
    target
        .extra
        .insert(DRAFTS.into(), serde_json::to_value(drafts).unwrap());
    candidate.save()?;
    *store = candidate;
    Ok(())
}

pub fn draft(profile: &Profile, input: &str, hyper: bool) -> Option<Intent> {
    serde_json::from_value::<Vec<Intent>>(profile.extra.get(DRAFTS)?.clone())
        .ok()?
        .into_iter()
        .find(|d| d.input == input && d.hyper == hyper)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::DeviceProfiles;
    use serde_json::json;
    #[test]
    fn save_reopen_preserves_executable_and_opaque_data_and_rejects_stale_owner() {
        let path = std::env::temp_dir().join(format!(
            "sensitivity-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut store = Store::load(&path).unwrap();
        let profile: Profile = serde_json::from_value(json!({"buttons":{"right":"dpi-up"},"opaque":{"future":[1,2]},"_synapseLiteSynapse4":{"decoded":{"mappings":[{"unknown":true}]}}})).unwrap();
        let expected = serde_json::to_value(&profile).unwrap();
        store.devices.insert(
            "sim".into(),
            DeviceProfiles {
                active: "A".into(),
                profiles: [("A".into(), profile.clone())].into(),
                ..Default::default()
            },
        );
        let range = DpiRange {
            min: 100,
            max: 26000,
            step: 50,
        };
        let intent = Intent {
            input: "Button4".into(),
            hyper: false,
            assignment: "DPI_Clutch".into(),
            x: 401,
            y: 800,
            independent: true,
        };
        save_draft(&mut store, "sim", "A", 196, &range, &expected, &intent).unwrap();
        let reopened = Store::load(&path).unwrap();
        let saved = &reopened.devices["sim"].profiles["A"];
        assert_eq!(draft(saved, "Button4", false), Some(intent.clone()));
        assert_eq!(saved.buttons, profile.buttons);
        assert_eq!(saved.extra["opaque"], profile.extra["opaque"]);
        assert_eq!(
            saved.extra["_synapseLiteSynapse4"],
            profile.extra["_synapseLiteSynapse4"]
        );
        let bytes = std::fs::read(&path).unwrap();
        assert!(save_draft(&mut store, "sim", "A", 196, &range, &expected, &intent).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let target = store
            .devices
            .get_mut("sim")
            .unwrap()
            .profiles
            .get_mut("A")
            .unwrap();
        let mut unknown = serde_json::to_value(&intent).unwrap();
        unknown["future"] = json!({"preserve":true});
        target.extra.insert(DRAFTS.into(), json!([unknown]));
        let expected_unknown = serde_json::to_value(&*target).unwrap();
        let memory = store.native_bytes().unwrap();
        assert!(save_draft(
            &mut store,
            "sim",
            "A",
            196,
            &range,
            &expected_unknown,
            &intent
        )
        .is_err());
        assert_eq!(store.native_bytes().unwrap(), memory);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn source_filters_and_device_bounds_are_enforced() {
        let range = DpiRange {
            min: 100,
            max: 26000,
            step: 50,
        };
        let mut i = Intent {
            input: "DKM_SB_03".into(),
            hyper: false,
            assignment: "DPI_Clutch".into(),
            x: 800,
            y: 800,
            independent: false,
        };
        assert!(i.validate(196, &range).is_err());
        i.input = "Button4".into();
        assert!(i.validate(196, &range).is_ok());
        i.y = 400;
        assert!(i.validate(196, &range).is_err());
        i.independent = true;
        assert!(i.validate(196, &range).is_ok());
        i.x = 26001;
        assert!(i.validate(196, &range).is_err());
        i.assignment = "DPI_OnTheFly".into();
        i.hyper = true;
        assert!(i.validate(196, &range).is_err());
        i.hyper = false;
        assert!(i.validate(605, &range).is_err());
    }
}
