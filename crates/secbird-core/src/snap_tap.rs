//! Ornata Snap Tap intent and digital last-input resolution. No input I/O.
//!
//! A live adapter must separately prove selected-device routing, exclusive input
//! interception, completion and recovery. This module does not install hooks.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_PAIRS: usize = 4;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub is_enabled: bool,
    pub key_list: Vec<Pair>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pair {
    pub key1: String,
    pub key2: String,
    pub id: u32,
    #[serde(default = "last_input")]
    pub mode: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn last_input() -> String {
    "LAST_INPUT".into()
}
fn invalid(message: &str) -> Error {
    Error::InvalidArg(message.into())
}

impl Settings {
    /// Validate the installed digital keyboard's configuration. Analog priority
    /// modes in the shared vendor library are not Ornata capabilities.
    pub fn validate(&self) -> Result<()> {
        if self.key_list.len() > MAX_PAIRS {
            return Err(invalid("Snap Tap supports at most four key pairs"));
        }
        let mut keys = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for pair in &self.key_list {
            if pair.id == 0 || !ids.insert(pair.id) {
                return Err(invalid("Snap Tap pairs need distinct nonzero identities"));
            }
            if pair.mode != "LAST_INPUT" {
                return Err(invalid("Ornata Snap Tap requires last-input priority"));
            }
            for key in [&pair.key1, &pair.key2] {
                let usage = key_usage(key)?;
                if !keys.insert(usage) {
                    return Err(invalid("A key can belong to only one Snap Tap pair"));
                }
            }
        }
        Ok(())
    }

    /// A bounded digital plan, not permission to intercept input. Unknown source
    /// extensions stay saved but cannot silently acquire runtime semantics.
    pub fn plan(&self) -> Result<Plan> {
        self.validate()?;
        if !self.extra.is_empty() || self.key_list.iter().any(|pair| !pair.extra.is_empty()) {
            return Err(invalid(
                "Unknown Snap Tap extensions require runtime review",
            ));
        }
        let mut pairs = Vec::new();
        if self.is_enabled {
            for pair in &self.key_list {
                pairs.push([key_usage(&pair.key1)?, key_usage(&pair.key2)?]);
            }
        }
        Ok(Plan { pairs })
    }
}

fn key_usage(key: &str) -> Result<u8> {
    // Reuse the source mapping table, and restrict it to actual Ornata keys.
    // The UI separately excludes Menu, left Windows, Fn and its special aliases.
    if matches!(
        key,
        "KEY_APPLICATION" | "KEY_LEFT_GUI" | "KEY_FN" | "DKM_F6" | "DKM_D2"
    ) {
        return Err(invalid("This key is excluded from Ornata Snap Tap"));
    }
    crate::profile_compat::source_key_usage(key)
        .filter(|usage| crate::buttons::KEYS.iter().any(|row| row.3 == *usage))
        .ok_or_else(|| invalid("Unknown or unavailable Ornata Snap Tap key"))
}

#[derive(Clone, Debug)]
pub struct Plan {
    pairs: Vec<[u8; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    /// Logical USB HID usage. This is not an RZCONTROL native scan code or a
    /// packet ready to submit; the adapter must retain the selected assignment.
    pub usage: u8,
    pub pressed: bool,
}

/// Logical changes only. A transport must track which submissions it accepted;
/// these changes alone do not prove OS delivery or ownership of held outputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    PassThrough,
    Replace(Vec<Edge>),
}

#[derive(Default, Debug)]
struct PairState {
    held: [bool; 2],
    winner: Option<usize>,
}

#[derive(Debug)]
pub struct Resolver {
    plan: Plan,
    states: Vec<PairState>,
}

impl Resolver {
    /// Start only after the live adapter has established that participating keys
    /// are up. Initializing this reducer is not a physical key-state query.
    pub fn new(plan: Plan) -> Self {
        let states = (0..plan.pairs.len())
            .map(|_| PairState::default())
            .collect();
        Self { plan, states }
    }

    pub fn event(&mut self, usage: u8, pressed: bool) -> Decision {
        let Some((index, side)) = self
            .plan
            .pairs
            .iter()
            .enumerate()
            .find_map(|(index, pair)| {
                pair.iter()
                    .position(|key| *key == usage)
                    .map(|side| (index, side))
            })
        else {
            return Decision::PassThrough;
        };
        let state = &mut self.states[index];
        if state.held[side] == pressed {
            return Decision::Replace(Vec::new());
        }
        let previous = state.winner;
        state.held[side] = pressed;
        if pressed {
            state.winner = Some(side);
        } else if previous == Some(side) {
            state.winner = state.held[1 - side].then_some(1 - side);
        }
        let pair = self.plan.pairs[index];
        let mut changes = Vec::with_capacity(2);
        if previous != state.winner {
            if let Some(side) = previous {
                changes.push(Edge {
                    usage: pair[side],
                    pressed: false,
                });
            }
            if let Some(side) = state.winner {
                changes.push(Edge {
                    usage: pair[side],
                    pressed: true,
                });
            }
        }
        Decision::Replace(changes)
    }

    /// Reset desired state on disable, profile/device change or cancellation.
    /// The adapter must release its accepted owned outputs and handle still-held
    /// physical keys before resuming ordinary input; no blanket release is valid.
    pub fn reset(&mut self) -> Vec<Edge> {
        let mut changes = Vec::with_capacity(MAX_PAIRS);
        for (pair, state) in self.plan.pairs.iter().zip(&mut self.states) {
            if let Some(side) = state.winner {
                changes.push(Edge {
                    usage: pair[side],
                    pressed: false,
                });
            }
            *state = PairState::default();
        }
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> Settings {
        serde_json::from_value(serde_json::json!({"isEnabled":true,"keyList":[
            {"id":1,"key1":"KEY_A","key2":"KEY_D","mode":"LAST_INPUT"}]}))
        .unwrap()
    }
    fn changes(edges: &[(u8, bool)]) -> Decision {
        Decision::Replace(
            edges
                .iter()
                .map(|&(usage, pressed)| Edge { usage, pressed })
                .collect(),
        )
    }
    #[test]
    fn newest_press_wins_and_releasing_it_restores_the_held_partner() {
        let mut resolver = Resolver::new(settings().plan().unwrap());
        assert_eq!(resolver.event(4, true), changes(&[(4, true)]));
        assert_eq!(resolver.event(7, true), changes(&[(4, false), (7, true)]));
        // Typematic repeats on the losing key cannot steal priority.
        assert_eq!(resolver.event(4, true), changes(&[]));
        assert_eq!(resolver.event(7, false), changes(&[(7, false), (4, true)]));
        assert_eq!(resolver.event(4, false), changes(&[(4, false)]));
        assert_eq!(resolver.event(4, false), changes(&[]));
        assert!(resolver.reset().is_empty());
    }
    #[test]
    fn releasing_the_losing_key_does_not_release_the_winner() {
        let mut resolver = Resolver::new(settings().plan().unwrap());
        resolver.event(7, true);
        resolver.event(4, true);
        assert_eq!(resolver.event(7, false), changes(&[]));
        assert_eq!(resolver.event(4, false), changes(&[(4, false)]));
        assert_eq!(resolver.event(42, true), Decision::PassThrough);
    }
    #[test]
    fn independent_pairs_reset_without_releasing_unrelated_keys() {
        let mut settings = settings();
        settings.key_list.push(Pair {
            key1: "KEY_W".into(),
            key2: "KEY_S".into(),
            id: 2,
            mode: last_input(),
            extra: BTreeMap::new(),
        });
        let mut resolver = Resolver::new(settings.plan().unwrap());
        resolver.event(4, true);
        resolver.event(7, true);
        resolver.event(26, true);
        assert_eq!(
            resolver.reset(),
            vec![
                Edge {
                    usage: 7,
                    pressed: false
                },
                Edge {
                    usage: 26,
                    pressed: false
                }
            ]
        );
        assert!(resolver.reset().is_empty());
        assert_eq!(resolver.event(4, true), changes(&[(4, true)]));
    }
    #[test]
    fn invalid_or_ambiguous_pairs_cannot_prepare_a_runtime_plan() {
        let base = settings();
        for key in [
            "KEY_APPLICATION",
            "KEY_LEFT_GUI",
            "KEY_FN",
            "DKM_F6",
            "DKM_D2",
            "UNKNOWN",
            "KEY_D",
        ] {
            let mut s = base.clone();
            s.key_list[0].key1 = key.into();
            assert!(s.plan().is_err(), "{key}");
        }
        let mut duplicate = base.clone();
        duplicate.key_list.push(base.key_list[0].clone());
        assert!(duplicate.plan().is_err());
        let mut mode = base.clone();
        mode.key_list[0].mode = "COMPARE_LEVEL".into();
        assert!(mode.plan().is_err());
        let mut zero = base.clone();
        zero.key_list[0].id = 0;
        assert!(zero.plan().is_err());
        let mut excess = base.clone();
        excess.key_list = vec![base.key_list[0].clone(); 5];
        assert!(excess.plan().is_err());
    }
    #[test]
    fn disabled_configuration_passes_through_and_unknown_fields_remain_saved() {
        let mut s = settings();
        s.is_enabled = false;
        assert_eq!(
            Resolver::new(s.plan().unwrap()).event(4, true),
            Decision::PassThrough
        );
        s.extra
            .insert("future".into(), serde_json::json!({"keep":true}));
        assert!(s.validate().is_ok());
        assert!(s.plan().is_err());
        assert_eq!(serde_json::to_value(s).unwrap()["future"]["keep"], true);
    }

    #[test]
    fn every_eight_event_trace_has_balanced_edges_and_at_most_one_winner() {
        for trace in 0..65536u32 {
            let mut resolver = Resolver::new(settings().plan().unwrap());
            let mut outputs = BTreeSet::new();
            for index in 0..8 {
                let event = (trace >> (index * 2)) & 3;
                let usage = if event & 2 == 0 { 4 } else { 7 };
                let Decision::Replace(edges) = resolver.event(usage, event & 1 != 0) else {
                    panic!("paired input passed through")
                };
                for edge in edges {
                    if edge.pressed {
                        assert!(outputs.insert(edge.usage));
                    } else {
                        assert!(outputs.remove(&edge.usage));
                    }
                    assert!(outputs.len() <= 1);
                }
            }
            for edge in resolver.reset() {
                assert!(!edge.pressed);
                assert!(outputs.remove(&edge.usage));
            }
            assert!(outputs.is_empty());
        }
    }

    #[test]
    fn profile_apply_refuses_to_pretend_that_intent_is_a_firmware_command() {
        let sim = crate::sim::Sim::keyboard();
        let dev = sim
            .open(605, crate::sim::Sim::spec_for(crate::sim::KEYBOARD_PID))
            .unwrap();
        let before = sim.state().log.len();
        let profile = crate::profile::Profile {
            snap_tap: Some(settings()),
            ..Default::default()
        };
        let applied = profile.apply(&dev);
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].setting, "Snap Tap");
        assert!(applied[0].result.is_err());
        assert_eq!(sim.state().log.len(), before);

        // Historical native backups have only retained source metadata. They
        // must report the same missing runtime without changing saved bytes.
        let mut historical = crate::profile::Profile::default();
        historical.extra.insert(
            "_synapseLiteSynapse4".into(),
            serde_json::json!({
                "decoded":{"snapTap":{"isEnabled":true,"keyList":[]}}
            }),
        );
        let saved = serde_json::to_vec(&historical).unwrap();
        let applied = historical.apply(&dev);
        assert_eq!(applied.len(), 1);
        assert!(applied[0].result.is_err());
        assert_eq!(sim.state().log.len(), before);
        assert_eq!(serde_json::to_vec(&historical).unwrap(), saved);
    }
}
