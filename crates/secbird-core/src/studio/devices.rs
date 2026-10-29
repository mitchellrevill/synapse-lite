//! Reversible composition placement edits. These never bind physical hardware.
use super::*;

impl Draft {
    /// Add an explicit source-shaped placement while retaining every existing
    /// device, layer and unknown field as raw JSON. Hardware admission is separate.
    pub fn add_device(&mut self, placement_json: &str) -> Result<bool> {
        if placement_json.len() > MAX_DOCUMENT_BYTES { return Err(invalid("placement exceeds 8 MiB")); }
        check_complexity(placement_json.as_bytes())?;
        let raw = RawValue::from_string(placement_json.into()).map_err(invalid)?;
        let object = Object::parse(&raw)?;
        let id = object.text("id")?;
        if id.is_empty() || id.len() > 512 { return Err(invalid("invalid placement identity")); }
        #[derive(Deserialize)]
        struct Geometry {
            #[serde(rename="productId")] _product: u16,
            #[serde(rename="editionId")] _edition: u16,
            #[serde(rename="layout")] _layout: u16,
            x: f64, y: f64, width: f64, height: f64,
            orientation: u16,
        }
        let p: Geometry = serde_json::from_str(raw.get()).map_err(invalid)?;
        if ![0,90,180,270].contains(&p.orientation)
            || ![p.x,p.y,p.width,p.height].iter().all(|v|v.is_finite())
            || p.x.abs()>10000. || p.y.abs()>10000.
            || p.width<=0. || p.height<=0. || p.width>10000. || p.height>10000. {
            return Err(invalid("placement is outside supported canvas bounds"));
        }
        if super::super::inventory::led_inventory(raw.get())?.is_empty() {
            return Err(invalid("placement needs an explicit nonempty LED inventory"));
        }
        let root = Object::parse(&self.current)?;
        let before = root.get("devices").ok_or_else(||invalid("missing devices"))?;
        let mut entries = array(before)?;
        if entries.len() >= 32 { return Err(invalid("canvas supports at most 32 placements")); }
        let mut seen = BTreeSet::new();
        for entry in &entries {
            let existing = Object::parse(entry)?.text("id")?;
            if existing == id || !seen.insert(existing) { return Err(invalid("duplicate placement identity")); }
        }
        entries.push(raw);
        self.commit_edit(Edit::Devices {before:before.to_owned(),after:encode(&entries)?})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = r#"{"id":"p","title":"Desk","devices":[],"layers":[ {"id":"s","type":"layer","name":"static","title":"STATIC","regions":[],"visible":true,"params":{"future":9.9900e+100}} ],"opaque":9007199254740993123456789}"#;
    const DEVICE: &str = r#"{"id":"one","productId":100,"editionId":0,"layout":0,"x":0,"y":0,"width":200,"height":320,"orientation":0,"editorPreset":true,"leds":[{"row":0,"col":2}]}"#;
    #[test]
    fn placement_history_save_and_discard_preserve_opaque_layers() {
        let mut draft=Draft::open(PROFILE).unwrap();
        draft.add_device(DEVICE).unwrap(); assert!(draft.is_dirty());
        assert!(draft.json().contains("9.9900e+100"));
        assert!(draft.json().contains("9007199254740993123456789"));
        let edited=draft.json().to_owned(); let saved=draft.checkpoint();
        draft.accept_saved(saved).unwrap(); assert!(!draft.is_dirty());
        draft.undo().unwrap(); assert!(draft.is_dirty());
        draft.redo().unwrap(); assert!(!draft.is_dirty()); assert_eq!(draft.json(),edited);
        draft.undo().unwrap();draft.discard();assert_eq!(draft.json(),edited);
    }
    #[test]
    fn invalid_and_duplicate_placements_refuse_atomically() {
        let mut draft=Draft::open(PROFILE).unwrap();
        for invalid_device in [DEVICE.replace("200","0"),DEVICE.replace("\"orientation\":0","\"orientation\":45"),DEVICE.replace("[{\"row\":0,\"col\":2}]","[]")] {
            assert!(draft.add_device(&invalid_device).is_err());assert_eq!(draft.json(),PROFILE);assert_eq!(draft.undo_len(),0);
        }
        draft.add_device(DEVICE).unwrap();let before=draft.json().to_owned();
        assert!(draft.add_device(DEVICE).is_err());assert_eq!(draft.json(),before);assert_eq!(draft.undo_len(),1);
    }
    #[test]
    fn placement_limit_is_independent_of_layers() {
        let mut draft=Draft::open(PROFILE).unwrap();
        for index in 0..32 { draft.add_device(&DEVICE.replace("\"one\"",&format!("\"device-{index}\""))).unwrap(); }
        let before=draft.json().to_owned(); assert!(draft.add_device(&DEVICE.replace("one","extra")).is_err()); assert_eq!(draft.json(),before);
        assert_eq!(draft.undo_len(),MAX_HISTORY);
    }
}
