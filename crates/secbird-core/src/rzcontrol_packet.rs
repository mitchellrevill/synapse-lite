//! Pure codecs for the ordinary 32-byte RZCONTROL input packet.
//!
//! Layout is corroborated by mapping_engine.dll RVA 112C60 / RzCommon.sys
//! keyboard and mouse dispatch. See `docs/RZCONTROL-PACKETS.md` and
//! `docs/DRIVER-CONTROL.md`. These values describe the installed vendor driver,
//! not HID macro bytecode or Windows SendInput's user-mode INPUT structure.
//!
//! Encoding does not open a driver or send input. Converting a UI key, native
//! macro scancode, or modifier into these raw fields requires separate evidence.
//! Only routing word zero and kinds 1/2 are supported; unknown fields stay raw.

use std::fmt;

pub const INPUT_PACKET_BYTES: usize = 32;

/// The 12-byte KEYBOARD_INPUT_DATA payload at packet offset 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardInput {
    pub unit_id: u16,
    pub make_code: u16,
    pub flags: u16,
    pub reserved: u16,
    pub extra_information: u32,
}

/// The 24-byte MOUSE_INPUT_DATA payload at packet offset 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseInput {
    pub unit_id: u16,
    pub flags: u16,
    pub button_flags: u16,
    /// Raw 16-bit button data, including the wheel's signed bit pattern.
    pub button_data: u16,
    pub raw_buttons: u32,
    pub last_x: i32,
    pub last_y: i32,
    pub extra_information: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputPacket {
    Keyboard(KeyboardInput),
    Mouse(MouseInput),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketError {
    WrongLength(usize),
    UnsupportedRouting(u32),
    UnsupportedKind(u32),
    NonzeroKeyboardTail,
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength(actual) => write!(f, "expected 32 input bytes, received {actual}"),
            Self::UnsupportedRouting(value) => {
                write!(f, "unsupported RZCONTROL routing word {value}")
            }
            Self::UnsupportedKind(value) => write!(f, "unsupported RZCONTROL input kind {value}"),
            Self::NonzeroKeyboardTail => f.write_str("ordinary keyboard packet has a nonzero tail"),
        }
    }
}

impl std::error::Error for PacketError {}

impl InputPacket {
    /// Encode explicit raw fields, preserving their little-endian bit patterns.
    pub fn to_bytes(self) -> [u8; INPUT_PACKET_BYTES] {
        let mut bytes = [0_u8; INPUT_PACKET_BYTES];
        match self {
            Self::Keyboard(input) => {
                bytes[4..8].copy_from_slice(&1_u32.to_le_bytes());
                bytes[8..10].copy_from_slice(&input.unit_id.to_le_bytes());
                bytes[10..12].copy_from_slice(&input.make_code.to_le_bytes());
                bytes[12..14].copy_from_slice(&input.flags.to_le_bytes());
                bytes[14..16].copy_from_slice(&input.reserved.to_le_bytes());
                bytes[16..20].copy_from_slice(&input.extra_information.to_le_bytes());
            }
            Self::Mouse(input) => {
                bytes[4..8].copy_from_slice(&2_u32.to_le_bytes());
                bytes[8..10].copy_from_slice(&input.unit_id.to_le_bytes());
                bytes[10..12].copy_from_slice(&input.flags.to_le_bytes());
                bytes[12..14].copy_from_slice(&input.button_flags.to_le_bytes());
                bytes[14..16].copy_from_slice(&input.button_data.to_le_bytes());
                bytes[16..20].copy_from_slice(&input.raw_buttons.to_le_bytes());
                bytes[20..24].copy_from_slice(&input.last_x.to_le_bytes());
                bytes[24..28].copy_from_slice(&input.last_y.to_le_bytes());
                bytes[28..32].copy_from_slice(&input.extra_information.to_le_bytes());
            }
        }
        bytes
    }

    /// Decode only a complete ordinary packet; never reinterpret an unknown kind.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PacketError> {
        if bytes.len() != INPUT_PACKET_BYTES {
            return Err(PacketError::WrongLength(bytes.len()));
        }
        let routing = read_u32(bytes, 0);
        if routing != 0 {
            return Err(PacketError::UnsupportedRouting(routing));
        }
        match read_u32(bytes, 4) {
            1 => {
                if bytes[20..].iter().any(|byte| *byte != 0) {
                    return Err(PacketError::NonzeroKeyboardTail);
                }
                Ok(Self::Keyboard(KeyboardInput {
                    unit_id: read_u16(bytes, 8),
                    make_code: read_u16(bytes, 10),
                    flags: read_u16(bytes, 12),
                    reserved: read_u16(bytes, 14),
                    extra_information: read_u32(bytes, 16),
                }))
            }
            2 => Ok(Self::Mouse(MouseInput {
                unit_id: read_u16(bytes, 8),
                flags: read_u16(bytes, 10),
                button_flags: read_u16(bytes, 12),
                button_data: read_u16(bytes, 14),
                raw_buttons: read_u32(bytes, 16),
                last_x: i32::from_le_bytes(bytes[20..24].try_into().unwrap()),
                last_y: i32::from_le_bytes(bytes[24..28].try_into().unwrap()),
                extra_information: read_u32(bytes, 28),
            })),
            kind => Err(PacketError::UnsupportedKind(kind)),
        }
    }
}

/// Stock internal metadata-mask translation at mapping_engine RVA 1219DC.
///
/// This does not translate a macro's make/break flag, a virtual key or a HID
/// usage. Bit meanings outside the numeric transformation remain unresolved.
pub fn translate_driver_metadata_mask(mask: u32) -> u32 {
    ((mask >> 10) & 1)
        | ((mask >> 8) & 2)
        | ((mask >> 6) & 4)
        | (mask & 0x40)
        | ((mask & 0x20) << 2)
        | ((mask & 0x10) << 4)
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_layout_matches_fixed_kernel_boundary_bytes() {
        let input = InputPacket::Keyboard(KeyboardInput {
            unit_id: 0x1234,
            make_code: 0x001e,
            flags: 0x0002,
            reserved: 0,
            extra_information: 0x0102_0304,
        });
        // Kind at4; 12-byte payload at8; ExtraInformation at16, not20.
        let expected = [
            0, 0, 0, 0, 1, 0, 0, 0, 0x34, 0x12, 0x1e, 0, 2, 0, 0, 0, 4, 3, 2, 1, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(input.to_bytes(), expected);
        assert_eq!(InputPacket::from_bytes(&expected), Ok(input));
    }

    #[test]
    fn mouse_layout_preserves_signed_movement_and_wheel_bits() {
        let input = InputPacket::Mouse(MouseInput {
            unit_id: 0x1234,
            flags: 0x0001,
            button_flags: 0x0400,
            button_data: 0xff88,
            raw_buttons: 0x0102_0304,
            last_x: -2,
            last_y: 0x0102_0304,
            extra_information: 0xa1b2_c3d4,
        });
        let expected = [
            0, 0, 0, 0, 2, 0, 0, 0, 0x34, 0x12, 1, 0, 0, 4, 0x88, 0xff, 4, 3, 2, 1, 0xfe, 0xff,
            0xff, 0xff, 4, 3, 2, 1, 0xd4, 0xc3, 0xb2, 0xa1,
        ];
        assert_eq!(input.to_bytes(), expected);
        assert_eq!(InputPacket::from_bytes(&expected), Ok(input));
    }

    #[test]
    fn keyboard_raw_fields_are_not_reclassified_or_cleared() {
        let input = InputPacket::Keyboard(KeyboardInput {
            unit_id: 0xabcd,
            make_code: 0xffff,
            flags: 0xffff,
            reserved: 0x5678,
            extra_information: u32::MAX,
        });
        assert_eq!(InputPacket::from_bytes(&input.to_bytes()), Ok(input));
    }

    #[test]
    fn incomplete_packets_and_unknown_routing_are_rejected() {
        for length in 0..=64 {
            if length != INPUT_PACKET_BYTES {
                assert_eq!(
                    InputPacket::from_bytes(&vec![0; length]),
                    Err(PacketError::WrongLength(length))
                );
            }
        }
        let mut bytes = [0; INPUT_PACKET_BYTES];
        assert_eq!(
            InputPacket::from_bytes(&bytes),
            Err(PacketError::UnsupportedKind(0))
        );
        bytes[4] = 12;
        assert_eq!(
            InputPacket::from_bytes(&bytes),
            Err(PacketError::UnsupportedKind(12))
        );
        bytes[0] = 1;
        assert_eq!(
            InputPacket::from_bytes(&bytes),
            Err(PacketError::UnsupportedRouting(1))
        );
        bytes[0] = 0;
        bytes[4] = 1;
        bytes[31] = 1;
        assert_eq!(
            InputPacket::from_bytes(&bytes),
            Err(PacketError::NonzeroKeyboardTail)
        );
    }

    #[test]
    fn metadata_mask_has_the_recovered_six_bit_mapping() {
        for (input, output) in [
            (0x400, 1),
            (0x200, 2),
            (0x100, 4),
            (0x40, 0x40),
            (0x20, 0x80),
            (0x10, 0x100),
        ] {
            assert_eq!(translate_driver_metadata_mask(input), output);
        }
        assert_eq!(translate_driver_metadata_mask(u32::MAX), 0x1c7);
        assert_eq!(translate_driver_metadata_mask(0x880f), 0);
    }
}
