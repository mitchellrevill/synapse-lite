//! Regional Wave authoring. Existing leaves remain raw until explicitly edited.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveProperty {
    Angle,
    Speed,
    Width,
    Pause,
}
impl WaveProperty {
    fn field(self) -> &'static str {
        match self {
            Self::Angle => "angle",
            Self::Speed => "speed",
            Self::Width => "width",
            Self::Pause => "pause",
        }
    }
    fn validate(self, value: u32) -> Result<()> {
        let valid = match self {
            Self::Angle => [0, 90, 180, 270].contains(&value),
            Self::Speed => value <= 50,
            Self::Width => (100..=400).contains(&value),
            Self::Pause => value <= 60000,
        };
        if valid {
            Ok(())
        } else {
            Err(invalid("choose a supported cardinal Wave value (speed0..50, width100..400, pause0..60000ms)"))
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedWaveValue {
    Empty,
    Uniform(Option<u32>),
    Mixed,
}

/// Historical Studio module6257.k, distinct from Spectrum presets and Quick Wave.
pub fn wave_preset(index: usize) -> Option<Vec<GradientStop>> {
    let rows: &[(u8, u32)] = match index {
        0 => &[
            (0, 0xff0000),
            (26, 0xfff300),
            (42, 0x36ff48),
            (55, 0x62f1ff),
            (68, 0x000cff),
            (81, 0x8000ff),
            (100, 0xff0000),
        ],
        1 => &[(0, 0), (25, 0xffff00), (50, 0), (75, 0x00ff00), (100, 0)],
        2 => &[(0, 0), (50, 0x00ff00), (100, 0)],
        3 => &[(0, 0xff0000), (100, 0xffff00)],
        4 => &[(0, 0x00ff00), (100, 0x00ff00)],
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
pub(super) fn validate_layer(item: &Object) -> Result<()> {
    if item.text("name")? != "wave" || item.get("value").is_some_and(|v| v.get() != "2") {
        return Err(invalid("selected layer is not Studio Wave2"));
    }
    Ok(())
}
fn merged(item: &Object, region: Option<&Object>, defaults: bool) -> Result<Object> {
    let mut params = if defaults {
        Object::parse(&encode(
            &serde_json::json!({"speed":15,"width":100,"pause":0,"angle":90,"split":false,"playbackStart":"random","playbackEnd":"never","cycles":-1,"colorStopsCustom":false,"colorStops":wave_preset(0).unwrap()}),
        )?)?
    } else {
        Object(vec![])
    };
    if let Some(raw) = item.get("params") {
        for (name, value) in Object::parse(raw)?.0 {
            params.set(&name, Some(&value));
        }
    }
    if let Some(region) = region {
        for (name, value) in Object::parse(
            region
                .get("params")
                .ok_or_else(|| invalid("region has no params"))?,
        )?
        .0
        {
            params.set(&name, Some(&value));
        }
    }
    Ok(params)
}
fn authorable(params: &Object) -> Result<()> {
    let start = params
        .get("playbackStart")
        .map(|v| serde_json::from_str::<String>(v.get()).map_err(invalid))
        .transpose()?;
    let end = params
        .get("playbackEnd")
        .map(|v| serde_json::from_str::<String>(v.get()).map_err(invalid))
        .transpose()?;
    let cycles = params
        .get("cycles")
        .map(|v| serde_json::from_str::<i64>(v.get()).map_err(invalid))
        .transpose()?;
    let split = params
        .get("split")
        .map(|v| serde_json::from_str::<bool>(v.get()).map_err(invalid))
        .transpose()?
        .unwrap_or(false);
    if split
        || params.get("counterclockwise").is_some()
        || cycles != Some(-1)
        || start.as_deref().is_some_and(|s| s != "random")
        || end.as_deref().is_some_and(|s| s != "never")
    {
        return Err(invalid("Wave rotational, split, finite and input-triggered editing is not implemented; imported settings were preserved"));
    }
    Ok(())
}
#[derive(Clone, Copy)]
pub(super) enum Edit {
    Property(WaveProperty, u32),
    Preset(usize),
    Motion(u32, u32, u32),
    Stop(StopEdit),
}
impl Edit {
    pub(super) fn apply(self, params: &mut Object) -> Result<()> {
        match self {
            Self::Stop(edit) => spectrum::edit_stop(params, edit, 7)?,
            Self::Motion(speed, width, pause) => {
                for (property, value) in [
                    (WaveProperty::Speed, speed),
                    (WaveProperty::Width, width),
                    (WaveProperty::Pause, pause),
                ] {
                    params.set(property.field(), Some(&encode(&value)?));
                }
            }
            Self::Property(property, value) => {
                params.set(property.field(), Some(&encode(&value)?));
            }
            Self::Preset(index) => {
                params.set(
                    "colorStops",
                    Some(&encode(
                        &wave_preset(index).ok_or_else(|| invalid("unknown Wave preset"))?,
                    )?),
                );
                params.set("colorStopsCustom", Some(&encode(&false)?));
            }
        }
        Ok(())
    }
}
impl Draft {
    fn wave_params(&self, id: &str, selection: &[LedSelection]) -> Result<Vec<Object>> {
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
                let device_id = device.text("id")?;
                for raw in entries(&device, "leds")? {
                    let key = led(&raw, &device_id)?;
                    if selection.contains(&key) {
                        if !seen.insert(key) {
                            return Err(invalid("overlapping selected region memberships"));
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
    pub fn wave_selection_value(
        &self,
        id: &str,
        selection: &[LedSelection],
        property: WaveProperty,
    ) -> Result<SelectedWaveValue> {
        let params = self.wave_params(id, selection)?;
        let value = |p: &Object| -> Result<Option<u32>> {
            p.get(property.field())
                .map(|v| serde_json::from_str(v.get()).map_err(invalid))
                .transpose()
        };
        let Some(first) = params.first() else {
            return Ok(SelectedWaveValue::Empty);
        };
        let first = value(first)?;
        for params in &params[1..] {
            if value(params)? != first {
                return Ok(SelectedWaveValue::Mixed);
            }
        }
        Ok(SelectedWaveValue::Uniform(first))
    }
    fn edit_wave(&mut self, id: &str, selection: &[LedSelection], edit: Edit) -> Result<bool> {
        for params in self.wave_params(id, selection)? {
            authorable(&params)?;
        }
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let projection = merged(&item, None, true)?;
        self.edit_regions(id, selection, RegionEdit::Wave(edit, &projection))
    }
    pub fn wave_selection_gradient(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<SelectedGradient> {
        let params = self.wave_params(id, selection)?;
        let Some(first) = params.first() else {
            return Ok(SelectedGradient::Empty);
        };
        let first = spectrum::gradient(first)?;
        for params in &params[1..] {
            if spectrum::gradient(params)? != first {
                return Ok(SelectedGradient::Mixed);
            }
        }
        Ok(SelectedGradient::Uniform(first))
    }
    pub fn edit_wave_stop(
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
            self.wave_selection_gradient(id, selection)?,
            SelectedGradient::Uniform(Some(_))
        ) {
            return Err(invalid(
                "choose one uniform Wave gradient before editing stops",
            ));
        }
        self.edit_wave(id, selection, Edit::Stop(edit))
    }
    pub fn set_wave_property(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        property: WaveProperty,
        value: u32,
    ) -> Result<bool> {
        property.validate(value)?;
        self.edit_wave(id, selection, Edit::Property(property, value))
    }
    pub fn set_wave_preset(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        index: usize,
    ) -> Result<bool> {
        wave_preset(index).ok_or_else(|| invalid("unknown Wave preset"))?;
        self.edit_wave(id, selection, Edit::Preset(index))
    }
    pub fn set_wave_motion(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        speed: u32,
        width: u32,
        pause_ms: u32,
    ) -> Result<bool> {
        for (property, value) in [
            (WaveProperty::Speed, speed),
            (WaveProperty::Width, width),
            (WaveProperty::Pause, pause_ms),
        ] {
            property.validate(value)?;
        }
        self.edit_wave(id, selection, Edit::Motion(speed, width, pause_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = r#"{"id":"p","title":"Wave","devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2},{"row":0,"col":3}]}],"layers":[{"id":"w","type":"layer","name":"wave","value":2,"title":"Wave","params":{"speed":15,"width":100,"pause":0,"angle":90,"cycles":-1,"opaque":9.9900e+100},"regions":[{"params":{"angle":270,"width":125,"future":9007199254740993123456789,"colorStops":[{"Stop":0,"Color":1,"futureStop":1.2300e-7},{"Stop":100,"Color":2}]},"devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2}]}]}]}]}"#;
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
    fn source_presets_match_independent_literal_extraction() {
        let data: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/studio-wave-presets.json"
        ))
        .unwrap();
        assert_eq!(
            data["source_sha256"],
            "109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19"
        );
        for (index, expected) in data["presets"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                serde_json::to_value(wave_preset(index).unwrap()).unwrap(),
                *expected
            );
        }
        assert!(wave_preset(5).is_none());
    }
    #[test]
    fn partition_preserves_raw_data_inheritance_and_atomic_motion_history() {
        let mut draft = Draft::open(PROFILE).unwrap();
        assert_eq!(
            draft
                .wave_selection_value("w", &keys(&[1, 3]), WaveProperty::Angle)
                .unwrap(),
            SelectedWaveValue::Mixed
        );
        assert_eq!(
            draft
                .wave_selection_value("w", &keys(&[1]), WaveProperty::Speed)
                .unwrap(),
            SelectedWaveValue::Uniform(Some(15))
        );
        assert!(draft
            .set_wave_motion("w", &keys(&[1, 3]), 0, 400, 79)
            .unwrap());
        assert!(draft.json().contains("9.9900e+100"));
        assert!(draft.json().contains("9007199254740993123456789"));
        assert!(draft.json().contains("1.2300e-7"));
        assert_eq!(
            draft
                .wave_selection_value("w", &keys(&[1, 3]), WaveProperty::Speed)
                .unwrap(),
            SelectedWaveValue::Uniform(Some(0))
        );
        assert_eq!(
            draft
                .wave_selection_value("w", &keys(&[2]), WaveProperty::Width)
                .unwrap(),
            SelectedWaveValue::Uniform(Some(125))
        );
        let params = draft.wave_params("w", &keys(&[3])).unwrap().remove(0);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(params.get("colorStops").unwrap().get())
                .unwrap(),
            serde_json::to_value(wave_preset(0).unwrap()).unwrap()
        );
        assert_eq!(params.get("cycles").unwrap().get(), "-1");
        assert!(draft.undo().unwrap());
        assert_eq!(draft.json(), PROFILE);
        assert!(!draft.is_dirty());
        assert!(draft.redo().unwrap());
        assert_eq!(
            draft
                .wave_selection_value("w", &keys(&[1]), WaveProperty::Pause)
                .unwrap(),
            SelectedWaveValue::Uniform(Some(79))
        );
    }
    #[test]
    fn invalid_and_unverified_edits_do_not_mutate_or_assign() {
        let mut draft = Draft::open(PROFILE).unwrap();
        assert!(!draft.set_wave_preset("w", &[], 0).unwrap());
        assert!(draft.set_wave_motion("w", &keys(&[1]), 10, 99, 0).is_err());
        assert!(draft
            .set_wave_property("w", &keys(&[1]), WaveProperty::Angle, 45)
            .is_err());
        assert!(draft.set_wave_preset("w", &keys(&[99]), 0).is_err());
        assert!(draft.set_wave_preset("w", &keys(&[1]), 5).is_err());
        assert_eq!(draft.json(), PROFILE);
        assert!(!draft.is_dirty());
        let mut raw: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
        raw["layers"][0]["params"]["counterclockwise"] = false.into();
        let before = raw.to_string();
        let mut draft = Draft::open(&before).unwrap();
        assert!(draft
            .set_wave_property("w", &keys(&[1]), WaveProperty::Angle, 180)
            .is_err());
        assert_eq!(draft.json(), before);
        assert!(!draft.is_dirty());
    }
    #[test]
    fn custom_stops_partition_preserve_raw_values_and_motion_and_history() {
        let mut draft = Draft::open(PROFILE).unwrap();
        draft
            .edit_wave_stop(
                "w",
                &keys(&[1]),
                StopEdit::Set {
                    index: 0,
                    position: 40,
                    color: None,
                },
            )
            .unwrap();
        assert!(draft.json().contains("1.2300e-7"));
        assert!(draft.json().contains("9.9900e+100"));
        let value: serde_json::Value = serde_json::from_str(draft.json()).unwrap();
        let regions = value["layers"][0]["regions"].as_array().unwrap();
        let edited = regions
            .iter()
            .find(|r| r["devices"][0]["leds"][0]["col"] == 1)
            .unwrap();
        assert_eq!(edited["params"]["colorStopsCustom"], true);
        assert_eq!(edited["params"]["colorStops"][0]["Stop"], 40);
        assert!(edited["params"]["colorStops"][0].get("Color").is_none());
        assert_eq!(edited["params"]["width"], 125);
        assert!(edited["params"].get("duration").is_none());
        assert_eq!(
            draft.wave_selection_gradient("w", &keys(&[2])).unwrap(),
            SelectedGradient::Uniform(Some(vec![
                GradientStop {
                    position: 0,
                    color: Some(1)
                },
                GradientStop {
                    position: 100,
                    color: Some(2)
                }
            ]))
        );
        assert_eq!(
            draft.wave_selection_gradient("w", &keys(&[1, 2])).unwrap(),
            SelectedGradient::Mixed
        );
        draft.undo().unwrap();
        assert_eq!(draft.json(), PROFILE);
        draft.redo().unwrap();
        draft
            .edit_wave_stop(
                "w",
                &keys(&[1]),
                StopEdit::Insert {
                    position: 40,
                    color: Some(0x112233),
                },
            )
            .unwrap();
        let SelectedGradient::Uniform(Some(stops)) =
            draft.wave_selection_gradient("w", &keys(&[1])).unwrap()
        else {
            panic!()
        };
        assert_eq!(
            stops.iter().map(|s| s.position).collect::<Vec<_>>(),
            vec![40, 40, 100]
        );
        assert_eq!(stops[0].color, None);
        draft
            .edit_wave_stop("w", &keys(&[1]), StopEdit::Delete { index: 1 })
            .unwrap();
        let before = draft.json().to_owned();
        assert!(draft
            .edit_wave_stop("w", &keys(&[1]), StopEdit::Delete { index: 0 })
            .is_err());
        assert!(draft
            .edit_wave_stop(
                "w",
                &keys(&[1]),
                StopEdit::Set {
                    index: 0,
                    position: 101,
                    color: Some(3)
                }
            )
            .is_err());
        assert!(draft
            .edit_wave_stop(
                "w",
                &keys(&[1]),
                StopEdit::Set {
                    index: 0,
                    position: 50,
                    color: Some(0x1000000)
                }
            )
            .is_err());
        assert!(draft
            .edit_wave_stop("w", &keys(&[1, 2]), StopEdit::Delete { index: 0 })
            .is_err());
        assert_eq!(draft.json(), before);
    }
    #[test]
    fn custom_stops_materialize_inherited_gradient_without_spectrum_defaults() {
        let mut value: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
        value["layers"][0]["params"]["colorStops"] =
            serde_json::json!([{"Stop":0,"Color":1},{"Stop":100,"Color":2}]);
        value["layers"][0]["regions"][0]["params"]
            .as_object_mut()
            .unwrap()
            .remove("colorStops");
        let mut draft = Draft::open(&value.to_string()).unwrap();
        draft
            .edit_wave_stop(
                "w",
                &keys(&[1, 3]),
                StopEdit::Set {
                    index: 0,
                    position: 0,
                    color: Some(0x123456),
                },
            )
            .unwrap();
        let SelectedGradient::Uniform(Some(stops)) =
            draft.wave_selection_gradient("w", &keys(&[1, 3])).unwrap()
        else {
            panic!()
        };
        assert_eq!(stops[0].color, Some(0x123456));
        let out: serde_json::Value = serde_json::from_str(draft.json()).unwrap();
        assert_eq!(out["layers"][0]["params"], value["layers"][0]["params"]);
        assert!(!draft.json().contains("duration"));
    }

    #[test]
    fn custom_stop_bounds_and_unverified_modes_refuse_atomically() {
        let mut draft = Draft::open(PROFILE).unwrap();
        for index in 0..5 {
            draft
                .edit_wave_stop(
                    "w",
                    &keys(&[1]),
                    StopEdit::Insert {
                        position: index * 10,
                        color: None,
                    },
                )
                .unwrap();
        }
        let before = draft.json().to_owned();
        assert!(draft
            .edit_wave_stop(
                "w",
                &keys(&[1]),
                StopEdit::Insert {
                    position: 90,
                    color: None
                }
            )
            .is_err());
        assert!(draft
            .edit_wave_stop(
                "w",
                &keys(&[1]),
                StopEdit::Set {
                    index: 7,
                    position: 50,
                    color: None
                }
            )
            .is_err());
        assert_eq!(draft.json(), before);
        let mut value: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
        value["layers"][0]["params"]["split"] = true.into();
        let before = value.to_string();
        let mut draft = Draft::open(&before).unwrap();
        assert!(draft
            .edit_wave_stop(
                "w",
                &keys(&[1]),
                StopEdit::Set {
                    index: 0,
                    position: 50,
                    color: None
                }
            )
            .is_err());
        assert_eq!(draft.json(), before);
        assert!(!draft.is_dirty());
    }
}
