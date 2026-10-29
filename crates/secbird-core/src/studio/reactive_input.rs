//! Explicit Studio LED input identities, independent of input observation.
//!
//! The source joins LedInputMap.MatrixPos to row/col and copies InputType and
//! JSON.stringify(InputData). A scan code is never a row/column or a virtual key.
//! This module opens no devices and does not authorize physical input capture.
use super::{invalid, inventory::led_inventory};
use crate::Result;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyboardKey {
    pub make_code: u16,
    /// Diagnostic prefix convention: 0, E0=2 or E1=4; break is a transition.
    /// Physical source-to-Windows translation still needs controlled proof.
    pub prefix: u16,
}
impl KeyboardKey {
    pub fn new(make_code: u16, prefix: u16) -> Result<Self> {
        if make_code == 0 || make_code >= 0xff || ![0, 2, 4].contains(&prefix) {
            return Err(invalid("unsupported Reactive keyboard identity"));
        }
        Ok(Self { make_code, prefix })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LedAddress {
    pub row: u16,
    pub col: u16,
}

/// Exact placement identity plus explicit source memberships. Unknown input
/// types remain unmapped; they never fall back to another keyboard or LED.
pub struct ReactiveInputMap {
    placement: String,
    keys: BTreeMap<KeyboardKey, Vec<LedAddress>>,
    unmapped: usize,
}
impl ReactiveInputMap {
    pub fn from_device_json(device: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct Placement {
            id: String,
        }
        #[derive(Deserialize)]
        struct Input {
            #[serde(default, rename = "inputType")]
            kind: Option<Box<serde_json::value::RawValue>>,
            #[serde(default, rename = "inputData")]
            data: Option<Box<serde_json::value::RawValue>>,
        }
        let inventory = led_inventory(device)?;
        let placement: Placement = serde_json::from_str(device).map_err(invalid)?;
        if placement.id.is_empty() || placement.id.len() > 512 {
            return Err(invalid("Reactive placement identity must be 1..512 bytes"));
        }
        let mut map = Self {
            placement: placement.id,
            keys: BTreeMap::new(),
            unmapped: 0,
        };
        for led in inventory {
            let input: Input = serde_json::from_str(led.raw.get()).map_err(invalid)?;
            let kind = input
                .kind
                .as_ref()
                .and_then(|raw| serde_json::from_str::<String>(raw.get()).ok());
            if kind.as_deref() != Some("kbd") {
                map.unmapped += 1;
                continue;
            }
            let data = input
                .data
                .ok_or_else(|| invalid("kbd LED is missing inputData"))?;
            if data.get().len() > 128 {
                return Err(invalid("Reactive inputData exceeds 128 bytes"));
            }
            let data: String = serde_json::from_str(data.get()).map_err(invalid)?;
            let [code, flags]: [u16; 2] = serde_json::from_str(&data).map_err(invalid)?;
            let key = KeyboardKey::new(code, flags)?;
            map.keys.entry(key).or_default().push(LedAddress {
                row: led.row,
                col: led.col,
            });
        }
        Ok(map)
    }
    pub fn placement(&self) -> &str {
        &self.placement
    }
    pub fn leds(&self, placement: &str, key: KeyboardKey) -> &[LedAddress] {
        if placement != self.placement {
            return &[];
        }
        self.keys.get(&key).map_or(&[], Vec::as_slice)
    }
    pub fn unmapped_leds(&self) -> usize {
        self.unmapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_identity_is_neither_virtual_key_nor_matrix_position() {
        let map = ReactiveInputMap::from_device_json(
            r#"{"id":"k","leds":[
            {"row":5,"col":3,"inputType":"kbd","inputData":"[56,0]"},
            {"row":5,"col":12,"inputType":"kbd","inputData":"[56,2]"},
            {"row":5,"col":13,"inputType":"dkm","inputData":"[1,16]"},
            {"row":0,"col":0}]}"#,
        )
        .unwrap();
        assert_eq!(
            map.leds("k", KeyboardKey::new(56, 0).unwrap()),
            &[LedAddress { row: 5, col: 3 }]
        );
        assert_eq!(
            map.leds("k", KeyboardKey::new(56, 2).unwrap()),
            &[LedAddress { row: 5, col: 12 }]
        );
        assert!(map
            .leds("other", KeyboardKey::new(56, 0).unwrap())
            .is_empty());
        assert!(map.leds("k", KeyboardKey::new(65, 0).unwrap()).is_empty());
        assert_eq!(map.unmapped_leds(), 2);
    }
    #[test]
    fn one_input_can_address_multiple_explicit_leds_without_deduplicating_keys() {
        let map = ReactiveInputMap::from_device_json(
            r#"{"id":"k","ledRef":[
            {"row":1,"col":1,"inputType":"kbd","inputData":"[30,0]"},
            {"row":1,"col":2,"inputType":"kbd","inputData":"[30,0]"}]}"#,
        )
        .unwrap();
        assert_eq!(map.leds("k", KeyboardKey::new(30, 0).unwrap()).len(), 2);
    }
    #[test]
    fn malformed_known_metadata_fails_atomically_instead_of_guessing() {
        for data in [
            "null", "[30]", "[30,0,1]", "[30,1]", "[0,0]", "[255,0]", "[256,0]", "[30,6]",
        ] {
            let device=serde_json::json!({"id":"k","leds":[{"row":0,"col":1,"inputType":"kbd","inputData":data}]}).to_string();
            assert!(
                ReactiveInputMap::from_device_json(&device).is_err(),
                "{data}"
            );
        }
        assert!(ReactiveInputMap::from_device_json(r#"{"id":"","leds":[]}"#).is_err());
    }
    #[test]
    fn future_nonkeyboard_fields_are_opaque_and_do_not_enable_a_fallback() {
        let raw = r#"{"id":"k","leds":[{"row":0,"col":1,"inputType":123,"inputData":{"future":9007199254740993123456789}}]}"#;
        let map = ReactiveInputMap::from_device_json(raw).unwrap();
        assert_eq!(map.unmapped_leds(), 1);
        assert!(map.leds("k", KeyboardKey::new(1, 0).unwrap()).is_empty());
    }
}
