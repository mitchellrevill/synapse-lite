//! Compile saved source GUI definitions without changing them or sending input.
use super::{collect, revision};
use crate::{
    profile_compat::{invalid, stable_json, synapse_hash},
    source_macro::{EventLocation, Limits, PlaybackMetadata, SourceJson, SymbolicPlan},
    Result, Store,
};
use base64::{
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::LazyLock;
mod unique;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheStatus {
    Generated,
    Verified,
}

pub struct Prepared {
    guid: String,
    name: String,
    revision: String,
    native_json: String,
    source_rows: Vec<usize>,
    source_row_count: usize,
    plan: SymbolicPlan,
    cache: CacheStatus,
}
impl Prepared {
    pub fn guid(&self) -> &str {
        &self.guid
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn revision(&self) -> &str {
        &self.revision
    }
    pub fn native_json(&self) -> &str {
        &self.native_json
    }
    /// Zero-based GUI row for every native event, including expanded actions.
    pub fn source_rows(&self) -> &[usize] {
        &self.source_rows
    }
    pub fn source_row_count(&self) -> usize {
        self.source_row_count
    }
    pub fn plan(&self) -> &SymbolicPlan {
        &self.plan
    }
    pub fn cache_status(&self) -> CacheStatus {
        self.cache
    }
}

#[derive(Deserialize)]
struct Key {
    input_id: Option<String>,
    key_code: Option<u16>,
    scan_code: u16,
    source_flag: u16,
    category: String,
}
static KEYS: LazyLock<Vec<Key>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../macro_definition_keys.json"))
        .expect("checked source lookup facts")
});

/// Binding is native macroGroup JSON (guid, macroPlaybackOption, repeatCount).
/// No playback option is inferred from the definition or an active profile.
/// All conversion, source hashes, cache comparison and budgets must succeed.
pub fn prepare(store: &Store, binding_json: &str) -> Result<Prepared> {
    let binding = unique::parse(binding_json, unique::BINDING)?;
    let playback = PlaybackMetadata::parse(&binding).map_err(|e| invalid(e.to_string()))?;
    let guid = binding["guid"].as_str().expect("validated playback GUID");
    let records = collect(store)?;
    let record = records
        .get(guid)
        .ok_or_else(|| invalid("Macro definition is missing"))?;
    let body = verified_body(record)?;
    if body["guid"].as_str() != Some(guid) {
        return Err(invalid("Macro and binding GUIDs disagree"));
    }
    let rows = body["macroEvents"]
        .as_array()
        .ok_or_else(|| invalid("Macro has no GUI event array"))?;
    if rows.len() > 512 {
        return Err(invalid("Macro exceeds512 GUI rows"));
    }
    let mut events = Vec::new();
    let mut source_rows = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let converted =
            convert(row).map_err(|e| invalid(format!("Macro row {}: {e}", index + 1)))?;
        if converted.len() > 512 - events.len() {
            return Err(invalid("Expanded macro exceeds512 native events"));
        }
        source_rows.extend(std::iter::repeat_n(index, converted.len()));
        events.extend(converted);
    }
    let events = Value::Array(events);
    let cache = verify_cache(&body, &events)?;
    let native_json = serde_json::to_string(&events).map_err(|e| invalid(e.to_string()))?;
    let plan = SourceJson::parse(&native_json, Limits::default())
        .and_then(|raw| raw.plan(EventLocation::RootArray, &playback))
        .map_err(|e| invalid(e.to_string()))?;
    Ok(Prepared {
        guid: guid.into(),
        name: record["name"]
            .as_str()
            .ok_or_else(|| invalid("Macro has no name"))?
            .into(),
        revision: revision(record)?,
        native_json,
        source_rows,
        source_row_count: rows.len(),
        plan,
        cache,
    })
}

fn verified_body(record: &Value) -> Result<Value> {
    let payload = record["payload"]
        .as_str()
        .ok_or_else(|| invalid("Macro has no payload"))?;
    const MAX_BYTES: usize = 1024 * 1024;
    if payload.len() > MAX_BYTES.div_ceil(3) * 4 {
        return Err(invalid("Macro payload exceeds1MiB"));
    }
    let bytes = STANDARD
        .decode(payload)
        .or_else(|_| STANDARD_NO_PAD.decode(payload))
        .or_else(|_| URL_SAFE.decode(payload))
        .or_else(|_| URL_SAFE_NO_PAD.decode(payload))
        .map_err(|e| invalid(e.to_string()))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| invalid(e.to_string()))?;
    let body = unique::parse(text, unique::DEFINITION)?;
    if record["hash"].as_str() != Some(synapse_hash(&body)?.as_str()) {
        return Err(invalid("Macro source hash mismatch"));
    }
    Ok(body)
}

fn verify_cache(body: &Value, generated: &Value) -> Result<CacheStatus> {
    let Some(cache) = body.get("appEngine") else {
        return Ok(CacheStatus::Generated);
    };
    let object = cache
        .as_object()
        .ok_or_else(|| invalid("Native macro cache is not an object"))?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "events" | "hash" | "mouseMoveType"))
    {
        return Err(invalid("Native macro cache has unsupported fields"));
    }
    if object
        .get("mouseMoveType")
        .is_some_and(|value| value != "none")
    {
        return Err(invalid(
            "This native mouse-motion setting is not verified for planning",
        ));
    }
    if cache["hash"].as_str() != Some(synapse_hash(cache)?.as_str()) {
        return Err(invalid("Native macro cache hash mismatch"));
    }
    // JS Numbers have one numeric type. Compare complete canonical values,
    // without rewriting the retained cache or treating strings as numbers.
    if !cache["events"].is_array() || stable_json(&cache["events"])? != stable_json(generated)? {
        return Err(invalid(
            "Native macro cache disagrees with the GUI definition",
        ));
    }
    Ok(CacheStatus::Verified)
}

fn convert(row: &Value) -> Result<Vec<Value>> {
    if !row.is_object() {
        return Err(invalid("Macro event is not an object"));
    }
    if row.get("phase").is_some_and(|phase| !phase.is_null()) {
        return Err(invalid("Phased macro conversion is not implemented"));
    }
    if row["Type"] == "actionBar" {
        return Ok(vec![]);
    }
    match source_uint(&row["Type"], u16::MAX.into()) {
        Some(0) => delay(row),
        Some(1) => keyboard(row),
        Some(2) => mouse(row),
        _ => Err(invalid(
            "This macro event type is not implemented; no partial plan was returned",
        )),
    }
}

/// JSON integer/float spellings denote the same JS Number. The bounded fields
/// are exactly representable; strings and booleans are not coerced into codes.
fn source_uint(value: &Value, max: u64) -> Option<u64> {
    if let Some(integer) = value.as_u64() {
        return (integer <= max).then_some(integer);
    }
    let number = value.as_f64()?;
    (number.is_finite() && number >= 0. && number <= max as f64 && number.fract() == 0.)
        .then_some(number as u64)
}

fn delay(row: &Value) -> Result<Vec<Value>> {
    let seconds = match row.get("Number").unwrap_or(&Value::Null) {
        Value::Null => 0.,
        Value::Number(value) => value
            .as_f64()
            .ok_or_else(|| invalid("Invalid delay number"))?,
        Value::String(text) if text.trim().is_empty() => 0.,
        Value::String(text) => text
            .trim()
            .parse::<f64>()
            .map_err(|_| invalid("Use a decimal delay"))?,
        _ => {
            return Err(invalid(
                "Random or structured delay conversion is not implemented",
            ))
        }
    };
    let milliseconds = seconds * 1000.;
    if !milliseconds.is_finite() {
        return Err(invalid("Delay is not finite"));
    }
    // Math.round ties toward positive infinity, unlike Rust's negative ties.
    // Subtraction avoids the near-half rounding error of floor(value + 0.5).
    let floor = milliseconds.floor();
    let rounded = if milliseconds - floor < 0.5 {
        floor
    } else {
        floor + 1.
    };
    if rounded < 0. || rounded > f64::from(u32::MAX) {
        return Err(invalid("Delay is outside native u32 milliseconds"));
    }
    Ok(vec![json!({"type":"delay","ms":rounded as u32})])
}

fn keyboard(row: &Value) -> Result<Vec<Value>> {
    let event = row["KeyEvent"]
        .as_object()
        .ok_or_else(|| invalid("Key event is missing"))?;
    let make = event
        .get("Makecode")
        .ok_or_else(|| invalid("Key make code is missing"))?;
    if make.is_null() {
        return Ok(vec![]);
    }
    let make = source_uint(make, u16::MAX.into())
        .ok_or_else(|| invalid("Key make code is not a native integer"))? as u16;
    let category = match event.get("Type") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => (!text.is_empty()).then_some(text.as_str()),
        _ => return Err(invalid("Key category is not text")),
    };
    let mut found = KEYS.iter().find(|key| {
        key.key_code == Some(make) && category.is_none_or(|value| value == key.category)
    });
    if let Some(id) = match make {
        285 => Some("KEY_RIGHT_CTRL"),
        310 => Some("KEY_RIGHT_SHIFT"),
        312 => Some("KEY_RIGHT_ALT"),
        _ => None,
    } {
        found = KEYS.iter().find(|key| key.input_id.as_deref() == Some(id));
    }
    if matches!(make, 256..=258) {
        found = KEYS.iter().find(|key| key.key_code == Some(make - 240));
    }
    let state = event
        .get("State")
        .ok_or_else(|| invalid("Key state is missing"))?;
    if state.is_null() {
        let scan = found
            .ok_or_else(|| invalid("Key has no verified descriptor"))?
            .scan_code;
        return Ok(vec![
            json!({"type":"keyboard","scancode":scan,"flag":0}),
            json!({"type":"keyboard","scancode":scan,"flag":1}),
        ]);
    }
    let flag = source_uint(state, 5).ok_or_else(|| invalid("Unsupported native key state"))?;
    if let Some(scan) = row.get("ScanCode").filter(|scan| !scan.is_null()) {
        let scan = source_uint(scan, u16::MAX.into())
            .ok_or_else(|| invalid("Stored scan code is not a native integer"))?
            as u16;
        if scan != 0 && found.is_none_or(|key| key.scan_code != scan) {
            if let Some(fallback) =
                source_uint(&row["flag"], u16::MAX.into()).and_then(|source_flag| {
                    KEYS.iter().find(|key| {
                        key.scan_code == scan && u64::from(key.source_flag) == source_flag
                    })
                })
            {
                found = Some(fallback);
            }
        }
    }
    let scan = found
        .ok_or_else(|| invalid("Key has no verified descriptor"))?
        .scan_code;
    Ok(vec![json!({"type":"keyboard","scancode":scan,"flag":flag})])
}

fn mouse(row: &Value) -> Result<Vec<Value>> {
    let event = row["MouseEvent"]
        .as_object()
        .ok_or_else(|| invalid("Mouse event is missing"))?;
    let button = event
        .get("MouseButton")
        .ok_or_else(|| invalid("Mouse button is missing"))?;
    if button.is_null() {
        return Ok(vec![]);
    }
    let button = source_uint(button, 9).ok_or_else(|| invalid("Unknown source mouse function"))?;
    let state = event
        .get("State")
        .ok_or_else(|| invalid("Mouse state is missing"))?;
    let state = if state.is_null() {
        None
    } else {
        Some(source_uint(state, 1).ok_or_else(|| invalid("Unsupported mouse state"))?)
    };
    if button < 5 {
        let down = [1, 4, 16, 64, 256][button as usize];
        let up = down * 2;
        return Ok(match state {
            None => vec![
                json!({"type":"mouse","id":up}),
                json!({"type":"mouse","id":down}),
            ],
            Some(0) => vec![json!({"type":"mouse","id":down})],
            _ => vec![json!({"type":"mouse","id":up})],
        });
    }
    if button == 5 {
        return Ok(if state == Some(0) {
            vec![
                json!({"type":"mouse","id":1}),
                json!({"type":"mouse","id":2}),
                json!({"type":"delay","ms":10}),
                json!({"type":"mouse","id":1}),
                json!({"type":"mouse","id":2}),
            ]
        } else {
            vec![]
        });
    }
    if state == Some(1) {
        return Ok(vec![]);
    }
    let id = if button < 8 { 2048 } else { 1024 };
    let data = if matches!(button, 6 | 9) { 65416 } else { 120 };
    Ok(vec![json!({"type":"mouse","id":id,"data":data})])
}

#[cfg(test)]
mod tests;
