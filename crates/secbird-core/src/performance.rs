//! Mouse stage intent and its packed hardware representation.
//!
//! Installed DeathAdder middleware 196 does not enable `useCycleSensitivity`.
//! Its normal branch keeps all profile rows, sends visible rows while cycling is
//! enabled, and sends only the selected original row while cycling is disabled.
use crate::{Device, DpiStages, Error, Profile, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DpiStage {
    pub x: u16,
    pub y: u16,
    pub independent: bool,
    pub visible: bool,
    /// Foreign stage fields survive edits to the fields understood here.
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Performance {
    #[serde(rename = "enable")]
    pub enabled: bool,
    /// One-based row in `stages`, before invisible rows are removed.
    pub active: u8,
    pub stages: Vec<DpiStage>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Performance {
    pub fn validate(&self) -> Result<()> {
        let n = self.stages.len();
        if !(1..=5).contains(&n) || self.active == 0 || usize::from(self.active) > n {
            return Err(Error::InvalidArg(
                "DPI stages require 1–5 rows and a valid one-based active row".into(),
            ));
        }
        if self
            .stages
            .iter()
            .any(|r| !(100..=26_000).contains(&r.x) || !(100..=26_000).contains(&r.y))
        {
            return Err(Error::InvalidArg(
                "DeathAdder DPI stage values must be within 100–26000".into(),
            ));
        }
        if !self.stages[usize::from(self.active - 1)].visible || self.visible_count() == 0 {
            return Err(Error::InvalidArg(
                "the active DPI row must be visible".into(),
            ));
        }
        Ok(())
    }

    pub fn visible_count(&self) -> usize {
        self.stages.iter().filter(|r| r.visible).count()
    }

    /// The one-based visible position shown by the stage selector.
    pub fn visible_active(&self) -> Result<u8> {
        self.validate()?;
        Ok(self.stages[..usize::from(self.active)]
            .iter()
            .filter(|r| r.visible)
            .count() as u8)
    }

    /// Select a one-based visible position without confusing it with a retained row.
    pub fn select_visible(&mut self, visible: u8) -> Result<()> {
        let index = self
            .stages
            .iter()
            .enumerate()
            .filter(|(_, r)| r.visible)
            .nth(usize::from(visible.saturating_sub(1)))
            .map(|(i, _)| i);
        if visible == 0 || index.is_none() {
            return Err(Error::InvalidArg("visible DPI stage does not exist".into()));
        }
        self.active = index.unwrap() as u8 + 1;
        self.validate()
    }

    /// Row switch behavior: the UI prevents hiding below two visible rows.
    /// Hiding the selected row selects the next visible row, wrapping to the first.
    pub fn set_visible(&mut self, row: usize, visible: bool) -> Result<()> {
        self.validate()?;
        let old = self
            .stages
            .get(row)
            .ok_or_else(|| Error::InvalidArg("DPI row does not exist".into()))?
            .visible;
        if old == visible {
            return Ok(());
        }
        if !visible && self.visible_count() <= 2 {
            return Err(Error::InvalidArg(
                "at least two DPI stages must remain visible".into(),
            ));
        }
        self.stages[row].visible = visible;
        if !visible && usize::from(self.active - 1) == row {
            let next = self
                .stages
                .iter()
                .enumerate()
                .find(|(i, r)| *i > row && r.visible)
                .or_else(|| self.stages.iter().enumerate().find(|(_, r)| r.visible))
                .map(|(i, _)| i)
                .unwrap();
            self.active = next as u8 + 1;
        }
        self.validate()
    }

    /// Show a prefix of retained rows. This is a model operation; the installed UI
    /// uses individual row switches and does not render its unused count handler.
    pub fn set_visible_count(&mut self, count: u8) -> Result<()> {
        if count == 0 || usize::from(count) > self.stages.len() {
            return Err(Error::InvalidArg(
                "visible stage count is outside retained rows".into(),
            ));
        }
        for (i, row) in self.stages.iter_mut().enumerate() {
            row.visible = i < usize::from(count);
        }
        self.active = self.active.min(count);
        self.validate()
    }

    /// Move a retained row, keeping the selected row's identity and metadata.
    pub fn move_row(&mut self, from: usize, to: usize) -> Result<()> {
        self.validate()?;
        if from >= self.stages.len() || to >= self.stages.len() {
            return Err(Error::InvalidArg("DPI row does not exist".into()));
        }
        let active = usize::from(self.active - 1);
        let row = self.stages.remove(from);
        self.stages.insert(to, row);
        let active = if active == from {
            to
        } else if from < active && to >= active {
            active - 1
        } else if from > active && to <= active {
            active + 1
        } else {
            active
        };
        self.active = active as u8 + 1;
        self.validate()
    }

    /// UI X edits couple Y while independent controls are off. Unedited foreign Y
    /// remains intact; merely loading a profile must not rewrite it.
    pub fn set_x(&mut self, row: usize, value: u16) -> Result<()> {
        if !(100..=26_000).contains(&value) {
            return Err(Error::InvalidArg("DPI outside 100–26000".into()));
        }
        let r = self
            .stages
            .get_mut(row)
            .ok_or_else(|| Error::InvalidArg("DPI row does not exist".into()))?;
        r.x = value;
        if !r.independent {
            r.y = value;
        }
        Ok(())
    }

    pub fn set_y(&mut self, row: usize, value: u16) -> Result<()> {
        if !(100..=26_000).contains(&value) {
            return Err(Error::InvalidArg("DPI outside 100-26000".into()));
        }
        let r = self
            .stages
            .get_mut(row)
            .ok_or_else(|| Error::InvalidArg("DPI row does not exist".into()))?;
        if !r.independent {
            return Err(Error::InvalidArg(
                "enable independent XY controls before editing Y".into(),
            ));
        }
        r.y = value;
        Ok(())
    }

    /// As in the shipped UI, explicitly disabling XY controls makes Y equal X.
    pub fn set_independent(&mut self, row: usize, independent: bool) -> Result<()> {
        let r = self
            .stages
            .get_mut(row)
            .ok_or_else(|| Error::InvalidArg("DPI row does not exist".into()))?;
        r.independent = independent;
        if !independent {
            r.y = r.x;
        }
        Ok(())
    }

    pub fn pack(&self) -> Result<DpiStages> {
        self.validate()?;
        let rows: Vec<_> = if self.enabled {
            self.stages.iter().filter(|r| r.visible).collect()
        } else {
            vec![&self.stages[usize::from(self.active - 1)]]
        };
        Ok(DpiStages {
            active: if self.enabled {
                self.visible_active()?
            } else {
                1
            },
            // He packs raw X/Y; `independent` controls editing, not transport.
            stages: rows.iter().map(|r| (r.x, r.y)).collect(),
            ..Default::default()
        })
    }

    /// Build a view from persisted intent or a legacy profile's physical stages.
    /// A hardware-only snapshot cannot recover hidden rows or the cycling switch.
    pub fn from_profile(profile: &Profile) -> Result<Option<Self>> {
        if let Some(value) = &profile.performance {
            value.validate()?;
            return Ok(Some(value.clone()));
        }
        let stages = match &profile.dpi_stages {
            Some(v) => v.clone(),
            None => match profile.dpi {
                Some(pair) => DpiStages {
                    active: 1,
                    stages: vec![pair],
                    ..Default::default()
                },
                None => return Ok(None),
            },
        };
        Self::from_hardware(&stages).map(Some)
    }

    pub fn from_hardware(stages: &DpiStages) -> Result<Self> {
        let value = Self {
            enabled: stages.stages.len() > 1,
            active: stages.active,
            stages: stages
                .stages
                .iter()
                .map(|&(x, y)| DpiStage {
                    x,
                    y,
                    independent: x != y,
                    visible: true,
                    extra: BTreeMap::new(),
                })
                .collect(),
            extra: BTreeMap::new(),
        };
        value.validate()?;
        Ok(value)
    }

    /// Keep the legacy physical projection usable by older readers. They cannot
    /// restore hidden rows after saving their own copy of this profile.
    pub fn store_in_profile(&self, profile: &mut Profile) -> Result<()> {
        let packed = self.pack()?;
        profile.dpi = Some(packed.stages[usize::from(packed.active - 1)]);
        profile.dpi_stages = Some(packed);
        profile.performance = Some(self.clone());
        Ok(())
    }

    /// Apply and verify the physical projection. Return snapped visible values;
    /// retained hidden rows and unknown source fields are never overwritten.
    pub fn apply(&self, device: &Device) -> Result<Self> {
        if !matches!(device.pid, 196 | 197) {
            return Err(Error::Unsupported(
                "stage intent is verified only for DeathAdder V3 HyperSpeed".into(),
            ));
        }
        let expected = device.set_dpi_stages(&self.pack()?)?;
        let read = device.dpi_stages()?;
        if read.active != expected.active || read.stages != expected.stages {
            return Err(Error::Unsupported(
                "the device did not retain the requested DPI stages".into(),
            ));
        }
        let mut result = self.clone();
        let indices: Vec<_> = if self.enabled {
            self.stages
                .iter()
                .enumerate()
                .filter(|(_, r)| r.visible)
                .map(|(i, _)| i)
                .collect()
        } else {
            vec![usize::from(self.active - 1)]
        };
        for (index, (x, y)) in indices.into_iter().zip(expected.stages) {
            result.stages[index].x = x;
            result.stages[index].y = y;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Performance {
        Performance {
            enabled: true,
            active: 3,
            extra: BTreeMap::new(),
            stages: (0..5)
                .map(|i| DpiStage {
                    x: 400 * (i + 1),
                    y: 500 * (i + 1),
                    independent: false,
                    visible: i != 1 && i != 4,
                    extra: BTreeMap::new(),
                })
                .collect(),
        }
    }
    #[test]
    fn packed_active_is_visible_position_and_raw_y_survives() {
        let mut v = sample();
        let p = v.pack().unwrap();
        assert_eq!(p.active, 2);
        assert_eq!(p.stages, vec![(400, 500), (1200, 1500), (1600, 2000)]);
        v.select_visible(3).unwrap();
        assert_eq!(v.active, 4);
        assert!(v.select_visible(0).is_err());
        assert!(v.select_visible(4).is_err());
    }
    #[test]
    fn disabled_cycles_only_selected_original_row() {
        let mut v = sample();
        v.enabled = false;
        let p = v.pack().unwrap();
        assert_eq!(p.active, 1);
        assert_eq!(p.stages, vec![(1200, 1500)]);
        assert_eq!(v.stages.len(), 5);
    }
    #[test]
    fn count_switch_retains_hidden_rows_and_xy_edit_matches_ui() {
        let mut v = sample();
        v.stages[4]
            .extra
            .insert("future".into(), serde_json::json!({"keep":true}));
        let saved = v.stages[4].clone();
        v.set_visible_count(2).unwrap();
        assert_eq!(v.active, 2);
        v.set_visible_count(5).unwrap();
        assert_eq!(v.stages[4].x, saved.x);
        assert_eq!(v.stages[4].y, saved.y);
        assert_eq!(v.stages[4].extra, saved.extra);
        v.set_x(0, 800).unwrap();
        assert_eq!(v.stages[0].y, 800);
        v.set_independent(1, true).unwrap();
        v.set_x(1, 1000).unwrap();
        assert_eq!(v.stages[1].y, 1000);
        v.stages[1].y = 1600;
        v.set_independent(1, false).unwrap();
        assert_eq!(v.stages[1].y, 1000);
    }
    #[test]
    fn persistence_retains_intent_legacy_projection_and_unknowns() {
        let mut v = sample();
        v.extra.insert("future".into(), serde_json::json!(42));
        let mut p = Profile::default();
        v.store_in_profile(&mut p).unwrap();
        let encoded = serde_json::to_vec(&p).unwrap();
        let p: Profile = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(Performance::from_profile(&p).unwrap(), Some(v));
        assert_eq!(p.dpi, Some((1200, 1500)));
    }
    #[test]
    fn row_switch_selects_next_visible_then_wraps_and_keeps_two() {
        let mut value = sample();
        value.set_visible(2, false).unwrap();
        assert_eq!(value.active, 4);
        let before = value.clone();
        assert!(value.set_visible(3, false).is_err());
        assert_eq!(value, before);
        value.set_visible(2, true).unwrap();
        value.set_visible(3, false).unwrap();
        assert_eq!(value.active, 1);
    }

    #[test]
    fn reordering_preserves_selected_identity_and_hidden_metadata() {
        let mut value = sample();
        let selected = value.stages[2].clone();
        let hidden = value.stages[1].clone();
        value.move_row(2, 0).unwrap();
        assert_eq!(value.active, 1);
        assert_eq!(value.stages[0], selected);
        assert_eq!(value.stages[2], hidden);
        value.move_row(0, 4).unwrap();
        assert_eq!(value.active, 5);
        assert_eq!(value.stages[4], selected);
        let before = value.clone();
        assert!(value.move_row(10, 1).is_err());
        assert_eq!(value, before);
    }

    #[test]
    fn profile_activation_prioritizes_intent_and_retains_hidden_rows_after_snap() {
        let sim = crate::sim::Sim::mouse();
        let dev = sim
            .open(196, crate::sim::Sim::spec_for(crate::sim::MOUSE_PID))
            .unwrap();
        let mut v = sample();
        v.stages[0].x = 423;
        let hidden = v.stages[1].clone();
        let applied = v.apply(&dev).unwrap();
        assert_eq!(applied.stages[0].x, 400);
        assert_eq!(applied.stages[1], hidden);
        let mut profile = Profile::default();
        applied.store_in_profile(&mut profile).unwrap();
        // Historical scalar DPI can disagree in source; activation must not send
        // it after stages and replace the selected stage's current DPI.
        profile.dpi = Some((800, 800));
        assert!(profile.apply(&dev).iter().all(|r| r.result.is_ok()));
        assert_eq!(dev.dpi().unwrap(), (1200, 1500));
        assert_eq!(dev.dpi_stages().unwrap().active, 2);
        v.enabled = false;
        let applied = v.apply(&dev).unwrap();
        assert_eq!(dev.dpi_stages().unwrap().stages, vec![(1200, 1500)]);
        assert_eq!(applied.stages[1], hidden);
    }
    #[test]
    fn rejects_hidden_active_and_invalid_values() {
        let mut v = sample();
        v.active = 2;
        assert!(v.pack().is_err());
        v.active = 3;
        v.stages[0].y = 0;
        assert!(v.pack().is_err());
    }
}
