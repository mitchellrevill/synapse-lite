//! Pure source mouse-event to RZCONTROL encoding. No input sender.
//!
//! mapping_engine.dll SHA256 6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a:
//! parser10BFED, constructor10D8A8, setters10E18C/10E2F8,
//! writers112C60/134C80 and metadata transform1219DC.
//! See docs/MOUSE-SYMBOLIC-BRIDGE.md for source evidence and runtime limits.

use crate::keyboard_symbolic_bridge::RoutingIdentity;
use crate::rzcontrol_packet::{
    translate_driver_metadata_mask, InputPacket, MouseInput, INPUT_PACKET_BYTES,
};
use crate::source_macro::{Button, Event, Transition, WheelAxis};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceUnitProvenance {
    /// Fresh mouse constructor10D8A8 zeroes+38; omitted setter leaves it.
    NativeDefaultZero,
    /// Setter10E2F8 records both the wire unit and optional presence.
    Explicit(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeError {
    NotMouse,
    UnsupportedButtonId(u16),
    InconsistentButton,
    InconsistentTransition,
    UnsupportedWheelId(u16),
    InconsistentWheelAxis,
    UnsupportedWheelData(u16),
    InconsistentWheelDelta,
    UnitIdConflict { source: u16, routing: u16 },
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotMouse => f.write_str("source event is not a supported mouse event"),
            Self::UnsupportedButtonId(id) => write!(f, "unsupported source mouse button ID {id}"),
            Self::InconsistentButton => f.write_str("source button disagrees with its native ID"),
            Self::InconsistentTransition => {
                f.write_str("source transition disagrees with its native mouse ID")
            }
            Self::UnsupportedWheelId(id) => write!(f, "unsupported source wheel ID {id}"),
            Self::InconsistentWheelAxis => {
                f.write_str("source wheel axis disagrees with its native ID")
            }
            Self::UnsupportedWheelData(data) => write!(f, "unsupported source wheel data {data}"),
            Self::InconsistentWheelDelta => {
                f.write_str("source wheel delta disagrees with its raw 16-bit data")
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
    ModifierExpansion,
}
const RUNTIME_GAPS: [RuntimeGap; 6] = [
    RuntimeGap::IdentityNotValidated,
    RuntimeGap::EndpointRoutingNotValidated,
    RuntimeGap::AsynchronousCompletion,
    RuntimeGap::Cancellation,
    RuntimeGap::HeldOutputOwnership,
    RuntimeGap::ModifierExpansion,
];

/// One immutable packet and source provenance; no sender or release operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MousePacketPlan {
    identity: RoutingIdentity,
    source_event: Event,
    source_unit: SourceUnitProvenance,
    modifiers: u32,
    packet: [u8; INPUT_PACKET_BYTES],
}

impl MousePacketPlan {
    pub fn identity(&self) -> RoutingIdentity {
        self.identity
    }
    pub fn source_event(&self) -> Event {
        self.source_event
    }
    pub fn source_unit_provenance(&self) -> SourceUnitProvenance {
        self.source_unit
    }
    pub fn modifiers(&self) -> u32 {
        self.modifiers
    }
    pub fn packet(&self) -> &[u8; INPUT_PACKET_BYTES] {
        &self.packet
    }
    /// Always unresolved, including when every packet byte is source-backed.
    pub fn runtime_gaps(&self) -> &'static [RuntimeGap] {
        &RUNTIME_GAPS
    }
}

/// Encode explicit source-native fields, never a UI/HID/Win32 conversion.
///
/// RoutingIdentity has no Default and is bookkeeping, not an OS endpoint.
/// Missing source_unit means native zero, never a wildcard. Optional metadata
/// is supplied separately; this does not broaden the source JSON parser.
pub fn plan_mouse_packet(
    event: Event,
    identity: RoutingIdentity,
    modifiers: u32,
    source_unit: Option<u16>,
) -> Result<MousePacketPlan, BridgeError> {
    let (button_flags, button_data) = match event {
        Event::MouseButton {
            id,
            button,
            transition,
        } => {
            let (expected_button, expected_transition) = match id {
                1 => (Button::Left, Transition::Down),
                2 => (Button::Left, Transition::Up),
                4 => (Button::Right, Transition::Down),
                8 => (Button::Right, Transition::Up),
                16 => (Button::Middle, Transition::Down),
                32 => (Button::Middle, Transition::Up),
                64 => (Button::Button4, Transition::Down),
                128 => (Button::Button4, Transition::Up),
                256 => (Button::Button5, Transition::Down),
                512 => (Button::Button5, Transition::Up),
                _ => return Err(BridgeError::UnsupportedButtonId(id)),
            };
            if button != expected_button {
                return Err(BridgeError::InconsistentButton);
            }
            if transition != expected_transition {
                return Err(BridgeError::InconsistentTransition);
            }
            (id, 0)
        }
        Event::Wheel {
            id,
            axis,
            raw_data,
            delta,
        } => {
            let expected_axis = match id {
                1024 => WheelAxis::Vertical,
                2048 => WheelAxis::Horizontal,
                _ => return Err(BridgeError::UnsupportedWheelId(id)),
            };
            if axis != expected_axis {
                return Err(BridgeError::InconsistentWheelAxis);
            }
            if raw_data != 120 && raw_data != 65416 {
                return Err(BridgeError::UnsupportedWheelData(raw_data));
            }
            if delta != raw_data as i16 {
                return Err(BridgeError::InconsistentWheelDelta);
            }
            (id, raw_data)
        }
        _ => return Err(BridgeError::NotMouse),
    };

    let unit_id = source_unit.unwrap_or(0);
    if unit_id != identity.unit_id() {
        return Err(BridgeError::UnitIdConflict {
            source: unit_id,
            routing: identity.unit_id(),
        });
    }
    let provenance = match source_unit {
        Some(unit) => SourceUnitProvenance::Explicit(unit),
        None => SourceUnitProvenance::NativeDefaultZero,
    };
    let packet = InputPacket::Mouse(MouseInput {
        unit_id,
        // Fresh constructor10D8A8 zeros these; button/wheel setters leave them.
        flags: 0,
        button_flags,
        button_data,
        raw_buttons: 0,
        last_x: 0,
        last_y: 0,
        extra_information: translate_driver_metadata_mask(modifiers),
    })
    .to_bytes();
    Ok(MousePacketPlan {
        identity,
        source_event: event,
        source_unit: provenance,
        modifiers,
        packet,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU128;

    fn identity(unit: u16) -> RoutingIdentity {
        RoutingIdentity::new(NonZeroU128::new(7).unwrap(), unit)
    }
    fn left_down() -> Event {
        Event::MouseButton {
            id: 1,
            button: Button::Left,
            transition: Transition::Down,
        }
    }

    #[test]
    fn static_derived_packet_vectors_cover_all_buttons_and_wheels() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/native-mouse-packets-static-derived.json"
        ))
        .unwrap();
        let vectors = fixture["vectors"].as_array().unwrap();
        assert_eq!(vectors.len(), 14);
        for vector in vectors {
            let id = vector["id"].as_u64().unwrap() as u16;
            let event = match vector["button"].as_str() {
                Some(name) => Event::MouseButton {
                    id,
                    button: match name {
                        "Left" => Button::Left,
                        "Right" => Button::Right,
                        "Middle" => Button::Middle,
                        "Button4" => Button::Button4,
                        "Button5" => Button::Button5,
                        _ => panic!("unknown fixture button"),
                    },
                    transition: match vector["transition"].as_str().unwrap() {
                        "Down" => Transition::Down,
                        "Up" => Transition::Up,
                        _ => panic!("unknown fixture transition"),
                    },
                },
                None => Event::Wheel {
                    id,
                    axis: match vector["axis"].as_str().unwrap() {
                        "Vertical" => WheelAxis::Vertical,
                        "Horizontal" => WheelAxis::Horizontal,
                        _ => panic!("unknown fixture axis"),
                    },
                    raw_data: vector["raw_data"].as_u64().unwrap() as u16,
                    delta: vector["delta"].as_i64().unwrap() as i16,
                },
            };
            let unit = vector["unit"].as_u64().unwrap() as u16;
            let modifiers = vector["modifiers"].as_u64().unwrap() as u32;
            let result = plan_mouse_packet(event, identity(unit), modifiers, Some(unit)).unwrap();
            let bytes: Vec<u8> = vector["packet_hex"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .map(|hex| u8::from_str_radix(hex, 16).unwrap())
                .collect();
            assert_eq!(result.packet().as_slice(), bytes, "{}", vector["label"]);
            assert_eq!(result.source_event(), event);
            assert_eq!(result.modifiers(), modifiers);
        }
    }

    #[test]
    fn omitted_unit_is_zero_not_a_routing_wildcard() {
        assert_eq!(
            plan_mouse_packet(left_down(), identity(1), 0, None),
            Err(BridgeError::UnitIdConflict {
                source: 0,
                routing: 1
            })
        );
        assert_eq!(
            plan_mouse_packet(left_down(), identity(1), 0, Some(2)),
            Err(BridgeError::UnitIdConflict {
                source: 2,
                routing: 1
            })
        );
        assert!(plan_mouse_packet(left_down(), identity(1), 0, Some(1)).is_ok());
    }

    #[test]
    fn explicit_zero_and_missing_unit_preserve_distinct_provenance() {
        let missing = plan_mouse_packet(left_down(), identity(0), 0, None).unwrap();
        let explicit = plan_mouse_packet(left_down(), identity(0), 0, Some(0)).unwrap();
        assert_eq!(missing.packet(), explicit.packet());
        assert_eq!(
            missing.source_unit_provenance(),
            SourceUnitProvenance::NativeDefaultZero
        );
        assert_eq!(
            explicit.source_unit_provenance(),
            SourceUnitProvenance::Explicit(0)
        );
        assert_ne!(missing, explicit);
    }

    #[test]
    fn modifiers_are_transformed_once_and_constructor_zeros_are_retained() {
        let event = left_down();
        let plan = plan_mouse_packet(event, identity(0), 0x10, None).unwrap();
        let InputPacket::Mouse(raw) = InputPacket::from_bytes(plan.packet()).unwrap() else {
            panic!("expected mouse packet");
        };
        assert_eq!(raw.extra_information, 0x100); // A second T would produce 4.
        assert_eq!(
            (raw.flags, raw.raw_buttons, raw.last_x, raw.last_y),
            (0, 0, 0, 0)
        );
        assert_eq!(event, left_down());
        assert_eq!(plan.identity(), identity(0));
        assert_eq!(plan.runtime_gaps(), &RUNTIME_GAPS);
        assert_eq!(std::mem::size_of_val(plan.packet()), 32);
    }

    #[test]
    fn inconsistent_public_button_fields_and_combined_ids_fail() {
        let cases = [
            (
                Event::MouseButton {
                    id: 1,
                    button: Button::Right,
                    transition: Transition::Down,
                },
                BridgeError::InconsistentButton,
            ),
            (
                Event::MouseButton {
                    id: 1,
                    button: Button::Left,
                    transition: Transition::Up,
                },
                BridgeError::InconsistentTransition,
            ),
            (
                Event::MouseButton {
                    id: 2,
                    button: Button::Left,
                    transition: Transition::Down,
                },
                BridgeError::InconsistentTransition,
            ),
            (
                Event::MouseButton {
                    id: 3,
                    button: Button::Left,
                    transition: Transition::Down,
                },
                BridgeError::UnsupportedButtonId(3),
            ),
        ];
        for (event, error) in cases {
            assert_eq!(plan_mouse_packet(event, identity(0), 0, None), Err(error));
        }
    }

    #[test]
    fn inconsistent_wheel_fields_and_other_events_fail() {
        let cases = [
            (
                Event::Wheel {
                    id: 1024,
                    axis: WheelAxis::Horizontal,
                    raw_data: 120,
                    delta: 120,
                },
                BridgeError::InconsistentWheelAxis,
            ),
            (
                Event::Wheel {
                    id: 1,
                    axis: WheelAxis::Vertical,
                    raw_data: 120,
                    delta: 120,
                },
                BridgeError::UnsupportedWheelId(1),
            ),
            (
                Event::Wheel {
                    id: 2048,
                    axis: WheelAxis::Horizontal,
                    raw_data: 65416,
                    delta: 120,
                },
                BridgeError::InconsistentWheelDelta,
            ),
            (
                Event::Wheel {
                    id: 1024,
                    axis: WheelAxis::Vertical,
                    raw_data: 0,
                    delta: 0,
                },
                BridgeError::UnsupportedWheelData(0),
            ),
            (Event::DelayMs(0), BridgeError::NotMouse),
            (
                Event::Keyboard {
                    scancode: 1,
                    flag: 0,
                    transition: Transition::Down,
                    modifiers: None,
                    unit_id: None,
                },
                BridgeError::NotMouse,
            ),
        ];
        for (event, error) in cases {
            assert_eq!(plan_mouse_packet(event, identity(0), 0, None), Err(error));
        }
    }
}
