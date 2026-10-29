//! Source Studio Audio settings; playback admission remains separate.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioProperty {
    Boost,
    AutoBoost,
    Decay,
}
impl AudioProperty {
    fn field(self) -> &'static str {
        match self {
            Self::Boost => "boost",
            Self::AutoBoost => "autoBoost",
            Self::Decay => "decay",
        }
    }
    fn validate(self, value: f64) -> Result<()> {
        let (min, max, step) = match self {
            Self::Boost => (0.25, 4.0, 0.25),
            Self::AutoBoost => (0.0, 1.0, 1.0),
            Self::Decay => (0.1, 2.0, 0.1),
        };
        if value.is_finite()
            && (min..=max).contains(&value)
            && (value / step - (value / step).round()).abs() < 1e-8
        {
            Ok(())
        } else {
            Err(invalid(
                "Audio Meter requires Boost0.25-4 step0.25, Auto boolean, or Decay0.1-2 step0.1",
            ))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SelectedAudioValue {
    Empty,
    Uniform(Option<f64>),
    Mixed,
}

pub fn audio_preset(index: usize) -> Option<Vec<GradientStop>> {
    let rows: &[(u8, u32)] = match index {
        0 => &[(0, 0x00ff00), (51, 0xffff00), (100, 0xff0000)],
        1 => &[
            (0, 0xff0000),
            (26, 0xfff300),
            (42, 0x36ff48),
            (55, 0x62f1ff),
            (68, 0x000cff),
            (81, 0x8000ff),
            (100, 0xff0000),
        ],
        2 => &[(0, 0x0000ff), (51, 0xffffff), (100, 0xff0000)],
        3 => &[(0, 0), (100, 0x00ff00)],
        _ => return None,
    };
    Some(
        rows.iter()
            .map(|&(position, color)| GradientStop {
                position,
                color: Some(color),
            })
            .collect(),
    )
}
fn validate_layer(item: &Object) -> Result<()> {
    if item.text("name")? != "audio"
        || item
            .get("value")
            .is_some_and(|v| serde_json::from_str::<u16>(v.get()).ok() != Some(12))
    {
        return Err(invalid("selected layer is not Studio Audio12"));
    }
    Ok(())
}
fn merged(item: &Object, region: Option<&Object>, defaults: bool) -> Result<Object> {
    let mut result = if defaults {
        Object::parse(&encode(
            &serde_json::json!({"boost":1,"autoBoost":false,"decay":1,"colorStopsCustom":false,"colorStops":audio_preset(0).unwrap()}),
        )?)?
    } else {
        Object(vec![])
    };
    for raw in [item.get("params"), region.and_then(|r| r.get("params"))]
        .into_iter()
        .flatten()
    {
        for (key, value) in Object::parse(raw)?.0 {
            result.set(&key, Some(&value));
        }
    }
    Ok(result)
}
#[derive(Clone, Copy)]
pub(super) enum Edit {
    Property(AudioProperty, f64),
    Preset(usize),
    Stop(StopEdit),
}
impl Edit {
    pub(super) fn apply(self, params: &mut Object, projection: &Object, new: bool) -> Result<()> {
        if new {
            for (key, value) in &projection.0 {
                params.set(key, Some(value));
            }
        }
        match self {
            Self::Property(property, value) => {
                let raw = if property == AudioProperty::AutoBoost {
                    encode(&(value != 0.0))?
                } else {
                    encode(&value)?
                };
                params.set(property.field(), Some(&raw));
            }
            Self::Preset(index) => {
                params.set(
                    "colorStops",
                    Some(&encode(
                        &audio_preset(index).ok_or_else(|| invalid("unknown Audio preset"))?,
                    )?),
                );
                params.set("colorStopsCustom", Some(&encode(&false)?));
            }
            Self::Stop(edit) => {
                if params.get("colorStops").is_none() {
                    params.set("colorStops", projection.get("colorStops"));
                }
                spectrum::edit_stop(params, edit, 7)?;
            }
        }
        Ok(())
    }
}
impl Draft {
    fn audio_params(&self, id: &str, selection: &[LedSelection]) -> Result<Vec<Object>> {
        let selection = selected(selection)?;
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        if selection.is_empty() {
            return Ok(vec![]);
        }
        inventory(self, &selection)?;
        let mut seen = BTreeSet::new();
        let mut result = vec![];
        for raw in entries(&item, "regions")? {
            let region = Object::parse(&raw)?;
            let mut matched = false;
            for raw in entries(&region, "devices")? {
                let device = Object::parse(&raw)?;
                let id = device.text("id")?;
                for raw in entries(&device, "leds")? {
                    let key = led(&raw, &id)?;
                    if selection.contains(&key) {
                        if !seen.insert(key) {
                            return Err(invalid("overlapping selected Audio memberships"));
                        }
                        matched = true;
                    }
                }
            }
            if matched {
                result.push(merged(&item, Some(&region), false)?);
            }
        }
        if seen.len() < selection.len() {
            result.push(merged(&item, None, true)?);
        }
        Ok(result)
    }
    pub fn audio_selection_value(
        &self,
        id: &str,
        selection: &[LedSelection],
        property: AudioProperty,
    ) -> Result<SelectedAudioValue> {
        let values = self.audio_params(id, selection)?;
        let read = |params: &Object| -> Result<Option<f64>> {
            let raw = params.get(property.field());
            if property == AudioProperty::AutoBoost {
                return raw
                    .map(|v| {
                        serde_json::from_str::<bool>(v.get())
                            .map(|v| if v { 1.0 } else { 0.0 })
                            .map_err(invalid)
                    })
                    .transpose();
            }
            let value = raw
                .map(|v| serde_json::from_str::<f64>(v.get()).map_err(invalid))
                .transpose()?;
            Ok(value)
        };
        let Some(first) = values.first() else {
            return Ok(SelectedAudioValue::Empty);
        };
        let first = read(first)?;
        for params in &values[1..] {
            if read(params)? != first {
                return Ok(SelectedAudioValue::Mixed);
            }
        }
        Ok(SelectedAudioValue::Uniform(first))
    }
    pub fn audio_selection_gradient(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<SelectedGradient> {
        let values = self.audio_params(id, selection)?;
        let Some(first) = values.first() else {
            return Ok(SelectedGradient::Empty);
        };
        let first = spectrum::gradient(first)?;
        for params in &values[1..] {
            if spectrum::gradient(params)? != first {
                return Ok(SelectedGradient::Mixed);
            }
        }
        Ok(SelectedGradient::Uniform(first))
    }
    fn edit_audio(&mut self, id: &str, selection: &[LedSelection], edit: Edit) -> Result<bool> {
        let params = self.audio_params(id, selection)?;
        if matches!(edit, Edit::Property(AudioProperty::Boost, _)) {
            for params in &params {
                if params
                    .get("autoBoost")
                    .map(|v| serde_json::from_str::<bool>(v.get()).map_err(invalid))
                    .transpose()?
                    .unwrap_or(false)
                {
                    return Err(invalid("turn off Auto before editing Audio Meter Boost"));
                }
            }
        }
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let projection = merged(&item, None, true)?;
        self.edit_regions(id, selection, RegionEdit::Audio(edit, &projection))
    }
    pub fn set_audio_property(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        property: AudioProperty,
        value: f64,
    ) -> Result<bool> {
        property.validate(value)?;
        self.edit_audio(id, selection, Edit::Property(property, value))
    }
    pub fn set_audio_preset(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        index: usize,
    ) -> Result<bool> {
        audio_preset(index).ok_or_else(|| invalid("unknown Audio preset"))?;
        self.edit_audio(id, selection, Edit::Preset(index))
    }
    pub fn edit_audio_stop(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        edit: StopEdit,
    ) -> Result<bool> {
        match edit {
            StopEdit::Set {
                position, color, ..
            }
            | StopEdit::Insert { position, color } => {
                spectrum::validate_stop(&GradientStop { position, color })?
            }
            StopEdit::Delete { .. } => {}
        }
        if !matches!(
            self.audio_selection_gradient(id, selection)?,
            SelectedGradient::Uniform(Some(_))
        ) {
            return Err(invalid("choose one uniform Audio gradient first"));
        }
        self.edit_audio(id, selection, Edit::Stop(edit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = r#"{"id":"p","title":"Audio","devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2},{"row":0,"col":3}]}],"layers":[{"id":"a","type":"layer","name":"audio","value":12,"title":"AUDIO_METER","params":{"future":9.9900e+100},"regions":[{"params":{"boost":1.2500,"decay":0.7000,"autoBoost":false,"colorStops":[{"Stop":0,"Color":123,"opaque":1.2300e-7},{"Stop":100}],"future":9007199254740993123456789},"devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2}]}]}]}]}"#;
    fn keys(cols: &[u16]) -> Vec<LedSelection> {
        cols.iter()
            .map(|&col| LedSelection {
                device: "k".into(),
                row: 0,
                col,
            })
            .collect()
    }
    #[test]
    fn source_defaults_presets_and_ranges_match_independent_ast_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/studio-audio-settings.json"
        ))
        .unwrap();
        for index in 0..4 {
            assert_eq!(
                serde_json::to_value(audio_preset(index).unwrap()).unwrap(),
                fixture["presets"][index]
            );
        }
        assert!(audio_preset(4).is_none());
        let mut draft = Draft::open(PROFILE).unwrap();
        draft.set_audio_preset("a", &keys(&[3]), 0).unwrap();
        for (property, field) in [
            (AudioProperty::Boost, "boost"),
            (AudioProperty::Decay, "decay"),
        ] {
            assert_eq!(
                draft
                    .audio_selection_value("a", &keys(&[3]), property)
                    .unwrap(),
                SelectedAudioValue::Uniform(fixture[field].as_f64())
            );
            for suffix in ["min", "max"] {
                property
                    .validate(fixture[format!("{field}_{suffix}")].as_f64().unwrap())
                    .unwrap();
            }
        }
        assert_eq!(fixture["max_stops"], 7);
        assert_eq!(
            draft
                .audio_selection_value("a", &keys(&[3]), AudioProperty::AutoBoost)
                .unwrap(),
            SelectedAudioValue::Uniform(Some(0.0))
        );
    }
    #[test]
    fn regional_edits_preserve_raw_unselected_memberships_and_auto_guards_boost() {
        let mut draft = Draft::open(PROFILE).unwrap();
        draft
            .set_audio_property("a", &keys(&[1]), AudioProperty::Decay, 0.3)
            .unwrap();
        assert_eq!(
            draft
                .audio_selection_value("a", &keys(&[1, 2]), AudioProperty::Decay)
                .unwrap(),
            SelectedAudioValue::Mixed
        );
        assert!(draft.json().contains("1.2500"));
        assert!(draft.json().contains("0.7000"));
        for raw in ["9.9900e+100", "1.2300e-7", "9007199254740993123456789"] {
            assert!(draft.json().contains(raw));
        }
        draft.undo().unwrap();
        assert_eq!(draft.json(), PROFILE);
        draft.redo().unwrap();
        draft
            .set_audio_property("a", &keys(&[1]), AudioProperty::AutoBoost, 1.0)
            .unwrap();
        let before = draft.json().to_owned();
        assert!(draft
            .set_audio_property("a", &keys(&[1, 2]), AudioProperty::Boost, 2.0)
            .is_err());
        assert_eq!(draft.json(), before);
        draft
            .set_audio_property("a", &keys(&[2]), AudioProperty::Boost, 2.0)
            .unwrap();
        draft.set_audio_preset("a", &keys(&[1]), 1).unwrap(); // Auto disables Boost only.
        draft
            .edit_audio_stop(
                "a",
                &keys(&[1]),
                StopEdit::Set {
                    index: 0,
                    position: 0,
                    color: None,
                },
            )
            .unwrap();
        let SelectedGradient::Uniform(Some(stops)) =
            draft.audio_selection_gradient("a", &keys(&[1])).unwrap()
        else {
            panic!()
        };
        assert_eq!(stops[0].color, None);
    }
    #[test]
    fn invalid_and_stale_edits_are_atomic_and_stop_bounds_are_enforced() {
        let mut draft = Draft::open(PROFILE).unwrap();
        for (property, value) in [
            (AudioProperty::Boost, 0.0),
            (AudioProperty::Boost, 4.25),
            (AudioProperty::Boost, 0.3),
            (AudioProperty::Decay, 0.0),
            (AudioProperty::Decay, 2.1),
            (AudioProperty::Decay, 0.15),
            (AudioProperty::AutoBoost, 2.0),
            (AudioProperty::Boost, f64::NAN),
            (AudioProperty::Decay, f64::INFINITY),
        ] {
            assert!(draft
                .set_audio_property("a", &keys(&[1]), property, value)
                .is_err());
            assert_eq!(draft.json(), PROFILE);
        }
        assert!(draft.set_audio_preset("a", &keys(&[99]), 0).is_err());
        assert!(draft.set_audio_preset("a", &keys(&[1]), 4).is_err());
        assert!(draft
            .set_audio_property("missing", &keys(&[1]), AudioProperty::Boost, 1.0)
            .is_err());
        assert!(draft
            .edit_audio_stop("a", &keys(&[1]), StopEdit::Delete { index: 0 })
            .is_err());
        assert_eq!(draft.json(), PROFILE);
        draft.set_audio_preset("a", &keys(&[1]), 1).unwrap();
        let before = draft.json().to_owned();
        assert!(draft
            .edit_audio_stop(
                "a",
                &keys(&[1]),
                StopEdit::Insert {
                    position: 20,
                    color: Some(1)
                }
            )
            .is_err());
        assert_eq!(draft.json(), before);
        let bad = PROFILE.replace("\"value\":12", "\"value\":2");
        let mut draft = Draft::open(&bad).unwrap();
        assert!(draft.set_audio_preset("a", &keys(&[1]), 0).is_err());
        assert_eq!(draft.json(), bad);
    }
    #[test]
    fn inherited_gradient_materializes_only_selected_region_and_retains_opaque_stops() {
        let mut value: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
        value["layers"][0]["params"]["colorStops"] =
            value["layers"][0]["regions"][0]["params"]["colorStops"].take();
        value["layers"][0]["regions"][0]["params"]
            .as_object_mut()
            .unwrap()
            .remove("colorStops");
        let mut draft = Draft::open(&value.to_string()).unwrap();
        draft
            .edit_audio_stop(
                "a",
                &keys(&[1]),
                StopEdit::Set {
                    index: 1,
                    position: 100,
                    color: Some(0x112233),
                },
            )
            .unwrap();
        let out: serde_json::Value = serde_json::from_str(draft.json()).unwrap();
        assert_eq!(out["layers"][0]["params"], value["layers"][0]["params"]);
        assert_eq!(
            draft.audio_selection_gradient("a", &keys(&[2])).unwrap(),
            SelectedGradient::Uniform(Some(vec![
                GradientStop {
                    position: 0,
                    color: Some(123)
                },
                GradientStop {
                    position: 100,
                    color: None
                }
            ]))
        );
        assert!(draft.json().contains("opaque"));
    }
}
