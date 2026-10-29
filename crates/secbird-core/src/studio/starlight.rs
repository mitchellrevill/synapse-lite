//! Source Studio Starlight settings; playback admission remains separate.
use super::*;

pub const STARLIGHT_DURATIONS: [u32; 3] = [3000, 1700, 500];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StarlightProperty {
    Duration,
    Density,
    Random,
}
impl StarlightProperty {
    fn field(self) -> &'static str {
        match self {
            Self::Duration => "duration",
            Self::Density => "density",
            Self::Random => "randomColor",
        }
    }
    fn validate(self, value: u32) -> Result<()> {
        if match self {
            Self::Duration => STARLIGHT_DURATIONS.contains(&value),
            Self::Density => (1..=10).contains(&value),
            Self::Random => value <= 1,
        } {
            Ok(())
        } else {
            Err(invalid(
                "Starlight requires a source duration, density1..10, or a boolean random setting",
            ))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedStarlightValue {
    Empty,
    Uniform(Option<u32>),
    Mixed,
}

fn validate_layer(item: &Object) -> Result<()> {
    if item.text("name")? != "starlight" || item.get("value").is_some_and(|v| v.get() != "7") {
        return Err(invalid("selected layer is not Studio Starlight7"));
    }
    Ok(())
}
fn merged(item: &Object, region: Option<&Object>, defaults: bool) -> Result<Object> {
    let mut result = if defaults {
        Object::parse(&encode(
            &serde_json::json!({"duration":1700,"density":1,"randomColor":false,"colorStopsCustom":false,"colorStops":wave_preset(0).unwrap()}),
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
    Property(StarlightProperty, u32),
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
                let raw = if property == StarlightProperty::Random {
                    encode(&(value != 0))?
                } else {
                    encode(&value)?
                };
                params.set(property.field(), Some(&raw));
            }
            Self::Preset(index) => {
                params.set(
                    "colorStops",
                    Some(&encode(
                        &wave_preset(index).ok_or_else(|| invalid("unknown Starlight preset"))?,
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
    fn starlight_params(&self, id: &str, selection: &[LedSelection]) -> Result<Vec<Object>> {
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
                            return Err(invalid("overlapping selected Starlight memberships"));
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
    pub fn starlight_selection_value(
        &self,
        id: &str,
        selection: &[LedSelection],
        property: StarlightProperty,
    ) -> Result<SelectedStarlightValue> {
        let values = self.starlight_params(id, selection)?;
        let read = |params: &Object| -> Result<Option<u32>> {
            let raw = params.get(property.field());
            if property == StarlightProperty::Random {
                return raw
                    .map(|v| {
                        serde_json::from_str::<bool>(v.get())
                            .map(u32::from)
                            .map_err(invalid)
                    })
                    .transpose();
            }
            let value = raw
                .map(|v| serde_json::from_str::<u32>(v.get()).map_err(invalid))
                .transpose()?;
            Ok(if property == StarlightProperty::Duration {
                Some(
                    value
                        .filter(|v| STARLIGHT_DURATIONS.contains(v))
                        .unwrap_or(1700),
                )
            } else {
                value
            })
        };
        let Some(first) = values.first() else {
            return Ok(SelectedStarlightValue::Empty);
        };
        let first = read(first)?;
        for params in &values[1..] {
            if read(params)? != first {
                return Ok(SelectedStarlightValue::Mixed);
            }
        }
        Ok(SelectedStarlightValue::Uniform(first))
    }
    pub fn starlight_selection_gradient(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<SelectedGradient> {
        let values = self.starlight_params(id, selection)?;
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
    fn edit_starlight(&mut self, id: &str, selection: &[LedSelection], edit: Edit) -> Result<bool> {
        let params = self.starlight_params(id, selection)?;
        if !matches!(edit, Edit::Property(..)) {
            for params in &params {
                if params
                    .get("randomColor")
                    .map(|v| serde_json::from_str::<bool>(v.get()).map_err(invalid))
                    .transpose()?
                    .unwrap_or(false)
                {
                    return Err(invalid(
                        "turn off Random Colour before editing the Starlight gradient",
                    ));
                }
            }
        }
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let projection = merged(&item, None, true)?;
        self.edit_regions(id, selection, RegionEdit::Starlight(edit, &projection))
    }
    pub fn set_starlight_property(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        property: StarlightProperty,
        value: u32,
    ) -> Result<bool> {
        property.validate(value)?;
        self.edit_starlight(id, selection, Edit::Property(property, value))
    }
    pub fn set_starlight_preset(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        index: usize,
    ) -> Result<bool> {
        wave_preset(index).ok_or_else(|| invalid("unknown Starlight preset"))?;
        self.edit_starlight(id, selection, Edit::Preset(index))
    }
    pub fn edit_starlight_stop(
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
            self.starlight_selection_gradient(id, selection)?,
            SelectedGradient::Uniform(Some(_))
        ) {
            return Err(invalid("choose one uniform Starlight gradient first"));
        }
        self.edit_starlight(id, selection, Edit::Stop(edit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = r#"{"id":"p","title":"Starlight","devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2},{"row":0,"col":3}]}],"layers":[{"id":"s","type":"layer","name":"starlight","value":7,"title":"STARLIGHT","params":{"future":9.9900e+100},"regions":[{"params":{"duration":9999,"density":4,"randomColor":false,"colorStops":[{"Stop":0,"Color":123,"opaque":1.2300e-7},{"Stop":100}],"future":9007199254740993123456789},"devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2}]}]}]}]}"#;
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
    fn defaults_match_independently_extracted_source_constants() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/studio-starlight-settings.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["source_sha256"],
            "109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19"
        );
        assert_eq!(
            serde_json::to_value(STARLIGHT_DURATIONS).unwrap(),
            fixture["durations"]
        );
        let mut draft = Draft::open(PROFILE).unwrap();
        draft
            .set_starlight_property(
                "s",
                &keys(&[3]),
                StarlightProperty::Density,
                fixture["default_density"].as_u64().unwrap() as u32,
            )
            .unwrap();
        assert_eq!(
            draft
                .starlight_selection_value("s", &keys(&[3]), StarlightProperty::Duration)
                .unwrap(),
            SelectedStarlightValue::Uniform(Some(
                fixture["default_duration"].as_u64().unwrap() as u32
            ))
        );
        let SelectedGradient::Uniform(Some(stops)) = draft
            .starlight_selection_gradient("s", &keys(&[3]))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(
            serde_json::to_value(stops).unwrap(),
            fixture["default_gradient"]
        );
    }
    #[test]
    fn source_settings_partition_and_history_keep_untouched_raw_data() {
        let mut draft = Draft::open(PROFILE).unwrap();
        assert_eq!(
            draft
                .starlight_selection_value("s", &keys(&[1]), StarlightProperty::Duration)
                .unwrap(),
            SelectedStarlightValue::Uniform(Some(1700))
        );
        draft
            .set_starlight_property("s", &keys(&[1, 3]), StarlightProperty::Density, 10)
            .unwrap();
        assert_eq!(
            draft
                .starlight_selection_value("s", &keys(&[1, 3]), StarlightProperty::Density)
                .unwrap(),
            SelectedStarlightValue::Uniform(Some(10))
        );
        assert_eq!(
            draft
                .starlight_selection_value("s", &keys(&[2]), StarlightProperty::Density)
                .unwrap(),
            SelectedStarlightValue::Uniform(Some(4))
        );
        assert_eq!(
            draft
                .starlight_selection_value("s", &keys(&[1, 2]), StarlightProperty::Density)
                .unwrap(),
            SelectedStarlightValue::Mixed
        );
        assert!(draft.json().contains("9.9900e+100"));
        assert!(draft.json().contains("1.2300e-7"));
        assert!(draft.json().contains("9007199254740993123456789"));
        assert!(draft.json().contains("9999")); // an unrelated edit preserves unsupported imported duration
        assert_eq!(
            draft
                .starlight_selection_gradient("s", &keys(&[3]))
                .unwrap(),
            SelectedGradient::Uniform(Some(wave_preset(0).unwrap()))
        );
        draft.undo().unwrap();
        assert_eq!(draft.json(), PROFILE);
        draft.redo().unwrap();
        draft
            .set_starlight_property("s", &keys(&[1]), StarlightProperty::Random, 1)
            .unwrap();
        let before = draft.json().to_owned();
        assert!(draft.set_starlight_preset("s", &keys(&[1]), 4).is_err());
        assert!(draft
            .edit_starlight_stop("s", &keys(&[1]), StopEdit::Delete { index: 0 })
            .is_err());
        assert_eq!(draft.json(), before);
        draft
            .set_starlight_property("s", &keys(&[1]), StarlightProperty::Random, 0)
            .unwrap();
        draft
            .edit_starlight_stop(
                "s",
                &keys(&[1]),
                StopEdit::Set {
                    index: 0,
                    position: 20,
                    color: None,
                },
            )
            .unwrap();
        assert!(draft.json().contains("1.2300e-7"));
        let SelectedGradient::Uniform(Some(stops)) = draft
            .starlight_selection_gradient("s", &keys(&[1]))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(
            stops[0],
            GradientStop {
                position: 20,
                color: None
            }
        );
    }
    #[test]
    fn invalid_properties_id_and_inventory_refuse_without_dirtying() {
        let mut draft = Draft::open(PROFILE).unwrap();
        for (p, v) in [
            (StarlightProperty::Density, 0),
            (StarlightProperty::Density, 11),
            (StarlightProperty::Duration, 2000),
            (StarlightProperty::Random, 2),
        ] {
            assert!(draft
                .set_starlight_property("s", &keys(&[1]), p, v)
                .is_err());
        }
        assert!(draft
            .set_starlight_property("s", &keys(&[99]), StarlightProperty::Density, 1)
            .is_err());
        assert!(draft.set_starlight_preset("s", &keys(&[1]), 5).is_err());
        assert_eq!(draft.json(), PROFILE);
        assert!(!draft.is_dirty());
        let before = PROFILE.replace("\"value\":7", "\"value\":8");
        let mut draft = Draft::open(&before).unwrap();
        assert!(draft
            .set_starlight_property("s", &keys(&[1]), StarlightProperty::Density, 1)
            .is_err());
        assert_eq!(draft.json(), before);
    }
    #[test]
    fn inherited_gradient_and_scalar_edits_remain_independent() {
        let mut value: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
        value["layers"][0]["params"]["colorStops"] =
            serde_json::json!([{"Stop":0,"Color":1},{"Stop":100,"Color":2}]);
        value["layers"][0]["regions"][0]["params"]
            .as_object_mut()
            .unwrap()
            .remove("colorStops");
        let mut draft = Draft::open(&value.to_string()).unwrap();
        draft
            .edit_starlight_stop(
                "s",
                &keys(&[1]),
                StopEdit::Set {
                    index: 0,
                    position: 0,
                    color: Some(0x112233),
                },
            )
            .unwrap();
        let out: serde_json::Value = serde_json::from_str(draft.json()).unwrap();
        assert_eq!(out["layers"][0]["params"], value["layers"][0]["params"]);
        draft
            .set_starlight_property("s", &keys(&[1]), StarlightProperty::Duration, 500)
            .unwrap();
        assert_eq!(
            draft
                .starlight_selection_value("s", &keys(&[1]), StarlightProperty::Duration)
                .unwrap(),
            SelectedStarlightValue::Uniform(Some(500))
        );
        let SelectedGradient::Uniform(Some(stops)) = draft
            .starlight_selection_gradient("s", &keys(&[1]))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(stops[0].color, Some(0x112233));
    }
}
