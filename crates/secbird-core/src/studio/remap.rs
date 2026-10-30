//! Explicit offline import reconciliation. No heuristic or all-LED fallback.
use super::*;

pub struct RemapPreview {
    snapshot: ReadSnapshot,
    candidate: Box<RawValue>,
    pub matched: usize,
    pub missing: Vec<String>,
}
const IDENTITY: &[&str] = &["id", "productId", "instanceId", "deviceId"];
fn identity(from: &Object, to: &mut Object) {
    for key in IDENTITY {
        to.set(key, from.get(key));
    }
}
fn key(led: &Object, keyboard: bool) -> Result<String> {
    let fields = if keyboard {
        ["inputType", "inputData"]
    } else {
        ["regionId", "regionIndex"]
    };
    let mut parts = Vec::new();
    for name in fields {
        let raw = led
            .get(name)
            .ok_or_else(|| invalid(format!("missing semantic LED {name}")))?;
        let value: serde_json::Value = serde_json::from_str(raw.get()).map_err(invalid)?;
        if value.is_null() || (!keyboard && (value == 0 || value == "")) {
            return Err(invalid("empty semantic LED identity"));
        }
        parts.push(value.to_string());
    }
    Ok(parts.join("/"))
}
impl Draft {
    pub fn remap_choices(&self) -> Result<Vec<(String, String)>> {
        let root = Object::parse(&self.current)?;
        let mut seen = BTreeSet::new();
        array(
            root.get("devices")
                .ok_or_else(|| invalid("missing devices"))?,
        )?
        .iter()
        .map(|raw| {
            let obj = Object::parse(raw)?;
            let id = obj.text("id")?;
            if id.is_empty() || !seen.insert(id.clone()) {
                return Err(invalid("ambiguous placement identity"));
            }
            Ok((
                id.clone(),
                format!(
                    "{id} / layout {}",
                    obj.get("layout").map_or("?", RawValue::get)
                ),
            ))
        })
        .collect()
    }
    /// Target must be an explicitly selected placement with source semantic LED metadata.
    pub fn preview_remap(&self, source: &str, target_json: &str) -> Result<RemapPreview> {
        self.remap_choices()?;
        if target_json.len() > MAX_DOCUMENT_BYTES {
            return Err(invalid("target exceeds 8 MiB"));
        }
        check_complexity(target_json.as_bytes())?;
        let target_raw = RawValue::from_string(target_json.into()).map_err(invalid)?;
        #[derive(Deserialize)]
        struct TargetIdentity {
            #[serde(rename = "productId")]
            product: u16,
            #[serde(rename = "editionId")]
            _edition: u16,
            #[serde(rename = "layout")]
            _layout: u16,
            width: f64,
            height: f64,
        }
        let declared: TargetIdentity = serde_json::from_str(target_raw.get()).map_err(invalid)?;
        if declared.product == 0
            || ![declared.width, declared.height]
                .iter()
                .all(|v| v.is_finite() && *v > 0. && *v <= 10000.)
        {
            return Err(invalid("invalid target identity or geometry"));
        }
        let target = Object::parse(&target_raw)?;
        let target_id = target.text("id")?;
        if target_id.is_empty() || target_id.len() > 512 || target_id.chars().any(char::is_control)
        {
            return Err(invalid("empty target identity"));
        }
        let mut root = Object::parse(&self.current)?;
        let mut devices = array(
            root.get("devices")
                .ok_or_else(|| invalid("missing devices"))?,
        )?;
        let index = devices
            .iter()
            .position(|raw| {
                Object::parse(raw)
                    .and_then(|o| o.text("id"))
                    .ok()
                    .as_deref()
                    == Some(source)
            })
            .ok_or_else(|| invalid("source placement missing"))?;
        if devices.iter().enumerate().any(|(i, raw)| {
            i != index
                && Object::parse(raw)
                    .and_then(|o| o.text("id"))
                    .ok()
                    .as_deref()
                    == Some(&target_id)
        }) {
            return Err(invalid("target identity collides with another placement"));
        }
        let original_placement = devices[index].clone();
        let mut placement = Object::parse(&devices[index])?;
        let category = placement.text("category")?;
        if target.text("category")? != category {
            return Err(invalid("source and target categories differ"));
        }
        let keyboard = matches!(category.as_str(), "keyboard" | "system");
        let inventory = super::super::inventory::led_inventory(target_json)?;
        let mut targets = std::collections::BTreeMap::new();
        for led in inventory {
            let object = Object::parse(led.raw)?;
            let semantic = key(&object, keyboard)?;
            if targets.insert(semantic, object).is_some() {
                return Err(invalid("ambiguous target LED identity"));
            }
        }
        if targets.is_empty() {
            return Err(invalid("target has no LEDs"));
        }
        let mut matched = 0;
        let mut missing = Vec::new();
        fn walk(
            raw: &RawValue,
            source: &str,
            target: &Object,
            targets: &std::collections::BTreeMap<String, Object>,
            keyboard: bool,
            matched: &mut usize,
            missing: &mut Vec<String>,
        ) -> Result<Box<RawValue>> {
            let mut node = Object::parse(raw)?;
            if let Some(children) = node.get("layers") {
                let children = array(children)?
                    .iter()
                    .map(|c| walk(c, source, target, targets, keyboard, matched, missing))
                    .collect::<Result<Vec<_>>>()?;
                node.set("layers", Some(&encode(&children)?));
            }
            if let Some(regions) = node.get("regions") {
                let mut output = Vec::new();
                for region in array(regions)? {
                    let mut region = Object::parse(&region)?;
                    let mut entries = array(
                        region
                            .get("devices")
                            .ok_or_else(|| invalid("region has no devices"))?,
                    )?;
                    for raw in &mut entries {
                        let mut device = Object::parse(raw)?;
                        if device.text("id")? != source {
                            continue;
                        }
                        let mut leds = array(
                            device
                                .get("leds")
                                .ok_or_else(|| invalid("region has no LEDs"))?,
                        )?;
                        let mut seen = BTreeSet::new();
                        for raw in &mut leds {
                            let mut led = Object::parse(raw)?;
                            let semantic = key(&led, keyboard)?;
                            if !seen.insert(semantic.clone()) {
                                return Err(invalid("ambiguous source LED identity"));
                            }
                            if let Some(to) = targets.get(&semantic) {
                                for field in [
                                    "row",
                                    "col",
                                    "inputType",
                                    "inputData",
                                    "regionId",
                                    "regionIndex",
                                ] {
                                    led.set(field, to.get(field));
                                }
                                *raw = encode(&led)?;
                                *matched += 1;
                            } else {
                                missing.push(format!("{source}: {semantic}"));
                            }
                        }
                        identity(target, &mut device);
                        device.set("leds", Some(&encode(&leds)?));
                        *raw = encode(&device)?;
                    }
                    region.set("devices", Some(&encode(&entries)?));
                    output.push(encode(&region)?);
                }
                node.set("regions", Some(&encode(&output)?));
            }
            encode(&node)
        }
        let layers = array(
            root.get("layers")
                .ok_or_else(|| invalid("missing layers"))?,
        )?
        .iter()
        .map(|raw| {
            walk(
                raw,
                source,
                &target,
                &targets,
                keyboard,
                &mut matched,
                &mut missing,
            )
        })
        .collect::<Result<Vec<_>>>()?;
        identity(&target, &mut placement);
        for field in [
            "productName",
            "editionId",
            "layout",
            "width",
            "height",
            "leds",
            "ledRef",
        ] {
            placement.set(field, target.get(field));
        }
        devices[index] = encode(&placement)?;
        root.set("devices", Some(&encode(&devices)?));
        root.set("layers", Some(&encode(&layers)?));
        // Retain replaced inventories and their unknown per-LED metadata verbatim.
        // Root provenance keeps repeated remaps linear rather than nesting snapshots.
        let mut provenance = match root.get("synapseLiteLayoutRemaps") {
            Some(raw) => array(raw)?,
            None => Vec::new(),
        };
        let record = Object(vec![
            ("source".into(), original_placement),
            ("target".into(), target_raw),
        ]);
        provenance.push(encode(&record)?);
        root.set("synapseLiteLayoutRemaps", Some(&encode(&provenance)?));
        let candidate = encode(&root)?;
        rows(&candidate)?;
        Ok(RemapPreview {
            snapshot: self.read_snapshot(),
            candidate,
            matched,
            missing,
        })
    }
    pub fn commit_remap(&mut self, preview: RemapPreview) -> Result<bool> {
        if !self.matches_snapshot(&preview.snapshot) {
            return Err(invalid("remap preview is stale"));
        }
        if !preview.missing.is_empty() {
            return Err(invalid("missing LEDs prevent lossless remapping"));
        }
        self.commit_edit(Edit::Composition {
            before: self.current.as_ref().to_owned(),
            after: preview.candidate,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = include_str!("../../tests/fixtures/remap-source.json");
    const TARGET: &str = include_str!("../../tests/fixtures/remap-target.json");
    #[test]
    fn preview_cancel_commit_history_and_archive_roundtrip() {
        let mut draft = Draft::open(SOURCE).unwrap();
        let preview = draft.preview_remap("old", TARGET).unwrap();
        assert_eq!(preview.matched, 1);
        assert!(preview.missing.is_empty());
        drop(preview);
        assert_eq!(draft.json(), SOURCE);
        assert_eq!(draft.undo_len(), 0);
        draft
            .commit_remap(draft.preview_remap("old", TARGET).unwrap())
            .unwrap();
        let changed = draft.json().to_owned();
        for token in [
            "9007199254740993123456789",
            "1.2300e-7",
            "9.9900e+100",
            "123456789012345678901234567890",
            "\"x\":17",
            "\"row\":3",
        ] {
            assert!(changed.contains(token), "{token}");
        }
        assert!(draft.is_dirty());
        draft.undo().unwrap();
        assert_eq!(draft.json(), SOURCE);
        draft.redo().unwrap();
        assert_eq!(draft.json(), changed);
        let archive = super::super::super::Archive::parse(
            format!("{{\"version\":1,\"profiles\":[{SOURCE}]}}").as_bytes(),
        )
        .unwrap();
        let bytes = archive.replace_profile_bytes(0, &changed).unwrap();
        let reopened = super::super::super::Archive::parse(&bytes).unwrap();
        assert_eq!(reopened.profile_json(0).unwrap(), changed);
        let saved = draft.checkpoint();
        draft.accept_saved(saved).unwrap();
        assert!(!draft.is_dirty());
    }
    #[test]
    fn missing_leds_refuse_atomically_without_all_led_fallback() {
        let mut draft = Draft::open(SOURCE).unwrap();
        let p = draft
            .preview_remap("old", &TARGET.replace("\"A\"", "\"B\""))
            .unwrap();
        assert_eq!(p.matched, 0);
        assert_eq!(p.missing.len(), 1);
        assert!(draft.commit_remap(p).is_err());
        assert_eq!(draft.json(), SOURCE);
        assert_eq!(draft.undo_len(), 0);
    }
    #[test]
    fn stale_preview_and_ambiguous_semantic_ids_refuse() {
        let mut draft = Draft::open(SOURCE).unwrap();
        let p = draft.preview_remap("old", TARGET).unwrap();
        draft.rename_layer("effect", "Changed").unwrap();
        assert!(draft.commit_remap(p).is_err());
        let duplicate = TARGET.replace(
            "\"leds\":[",
            "\"leds\":[{\"row\":0,\"col\":0,\"inputType\":1,\"inputData\":\"A\"},",
        );
        assert!(draft.preview_remap("old", &duplicate).is_err());
        assert!(draft.preview_remap("absent", TARGET).is_err());
        assert!(draft
            .preview_remap("old", &TARGET.replace("keyboard", "mouse"))
            .is_err());
        assert!(draft
            .preview_remap(
                "old",
                &TARGET.replace("\"inputData\":\"A\"", "\"unknown\":42")
            )
            .is_err());
    }
    #[test]
    fn non_keyboard_regions_and_colliding_placements_are_explicit() {
        let source = SOURCE
            .replace("keyboard", "mouse")
            .replace("inputType", "regionId")
            .replace("inputData", "regionIndex");
        let target = TARGET
            .replace("keyboard", "mouse")
            .replace("inputType", "regionId")
            .replace("inputData", "regionIndex");
        let mut draft = Draft::open(&source).unwrap();
        let p = draft.preview_remap("old", &target).unwrap();
        assert_eq!(p.matched, 1);
        draft.commit_remap(p).unwrap();
        let colliding = SOURCE.replacen("\"devices\":[", "\"devices\":[{\"id\":\"new\"},", 1);
        let draft = Draft::open(&colliding).unwrap();
        assert!(draft.preview_remap("old", TARGET).is_err());
        let ambiguous = SOURCE.replacen("\"devices\":[", "\"devices\":[{\"id\":\"old\"},", 1);
        let draft = Draft::open(&ambiguous).unwrap();
        assert!(draft.remap_choices().is_err());
    }
    #[test]
    fn replaced_inventory_metadata_is_retained_as_raw_provenance() {
        let source = SOURCE.replace(
            "\"x\":17",
            "\"leds\":[{\"row\":1,\"col\":2,\"futureInventory\":9.0012300e+109}],\"x\":17",
        );
        let mut draft = Draft::open(&source).unwrap();
        let p = draft.preview_remap("old", TARGET).unwrap();
        draft.commit_remap(p).unwrap();
        assert!(draft.json().contains("9.0012300e+109"));
        #[derive(Deserialize)]
        struct Record {
            source: Box<RawValue>,
        }
        #[derive(Deserialize)]
        struct Root {
            devices: Vec<Box<RawValue>>,
            #[serde(rename = "synapseLiteLayoutRemaps")]
            history: Vec<Record>,
        }
        let original: Root = serde_json::from_str(
            &source.replace("\"opaque\":", "\"synapseLiteLayoutRemaps\":[],\"opaque\":"),
        )
        .unwrap();
        let result: Root = serde_json::from_str(draft.json()).unwrap();
        assert_eq!(result.history[0].source.get(), original.devices[0].get());
        draft.undo().unwrap();
        assert_eq!(draft.json(), source);
    }
}
