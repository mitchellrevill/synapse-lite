//! Pure source-keyboard to RZCONTROL packet planning. No input sender.
//!
//! Evidence: mapping_engine.dll SHA256
//! 6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a,
//! parser10BC0D, constructor10CBC4, setters10D3D2/10D3F8, senders112C60/
//! 134C80 and metadata transform1219DC. See docs/KEYBOARD-SYMBOLIC-BRIDGE.md.

use crate::rzcontrol_packet::{
    translate_driver_metadata_mask, InputPacket, KeyboardInput, INPUT_PACKET_BYTES,
};
use crate::source_macro::{Event, Transition};
use std::fmt;
use std::num::NonZeroU128;

/// Explicit ordinary/E0/E1 make/break flags, never inferred from a scan code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeKeyFlags(u16);

impl NativeKeyFlags {
    pub fn new(raw: u16) -> Result<Self, BridgeError> {
        match raw {
            0..=5 => Ok(Self(raw)),
            _ => Err(BridgeError::UnsupportedFlags(raw)),
        }
    }

    pub fn raw(self) -> u16 {
        self.0
    }

    pub fn transition(self) -> Transition {
        if self.0 & 1 == 0 {
            Transition::Down
        } else {
            Transition::Up
        }
    }
}

/// Caller-assigned symbolic endpoint context; not a handle or packet field.
///
/// There is deliberately no Default. Construction proves neither that this
/// endpoint exists nor that the caller owns it or may route input through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingIdentity {
    endpoint_token: NonZeroU128,
    unit_id: u16,
}

impl RoutingIdentity {
    pub fn new(endpoint_token: NonZeroU128, unit_id: u16) -> Self {
        Self {
            endpoint_token,
            unit_id,
        }
    }

    pub fn endpoint_token(self) -> NonZeroU128 {
        self.endpoint_token
    }
    pub fn unit_id(self) -> u16 {
        self.unit_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceUnitProvenance {
    /// Fresh native constructor10CBC4 zeroes+48; omitted setter leaves it.
    NativeDefaultZero,
    Explicit(u16),
}

/// Native source fields only: no UI key, virtual-key or HID usage conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExplicitNativeKeyboard {
    make_code: u16,
    flags: NativeKeyFlags,
    modifiers: u32,
    source_unit: Option<u16>,
}

impl ExplicitNativeKeyboard {
    pub fn new(
        make_code: u16,
        flags: NativeKeyFlags,
        modifiers: u32,
        source_unit: Option<u16>,
    ) -> Self {
        Self {
            make_code,
            flags,
            modifiers,
            source_unit,
        }
    }

    /// Optional unit/modifiers are explicit caller inputs. The source parser
    /// still rejects those extra action fields rather than silently dropping them.
    pub fn from_source_event(
        event: Event,
        modifiers: u32,
        source_unit: Option<u16>,
    ) -> Result<Self, BridgeError> {
        match event {
            Event::Keyboard {
                scancode,
                flag,
                transition,
            } => {
                let flags = NativeKeyFlags::new(flag)?;
                if flags.transition() != transition {
                    return Err(BridgeError::InconsistentTransition);
                }
                Ok(Self::new(scancode, flags, modifiers, source_unit))
            }
            _ => Err(BridgeError::NotKeyboard),
        }
    }

    pub fn make_code(self) -> u16 {
        self.make_code
    }
    pub fn flags(self) -> NativeKeyFlags {
        self.flags
    }
    pub fn modifiers(self) -> u32 {
        self.modifiers
    }
    pub fn source_unit(self) -> Option<u16> {
        self.source_unit
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeError {
    UnsupportedFlags(u16),
    NotKeyboard,
    InconsistentTransition,
    UnitIdConflict { source: u16, routing: u16 },
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFlags(raw) => write!(f, "unsupported native keyboard flags {raw:#x}"),
            Self::NotKeyboard => f.write_str("source event is not a keyboard event"),
            Self::InconsistentTransition => {
                f.write_str("source transition disagrees with native flags")
            }
            Self::UnitIdConflict { source, routing } => write!(
                f,
                "source UnitId {source} conflicts with routing UnitId {routing}"
            ),
        }
    }
}
impl std::error::Error for BridgeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGap {
    IdentityNotValidated,
    EndpointRoutingNotValidated,
    AsynchronousCompletion,
    Cancellation,
    HeldOutputOwnership,
}
const RUNTIME_GAPS: [RuntimeGap; 5] = [
    RuntimeGap::IdentityNotValidated,
    RuntimeGap::EndpointRoutingNotValidated,
    RuntimeGap::AsynchronousCompletion,
    RuntimeGap::Cancellation,
    RuntimeGap::HeldOutputOwnership,
];

/// Submit-helper boolean acceptance, explicitly not I/O completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstSubmission {
    Accepted,
    Rejected,
}

/// Fixed storage for at most two packets, with a conditional second step.
///
/// This immutable value does not send, track completion, or grant ownership.
/// It intentionally exposes no unconditional packet iterator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardPacketPlan {
    identity: RoutingIdentity,
    source_unit: SourceUnitProvenance,
    first: [u8; INPUT_PACKET_BYTES],
    second_after_acceptance: Option<[u8; INPUT_PACKET_BYTES]>,
}

impl KeyboardPacketPlan {
    pub fn identity(&self) -> RoutingIdentity {
        self.identity
    }
    pub fn source_unit_provenance(&self) -> SourceUnitProvenance {
        self.source_unit
    }
    pub fn first_packet(&self) -> &[u8; INPUT_PACKET_BYTES] {
        &self.first
    }
    pub fn has_conditional_second(&self) -> bool {
        self.second_after_acceptance.is_some()
    }

    /// Source senders recurse only after the first submit helper returns true.
    /// This queries a symbolic condition; repeated calls do not represent sends
    /// or prove first completion, exactly-once handling, or a cancellation drain.
    pub fn second_after_first_submission(
        &self,
        result: FirstSubmission,
    ) -> Option<&[u8; INPUT_PACKET_BYTES]> {
        match result {
            FirstSubmission::Accepted => self.second_after_acceptance.as_ref(),
            FirstSubmission::Rejected => None,
        }
    }

    /// Always unresolved, even when all bytes have a source-backed encoding.
    pub fn runtime_gaps(&self) -> &'static [RuntimeGap] {
        &RUNTIME_GAPS
    }
}

pub fn plan_keyboard_packets(
    event: ExplicitNativeKeyboard,
    identity: RoutingIdentity,
) -> Result<KeyboardPacketPlan, BridgeError> {
    let unit_id = event.source_unit.unwrap_or(0);
    if unit_id != identity.unit_id {
        return Err(BridgeError::UnitIdConflict {
            source: unit_id,
            routing: identity.unit_id,
        });
    }
    let source_unit = match event.source_unit {
        Some(unit) => SourceUnitProvenance::Explicit(unit),
        None => SourceUnitProvenance::NativeDefaultZero,
    };
    let extra = translate_driver_metadata_mask(event.modifiers);
    let first = packet(unit_id, event.make_code, event.flags.raw(), extra);
    // Sender112C60/134C80: original1D+E1 triggers a fresh45 object. Its
    // modifiers receive T(original), then recursive encoding applies T again.
    let second_after_acceptance = if event.make_code == 0x1d && event.flags.raw() & 4 != 0 {
        Some(packet(
            unit_id,
            0x45,
            event.flags.raw() & 1,
            translate_driver_metadata_mask(extra),
        ))
    } else {
        None
    };
    Ok(KeyboardPacketPlan {
        identity,
        source_unit,
        first,
        second_after_acceptance,
    })
}

fn packet(
    unit_id: u16,
    make_code: u16,
    flags: u16,
    extra_information: u32,
) -> [u8; INPUT_PACKET_BYTES] {
    InputPacket::Keyboard(KeyboardInput {
        unit_id,
        make_code,
        flags,
        reserved: 0,
        extra_information,
    })
    .to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(unit: u16) -> RoutingIdentity {
        RoutingIdentity::new(NonZeroU128::new(0x123456).unwrap(), unit)
    }
    fn event(scan: u16, flag: u16, modifiers: u32, unit: Option<u16>) -> ExplicitNativeKeyboard {
        ExplicitNativeKeyboard::new(scan, NativeKeyFlags::new(flag).unwrap(), modifiers, unit)
    }
    fn keyboard(bytes: &[u8]) -> KeyboardInput {
        match InputPacket::from_bytes(bytes).unwrap() {
            InputPacket::Keyboard(k) => k,
            _ => panic!("wrong packet kind"),
        }
    }

    #[test]
    fn ordinary_native_key_matches_fixed_bytes_and_keeps_gaps() {
        let plan = plan_keyboard_packets(event(0x1e, 0, 0, None), identity(0)).unwrap();
        assert_eq!(
            *plan.first_packet(),
            [
                0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0x1e, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0
            ]
        );
        assert_eq!(
            plan.source_unit_provenance(),
            SourceUnitProvenance::NativeDefaultZero
        );
        assert_eq!(plan.identity(), identity(0));
        assert_eq!(plan.runtime_gaps(), &RUNTIME_GAPS);
        assert!(!plan.has_conditional_second());
        assert!(plan
            .second_after_first_submission(FirstSubmission::Accepted)
            .is_none());
    }

    #[test]
    fn explicit_e0_direction_is_preserved_without_inferring_from_scan() {
        for flag in 0..=3 {
            let plan = plan_keyboard_packets(event(0x1d, flag, 0, None), identity(0)).unwrap();
            assert_eq!(keyboard(plan.first_packet()).flags, flag);
            assert_eq!(keyboard(plan.first_packet()).make_code, 0x1d);
            assert!(!plan.has_conditional_second());
        }
        // Explicit0 stays0 even for scan code with a nonzero high byte.
        let p = plan_keyboard_packets(event(0xe01d, 0, 0, None), identity(0)).unwrap();
        assert_eq!(keyboard(p.first_packet()).make_code, 0xe01d);
        assert_eq!(keyboard(p.first_packet()).flags, 0);
    }

    #[test]
    fn pause_second_step_requires_first_boolean_acceptance_and_double_metadata() {
        for (flag, second_flag) in [(4, 0), (5, 1)] {
            let p = plan_keyboard_packets(event(0x1d, flag, 0xffff_ffff, Some(7)), identity(7))
                .unwrap();
            assert_eq!(
                keyboard(p.first_packet()),
                KeyboardInput {
                    unit_id: 7,
                    make_code: 0x1d,
                    flags: flag,
                    reserved: 0,
                    extra_information: 0x1c7
                }
            );
            assert!(p.has_conditional_second());
            assert!(p
                .second_after_first_submission(FirstSubmission::Rejected)
                .is_none());
            assert_eq!(
                keyboard(
                    p.second_after_first_submission(FirstSubmission::Accepted)
                        .unwrap()
                ),
                KeyboardInput {
                    unit_id: 7,
                    make_code: 0x45,
                    flags: second_flag,
                    reserved: 0,
                    extra_information: 0x44
                }
            );
            assert_eq!(p.runtime_gaps(), &RUNTIME_GAPS);
        }
    }

    #[test]
    fn prefix_flag_alone_does_not_create_pause_second_step() {
        for (scan, flag) in [(0x45, 4), (0x45, 5), (0x37, 2), (0x1e, 4)] {
            let p = plan_keyboard_packets(event(scan, flag, 0, None), identity(0)).unwrap();
            assert_eq!(keyboard(p.first_packet()).flags, flag);
            assert!(!p.has_conditional_second());
        }
    }

    #[test]
    fn explicit_source_unit_and_native_default_must_match_identity() {
        assert_eq!(
            plan_keyboard_packets(event(30, 0, 0, Some(2)), identity(3)),
            Err(BridgeError::UnitIdConflict {
                source: 2,
                routing: 3
            })
        );
        assert_eq!(
            plan_keyboard_packets(event(30, 0, 0, None), identity(3)),
            Err(BridgeError::UnitIdConflict {
                source: 0,
                routing: 3
            })
        );
        let p = plan_keyboard_packets(event(30, 0, 0, Some(3)), identity(3)).unwrap();
        assert_eq!(
            p.source_unit_provenance(),
            SourceUnitProvenance::Explicit(3)
        );
        assert_eq!(keyboard(p.first_packet()).unit_id, 3);
        assert_eq!(&p.first_packet()[0..4], &[0; 4]);
        assert_eq!(p.identity().endpoint_token(), identity(3).endpoint_token());
    }

    #[test]
    fn metadata_vectors_and_double_transform_match_numeric_evidence() {
        for (input, first, second) in [
            (0x400, 1, 0),
            (0x200, 2, 0),
            (0x100, 4, 0),
            (0x40, 0x40, 0x40),
            (0x20, 0x80, 0),
            (0x10, 0x100, 4),
            (u32::MAX, 0x1c7, 0x44),
        ] {
            let p = plan_keyboard_packets(event(0x1d, 4, input, None), identity(0)).unwrap();
            assert_eq!(keyboard(p.first_packet()).extra_information, first);
            assert_eq!(
                keyboard(
                    p.second_after_first_submission(FirstSubmission::Accepted)
                        .unwrap()
                )
                .extra_information,
                second
            );
        }
        // Exhaust every relevant low-bit combination; independent composition
        // identity from static dataflow, rather than a copy of transform T.
        for mask in 0..=u16::MAX as u32 {
            let t = translate_driver_metadata_mask(mask);
            assert_eq!(
                translate_driver_metadata_mask(t),
                (mask & 0x40) | ((mask & 0x10) >> 2)
            );
        }
    }

    #[test]
    fn invalid_prefixes_and_public_event_inconsistency_are_rejected() {
        for flag in [6, 7, 8, 0x8000, 65535] {
            assert_eq!(
                NativeKeyFlags::new(flag),
                Err(BridgeError::UnsupportedFlags(flag))
            );
        }
        assert_eq!(
            ExplicitNativeKeyboard::from_source_event(Event::DelayMs(0), 0, None),
            Err(BridgeError::NotKeyboard)
        );
        assert_eq!(
            ExplicitNativeKeyboard::from_source_event(
                Event::Keyboard {
                    scancode: 29,
                    flag: 3,
                    transition: Transition::Down
                },
                0,
                None
            ),
            Err(BridgeError::InconsistentTransition)
        );
        for flag in 0..=5 {
            let transition = if flag & 1 == 0 {
                Transition::Down
            } else {
                Transition::Up
            };
            let e = ExplicitNativeKeyboard::from_source_event(
                Event::Keyboard {
                    scancode: 29,
                    flag,
                    transition,
                },
                0,
                None,
            )
            .unwrap();
            assert_eq!(e.flags().raw(), flag);
            assert_eq!(e.flags().transition(), transition);
        }
    }

    #[test]
    fn planning_is_fixed_size_and_does_not_mutate_event() {
        assert!(std::mem::size_of::<KeyboardPacketPlan>() <= 160);
        let e = event(0x1d, 5, 0x50, Some(2));
        let before = e;
        let _ = plan_keyboard_packets(e, identity(2)).unwrap();
        assert_eq!(e, before);
        assert_eq!(
            (
                e.make_code(),
                e.flags().raw(),
                e.modifiers(),
                e.source_unit()
            ),
            (0x1d, 5, 0x50, Some(2))
        );
    }
}
