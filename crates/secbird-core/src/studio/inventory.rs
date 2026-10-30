//! Current exported `leds` and saved `ledRef` inventories, without rewriting data.
use super::{check_complexity, invalid, MAX_DOCUMENT_BYTES};
use crate::Result;
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::value::RawValue;
use std::{collections::BTreeSet, fmt, marker::PhantomData};

pub const MAX_DEVICE_LEDS: usize = 512;
struct BorrowedLeds<'a>(Vec<&'a RawValue>);
impl<'de: 'a, 'a> Deserialize<'de> for BorrowedLeds<'a> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct Leds<'a>(PhantomData<&'a ()>);
        impl<'de: 'a, 'a> Visitor<'de> for Leds<'a> {
            type Value = BorrowedLeds<'a>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("at most 512 LED records")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut leds = Vec::new();
                while let Some(raw) = sequence.next_element::<&'de RawValue>()? {
                    if leds.len() == MAX_DEVICE_LEDS {
                        return Err(de::Error::custom("LED inventory exceeds 512 records"));
                    }
                    leds.push(raw);
                }
                Ok(BorrowedLeds(leds))
            }
        }
        deserializer.deserialize_seq(Leds(PhantomData))
    }
}
#[derive(Deserialize)]
struct Device<'a> {
    #[serde(default, borrow, deserialize_with = "read_field")]
    leds: Option<BorrowedLeds<'a>>,
    #[serde(default, borrow, rename = "ledRef", deserialize_with = "read_field")]
    reference: Option<BorrowedLeds<'a>>,
}
fn read_field<'de: 'a, 'a, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<BorrowedLeds<'a>>, D::Error> {
    BorrowedLeds::deserialize(deserializer).map(Some)
}
#[derive(Debug, Clone, Copy)]
pub struct InventoryLed<'a> {
    pub row: u16,
    pub col: u16,
    pub raw: &'a RawValue,
}

fn decode<'a>(leds: BorrowedLeds<'a>) -> Result<Vec<InventoryLed<'a>>> {
    #[derive(Deserialize)]
    struct Coordinate {
        row: u16,
        col: u16,
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::with_capacity(leds.0.len());
    for raw in leds.0 {
        let coordinate: Coordinate = serde_json::from_str(raw.get()).map_err(invalid)?;
        if !seen.insert((coordinate.row, coordinate.col)) {
            return Err(invalid("duplicate LED inventory coordinate"));
        }
        result.push(InventoryLed {
            row: coordinate.row,
            col: coordinate.col,
            raw,
        });
    }
    Ok(result)
}

/// Prefer current exported records when present. When both forms are supplied,
/// their coordinate sets must agree; historical reference data is not silently
/// used to remap a conflicting current inventory. Raw metadata stays borrowed.
pub fn led_inventory(device_json: &str) -> Result<Vec<InventoryLed<'_>>> {
    if device_json.len() > MAX_DOCUMENT_BYTES {
        return Err(invalid("device inventory exceeds 8 MiB"));
    }
    check_complexity(device_json.as_bytes())?;
    let device: Device = serde_json::from_str(device_json).map_err(invalid)?;
    let current = device.leds.map(decode).transpose()?;
    let reference = device.reference.map(decode).transpose()?;
    match (current, reference) {
        (Some(current), Some(reference)) => {
            let coords = |items: &[InventoryLed<'_>]| {
                items
                    .iter()
                    .map(|led| (led.row, led.col))
                    .collect::<BTreeSet<_>>()
            };
            if coords(&current) != coords(&reference) {
                return Err(invalid(
                    "leds and ledRef disagree; explicit layout reconciliation is required",
                ));
            }
            Ok(current)
        }
        (Some(leds), None) | (None, Some(leds)) => Ok(leds),
        (None, None) => Err(invalid("placement has neither leds nor ledRef inventory")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_and_reference_forms_keep_raw_metadata_and_current_order() {
        let current =
            r#"{"leds":[{"row":2,"col":3,"future":9007199254740993123456789},{"row":0,"col":1}]}"#;
        let reference = current.replace("leds", "ledRef");
        for json in [&current, &reference.as_str()] {
            let leds = led_inventory(json).unwrap();
            assert_eq!((leds[0].row, leds[0].col), (2, 3));
            assert!(leds[0].raw.get().contains("9007199254740993123456789"));
        }
        let both = r#"{"leds":[{"row":2,"col":3,"future":1.2300e-7},{"row":0,"col":1}],"ledRef":[{"row":0,"col":1},{"row":2,"col":3}]}"#;
        let leds = led_inventory(both).unwrap();
        assert_eq!((leds[0].row, leds[0].col), (2, 3));
        assert!(leds[0].raw.get().contains("1.2300e-7"));
    }
    #[test]
    fn conflicting_duplicate_invalid_and_excessive_inventories_refuse() {
        for json in [
            r#"{"leds":[{"row":0,"col":1}],"ledRef":[{"row":0,"col":2}]}"#,
            r#"{"leds":[{"row":0,"col":1},{"row":0,"col":1}]}"#,
            r#"{"ledRef":[{"row":0.5,"col":1}]}"#,
            r#"{"leds":17}"#,
            r#"{"leds":null,"ledRef":[]}"#,
            r#"{"leds":[],"leds":[]}"#,
            r#"{}"#,
        ] {
            assert!(led_inventory(json).is_err(), "accepted {json}");
        }
        let entries = (0..513)
            .map(|col| format!("{{\"row\":0,\"col\":{col}}}"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(led_inventory(&format!("{{\"leds\":[{entries}]}}")).is_err());
        assert!(led_inventory(r#"{"leds":[]}"#).unwrap().is_empty());
    }
}
