//! Source-shaped layer construction and reversible sibling-array operations.
use super::*;

/// Studio IDs, not firmware effect IDs. Runtime support is a separate contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewEffect {
    Ambient, Audio, Breathing, ChromaGenerate, Fire, Reactive, Ripple,
    Spectrum, Starlight, Static, Tidal, Wave, Wheel,
}
impl NewEffect {
    fn fields(self) -> (&'static str, u16, &'static str) {
        match self {
            Self::Ambient => ("ambient",11,"AMBIENT"), Self::Audio => ("audio",12,"AUDIO_METER"),
            Self::Breathing => ("breathing",5,"BREATHING"), Self::ChromaGenerate => ("chroma-generate",18,"CHROMA_GENERATE"),
            Self::Fire => ("fire",8,"FIRE"), Self::Reactive => ("reactive",3,"REACTIVE"),
            Self::Ripple => ("ripple",1,"RIPPLE"), Self::Spectrum => ("spectrum",4,"SPECTRUM_CYCLING"),
            Self::Starlight => ("starlight",7,"STARLIGHT"), Self::Static => ("static",6,"STATIC"),
            Self::Tidal => ("tidal",19,"TIDAL"), Self::Wave => ("wave",2,"WAVE"), Self::Wheel => ("wheel",13,"WHEEL"),
        }
    }
}

fn validate_title(title: &str) -> Result<()> {
    if title.trim().is_empty() || title.encode_utf16().count() > 32 {
        return Err(invalid("layer names require 1–32 characters"));
    }
    Ok(())
}

impl Draft {
    fn fresh_ids(&self, ids: &[String]) -> Result<()> {
        let mut seen: BTreeSet<_> = self.rows.iter().map(|row|row.id.as_str()).collect();
        for id in ids {
            if id.len() != 12 || !id.bytes().all(|c|c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                || !seen.insert(id.as_str()) {
                return Err(invalid("new layer IDs require unique twelve-character lowercase hex"));
            }
        }
        Ok(())
    }

    fn replace_tree(&mut self, entries: Vec<Box<RawValue>>) -> Result<bool> {
        let root = Object::parse(&self.current)?;
        let before = root.get("layers").ok_or_else(||invalid("missing layers"))?;
        let after = encode(&entries)?;
        if before.get() == after.get() { return Ok(false); }
        self.commit_edit(Edit::Layers {before: before.to_owned(), after})
    }

    /// Click-add creates an empty root layer. Selection assignment is a later edit.
    pub fn add_effect(&mut self, id: &str, effect: NewEffect) -> Result<bool> {
        self.fresh_ids(&[id.into()])?;
        let (name, value, title) = effect.fields();
        let params = if matches!(effect, NewEffect::Wheel | NewEffect::Tidal) {
            serde_json::json!({"centerPointX":125,"centerPointX2":5000,
                "centerPointY":123,"centerPointY2":5000,"counterclockwise":false})
        } else { serde_json::json!({}) };
        let mut entries = layers(&Object::parse(&self.current)?)?;
        entries.insert(0, encode(&serde_json::json!({"id":id,"type":"layer","name":name,
            "value":value,"title":title,"params":params,"regions":[],"visible":true}))?);
        self.replace_tree(entries)
    }

    /// New groups are empty and prepended; they never wrap selected layers.
    pub fn add_group(&mut self, id: &str, title: &str) -> Result<bool> {
        self.fresh_ids(&[id.into()])?; validate_title(title)?;
        let mut entries = layers(&Object::parse(&self.current)?)?;
        entries.insert(0, encode(&serde_json::json!({"id":id,"type":"group","title":title,
            "layers":[],"visible":true,"opened":false}))?);
        self.replace_tree(entries)
    }

    fn edit_siblings(&mut self, id: &str,
        edit: impl FnOnce(&mut Vec<Box<RawValue>>, usize) -> Result<bool>) -> Result<bool> {
        let row = self.rows.iter().find(|row|row.id == id).ok_or_else(||invalid("unknown layer"))?;
        let parent = row.parent.clone();
        let object = item(&self.current, id)?.ok_or_else(||invalid("layer disappeared"))?;
        let reference = object.get("groupId").map(|_|object.text("groupId")).transpose()?;
        if reference.as_deref().filter(|id|!id.is_empty()) != parent.as_deref() {
            return Err(invalid("layer tree and groupId disagree; preserve imported data without guessing"));
        }
        let mut root = layers(&Object::parse(&self.current)?)?;
        if let Some(parent) = parent {
            let position = root.iter().position(|raw|Object::parse(raw).ok()
                .and_then(|item|item.text("id").ok()).as_deref() == Some(parent.as_str()))
                .ok_or_else(||invalid("missing parent group"))?;
            let mut group = Object::parse(&root[position])?;
            let mut siblings = layers(&group)?;
            let index = siblings.iter().position(|raw|Object::parse(raw).ok()
                .and_then(|item|item.text("id").ok()).as_deref() == Some(id))
                .ok_or_else(||invalid("missing child layer"))?;
            if !edit(&mut siblings, index)? { return Ok(false); }
            group.set("layers", Some(&encode(&siblings)?));
            root[position] = encode(&group)?;
        } else {
            let index = root.iter().position(|raw|Object::parse(raw).ok()
                .and_then(|item|item.text("id").ok()).as_deref() == Some(id))
                .ok_or_else(||invalid("missing root layer"))?;
            if !edit(&mut root, index)? { return Ok(false); }
        }
        self.replace_tree(root)
    }

    /// IDs are supplied by the caller: group ID first, then direct child IDs.
    /// Copies prepend to the same sibling list, matching click-duplicate.
    pub fn duplicate_item(&mut self, id: &str, ids: &[String], title: &str) -> Result<bool> {
        if title.trim().is_empty() || title.len() > 4096 { return Err(invalid("invalid copied title")); }
        self.fresh_ids(ids)?;
        let mut copy = item(&self.current, id)?.ok_or_else(||invalid("unknown layer"))?;
        let group = copy.text("type")? == "group";
        let mut children = if group { layers(&copy)? } else { vec![] };
        if ids.len() != children.len() + 1 { return Err(invalid("duplicate IDs must cover the complete subtree")); }
        copy.set("id", Some(&encode(&ids[0])?));
        copy.set("title", Some(&encode(&title)?));
        for (child, fresh) in children.iter_mut().zip(&ids[1..]) {
            let mut object = Object::parse(child)?;
            object.set("id", Some(&encode(fresh)?));
            object.set("groupId", Some(&encode(&ids[0])?));
            *child = encode(&object)?;
        }
        if group { copy.set("layers", Some(&encode(&children)?)); }
        let copy = encode(&copy)?;
        self.edit_siblings(id, |siblings, _| { siblings.insert(0, copy); Ok(true) })
    }

    pub fn can_delete_item(&self, id: &str) -> bool {
        self.rows.iter().any(|row|row.id == id) && self.rows.iter().any(|row|
            row.kind == Kind::Layer && row.id != id && row.parent.as_deref() != Some(id))
    }

    pub fn delete_item(&mut self, id: &str) -> Result<bool> {
        if !self.can_delete_item(id) { return Err(invalid("keep at least one effect layer")); }
        self.edit_siblings(id, |siblings, index| { siblings.remove(index); Ok(true) })
    }

    /// Adjacent sibling ordering. Cross-group drag/reparent is a separate tool.
    pub fn move_item(&mut self, id: &str, direction: i8) -> Result<bool> {
        if !matches!(direction, -1 | 1) { return Err(invalid("choose up or down")); }
        self.edit_siblings(id, |siblings, index| {
            let target = index as isize + direction as isize;
            if target < 0 || target as usize >= siblings.len() { return Ok(false); }
            siblings.swap(index, target as usize); Ok(true)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = r#"{"id":"p","title":"Desk","devices":[],"opaque":9.9900e+100,"layers":[{"id":"g","type":"group","title":"Group","visible":true,"opened":true,"future":9007199254740993123456789,"layers":[{"id":"a","groupId":"g","type":"layer","title":"STATIC","name":"static","value":6,"visible":true,"params":{},"regions":[{"params":{"color":1,"opaque":1.2300e-7},"devices":[]}]},{"id":"b","groupId":"g","type":"layer","title":"FIRE","name":"fire","value":8,"visible":false,"regions":[]}]},{"id":"c","type":"layer","title":"SPECTRUM_CYCLING","name":"spectrum","value":4,"visible":true,"regions":[]}]}"#;
    fn id(n: u16) -> String { format!("{n:012x}") }
    fn object(draft: &Draft, id: &str) -> Object { item(&draft.current, id).unwrap().unwrap() }

    #[test]
    fn all_source_constructors_create_empty_root_layers_without_led_assignment() {
        for effect in [NewEffect::Ambient,NewEffect::Audio,NewEffect::Breathing,NewEffect::ChromaGenerate,
            NewEffect::Fire,NewEffect::Reactive,NewEffect::Ripple,NewEffect::Spectrum,NewEffect::Starlight,
            NewEffect::Static,NewEffect::Tidal,NewEffect::Wave,NewEffect::Wheel] {
            let mut draft = Draft::open(PROFILE).unwrap();
            draft.add_effect(&id(1), effect).unwrap();
            let created = object(&draft, &id(1)); let (name, value, title) = effect.fields();
            assert_eq!(created.text("name").unwrap(), name);
            assert_eq!(created.text("title").unwrap(), title);
            assert_eq!(created.get("value").unwrap().get(), value.to_string());
            assert_eq!(created.get("regions").unwrap().get(), "[]");
            assert!(created.get("groupId").is_none());
            assert_eq!(draft.layers()[0].id, id(1)); assert!(draft.is_dirty());
            draft.undo().unwrap(); assert!(!draft.is_dirty());
            draft.redo().unwrap(); assert_eq!(draft.layers()[0].id, id(1));
        }
    }

    #[test]
    fn group_and_child_copies_prepend_with_fresh_back_references_and_raw_metadata() {
        let mut draft = Draft::open(PROFILE).unwrap();
        let source_regions = object(&draft,"a").get("regions").unwrap().get().to_string();
        draft.duplicate_item("a", &[id(1)], "STATIC").unwrap();
        assert_eq!(draft.layers()[1].id, id(1));
        assert_eq!(object(&draft,&id(1)).text("groupId").unwrap(), "g");
        assert_eq!(object(&draft,&id(1)).get("regions").unwrap().get(), source_regions);
        draft.undo().unwrap();
        draft.duplicate_item("g", &[id(2),id(3),id(4)], "Group (1)").unwrap();
        assert_eq!(draft.layers()[0].id, id(2));
        for child in [id(3),id(4)] { assert_eq!(object(&draft,&child).text("groupId").unwrap(), id(2)); }
        assert_eq!(object(&draft,&id(2)).get("future").unwrap().get(), "9007199254740993123456789");
        assert!(draft.json().contains("1.2300e-7")); assert!(draft.json().contains("9.9900e+100"));
        draft.undo().unwrap(); assert!(!draft.is_dirty());
    }

    #[test]
    fn delete_keeps_empty_groups_but_refuses_removing_the_last_effect() {
        let mut draft = Draft::open(PROFILE).unwrap();
        draft.delete_item("a").unwrap(); draft.delete_item("b").unwrap();
        assert_eq!(object(&draft,"g").get("layers").unwrap().get(), "[]");
        assert!(!draft.can_delete_item("c"));
        let before = draft.json().to_string(); let undo = draft.undo_len();
        assert!(draft.delete_item("c").is_err());
        assert_eq!(draft.json(), before); assert_eq!(draft.undo_len(), undo);
        draft.delete_item("g").unwrap(); assert_eq!(draft.layers().len(), 1);
        draft.undo().unwrap(); draft.undo().unwrap(); draft.undo().unwrap();
        assert!(!draft.is_dirty());
    }

    #[test]
    fn ordering_and_new_groups_preserve_sibling_scope_and_history_with_leaf_edits() {
        let mut draft = Draft::open(PROFILE).unwrap();
        draft.add_group(&id(1), "Group 1").unwrap();
        assert_eq!(object(&draft,&id(1)).get("opened").unwrap().get(), "false");
        assert_eq!(object(&draft,&id(1)).get("layers").unwrap().get(), "[]");
        draft.move_item("b", -1).unwrap();
        assert_eq!(draft.layers()[2].id, "b");
        draft.rename_layer("a", "Edited").unwrap();
        draft.move_item("g", 1).unwrap();
        draft.set_visible("g", false).unwrap();
        for _ in 0..5 { assert!(draft.undo().unwrap()); }
        assert!(!draft.is_dirty());
        for _ in 0..5 { assert!(draft.redo().unwrap()); }
        assert_eq!(object(&draft,"a").text("title").unwrap(), "Edited");
        assert_eq!(object(&draft,"a").visible().unwrap(), Some(false));
    }

    #[test]
    fn bad_ids_parent_references_and_tree_limits_fail_before_mutation() {
        let mut draft = Draft::open(PROFILE).unwrap();
        for bad in ["", "a", "00000000000G", "00000000000A"] {
            assert!(draft.add_effect(bad, NewEffect::Static).is_err());
        }
        assert!(draft.duplicate_item("g", &[id(1),id(1),id(2)], "copy").is_err());
        assert!(draft.duplicate_item("g", &[id(1)], "copy").is_err());
        assert!(draft.move_item("a", 0).is_err()); assert!(!draft.is_dirty());
        let malformed = PROFILE.replace("\"groupId\":\"g\"", "\"groupId\":\"missing\"");
        let mut bad = Draft::open(&malformed).unwrap();
        assert!(bad.move_item("b", -1).is_err()); assert!(!bad.is_dirty());
        let mut large = Draft::open(r#"{"id":"p","title":"empty","devices":[],"layers":[]}"#).unwrap();
        for n in 0..MAX_LAYERS { large.add_effect(&id(n as u16), NewEffect::Static).unwrap(); }
        let before = large.json().to_string(); let undo = large.undo_len();
        assert!(large.add_effect(&id(600), NewEffect::Fire).is_err());
        assert_eq!(large.json(), before); assert_eq!(large.undo_len(), undo);
    }

    #[test]
    fn structural_save_acknowledgement_keeps_newer_changes_and_raw_imports() {
        let mut draft = Draft::open(PROFILE).unwrap();
        draft.add_effect(&id(1), NewEffect::Static).unwrap();
        let checkpoint = draft.checkpoint();
        draft.move_item(&id(1), 1).unwrap();
        draft.accept_saved(checkpoint).unwrap();
        assert!(draft.is_dirty()); draft.undo().unwrap(); assert!(!draft.is_dirty());
        draft.undo().unwrap(); assert!(draft.is_dirty()); draft.redo().unwrap(); assert!(!draft.is_dirty());
        draft.duplicate_item("a", &[id(2)], &format!("{} (1)", "x".repeat(32))).unwrap();
        draft.discard(); assert!(!draft.is_dirty());
        assert!(draft.layers().iter().any(|row|row.id == id(1)));
        assert!(!draft.layers().iter().any(|row|row.id == id(2)));
        assert!(draft.json().contains("9007199254740993123456789"));
    }
}
