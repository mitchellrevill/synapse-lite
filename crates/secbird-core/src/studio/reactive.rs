//! Studio Reactive region authoring. Key-triggered playback is a separate contract.
use super::*;

pub const REACTIVE_DURATIONS: [u32; 3] = [2000, 1500, 500];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactiveProperty {
    Color,
    Duration,
    Random,
}
impl ReactiveProperty {
    fn field(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Duration => "duration",
            Self::Random => "randomColor",
        }
    }
    fn validate(self, value: Option<u32>) -> Result<()> {
        if match (self, value) {
            (Self::Color, None) => true,
            (Self::Color, Some(v)) => v <= 0xffffff,
            (Self::Duration, Some(v)) => REACTIVE_DURATIONS.contains(&v),
            (Self::Random, Some(v)) => v <= 1,
            _ => false,
        } {
            Ok(())
        } else {
            Err(invalid(
                "Reactive requires RGB24/None, a source duration, or a boolean random setting",
            ))
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedReactiveValue {
    Empty,
    Uniform(Option<u32>),
    Mixed,
}
fn validate_layer(item: &Object) -> Result<()> {
    if item.text("name")? != "reactive" || item.get("value").is_some_and(|v| v.get() != "3") {
        return Err(invalid("selected layer is not Studio Reactive3"));
    }
    Ok(())
}
fn merged(item: &Object, region: Option<&Object>, defaults: bool) -> Result<Object> {
    let mut result = if defaults {
        Object::parse(&encode(&serde_json::json!({
            "color":65280,"randomColor":false,"duration":1500,
            "playbackStart":"onPress","playbackEnd":"after","cycles":1
        }))?)?
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
pub(super) struct Edit {
    pub property: ReactiveProperty,
    pub value: Option<u32>,
}
impl Edit {
    pub(super) fn apply(self, params: &mut Object, projection: &Object, new: bool) -> Result<()> {
        if new {
            for (key, value) in &projection.0 {
                params.set(key, Some(value));
            }
        }
        let value = match (self.property, self.value) {
            (ReactiveProperty::Random, Some(value)) => Some(encode(&(value != 0))?),
            (_, value) => value.map(|value| encode(&value)).transpose()?,
        };
        params.set(self.property.field(), value.as_deref());
        Ok(())
    }
}
impl Draft {
    fn reactive_params(&self, id: &str, selection: &[LedSelection]) -> Result<Vec<Object>> {
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
                            return Err(invalid("overlapping selected Reactive memberships"));
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
    pub fn reactive_selection_value(
        &self,
        id: &str,
        selection: &[LedSelection],
        property: ReactiveProperty,
    ) -> Result<SelectedReactiveValue> {
        let values = self.reactive_params(id, selection)?;
        let read = |params: &Object| -> Result<Option<u32>> {
            params
                .get(property.field())
                .map(|raw| {
                    if property == ReactiveProperty::Random {
                        serde_json::from_str::<bool>(raw.get())
                            .map(u32::from)
                            .map_err(invalid)
                    } else {
                        serde_json::from_str::<u32>(raw.get()).map_err(invalid)
                    }
                })
                .transpose()
        };
        let Some(first) = values.first() else {
            return Ok(SelectedReactiveValue::Empty);
        };
        let first = read(first)?;
        for params in &values[1..] {
            if read(params)? != first {
                return Ok(SelectedReactiveValue::Mixed);
            }
        }
        Ok(SelectedReactiveValue::Uniform(first))
    }
    pub fn set_reactive_property(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        property: ReactiveProperty,
        value: Option<u32>,
    ) -> Result<bool> {
        property.validate(value)?;
        let params = self.reactive_params(id, selection)?;
        if property == ReactiveProperty::Color {
            for params in params {
                if params
                    .get("randomColor")
                    .map(|v| serde_json::from_str::<bool>(v.get()).map_err(invalid))
                    .transpose()?
                    .unwrap_or(false)
                {
                    return Err(invalid(
                        "turn off Random Colour before editing the Reactive colour",
                    ));
                }
            }
        }
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        if property == ReactiveProperty::Color
            && value.is_none()
            && item
                .get("params")
                .map(Object::parse)
                .transpose()?
                .is_some_and(|p| p.get("color").is_some())
        {
            return Err(invalid(
                "clearing an inherited Reactive layer colour is not verified; original retained",
            ));
        }
        let projection = merged(&item, None, true)?;
        self.edit_regions(
            id,
            selection,
            RegionEdit::Reactive(Edit { property, value }, &projection),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = r#"{"id":"p","title":"Reactive","devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2},{"row":0,"col":3}]}],"layers":[{"id":"r","type":"layer","name":"reactive","value":3,"title":"REACTIVE","params":{"future":9.9900e+100},"regions":[{"params":{"color":123,"duration":9999,"randomColor":false,"opaque":1.2300e-7,"future":9007199254740993123456789},"devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2}]}]}]}]}"#;
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
    fn new_memberships_use_independently_extracted_reactive_defaults() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/studio-reactive-settings.json"
        ))
        .unwrap();
        assert_eq!(fixture["durations"], serde_json::json!(REACTIVE_DURATIONS));
        let mut draft = Draft::open(PROFILE).unwrap();
        draft
            .set_reactive_property("r", &keys(&[3]), ReactiveProperty::Duration, Some(1500))
            .unwrap();
        let saved: serde_json::Value = serde_json::from_str(draft.json()).unwrap();
        let region = saved["layers"][0]["regions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["devices"][0]["leds"][0]["col"] == 3)
            .unwrap();
        for name in [
            "duration",
            "color",
            "playbackStart",
            "playbackEnd",
            "cycles",
        ] {
            assert_eq!(region["params"][name], fixture[name], "{name}");
        }
        assert_eq!(region["params"]["randomColor"], false);
        assert!(draft.json().contains("9.9900e+100"));
    }
    #[test]
    fn split_random_none_and_history_preserve_unselected_and_raw_data() {
        let mut draft = Draft::open(PROFILE).unwrap();
        draft
            .set_reactive_property("r", &keys(&[1]), ReactiveProperty::Random, Some(1))
            .unwrap();
        let before = draft.json().to_string();
        assert!(draft
            .set_reactive_property("r", &keys(&[1]), ReactiveProperty::Color, Some(0x112233))
            .is_err());
        assert_eq!(draft.json(), before);
        draft
            .set_reactive_property("r", &keys(&[1]), ReactiveProperty::Random, Some(0))
            .unwrap();
        draft
            .set_reactive_property("r", &keys(&[1]), ReactiveProperty::Color, None)
            .unwrap();
        assert_eq!(
            draft
                .reactive_selection_value("r", &keys(&[1]), ReactiveProperty::Color)
                .unwrap(),
            SelectedReactiveValue::Uniform(None)
        );
        assert_eq!(
            draft
                .reactive_selection_value("r", &keys(&[2]), ReactiveProperty::Color)
                .unwrap(),
            SelectedReactiveValue::Uniform(Some(123))
        );
        assert_eq!(
            draft
                .reactive_selection_value("r", &keys(&[1, 2]), ReactiveProperty::Color)
                .unwrap(),
            SelectedReactiveValue::Mixed
        );
        assert_eq!(
            draft
                .reactive_selection_value("r", &keys(&[1]), ReactiveProperty::Duration)
                .unwrap(),
            SelectedReactiveValue::Uniform(Some(9999))
        );
        assert!(
            draft.json().contains("1.2300e-7")
                && draft.json().contains("9007199254740993123456789")
        );
        let edited = draft.json().to_string();
        draft.undo().unwrap();
        assert_ne!(draft.json(), edited);
        draft.redo().unwrap();
        assert_eq!(draft.json(), edited);
    }
    #[test]
    fn invalid_identity_inventory_properties_and_inherited_clear_are_atomic() {
        let mut draft = Draft::open(PROFILE).unwrap();
        for (property, value) in [
            (ReactiveProperty::Color, Some(0x1000000)),
            (ReactiveProperty::Duration, Some(0)),
            (ReactiveProperty::Duration, None),
            (ReactiveProperty::Random, Some(2)),
        ] {
            assert!(draft
                .set_reactive_property("r", &keys(&[1]), property, value)
                .is_err());
        }
        assert!(draft
            .set_reactive_property("r", &keys(&[99]), ReactiveProperty::Duration, Some(500))
            .is_err());
        assert_eq!(draft.json(), PROFILE);
        assert!(!draft.is_dirty());
        let wrong = PROFILE.replace("\"value\":3", "\"value\":4");
        let mut draft = Draft::open(&wrong).unwrap();
        assert!(draft
            .set_reactive_property("r", &keys(&[1]), ReactiveProperty::Random, Some(1))
            .is_err());
        assert_eq!(draft.json(), wrong);
        let inherited = PROFILE.replace(
            "\"params\":{\"future\":",
            "\"params\":{\"color\":456,\"future\":",
        );
        let mut draft = Draft::open(&inherited).unwrap();
        assert!(draft
            .set_reactive_property("r", &keys(&[1]), ReactiveProperty::Color, None)
            .is_err());
        assert_eq!(draft.json(), inherited);
    }
}
