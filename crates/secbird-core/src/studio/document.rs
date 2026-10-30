//! Local Studio drafts and explicitly requested pure preview. No device/file I/O.
//! Edited leaves stay explicit; unrelated profile JSON remains raw.
use super::{check_complexity, invalid, BoundedOutput, MAX_DOCUMENT_BYTES};
use crate::Result;
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeMap,
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_json::value::RawValue;
use std::{
    collections::{BTreeSet, VecDeque},
    fmt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

#[path = "regions.rs"]
mod regions;
pub use regions::{
    BreathingEdit, BreathingSettings, SelectedBreathing,
    ColorProperty, GradientStop, LedSelection, SelectedColor, SelectedDuration, SelectedGradient,
    StopEdit, SPECTRUM_DURATIONS,
    WaveProperty, SelectedWaveValue, wave_preset,
    StarlightDescriptor, StarlightProperty, SelectedStarlightValue, STARLIGHT_DURATIONS,
    WheelProperty, SelectedWheelValue, wheel_preset, WheelGeometry,
    RippleProperty, SelectedRippleValue, RippleStart, RippleEnd, RipplePlayback, SelectedRipplePlayback, ripple_preset,
    ReactiveProperty, SelectedReactiveValue, REACTIVE_DURATIONS,
    AudioProperty, SelectedAudioValue, audio_preset,
    AmbientEdit, AmbientProperty, AmbientValue, SelectedAmbientValue, AmbientRect, AMBIENT_PRESETS, AMBIENT_SCALE,
    TidalDescriptor, TidalProperty, SelectedTidalValue, TidalLayout, TIDAL_NONE,
};

#[path = "preview.rs"]
mod preview;
#[path = "tree.rs"]
mod tree;
#[path = "devices.rs"]
mod devices;
pub use tree::NewEffect;
pub use preview::Preview;

pub const MAX_HISTORY: usize = 20;
pub const MAX_HISTORY_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_LAYERS: usize = 512;
static SESSION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Layer,
    Group,
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerRow {
    pub id: String,
    pub title: String,
    pub kind: Kind,
    pub parent: Option<String>,
    pub visible: Option<bool>,
    pub opened: Option<bool>,
    pub effect: Option<String>,
}

struct Object(Vec<(String, Box<RawValue>)>);
impl Object {
    fn parse(raw: &RawValue) -> Result<Self> {
        serde_json::from_str(raw.get()).map_err(invalid)
    }
    fn get(&self, name: &str) -> Option<&RawValue> {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_ref())
    }
    fn text(&self, name: &str) -> Result<String> {
        let value = self
            .get(name)
            .ok_or_else(|| invalid(format!("missing {name}")))?;
        let text: String = serde_json::from_str(value.get()).map_err(invalid)?;
        if text.len() > 4096 {
            return Err(invalid("editor text field exceeds 4096 bytes"));
        }
        Ok(text)
    }
    fn set(&mut self, name: &str, value: Option<&RawValue>) {
        match (self.0.iter().position(|(key, _)| key == name), value) {
            (Some(index), Some(value)) => self.0[index].1 = value.to_owned(),
            (Some(index), None) => {
                self.0.remove(index);
            }
            (None, Some(value)) => self.0.push((name.into(), value.to_owned())),
            (None, None) => {}
        }
    }
    fn visible(&self) -> Result<Option<bool>> {
        self.get("visible")
            .map(|raw| serde_json::from_str(raw.get()).map_err(invalid))
            .transpose()
    }
}
impl Serialize for Object {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, value) in &self.0 {
            map.serialize_entry(name, value)?;
        }
        map.end()
    }
}
impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct Fields;
        impl<'de> Visitor<'de> for Fields {
            type Value = Object;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an unambiguous Studio object")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Object, A::Error> {
                let (mut fields, mut names) = (Vec::new(), BTreeSet::new());
                while let Some(name) = map.next_key::<String>()? {
                    if names.len() == MAX_LAYERS || !names.insert(name.clone()) {
                        return Err(de::Error::custom(
                            "duplicate or excessive editor object fields",
                        ));
                    }
                    fields.push((name, map.next_value()?));
                }
                Ok(Object(fields))
            }
        }
        deserializer.deserialize_map(Fields)
    }
}
fn encode(value: &impl Serialize) -> Result<Box<RawValue>> {
    let mut output = BoundedOutput(Vec::new());
    serde_json::to_writer(&mut output, value).map_err(invalid)?;
    RawValue::from_string(String::from_utf8(output.0).map_err(invalid)?).map_err(invalid)
}
fn array(raw: &RawValue) -> Result<Vec<Box<RawValue>>> {
    struct Limited(Vec<Box<RawValue>>);
    impl<'de> Deserialize<'de> for Limited {
        fn deserialize<D: Deserializer<'de>>(
            deserializer: D,
        ) -> std::result::Result<Self, D::Error> {
            struct Items;
            impl<'de> Visitor<'de> for Items {
                type Value = Limited;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("a bounded Studio layer array")
                }
                fn visit_seq<A: SeqAccess<'de>>(
                    self,
                    mut array: A,
                ) -> std::result::Result<Limited, A::Error> {
                    let mut values = Vec::new();
                    while let Some(value) = array.next_element()? {
                        if values.len() == MAX_LAYERS {
                            return Err(de::Error::custom("more than 512 editor layer entries"));
                        }
                        values.push(value);
                    }
                    Ok(Limited(values))
                }
            }
            deserializer.deserialize_seq(Items)
        }
    }
    serde_json::from_str::<Limited>(raw.get())
        .map(|limited| limited.0)
        .map_err(invalid)
}
fn layers(object: &Object) -> Result<Vec<Box<RawValue>>> {
    array(
        object
            .get("layers")
            .ok_or_else(|| invalid("composition/group has no layers array"))?,
    )
}

fn rows(raw: &RawValue) -> Result<Vec<LayerRow>> {
    fn collect(
        items: &[Box<RawValue>],
        parent: Option<&str>,
        out: &mut Vec<LayerRow>,
        ids: &mut BTreeSet<String>,
    ) -> Result<()> {
        for raw in items {
            let item = Object::parse(raw)?;
            let id = item.text("id")?;
            if id.is_empty() || id.len() > 512 || !ids.insert(id.clone()) || out.len() == MAX_LAYERS
            {
                return Err(invalid("empty/duplicate/excessive layer identities"));
            }
            let kind = match item.text("type")?.as_str() {
                "layer" => Kind::Layer,
                "group" => Kind::Group,
                other => Kind::Unsupported(other.into()),
            };
            if parent.is_some() && kind == Kind::Group {
                return Err(invalid(
                    "nested groups remain in the archive but cannot be edited",
                ));
            }
            let effect = item.get("name").map(|_| item.text("name")).transpose()?;
            out.push(LayerRow {
                id: id.clone(),
                title: item.text("title")?,
                kind: kind.clone(),
                parent: parent.map(str::to_owned),
                visible: item.visible()?,
                opened: if kind == Kind::Group {
                    item.get("opened")
                        .map(|raw| serde_json::from_str(raw.get()).map_err(invalid))
                        .transpose()?
                } else {
                    None
                },
                effect,
            });
            if kind == Kind::Group {
                collect(&layers(&item)?, Some(&id), out, ids)?;
            }
        }
        Ok(())
    }
    let root = Object::parse(raw)?;
    // This is editor admission, not device/placement/LED compatibility admission.
    root.text("id")?;
    root.text("title")?;
    let devices = root
        .get("devices")
        .ok_or_else(|| invalid("composition has no devices array"))?;
    if !devices.get().trim_start().starts_with('[') {
        return Err(invalid("devices must be an array"));
    }
    let mut out = Vec::new();
    collect(&layers(&root)?, None, &mut out, &mut BTreeSet::new())?;
    Ok(out)
}

struct Change {
    id: String,
    field: &'static str,
    before: Option<Box<RawValue>>,
    after: Box<RawValue>,
}
enum Edit {
    Fields(Vec<Change>),
    Layers { before: Box<RawValue>, after: Box<RawValue> },
    Devices { before: Box<RawValue>, after: Box<RawValue> },
}
impl Edit {
    fn bytes(&self) -> usize {
        match self {
            Self::Fields(changes) => changes.iter().map(|change|change.id.len() + change.after.get().len()
                + change.before.as_ref().map_or(0, |raw|raw.get().len())).sum(),
            Self::Layers {before, after} | Self::Devices {before, after} => before.get().len() + after.get().len(),
        }
    }
}

/// Borrow region leaves directly from the two documents; large raw JSON is not
/// cloned just to compute dirty state. Only fields this editor changes matter.
fn regions_equal(left: &RawValue, right: &RawValue) -> Result<bool> {
    #[derive(Deserialize)]
    struct Node<'a> {
        #[serde(borrow)]
        devices: Option<&'a RawValue>,
        #[serde(borrow, rename = "type")]
        kind: Option<&'a RawValue>,
        #[serde(borrow)]
        regions: Option<&'a RawValue>,
        #[serde(borrow)]
        params: Option<&'a RawValue>,
        #[serde(borrow)]
        layers: Option<&'a RawValue>,
    }
    fn compare(left: &RawValue, right: &RawValue, root: bool) -> Result<bool> {
        let a: Node = serde_json::from_str(left.get()).map_err(invalid)?;
        let b: Node = serde_json::from_str(right.get()).map_err(invalid)?;
        if root && a.devices.map(RawValue::get) != b.devices.map(RawValue::get) { return Ok(false); }
        if !root {
            let kind_a = a
                .kind
                .map(|raw| serde_json::from_str::<String>(raw.get()).map_err(invalid))
                .transpose()?;
            let kind_b = b
                .kind
                .map(|raw| serde_json::from_str::<String>(raw.get()).map_err(invalid))
                .transpose()?;
            if kind_a != kind_b {
                return Ok(false);
            }
            match kind_a.as_deref() {
                Some("layer") => {
                    return Ok(a.regions.map(RawValue::get) == b.regions.map(RawValue::get)
                        && a.params.map(RawValue::get) == b.params.map(RawValue::get))
                }
                Some("group") => {}
                _ => return Ok(true),
            }
        }
        match (a.layers, b.layers) {
            (None, None) => Ok(true),
            (Some(a), Some(b)) => {
                let a: Vec<&RawValue> = serde_json::from_str(a.get()).map_err(invalid)?;
                let b: Vec<&RawValue> = serde_json::from_str(b.get()).map_err(invalid)?;
                if a.len() != b.len() {
                    return Ok(false);
                }
                for (a, b) in a.iter().zip(&b) {
                    if !compare(a, b, false)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    compare(left, right, true)
}

fn item(raw: &RawValue, id: &str) -> Result<Option<Object>> {
    fn find(items: &[Box<RawValue>], id: &str, nested: bool) -> Result<Option<Object>> {
        for raw in items {
            let object = Object::parse(raw)?;
            if object.text("id")? == id {
                return Ok(Some(object));
            }
            if !nested && object.text("type")? == "group" {
                if let Some(found) = find(&layers(&object)?, id, true)? {
                    return Ok(Some(found));
                }
            }
        }
        Ok(None)
    }
    find(&layers(&Object::parse(raw)?)?, id, false)
}

fn rewrite(raw: &RawValue, edit: &Edit, inverse: bool) -> Result<Box<RawValue>> {
    let changes = match edit {
        Edit::Devices {before, after} => {
            let mut root = Object::parse(raw)?;
            root.set("devices", Some(if inverse {before} else {after}));
            return encode(&root);
        }
        Edit::Layers {before, after} => {
            let mut root = Object::parse(raw)?;
            root.set("layers", Some(if inverse {before} else {after}));
            return encode(&root);
        }
        Edit::Fields(changes) => changes,
    };
    fn update(
        raw: &RawValue,
        changes: &[Change],
        inverse: bool,
        nested: bool,
    ) -> Result<Option<Box<RawValue>>> {
        let mut object = Object::parse(raw)?;
        let id = object.text("id")?;
        let mut changed = false;
        for change in changes {
            if change.id == id {
                object.set(
                    change.field,
                    if inverse {
                        change.before.as_deref()
                    } else {
                        Some(&change.after)
                    },
                );
                changed = true;
            }
        }
        if !nested && object.text("type")? == "group" {
            let mut children = layers(&object)?;
            let mut children_changed = false;
            for child in &mut children {
                if let Some(updated) = update(child, changes, inverse, true)? {
                    *child = updated;
                    children_changed = true;
                }
            }
            if children_changed {
                object.set("layers", Some(&encode(&children)?));
                changed = true;
            }
        }
        if changed {
            Ok(Some(encode(&object)?))
        } else {
            Ok(None)
        }
    }
    let mut root = Object::parse(raw)?;
    let mut entries = layers(&root)?;
    for entry in &mut entries {
        if let Some(updated) = update(entry, changes, inverse, false)? {
            *entry = updated;
        }
    }
    root.set("layers", Some(&encode(&entries)?));
    encode(&root)
}

/// A bounded local editing session. Accepting a saved baseline performs no file
/// or vendor save; the caller must complete its persistence action first.
pub struct Draft {
    owner: u64,
    saved: Arc<RawValue>,
    current: Arc<RawValue>,
    saved_rows: Vec<LayerRow>,
    rows: Vec<LayerRow>,
    undo: VecDeque<Edit>,
    redo: VecDeque<Edit>,
    regions_dirty: bool,
}

/// Exact snapshot offered to persistence. No Clone: acknowledgement consumes
/// this checkpoint and can only update the editing session that created it.
pub struct SaveCheckpoint {
    owner: u64,
    raw: Arc<RawValue>,
    rows: Vec<LayerRow>,
}
/// Immutable background-reader token; never acknowledges a save or clones rows.
#[derive(Clone)]
pub struct ReadSnapshot {
    owner: u64,
    raw: Arc<RawValue>,
}
impl ReadSnapshot {
    pub fn json(&self) -> &str {
        self.raw.get()
    }
}
impl SaveCheckpoint {
    pub fn json(&self) -> &str {
        self.raw.get()
    }
}
impl Draft {
    pub fn open(profile_json: &str) -> Result<Self> {
        if profile_json.len() > MAX_DOCUMENT_BYTES {
            return Err(invalid("composition exceeds 8 MiB"));
        }
        check_complexity(profile_json.as_bytes())?;
        let raw: Arc<RawValue> = RawValue::from_string(profile_json.into())
            .map_err(invalid)?
            .into();
        let rows = rows(&raw)?;
        let owner = SESSION
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| invalid("draft session identities exhausted"))?
            + 1;
        Ok(Self {
            owner,
            saved: raw.clone(),
            current: raw,
            saved_rows: rows.clone(),
            rows,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            regions_dirty: false,
        })
    }
    pub fn json(&self) -> &str {
        self.current.get()
    }
    pub fn read_snapshot(&self) -> ReadSnapshot {
        ReadSnapshot {
            owner: self.owner,
            raw: self.current.clone(),
        }
    }
    pub fn matches_snapshot(&self, snapshot: &ReadSnapshot) -> bool {
        self.owner == snapshot.owner && Arc::ptr_eq(&self.current, &snapshot.raw)
    }
    pub fn layers(&self) -> &[LayerRow] {
        &self.rows
    }
    pub fn is_dirty(&self) -> bool {
        self.rows != self.saved_rows || self.regions_dirty
    }
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    fn commit(&mut self, changes: Vec<Change>) -> Result<bool> {
        if changes.is_empty() {
            return Ok(false);
        }
        self.commit_edit(Edit::Fields(changes))
    }
    fn commit_edit(&mut self, edit: Edit) -> Result<bool> {
        if edit.bytes() > MAX_HISTORY_BYTES {
            return Err(invalid(
                "edit exceeds the 8 MiB reversible-history budget",
            ));
        }
        let candidate = rewrite(&self.current, &edit, false)?;
        if candidate.get().len() > MAX_DOCUMENT_BYTES { return Err(invalid("composition exceeds 8 MiB")); }
        check_complexity(candidate.get().as_bytes())?;
        let candidate_rows = rows(&candidate)?;
        let regions_dirty = !regions_equal(&candidate, &self.saved)?;
        self.current = candidate.into();
        self.rows = candidate_rows;
        self.regions_dirty = regions_dirty;
        self.undo.push_back(edit);
        while self.undo.len() > MAX_HISTORY
            || self.undo.iter().map(Edit::bytes).sum::<usize>() > MAX_HISTORY_BYTES
        {
            self.undo.pop_front();
        }
        self.redo.clear();
        Ok(true)
    }
    pub fn rename_layer(&mut self, id: &str, title: &str) -> Result<bool> {
        if title.trim().is_empty() || title.encode_utf16().count() > 32 {
            return Err(invalid("layer names require 1–32 characters"));
        }
        let row = self
            .rows
            .iter()
            .find(|row| row.id == id)
            .ok_or_else(|| invalid("unknown layer"))?;
        if row.title == title {
            return Ok(false);
        }
        let object =
            item(&self.current, id)?.ok_or_else(|| invalid("layer identity disappeared"))?;
        self.commit(vec![Change {
            id: id.into(),
            field: "title",
            before: object.get("title").map(RawValue::to_owned),
            after: encode(&title)?,
        }])
    }
    /// Source group visibility overwrites every child's visibility in one undo
    /// step. It is not a temporary mask retaining each child's previous toggle.
    pub fn set_visible(&mut self, id: &str, visible: bool) -> Result<bool> {
        let selected = self
            .rows
            .iter()
            .find(|row| row.id == id)
            .ok_or_else(|| invalid("unknown layer"))?;
        if matches!(selected.kind, Kind::Unsupported(_)) {
            return Err(invalid("unknown layer type cannot change visibility"));
        }
        let mut changes = Vec::new();
        for row in &self.rows {
            if row.id == id || (selected.kind == Kind::Group && row.parent.as_deref() == Some(id)) {
                if matches!(row.kind, Kind::Unsupported(_)) {
                    return Err(invalid("unknown child type cannot change visibility"));
                }
                if row.visible != Some(visible) {
                    changes.push(Change {
                        id: row.id.clone(),
                        field: "visible",
                        before: row.visible.map(|value| encode(&value)).transpose()?,
                        after: encode(&visible)?,
                    });
                }
            }
        }
        self.commit(changes)
    }
    pub fn undo(&mut self) -> Result<bool> {
        let Some(edit) = self.undo.back() else {
            return Ok(false);
        };
        let candidate = rewrite(&self.current, edit, true)?;
        let candidate_rows = rows(&candidate)?;
        let regions_dirty = !regions_equal(&candidate, &self.saved)?;
        self.current = candidate.into();
        self.rows = candidate_rows;
        self.regions_dirty = regions_dirty;
        self.redo
            .push_back(self.undo.pop_back().expect("validated undo entry"));
        Ok(true)
    }
    pub fn redo(&mut self) -> Result<bool> {
        let Some(edit) = self.redo.back() else {
            return Ok(false);
        };
        let candidate = rewrite(&self.current, edit, false)?;
        let candidate_rows = rows(&candidate)?;
        let regions_dirty = !regions_equal(&candidate, &self.saved)?;
        self.current = candidate.into();
        self.rows = candidate_rows;
        self.regions_dirty = regions_dirty;
        self.undo
            .push_back(self.redo.pop_back().expect("validated redo entry"));
        Ok(true)
    }
    pub fn checkpoint(&self) -> SaveCheckpoint {
        SaveCheckpoint {
            owner: self.owner,
            raw: self.current.clone(),
            rows: self.rows.clone(),
        }
    }
    pub fn accept_saved(&mut self, checkpoint: SaveCheckpoint) -> Result<()> {
        if checkpoint.owner != self.owner {
            return Err(invalid("save checkpoint belongs to a different draft"));
        }
        let regions_dirty = !regions_equal(&self.current, &checkpoint.raw)?;
        self.saved = checkpoint.raw;
        self.saved_rows = checkpoint.rows;
        self.regions_dirty = regions_dirty;
        Ok(())
    }
    pub fn discard(&mut self) {
        self.current = self.saved.clone();
        self.rows = self.saved_rows.clone();
        self.regions_dirty = false;
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = r#"{"id":"composition","title":"Desk","devices":[{"id":"offline","unknown":9007199254740993123456789}],"layers":[{"id":"group","type":"group","title":"Group","visible":true,"opened":true,"future":1.2300e-7,"layers":[{"id":"a","groupId":"group","type":"layer","name":"static","title":"STATIC","visible":false,"params":{},"regions":[{"params":{"color":65280},"devices":[{"id":"offline","leds":[{"row":0,"col":1,"inputData":"{\"Key\":\"A\"}"}]}]}]},{"id":"b","type":"layer","name":"spectrum","title":"SPECTRUM_CYCLING","visible":true,"regions":[]}]}],"unknown":{"value":9.9900e+100}}"#;
    #[test]
    fn dirty_check_does_not_interpret_unknown_items_layers_or_regions() {
        let raw = r#"{"id":"p","title":"T","devices":[],"layers":[{"id":"future","type":"unknown","title":"Opaque","layers":17,"regions":{"opaque":9.9900e+100}}]}"#;
        let mut draft = Draft::open(raw).unwrap();
        draft.rename_layer("future", "Renamed").unwrap();
        assert!(draft.is_dirty());
        assert!(draft.json().contains("9.9900e+100"));
        draft.undo().unwrap();
        assert!(!draft.is_dirty());
    }
    #[test]
    fn group_visibility_is_one_reversible_transaction_and_preserves_regions() {
        let mut draft = Draft::open(PROFILE).unwrap();
        assert!(draft.set_visible("group", false).unwrap());
        assert!(draft.layers().iter().all(|row| row.visible == Some(false)));
        assert_eq!(draft.undo_len(), 1);
        assert!(draft.is_dirty());
        assert!(draft.json().contains("9007199254740993123456789"));
        assert!(draft.json().contains("1.2300e-7"));
        assert!(draft.json().contains("9.9900e+100"));
        assert!(draft.json().contains(r#""inputData":"{\"Key\":\"A\"}""#));
        draft.undo().unwrap();
        assert!(!draft.is_dirty());
        assert_eq!(draft.layers()[1].visible, Some(false));
        assert_eq!(draft.layers()[2].visible, Some(true));
        draft.redo().unwrap();
        assert!(draft.layers().iter().all(|row| row.visible == Some(false)));
    }
    #[test]
    fn rename_dirty_save_and_discard_keep_saved_and_working_content_separate() {
        let mut draft = Draft::open(PROFILE).unwrap();
        assert!(!draft.rename_layer("a", "STATIC").unwrap());
        draft.rename_layer("a", "Desk green").unwrap();
        assert!(draft.is_dirty());
        draft.accept_saved(draft.checkpoint()).unwrap();
        let saved = draft.json().to_owned();
        assert!(!draft.is_dirty());
        draft.set_visible("a", true).unwrap();
        draft.rename_layer("group", "Working group").unwrap();
        draft.discard();
        assert_eq!(draft.json(), saved);
        assert!(!draft.is_dirty());
        assert_eq!((draft.undo_len(), draft.redo_len()), (0, 0));
    }
    #[test]
    fn returning_to_saved_values_clears_dirty_and_new_edits_drop_redo() {
        let mut draft = Draft::open(PROFILE).unwrap();
        draft.rename_layer("a", "New").unwrap();
        draft.rename_layer("a", "STATIC").unwrap();
        assert!(!draft.is_dirty());
        draft.undo().unwrap();
        assert!(draft.is_dirty());
        draft.set_visible("b", false).unwrap();
        assert_eq!(draft.redo_len(), 0);
        for index in 0..40 {
            draft.rename_layer("a", &format!("Name{index}")).unwrap();
        }
        assert_eq!(draft.undo_len(), MAX_HISTORY);
        for _ in 0..MAX_HISTORY {
            assert!(draft.undo().unwrap());
        }
        assert!(!draft.undo().unwrap());
    }
    #[test]
    fn compact_history_matches_full_snapshot_reference_across_mixed_operations() {
        #[derive(Clone, Debug, PartialEq, Eq)]
        struct Snapshot {
            titles: Vec<String>,
            visible: Vec<bool>,
        }
        let mut expected = Snapshot {
            titles: vec!["Group".into(), "STATIC".into(), "SPECTRUM_CYCLING".into()],
            visible: vec![true, false, true],
        };
        let mut saved = expected.clone();
        let (mut past, mut future) = (Vec::new(), Vec::new());
        let mut draft = Draft::open(PROFILE).unwrap();
        let mut random = 0x5eed_u64;
        for _ in 0..1000 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let operation = (random >> 32) % 7;
            let before = expected.clone();
            match operation {
                0 => {
                    let title = format!("Name{}", random % 3);
                    draft.rename_layer("a", &title).unwrap();
                    expected.titles[1] = title;
                }
                1 => {
                    expected.visible[1] = !expected.visible[1];
                    draft.set_visible("a", expected.visible[1]).unwrap();
                }
                2 => {
                    let on = !expected.visible[0];
                    expected.visible.fill(on);
                    draft.set_visible("group", on).unwrap();
                }
                3 => {
                    let previous = past.pop();
                    assert_eq!(draft.undo().unwrap(), previous.is_some());
                    if let Some(previous) = previous {
                        future.push(expected);
                        expected = previous;
                    }
                }
                4 => {
                    let next = future.pop();
                    assert_eq!(draft.redo().unwrap(), next.is_some());
                    if let Some(next) = next {
                        past.push(expected);
                        expected = next;
                    }
                }
                5 => {
                    saved = expected.clone();
                    draft.accept_saved(draft.checkpoint()).unwrap();
                }
                _ => {
                    expected = saved.clone();
                    past.clear();
                    future.clear();
                    draft.discard();
                }
            }
            if operation < 3 && expected != before {
                past.push(before);
                if past.len() > 20 {
                    past.remove(0);
                }
                future.clear();
            }
            assert_eq!(
                draft
                    .layers()
                    .iter()
                    .map(|row| row.title.clone())
                    .collect::<Vec<_>>(),
                expected.titles
            );
            assert_eq!(
                draft
                    .layers()
                    .iter()
                    .map(|row| row.visible.unwrap())
                    .collect::<Vec<_>>(),
                expected.visible
            );
            assert_eq!(draft.is_dirty(), expected != saved);
            assert_eq!(
                (draft.undo_len(), draft.redo_len()),
                (past.len(), future.len())
            );
            assert!(draft.json().contains("9007199254740993123456789"));
        }
    }

    #[test]
    fn save_acknowledgement_preserves_newer_edits_and_refuses_foreign_checkpoints() {
        let mut draft = Draft::open(PROFILE).unwrap();
        draft.rename_layer("a", "Written snapshot").unwrap();
        let checkpoint = draft.checkpoint();
        let written = checkpoint.json().to_owned();
        draft.rename_layer("a", "Newer working edit").unwrap();
        draft.accept_saved(checkpoint).unwrap();
        assert!(draft.is_dirty());
        assert_eq!(draft.layers()[1].title, "Newer working edit");
        let foreign = Draft::open(PROFILE).unwrap().checkpoint();
        assert!(draft.accept_saved(foreign).is_err());
        assert!(draft.is_dirty());
        draft.discard();
        assert_eq!(draft.json(), written);
        assert_eq!(draft.layers()[1].title, "Written snapshot");
        assert!(!draft.is_dirty());
    }

    #[test]
    fn rejected_edits_leave_document_and_history_unchanged() {
        let mut draft = Draft::open(PROFILE).unwrap();
        for title in ["".to_owned(), " ".into(), "x".repeat(33), "😀".repeat(17)] {
            assert!(draft.rename_layer("a", &title).is_err());
        }
        assert!(draft.rename_layer("missing", "Title").is_err());
        assert!(draft.set_visible("missing", false).is_err());
        assert_eq!(draft.json(), PROFILE);
        assert_eq!(draft.undo_len(), 0);
        assert!(!draft.is_dirty());
        assert!(Draft::open(&PROFILE.replace("\"id\":\"b\"", "\"id\":\"a\"")).is_err());
        assert!(
            Draft::open(&PROFILE.replace("\"visible\":true", "\"visible\":\"future\"")).is_err()
        );
    }
    #[test]
    fn empty_layers_are_valid_and_missing_visibility_is_restored_by_undo() {
        let empty = r#"{"id":"empty","title":"Empty","devices":[],"layers":[]}"#;
        assert!(Draft::open(empty).unwrap().layers().is_empty());
        let raw = r#"{"id":"p","title":"T","devices":[],"layers":[{"id":"a","type":"layer","title":"A","regions":[]}]}"#;
        let mut draft = Draft::open(raw).unwrap();
        assert_eq!(draft.layers()[0].visible, None);
        draft.set_visible("a", true).unwrap();
        draft.undo().unwrap();
        assert_eq!(draft.layers()[0].visible, None);
        assert!(!draft.json().contains("visible"));
        assert!(!draft.is_dirty());
    }

    #[test]
    fn preparing_saved_container_preserves_other_profiles_and_does_not_accept_draft() {
        let envelope = format!(
            r#"{{"version":1,"profiles":[{PROFILE},{{"id":"untouched","opaque":9.9900e+100}}],"future":{{"large":9007199254740993123456789}}}}"#
        );
        let archive = super::super::Archive::parse(envelope.as_bytes()).unwrap();
        let mut draft = Draft::open(archive.profile_json(0).unwrap()).unwrap();
        draft.rename_layer("a", "Edited Static").unwrap();
        let prepared = archive.replace_profile_bytes(0, draft.json()).unwrap();
        assert!(
            draft.is_dirty(),
            "preparing bytes is not a successful disk save"
        );
        assert_eq!(archive.profile_json(0), Some(PROFILE));
        let saved = super::super::Archive::parse(&prepared).unwrap();
        assert_eq!(saved.profile_json(1), archive.profile_json(1));
        let reloaded = Draft::open(saved.profile_json(0).unwrap()).unwrap();
        assert_eq!(reloaded.layers()[1].title, "Edited Static");
        assert!(String::from_utf8_lossy(&prepared).contains("9007199254740993123456789"));
        assert!(archive.replace_profile_bytes(2, draft.json()).is_err());
        assert!(archive.replace_profile_bytes(0, "[]").is_err());
        draft.accept_saved(draft.checkpoint()).unwrap();
        assert!(!draft.is_dirty());
    }

    #[test]
    fn unknown_children_refuse_group_visibility_atomically_and_archive_keeps_nested_groups() {
        let unknown = PROFILE.replace(
            r#""type":"layer","name":"static""#,
            r#""type":"future","name":"static""#,
        );
        let mut draft = Draft::open(&unknown).unwrap();
        assert!(draft.set_visible("group", false).is_err());
        assert_eq!(draft.json(), unknown);
        assert_eq!(draft.undo_len(), 0);
        let nested = r#"{"id":"p","title":"T","devices":[],"layers":[{"id":"outer","type":"group","title":"O","layers":[{"id":"inner","type":"group","title":"I","layers":[]}]}]}"#;
        assert!(Draft::open(nested).is_err());
        let archive = super::super::Archive::parse(
            format!(r#"{{"version":1,"profiles":[{nested}]}}"#).as_bytes(),
        )
        .unwrap();
        assert_eq!(archive.profile_json(0), Some(nested));
    }
}
