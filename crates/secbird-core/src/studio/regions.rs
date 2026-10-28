//! Partition selected LED memberships without flattening unknown JSON values.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LedSelection {
    pub device: String,
    pub row: u16,
    pub col: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedColor {
    Empty,
    Uniform(Option<u32>),
    Mixed,
}

fn entries(object: &Object, key: &str) -> Result<Vec<Box<RawValue>>> {
    array(
        object
            .get(key)
            .ok_or_else(|| invalid(format!("missing {key} array")))?,
    )
}
fn led(raw: &RawValue, device: &str) -> Result<LedSelection> {
    #[derive(Deserialize)]
    struct Coordinate {
        row: u16,
        col: u16,
    }
    let point: Coordinate = serde_json::from_str(raw.get()).map_err(invalid)?;
    Ok(LedSelection {
        device: device.into(),
        row: point.row,
        col: point.col,
    })
}
fn selected(selection: &[LedSelection]) -> Result<BTreeSet<LedSelection>> {
    if selection.len() > 16384
        || selection
            .iter()
            .any(|s| s.device.is_empty() || s.device.len() > 512)
    {
        return Err(invalid("invalid or excessive LED selection"));
    }
    Ok(selection.iter().cloned().collect())
}
fn region_color(region: &Object) -> Result<Option<u32>> {
    let params = Object::parse(
        region
            .get("params")
            .ok_or_else(|| invalid("region has no params"))?,
    )?;
    params
        .get("color")
        .map(|value| {
            let color: u32 = serde_json::from_str(value.get()).map_err(invalid)?;
            if color > 0xffffff {
                Err(invalid("Static color is outside RGB24"))
            } else {
                Ok(color)
            }
        })
        .transpose()
}
fn layer(draft: &Draft, id: &str, static_only: bool) -> Result<Object> {
    let row = draft
        .rows
        .iter()
        .find(|row| row.id == id)
        .ok_or_else(|| invalid("unknown layer"))?;
    if row.kind != Kind::Layer || static_only && row.effect.as_deref() != Some("static") {
        return Err(invalid(
            "selected layer does not support this region operation",
        ));
    }
    let item = item(&draft.current, id)?.ok_or_else(|| invalid("layer identity disappeared"))?;
    if static_only {
        if let Some(value) = item.get("value") {
            if serde_json::from_str::<u16>(value.get()).map_err(invalid)? != 6 {
                return Err(invalid("Static layer has a conflicting effect ID"));
            }
        }
    }
    Ok(item)
}

/// Keep metadata from an existing membership, using ledRef for newly assigned
/// LEDs. Never infer device identity or LED membership from canvas coordinates.
fn inventory(
    draft: &Draft,
    selection: &BTreeSet<LedSelection>,
) -> Result<BTreeMap<LedSelection, Box<RawValue>>> {
    let root = Object::parse(&draft.current)?;
    let mut result = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for raw in entries(&root, "devices")? {
        let device = Object::parse(&raw)?;
        let id = device.text("id")?;
        if !ids.insert(id.clone()) {
            return Err(invalid("ambiguous placement device identity"));
        }
        if !selection.iter().any(|key| key.device == id) {
            continue;
        }
        for record in crate::studio::inventory::led_inventory(raw.get())? {
            let key = LedSelection {
                device: id.clone(),
                row: record.row,
                col: record.col,
            };
            if selection.contains(&key) && result.insert(key, record.raw.to_owned()).is_some() {
                return Err(invalid("ambiguous placement LED identity"));
            }
        }
    }
    if result.len() != selection.len() {
        return Err(invalid(
            "selected LEDs are not in the composition's explicit LED inventory",
        ));
    }
    Ok(result)
}

impl Draft {
    pub fn static_selection_color(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<SelectedColor> {
        let selection = selected(selection)?;
        if selection.is_empty() {
            return Ok(SelectedColor::Empty);
        }
        let item = layer(self, id, true)?;
        inventory(self, &selection)?;
        let mut colors: BTreeMap<LedSelection, Option<u32>> = BTreeMap::new();
        for raw in entries(&item, "regions")? {
            let region = Object::parse(&raw)?;
            let mut matching = Vec::new();
            for raw in entries(&region, "devices")? {
                let device = Object::parse(&raw)?;
                let id = device.text("id")?;
                for raw in entries(&device, "leds")? {
                    let key = led(&raw, &id)?;
                    if selection.contains(&key) {
                        matching.push(key);
                    }
                }
            }
            if !matching.is_empty() {
                let color = region_color(&region)?;
                for key in matching {
                    if colors.insert(key, color).is_some() {
                        return Err(invalid("overlapping selected region memberships"));
                    }
                }
            }
        }
        let mut values = selection
            .iter()
            .map(|key| colors.get(key).copied().flatten());
        let first = values.next().unwrap();
        Ok(if values.all(|value| value == first) {
            SelectedColor::Uniform(first)
        } else {
            SelectedColor::Mixed
        })
    }

    pub fn set_static_color(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        color: u32,
    ) -> Result<bool> {
        if color > 0xffffff {
            return Err(invalid("Static color is outside RGB24"));
        }
        if self.static_selection_color(id, selection)? == SelectedColor::Uniform(Some(color)) {
            return Ok(false);
        }
        self.edit_regions(id, selection, Some(color))
    }
    pub fn erase_leds(&mut self, id: &str, selection: &[LedSelection]) -> Result<bool> {
        self.edit_regions(id, selection, None)
    }

    fn edit_regions(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        color: Option<u32>,
    ) -> Result<bool> {
        let selection = selected(selection)?;
        if selection.is_empty() {
            return Ok(false);
        }
        let item = layer(self, id, color.is_some())?;
        let available = inventory(self, &selection)?;
        let mut seen = BTreeSet::new();
        let mut kept = Vec::new();
        let mut added = Vec::new();
        for raw in entries(&item, "regions")? {
            let mut region = Object::parse(&raw)?;
            let mut remaining_devices = Vec::new();
            let mut selected_devices = Vec::new();
            let mut changed = false;
            for raw in entries(&region, "devices")? {
                let mut device = Object::parse(&raw)?;
                let device_id = device.text("id")?;
                let mut remaining = Vec::new();
                let mut chosen = Vec::new();
                for raw in entries(&device, "leds")? {
                    let key = led(&raw, &device_id)?;
                    if selection.contains(&key) {
                        if !seen.insert(key) {
                            return Err(invalid("overlapping selected region memberships"));
                        }
                        chosen.push(raw);
                    } else {
                        remaining.push(raw);
                    }
                }
                if chosen.is_empty() {
                    remaining_devices.push(raw);
                    continue;
                }
                changed = true;
                if color.is_some() {
                    let mut selected_device = Object::parse(&raw)?;
                    selected_device.set("leds", Some(&encode(&chosen)?));
                    selected_devices.push(encode(&selected_device)?);
                }
                if !remaining.is_empty() {
                    device.set("leds", Some(&encode(&remaining)?));
                    remaining_devices.push(encode(&device)?);
                }
            }
            if !changed {
                kept.push(raw);
                continue;
            }
            if let Some(color) = color {
                let mut new_region = Object::parse(&raw)?;
                let mut params = Object::parse(
                    region
                        .get("params")
                        .ok_or_else(|| invalid("region has no params"))?,
                )?;
                params.set("color", Some(&encode(&color)?));
                new_region.set("params", Some(&encode(&params)?));
                new_region.set("devices", Some(&encode(&selected_devices)?));
                added.push(encode(&new_region)?);
            }
            if !remaining_devices.is_empty() {
                region.set("devices", Some(&encode(&remaining_devices)?));
                kept.push(encode(&region)?);
            }
        }
        if color.is_none() && seen.is_empty() {
            return Ok(false);
        }
        if let Some(color) = color {
            let root = Object::parse(&self.current)?;
            let mut devices = Vec::new();
            for raw in entries(&root, "devices")? {
                let source = Object::parse(&raw)?;
                let device_id = source.text("id")?;
                let leds: Vec<_> = selection
                    .iter()
                    .filter(|key| key.device == device_id && !seen.contains(*key))
                    .map(|key| available[key].to_owned())
                    .collect();
                if leds.is_empty() {
                    continue;
                }
                let mut device = Object(vec![]);
                for field in ["id", "deviceId", "instanceId", "productId"] {
                    device.set(field, source.get(field));
                }
                device.set("leds", Some(&encode(&leds)?));
                devices.push(encode(&device)?);
            }
            if !devices.is_empty() {
                let mut region = Object(vec![]);
                region.set(
                    "params",
                    Some(&encode(&serde_json::json!({"color":color}))?),
                );
                region.set("devices", Some(&encode(&devices)?));
                added.push(encode(&region)?);
            }
        }
        kept.extend(added);
        let merged = merge_regions(kept)?;
        if merged.len() > MAX_LAYERS {
            return Err(invalid("region partition exceeds 512 entries"));
        }
        self.commit(vec![Change {
            id: id.into(),
            field: "regions",
            before: item.get("regions").map(RawValue::to_owned),
            after: encode(&merged)?,
        }])
    }
}

/// Merge only identical full non-device metadata. Unknown regional fields and
/// numeric spellings must not be discarded to make two encodings look equal.
fn merge_regions(regions: Vec<Box<RawValue>>) -> Result<Vec<Box<RawValue>>> {
    let mut result: Vec<Box<RawValue>> = Vec::new();
    let mut indexes = BTreeMap::new();
    for raw in regions {
        let mut region = Object::parse(&raw)?;
        let devices = entries(&region, "devices")?;
        region.set("devices", None);
        let signature = encode(&region)?.get().to_owned();
        if let Some(&index) = indexes.get(&signature) {
            let existing: &Box<RawValue> = &result[index];
            let mut prior = Object::parse(existing)?;
            let mut all = entries(&prior, "devices")?;
            for raw in devices {
                let mut device = Object::parse(&raw)?;
                let id = device.text("id")?;
                let mut merged = false;
                for existing in &mut all {
                    let mut old = Object::parse(existing)?;
                    if old.text("id")? != id {
                        continue;
                    }
                    let mut leds = entries(&old, "leds")?;
                    leds.extend(entries(&device, "leds")?);
                    old.set("leds", None);
                    device.set("leds", None);
                    if encode(&old)?.get() != encode(&device)?.get() {
                        continue;
                    }
                    old.set("leds", Some(&encode(&leds)?));
                    *existing = encode(&old)?;
                    merged = true;
                    break;
                }
                if !merged {
                    all.push(raw);
                }
            }
            prior.set("devices", Some(&encode(&all)?));
            result[index] = encode(&prior)?;
        } else {
            indexes.insert(signature, result.len());
            result.push(raw);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = r#"{"id":"p","title":"Desk","devices":[{"id":"kbd","deviceId":"physical-ref","instanceId":2,"productId":605,"ledRef":[{"row":0,"col":1,"inputData":"{\"Key\":\"A\"}","future":9007199254740993123456789},{"row":0,"col":2},{"row":0,"col":3},{"row":0,"col":4}]},{"id":"offline","opaque":9.9900e+100}],"layers":[{"id":"g","type":"group","title":"Group","visible":true,"layers":[{"id":"a","type":"layer","name":"static","value":6,"title":"STATIC","visible":true,"regions":[{"params":{"color":65280,"future":1.2300e-7},"regionExtra":true,"devices":[{"id":"kbd","deviceId":"physical-ref","instanceId":2,"productId":605,"unknown":9007199254740993123456789,"leds":[{"row":0,"col":1,"inputData":"{\"Key\":\"A\"}","future":9007199254740993123456789},{"row":0,"col":2}]}]},{"params":{"color":255,"future":1.2300e-7},"regionExtra":true,"devices":[{"id":"kbd","deviceId":"physical-ref","instanceId":2,"productId":605,"unknown":9007199254740993123456789,"leds":[{"row":0,"col":3}]},{"id":"offline","leds":[{"row":9,"col":9,"opaque":9.9900e+100}]}]}]}]}],"opaque":9007199254740993123456789}"#;
    fn keys(cols: &[u16]) -> Vec<LedSelection> {
        cols.iter()
            .map(|col| LedSelection {
                device: "kbd".into(),
                row: 0,
                col: *col,
            })
            .collect()
    }

    #[test]
    fn recolor_partitions_merges_and_preserves_unselected_and_offline_metadata() {
        let mut draft = Draft::open(PROFILE).unwrap();
        assert_eq!(
            draft.static_selection_color("a", &keys(&[1, 3])).unwrap(),
            SelectedColor::Mixed
        );
        assert!(draft
            .set_static_color("a", &keys(&[1, 3]), 0xff0000)
            .unwrap());
        for (col, color) in [(1, 0xff0000), (2, 0x00ff00), (3, 0xff0000)] {
            assert_eq!(
                draft.static_selection_color("a", &keys(&[col])).unwrap(),
                SelectedColor::Uniform(Some(color))
            );
        }
        let layer = layer(&draft, "a", true).unwrap();
        assert_eq!(entries(&layer, "regions").unwrap().len(), 3);
        for literal in [
            "9007199254740993123456789",
            "1.2300e-7",
            "9.9900e+100",
            r#""inputData":"{\"Key\":\"A\"}""#,
        ] {
            assert!(draft.json().contains(literal), "lost {literal}");
        }
        assert!(draft.is_dirty());
        assert_eq!(draft.undo_len(), 1);
        draft.undo().unwrap();
        assert!(!draft.is_dirty());
        assert_eq!(
            item(&draft.current, "a")
                .unwrap()
                .unwrap()
                .get("regions")
                .unwrap()
                .get(),
            item(&RawValue::from_string(PROFILE.into()).unwrap(), "a")
                .unwrap()
                .unwrap()
                .get("regions")
                .unwrap()
                .get()
        );
        draft.redo().unwrap();
        assert!(draft.is_dirty());
    }

    #[test]
    fn root_type_metadata_is_opaque_to_region_dirty_tracking() {
        let raw = PROFILE.replacen('{', r#"{"type":{"future":17},"#, 1);
        let mut draft = Draft::open(&raw).unwrap();
        draft.set_static_color("a", &keys(&[1]), 1).unwrap();
        assert!(draft.is_dirty());
        assert!(draft.json().contains(r#""type":{"future":17}"#));
        draft.undo().unwrap();
        assert!(!draft.is_dirty());
    }

    #[test]
    fn exported_leds_form_assigns_without_rewriting_inventory_and_conflicts_are_atomic() {
        let raw=PROFILE.replace("\"ledRef\":","\"leds\":");
        let mut draft=Draft::open(&raw).unwrap();
        draft.set_static_color("a",&keys(&[1,4]),0xabcdef).unwrap();
        assert!(!draft.json().contains("ledRef"));assert!(draft.json().contains("9007199254740993123456789"));
        assert_eq!(draft.static_selection_color("a",&keys(&[1,4])).unwrap(),SelectedColor::Uniform(Some(0xabcdef)));
        let raw=PROFILE.replacen("\"ledRef\":[","\"leds\":[{\"row\":9,\"col\":9}],\"ledRef\":[",1);
        let mut draft=Draft::open(&raw).unwrap();
        assert!(draft.set_static_color("a",&keys(&[1]),0).is_err());assert_eq!(draft.json(),raw);assert!(!draft.is_dirty());
    }

    #[test]
    fn new_memberships_keep_ledref_literals_and_default_only_color() {
        let mut draft = Draft::open(PROFILE).unwrap();
        draft.erase_leds("a", &keys(&[1])).unwrap();
        assert_eq!(
            draft.static_selection_color("a", &keys(&[1])).unwrap(),
            SelectedColor::Uniform(None)
        );
        draft.set_static_color("a", &keys(&[1, 4]), 0).unwrap();
        assert_eq!(
            draft.static_selection_color("a", &keys(&[1, 4])).unwrap(),
            SelectedColor::Uniform(Some(0))
        );
        let item = layer(&draft, "a", true).unwrap();
        let regions = entries(&item, "regions").unwrap();
        let new = regions
            .iter()
            .map(|raw| Object::parse(raw).unwrap())
            .find(|region| region_color(region).unwrap() == Some(0))
            .unwrap();
        assert_eq!(new.get("params").unwrap().get(), r#"{"color":0}"#);
        assert!(new
            .get("devices")
            .unwrap()
            .get()
            .contains("9007199254740993123456789"));
        assert!(new.get("devices").unwrap().get().contains("physical-ref"));
    }

    #[test]
    fn region_only_save_acknowledgement_and_discard_track_exact_snapshots() {
        let mut draft = Draft::open(PROFILE).unwrap();
        draft.set_static_color("a", &keys(&[1]), 1).unwrap();
        let checkpoint = draft.checkpoint();
        draft.set_static_color("a", &keys(&[1]), 2).unwrap();
        draft.accept_saved(checkpoint).unwrap();
        assert!(draft.is_dirty());
        draft.undo().unwrap();
        assert!(!draft.is_dirty());
        draft.redo().unwrap();
        assert!(draft.is_dirty());
        draft.discard();
        assert!(!draft.is_dirty());
        assert_eq!(draft.undo_len(), 0);
        assert_eq!(
            draft.static_selection_color("a", &keys(&[1])).unwrap(),
            SelectedColor::Uniform(Some(1))
        );
        assert!(!draft.set_static_color("a", &keys(&[1]), 1).unwrap());
        assert_eq!(draft.undo_len(), 0);
    }

    #[test]
    fn invalid_inventory_color_and_overlapping_memberships_are_atomic() {
        let mut draft = Draft::open(PROFILE).unwrap();
        let before = draft.json().to_owned();
        assert!(draft.set_static_color("a", &keys(&[99]), 1).is_err());
        assert!(draft.set_static_color("a", &keys(&[1]), 0x1000000).is_err());
        assert!(draft.erase_leds("g", &keys(&[1])).is_err());
        assert_eq!(draft.json(), before);
        assert_eq!(draft.undo_len(), 0);
        let overlapping = PROFILE.replace(
            r#""row":0,"col":3}]},{"id":"offline""#,
            r#""row":0,"col":1}]},{"id":"offline""#,
        );
        let mut draft = Draft::open(&overlapping).unwrap();
        let before = draft.json().to_owned();
        assert!(draft.set_static_color("a", &keys(&[1]), 1).is_err());
        assert_eq!(draft.json(), before);
        assert!(!draft.is_dirty());
    }

    #[test]
    fn erase_removes_only_selected_memberships_and_preserves_empty_noop() {
        let mut draft = Draft::open(PROFILE).unwrap();
        assert!(!draft.erase_leds("a", &keys(&[4])).unwrap());
        assert!(draft.erase_leds("a", &keys(&[1, 2])).unwrap());
        assert_eq!(
            draft.static_selection_color("a", &keys(&[1, 2])).unwrap(),
            SelectedColor::Uniform(None)
        );
        assert_eq!(
            draft.static_selection_color("a", &keys(&[3])).unwrap(),
            SelectedColor::Uniform(Some(255))
        );
        assert!(draft.json().contains(r#""id":"offline","leds""#));
        draft.undo().unwrap();
        assert!(!draft.is_dirty());
    }

    #[test]
    fn large_regional_history_is_bounded_and_oversize_edits_do_not_mutate() {
        let profile = PROFILE.replace("1.2300e-7", &format!("\"{}\"", "x".repeat(256 * 1024)));
        let mut draft = Draft::open(&profile).unwrap();
        for color in 1..=20 {
            draft.set_static_color("a", &keys(&[1]), color).unwrap();
        }
        assert!(draft.undo_len() > 0 && draft.undo_len() < 20);
        assert!(draft.undo.iter().map(Edit::bytes).sum::<usize>() <= MAX_HISTORY_BYTES);
        let profile = PROFILE.replacen("1.2300e-7", &format!("\"{}\"", "x".repeat(4300000)), 1);
        let mut draft = Draft::open(&profile).unwrap();
        let before = draft.json().to_owned();
        assert!(draft.set_static_color("a", &keys(&[1]), 1).is_err());
        assert_eq!(draft.json(), before);
        assert_eq!(draft.undo_len(), 0);
    }

    #[test]
    fn mixed_region_operations_match_an_independent_led_and_snapshot_reference() {
        type Colors = BTreeMap<(String, u16), Option<u32>>;
        fn region_json(draft: &Draft) -> String {
            item(&draft.current, "layer")
                .unwrap()
                .unwrap()
                .get("regions")
                .unwrap()
                .get()
                .into()
        }
        fn flat(draft: &Draft) -> Colors {
            let mut result: Colors = ["a", "b"]
                .into_iter()
                .flat_map(|device| (0..8).map(move |col| ((device.into(), col), None)))
                .collect();
            let regions: serde_json::Value = serde_json::from_str(&region_json(draft)).unwrap();
            let mut assigned = BTreeSet::new();
            for region in regions.as_array().unwrap() {
                for device in region["devices"].as_array().unwrap() {
                    for led in device["leds"].as_array().unwrap() {
                        let key = (
                            device["id"].as_str().unwrap().to_owned(),
                            led["col"].as_u64().unwrap() as u16,
                        );
                        assert_eq!(led["row"], 0);
                        assert!(assigned.insert(key.clone()));
                        *result.get_mut(&key).unwrap() =
                            region["params"]["color"].as_u64().map(|v| v as u32);
                    }
                }
            }
            result
        }
        let devices=["a","b"].map(|id|serde_json::json!({"id":id,"productId":605,"ledRef":(0..8).map(|col|serde_json::json!({"row":0,"col":col})).collect::<Vec<_>>()}));
        let profile=serde_json::json!({"id":"p","title":"Desk","devices":devices,"layers":[{"id":"layer","type":"layer","name":"static","value":6,"title":"STATIC","visible":true,"regions":[]}]}).to_string();
        let mut draft = Draft::open(&profile).unwrap();
        let mut colors = flat(&draft);
        let mut undo: VecDeque<(Colors, String)> = VecDeque::new();
        let mut redo = VecDeque::new();
        let mut saved = (colors.clone(), region_json(&draft));
        let mut seed = 0x13579bdu32;
        for step in 0..600 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let operation = (seed >> 24) % 10;
            let selection: Vec<_> = (0..16)
                .filter(|bit| seed & (1 << bit) != 0)
                .map(|bit| LedSelection {
                    device: if bit < 8 { "a" } else { "b" }.into(),
                    row: 0,
                    col: bit % 8,
                })
                .collect();
            let before = (colors.clone(), region_json(&draft));
            match operation {
                0..=4 => {
                    let value = if operation == 4 {
                        None
                    } else {
                        Some([0, 0xff00, 0xff0000, 0x123456][operation as usize])
                    };
                    let changed = selection
                        .iter()
                        .any(|key| colors[&(key.device.clone(), key.col)] != value);
                    let actual = if let Some(color) = value {
                        draft.set_static_color("layer", &selection, color)
                    } else {
                        draft.erase_leds("layer", &selection)
                    }
                    .unwrap();
                    assert_eq!(actual, changed, "step {step}");
                    if changed {
                        undo.push_back(before);
                        if undo.len() > MAX_HISTORY {
                            undo.pop_front();
                        }
                        redo.clear();
                        for key in &selection {
                            *colors.get_mut(&(key.device.clone(), key.col)).unwrap() = value;
                        }
                    }
                }
                5 => {
                    let expected = undo.pop_back();
                    assert_eq!(draft.undo().unwrap(), expected.is_some());
                    if let Some((prior, raw)) = expected {
                        redo.push_back(before);
                        colors = prior;
                        assert_eq!(region_json(&draft), raw);
                    }
                }
                6 => {
                    let expected = redo.pop_back();
                    assert_eq!(draft.redo().unwrap(), expected.is_some());
                    if let Some((next, raw)) = expected {
                        undo.push_back(before);
                        colors = next;
                        assert_eq!(region_json(&draft), raw);
                    }
                }
                7 => {
                    let checkpoint = draft.checkpoint();
                    draft.accept_saved(checkpoint).unwrap();
                    saved = before;
                }
                8 => {
                    draft.discard();
                    colors = saved.0.clone();
                    undo.clear();
                    redo.clear();
                    assert_eq!(region_json(&draft), saved.1);
                }
                _ => {
                    assert!(draft
                        .set_static_color("layer", &selection, 0x1000000)
                        .is_err());
                    assert_eq!(region_json(&draft), before.1);
                }
            }
            assert_eq!(flat(&draft), colors, "LED mismatch at step {step}");
            assert_eq!(draft.undo_len(), undo.len());
            assert_eq!(draft.redo_len(), redo.len());
            assert_eq!(
                draft.is_dirty(),
                region_json(&draft) != saved.1,
                "dirty mismatch at step {step}"
            );
        }
    }
}
