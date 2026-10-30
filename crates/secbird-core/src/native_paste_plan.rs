//! Recovered endpoint paste protocol, not an input sender or clipboard owner.
//! Snapshot releases may target physical input; they convey no output ownership.
//! Do not submit them as ordinary owned key-ups through the macro output ledger.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    SuspendSnapshotModifier,
    PasteChord,
    RestoreSnapshotModifier,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    EndpointOnly,
    EndpointElseOsFallback,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyStep {
    pub scan_code: u16,
    pub flags: u16,
    pub role: Role,
    pub route: Route,
    /// Argument captured by the source callback, not a verified wall-clock unit.
    pub delay_argument: u32,
}
impl KeyStep {
    /// Source OnSendKeyOnThread scales the argument only on callback true.
    /// Neither true nor reaching the next callback proves terminal delivery.
    pub fn source_runner_delay(self, callback_true: bool) -> u64 {
        if callback_true {
            u64::from(self.delay_argument) * 1000
        } else {
            0
        }
    }
}

pub const PASTE_CHORD: [KeyStep; 4] = [
    KeyStep {
        scan_code: 0x1d,
        flags: 0,
        role: Role::PasteChord,
        route: Route::EndpointOnly,
        delay_argument: 30,
    },
    KeyStep {
        scan_code: 0x2f,
        flags: 0,
        role: Role::PasteChord,
        route: Route::EndpointOnly,
        delay_argument: 20,
    },
    KeyStep {
        scan_code: 0x2f,
        flags: 1,
        role: Role::PasteChord,
        route: Route::EndpointOnly,
        delay_argument: 0,
    },
    KeyStep {
        scan_code: 0x1d,
        flags: 1,
        role: Role::PasteChord,
        route: Route::EndpointOnly,
        delay_argument: 10,
    },
];

/// Actual GetKeyState call order. Supplying states does not read Windows input.
/// A runtime collection of these states is non-atomic and grants no ownership.
pub const SAMPLED_VIRTUAL_KEYS: [u16; 8] = [0xa4, 0xa2, 0xa0, 0x5b, 0xa5, 0xa3, 0xa1, 0x5c];
const SNAPSHOT_BITS: [u16; 8] = [0x100, 0x200, 0x400, 0x800, 0x10, 0x20, 0x40, 0x80];
const VALID_MASK: u32 = 0xff0;
// Source callback construction order; execution reverses the wrapper chain.
const MODIFIERS: [(u16, u16, u16); 8] = [
    (0x100, 0x38, 0),
    (0x10, 0x38, 2),
    (0x200, 0x1d, 0),
    (0x20, 0x1d, 2),
    (0x400, 0x2a, 0),
    (0x40, 0x36, 0),
    (0x800, 0x5b, 2),
    (0x80, 0x5c, 2),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownSnapshotBits(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModifierSnapshot(u16);
impl ModifierSnapshot {
    pub fn from_source_mask(mask: u32) -> Result<Self, UnknownSnapshotBits> {
        if mask & !VALID_MASK != 0 {
            return Err(UnknownSnapshotBits(mask & !VALID_MASK));
        }
        Ok(Self(mask as u16))
    }
    pub fn from_key_states(states: [i16; 8]) -> Self {
        let mask = states
            .into_iter()
            .zip(SNAPSHOT_BITS)
            .fold(
                0,
                |mask, (state, bit)| if state < 0 { mask | bit } else { mask },
            );
        Self(mask)
    }
    pub fn source_mask(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeGap {
    ClipboardPublicationAndRestore,
    NonAtomicQueuedKeyState,
    PhysicalKeyArbitration,
    EndpointIdentityAndRouting,
    TerminalDeliveryAndCancellation,
    RunnerDurationUnit,
}
pub const RUNTIME_GAPS: [RuntimeGap; 6] = [
    RuntimeGap::ClipboardPublicationAndRestore,
    RuntimeGap::NonAtomicQueuedKeyState,
    RuntimeGap::PhysicalKeyArbitration,
    RuntimeGap::EndpointIdentityAndRouting,
    RuntimeGap::TerminalDeliveryAndCancellation,
    RuntimeGap::RunnerDurationUnit,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasteProtocol {
    snapshot: ModifierSnapshot,
    steps: Vec<KeyStep>,
}
impl PasteProtocol {
    pub fn from_snapshot(snapshot: ModifierSnapshot) -> Self {
        let mut steps = Vec::with_capacity(20);
        let append = |steps: &mut Vec<KeyStep>, role, up: u16| {
            for &(mask, scan, extended) in MODIFIERS.iter().rev() {
                if snapshot.0 & mask != 0 {
                    steps.push(KeyStep {
                        scan_code: scan,
                        flags: extended | up,
                        role,
                        route: Route::EndpointElseOsFallback,
                        delay_argument: 0,
                    });
                }
            }
        };
        append(&mut steps, Role::SuspendSnapshotModifier, 1);
        steps.extend(PASTE_CHORD);
        append(&mut steps, Role::RestoreSnapshotModifier, 0);
        Self { snapshot, steps }
    }
    pub fn snapshot(&self) -> ModifierSnapshot {
        self.snapshot
    }
    /// Protocol diagnostics only: no UnitId, packet, sender or output lease.
    pub fn steps(&self) -> &[KeyStep] {
        &self.steps
    }
    pub fn runtime_gaps(&self) -> &[RuntimeGap] {
        &RUNTIME_GAPS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_matches_independently_extracted_source_fields() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/native-paste-protocol.json"))
                .unwrap();
        assert_eq!(
            serde_json::json!(SAMPLED_VIRTUAL_KEYS),
            fixture["sampled_virtual_keys"]
        );
        for case in fixture["cases"].as_array().unwrap() {
            let p = PasteProtocol::from_snapshot(
                ModifierSnapshot::from_source_mask(case["snapshot_mask"].as_u64().unwrap() as u32)
                    .unwrap(),
            );
            let actual:Vec<_>=p.steps().iter().map(|s|serde_json::json!({"scan_code":s.scan_code,"flags":s.flags,"delay_argument":s.delay_argument,"role":format!("{:?}",s.role),"route":format!("{:?}",s.route)})).collect();
            assert_eq!(serde_json::json!(actual), case["steps"]);
        }
    }
    #[test]
    fn snapshot_uses_high_state_bits_and_exact_source_key_order() {
        assert_eq!(ModifierSnapshot::from_key_states([1; 8]).source_mask(), 0);
        for (index, bit) in [0x100, 0x200, 0x400, 0x800, 0x10, 0x20, 0x40, 0x80]
            .into_iter()
            .enumerate()
        {
            let mut states = [0; 8];
            states[index] = i16::MIN;
            assert_eq!(ModifierSnapshot::from_key_states(states).source_mask(), bit);
        }
        for bit in [1, 2, 4, 8, 0x1000, 0x80000000] {
            assert!(ModifierSnapshot::from_source_mask(bit).is_err());
        }
    }
    #[test]
    fn paste_timing_follows_execution_order_and_source_callback_branch() {
        let p = PasteProtocol::from_snapshot(ModifierSnapshot::from_source_mask(0).unwrap());
        assert_eq!(
            p.steps()
                .iter()
                .map(|s| (s.scan_code, s.flags, s.delay_argument))
                .collect::<Vec<_>>(),
            vec![(0x1d, 0, 30), (0x2f, 0, 20), (0x2f, 1, 0), (0x1d, 1, 10)]
        );
        for step in p.steps() {
            assert_eq!(step.source_runner_delay(false), 0);
            assert_eq!(
                step.source_runner_delay(true),
                u64::from(step.delay_argument) * 1000
            );
            assert_eq!(step.route, Route::EndpointOnly);
        }
        assert_eq!(p.runtime_gaps(), RUNTIME_GAPS);
    }
    #[test]
    fn every_snapshot_preserves_borrowed_modifier_roles_and_bounded_order() {
        for mask in 0..256u32 {
            let snapshot = ModifierSnapshot::from_source_mask(mask << 4).unwrap();
            let p = PasteProtocol::from_snapshot(snapshot);
            let n = mask.count_ones() as usize;
            assert_eq!(p.snapshot(), snapshot);
            assert_eq!(p.steps().len(), 4 + 2 * n);
            assert!(p.steps().len() <= 20);
            assert_eq!(&p.steps()[n..n + 4], PASTE_CHORD);
            for (release, restore) in p.steps()[..n].iter().zip(&p.steps()[n + 4..]) {
                assert_eq!(release.role, Role::SuspendSnapshotModifier);
                assert_eq!(restore.role, Role::RestoreSnapshotModifier);
                assert_eq!(release.scan_code, restore.scan_code);
                assert_eq!(release.flags, restore.flags | 1);
                assert_eq!(release.route, Route::EndpointElseOsFallback);
                assert_eq!(restore.route, release.route);
            }
        }
        let all = PasteProtocol::from_snapshot(ModifierSnapshot::from_source_mask(0xff0).unwrap());
        assert_eq!(
            all.steps()[..8]
                .iter()
                .map(|s| (s.scan_code, s.flags))
                .collect::<Vec<_>>(),
            vec![
                (0x5c, 3),
                (0x5b, 3),
                (0x36, 1),
                (0x2a, 1),
                (0x1d, 3),
                (0x1d, 1),
                (0x38, 3),
                (0x38, 1)
            ]
        );
    }
}
