//! Editing retained randomized delay rows, without choosing a runtime sample.
use super::*;

pub const MAX_MS: u32 = 5000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub min_ms: u32,
    pub max_ms: u32,
}
impl Range {
    pub fn new(min_ms: u32, max_ms: u32) -> Result<Self> {
        if min_ms > max_ms || max_ms > MAX_MS {
            return Err(invalid(
                "Use an ordered delay range from 0 to 5000 milliseconds",
            ));
        }
        Ok(Self { min_ms, max_ms })
    }
}

fn milliseconds(value: &Value) -> Result<u32> {
    let seconds = value
        .as_f64()
        .or_else(|| value.as_str()?.parse::<f64>().ok())
        .ok_or_else(|| invalid("Retained delay bound is not a number"))?;
    let ms = (seconds * 1000.).round();
    // Decimal millisecond values such as1.001 can multiply to1000.9999999999.
    // Compare the inverse conversion rather than rejecting that floating error.
    if !ms.is_finite() || ms < 0. || ms > f64::from(MAX_MS) || ms / 1000. != seconds {
        return Err(invalid(
            "Retained range cannot be edited losslessly in milliseconds",
        ));
    }
    Ok(ms as u32)
}

/// Opening a row never rounds or changes retained sub-millisecond values.
pub fn from_row(row: &Value) -> Result<Option<Range>> {
    if row["Type"] != 0 || !row["Number"].is_object() {
        return Ok(None);
    }
    Range::new(
        milliseconds(&row["Number"]["min"])?,
        milliseconds(&row["Number"]["max"])?,
    )
    .map(Some)
}

pub(super) fn update(old: &RawValue, range: Range) -> Result<Box<RawValue>> {
    let range = Range::new(range.min_ms, range.max_ms)?;
    let value: Value = serde_json::from_str(old.get()).map_err(|e| invalid(e.to_string()))?;
    from_row(&value)?.ok_or_else(|| invalid("Select a randomized delay event"))?;
    if value.get("phase").is_some_and(|v| !v.is_null()) {
        return Err(invalid("Phased delay editing is not implemented"));
    }
    let mut fields: Fields = serde_json::from_str(old.get()).map_err(|e| invalid(e.to_string()))?;
    if fields.0.iter().filter(|(name, _)| name == "Number").count() != 1 {
        return Err(invalid("Ambiguous delay Number field"));
    }
    let (_, number) = fields
        .0
        .iter_mut()
        .find(|(name, _)| name == "Number")
        .unwrap();
    let mut bounds: Fields =
        serde_json::from_str(number.get()).map_err(|e| invalid(e.to_string()))?;
    for (name, ms) in [("min", range.min_ms), ("max", range.max_ms)] {
        if bounds.0.iter().filter(|(key, _)| key == name).count() != 1 {
            return Err(invalid("Ambiguous randomized delay bound"));
        }
        let (_, value) = bounds.0.iter_mut().find(|(key, _)| key == name).unwrap();
        *value = raw(&format!("{}.{:03}", ms / 1000, ms % 1000))?;
    }
    *number = raw(&bounds)?;
    raw(&fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn randomized_delay_bounds_match_six_independent_source_conversions() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/macro-random-delay-source.json"
        ))
        .unwrap();
        for case in fixture["vectors"].as_array().unwrap() {
            let expected = &case["native"][0];
            let range = from_row(&case["row"]).unwrap().unwrap();
            assert_eq!(json!(range.min_ms), expected["minMs"]);
            assert_eq!(json!(range.max_ms), expected["maxMs"]);
        }
    }

    #[test]
    fn randomized_delay_record_edit_preserves_envelope_and_rejects_native_cache() {
        let source = r#"{"guid":"range","macroEvents":[{"Type":"actionBar"},{"Type":0,"Number":{"min":"0.100","max":"0.900","future":9.9900e+100}},{"future":9007199254740993123456789}],"future":1.2300e-7}"#;
        let body: Value = serde_json::from_str(source).unwrap();
        let original = json!({"name":"Range","payload":STANDARD.encode(source),"hash":synapse_hash(&body).unwrap(),"future":true});
        let change = || Change::SetRandomDelay {
            guid: "range".into(),
            index: 1,
            min_ms: 250,
            max_ms: 1250,
        };
        let updated = edit_definition(&original, change()).unwrap();
        let bytes = STANDARD
            .decode(updated["payload"].as_str().unwrap())
            .unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(
            text.contains("9.9900e+100")
                && text.contains("1.2300e-7")
                && text.contains("9007199254740993123456789")
        );
        assert_eq!(updated["future"], true);
        assert_eq!(describe(&updated).unwrap().guid, "range");
        assert_eq!(
            from_row(&describe(&updated).unwrap().events[1]).unwrap(),
            Some(Range::new(250, 1250).unwrap())
        );
        let mut cached = body;
        cached["appEngine"] = json!({"events":[]});
        let record = json!({"name":"Range","payload":STANDARD.encode(cached.to_string()),"hash":synapse_hash(&cached).unwrap()});
        assert!(edit_definition(&record, change()).is_err());
        assert_eq!(original["payload"], STANDARD.encode(source));
    }

    #[test]
    fn randomized_delay_bounds_are_lossless_and_source_limited() {
        for ms in 0..=MAX_MS {
            let value = json!({"Type":0,"Number":{"min":format!("{}.{:03}",ms/1000,ms%1000),"max":ms as f64/1000.}});
            assert_eq!(from_row(&value).unwrap(), Some(Range::new(ms, ms).unwrap()));
        }
        for (min, max) in [(0, 0), (0, 5000), (123, 4567), (5000, 5000)] {
            let range = Range::new(min, max).unwrap();
            let row = json!({"Type":0,"Number":{"min":format!("{}.{:03}",min/1000,min%1000),"max":max as f64/1000.}});
            assert_eq!(from_row(&row).unwrap(), Some(range));
        }
        for number in [
            json!({"min":0.0001,"max":1}),
            json!({"min":1,"max":0}),
            json!({"min":0,"max":5.001}),
            json!({"min":-1,"max":0}),
            json!({"min":"NaN","max":1}),
            json!({"max":1}),
        ] {
            assert!(from_row(&json!({"Type":0,"Number":number})).is_err());
        }
        assert_eq!(from_row(&json!({"Type":0,"Number":"0.1"})).unwrap(), None);
    }

    #[test]
    fn randomized_delay_updates_only_explicit_bounds_and_refuses_ambiguous_rows() {
        let old=RawValue::from_string(r#"{"Type":0,"Number":{"min":"0.100","max":1,"future":9.9900e+100,"displayText":"retained"},"future":1.2300e-7,"selected":false}"#.into()).unwrap();
        let new = update(&old, Range::new(250, 1250).unwrap()).unwrap();
        assert!(new.get().contains("9.9900e+100") && new.get().contains("1.2300e-7"));
        let value: Value = serde_json::from_str(new.get()).unwrap();
        assert_eq!(value["Number"]["min"], "0.250");
        assert_eq!(value["Number"]["max"], "1.250");
        assert_eq!(value["Number"]["displayText"], "retained");
        for row in [
            r#"{"Type":0,"Number":{"min":0,"min":0,"max":1}}"#,
            r#"{"Type":0,"Number":{"min":0,"max":1},"Number":{"min":0,"max":1}}"#,
            r#"{"Type":0,"Number":{"min":0,"max":1},"phase":2}"#,
        ] {
            assert!(update(
                &RawValue::from_string(row.into()).unwrap(),
                Range::new(0, 10).unwrap()
            )
            .is_err());
        }
    }
}
