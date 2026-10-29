//! Spectrum regional authoring. Imported values remain raw until explicitly edited.
use super::*;

pub const SPECTRUM_DURATIONS: [u32; 3] = [75480, 37740, 18870];
const DEFAULT_DURATION: u32 = SPECTRUM_DURATIONS[1];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GradientStop {
    #[serde(rename = "Stop")]
    pub position: u8,
    #[serde(rename = "Color", default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedDuration {
    Empty,
    Uniform(u32),
    Mixed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectedGradient {
    Empty,
    Uniform(Option<Vec<GradientStop>>),
    Mixed,
}
#[derive(Clone, Copy)]
pub enum StopEdit {
    Set {
        index: usize,
        position: u8,
        color: Option<u32>,
    },
    Insert {
        position: u8,
        color: Option<u32>,
    },
    Delete {
        index: usize,
    },
}

#[derive(Clone, Copy)]
pub(super) enum Edit<'a> {
    Duration(u32),
    Gradient {
        stops: &'a [GradientStop],
        custom: bool,
        normalize_duration: bool,
    },
    Stop {
        edit: StopEdit,
        normalize_duration: bool,
    },
}

pub(super) fn validate_layer(item: &Object) -> Result<()> {
    if item.text("name")? != "spectrum" || item.get("value").is_some_and(|value| value.get() != "4")
    {
        return Err(invalid("selected layer is not Studio Spectrum4"));
    }
    Ok(())
}
fn duration(params: &Object) -> u32 {
    params
        .get("duration")
        .and_then(|v| serde_json::from_str::<u32>(v.get()).ok())
        .filter(|v| SPECTRUM_DURATIONS.contains(v))
        .unwrap_or(DEFAULT_DURATION)
}
pub(super) fn validate_stop(stop: &GradientStop) -> Result<()> {
    if stop.position > 100 || stop.color.is_some_and(|c| c > 0xffffff) {
        return Err(invalid(
            "gradient stop requires 0..100 percent and RGB24 or None",
        ));
    }
    Ok(())
}
pub(super) fn gradient(params: &Object) -> Result<Option<Vec<GradientStop>>> {
    let Some(raw) = params.get("colorStops") else {
        return Ok(None);
    };
    // Import admission is separate from the UI's 2..10 authoring rule. Do not
    // truncate larger imported gradients or require endpoint anchors.
    let raw_stops = array(raw)?;
    if raw_stops.len() > 512 {
        return Err(invalid("excessive imported gradient"));
    }
    raw_stops
        .iter()
        .map(|raw| {
            let object = Object::parse(raw)?;
            let position = serde_json::from_str(
                object
                    .get("Stop")
                    .ok_or_else(|| invalid("missing Stop"))?
                    .get(),
            )
            .map_err(invalid)?;
            let color = object
                .get("Color")
                .map(|raw| serde_json::from_str::<u32>(raw.get()).map_err(invalid))
                .transpose()?;
            let stop = GradientStop { position, color };
            validate_stop(&stop)?;
            Ok(stop)
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

impl Edit<'_> {
    pub(super) fn apply(self, params: &mut Object, new: bool) -> Result<()> {
        match self {
            Self::Duration(value) => {
                params.set("duration", Some(&encode(&value)?));
            }
            Self::Gradient {
                stops,
                custom,
                normalize_duration,
            } => {
                let mut stops = stops.to_vec();
                stops.sort_by_key(|stop| stop.position); // stable, duplicates retained
                params.set("colorStops", Some(&encode(&stops)?));
                params.set("colorStopsCustom", Some(&encode(&custom)?));
                if new || normalize_duration {
                    params.set("duration", Some(&encode(&duration(params))?));
                }
            }
            Self::Stop {
                edit,
                normalize_duration,
            } => {
                edit_stop(params, edit)?;
                if new || normalize_duration {
                    params.set("duration", Some(&encode(&duration(params))?));
                }
            }
        }
        Ok(())
    }
}

// Shared source gradient mutation, without any effect-specific scalar defaults.
pub(super) fn edit_stop(params: &mut Object, edit: StopEdit) -> Result<()> {
    let mut stops = array(
        params
            .get("colorStops")
            .ok_or_else(|| invalid("choose a gradient first"))?,
    )?;
    match edit {
        StopEdit::Set {
            index,
            position,
            color,
        } => {
            let raw = stops
                .get_mut(index)
                .ok_or_else(|| invalid("unknown gradient stop"))?;
            let mut stop = Object::parse(raw)?;
            stop.set("Stop", Some(&encode(&position)?));
            let color = color.map(|c| encode(&c)).transpose()?;
            stop.set("Color", color.as_deref());
            *raw = encode(&stop)?;
        }
        StopEdit::Insert { position, color } => {
            if stops.len() >= 10 {
                return Err(invalid("gradient already has ten stops"));
            }
            stops.push(encode(&GradientStop { position, color })?);
        }
        StopEdit::Delete { index } => {
            if stops.len() <= 2 || index >= stops.len() {
                return Err(invalid("gradient requires at least two stops"));
            }
            stops.remove(index);
        }
    }
    // Keep untouched raw stop fields and numeric spellings. Sorting
    // moves complete records, never reconstructs their unknown fields.
    let mut keyed = stops
        .into_iter()
        .map(|raw| {
            let stop = Object::parse(&raw)?;
            let position: u8 = serde_json::from_str(
                stop.get("Stop")
                    .ok_or_else(|| invalid("missing Stop"))?
                    .get(),
            )
            .map_err(invalid)?;
            Ok((position, raw))
        })
        .collect::<Result<Vec<_>>>()?;
    keyed.sort_by_key(|(position, _)| *position);
    let stops: Vec<_> = keyed.into_iter().map(|(_, raw)| raw).collect();
    params.set("colorStops", Some(&encode(&stops)?));
    params.set("colorStopsCustom", Some(&encode(&true)?));

    Ok(())
}

impl Draft {
    fn spectrum_params(&self, id: &str, selection: &[LedSelection]) -> Result<Vec<Object>> {
        let ordered_selection = selection;
        let selection = selected(selection)?;
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        if selection.is_empty() {
            return Ok(vec![]);
        }
        inventory(self, &selection)?;
        let mut seen = BTreeMap::new();
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
                        if seen.insert(key, result.len()).is_some() {
                            return Err(invalid("overlapping selected region memberships"));
                        }
                        matched = true;
                    }
                }
            }
            if matched {
                result.push(Object::parse(
                    region
                        .get("params")
                        .ok_or_else(|| invalid("region has no params"))?,
                )?);
            }
        }
        if seen.len() < selection.len() {
            result.push(Object::parse(&encode(
                &serde_json::json!({"colorStopsCustom":false,"duration":DEFAULT_DURATION}),
            )?)?);
        }
        // Source ze visits selected devices/LEDs in selection order and projects
        // the first encountered region (or unassigned baseline). It does not
        // choose the first region in the file's layer array.
        let unassigned = result.len().saturating_sub(1);
        let mut result: Vec<_> = result.into_iter().map(Some).collect();
        let mut ordered = Vec::new();
        for key in ordered_selection {
            let index = seen.get(key).copied().unwrap_or(unassigned);
            if let Some(params) = result[index].take() {
                ordered.push(params);
            }
        }
        Ok(ordered)
    }

    fn spectrum_projection(&self, id: &str, selection: &[LedSelection]) -> Result<Object> {
        let params = self.spectrum_params(id, selection)?;
        let mut projection = Object(vec![]);
        if let Some(first) = params.first() {
            for field in ["colorStops", "colorStopsCustom"] {
                projection.set(field, first.get(field));
            }
            projection.set("duration", Some(&encode(&duration(first))?));
        }
        Ok(projection)
    }

    pub fn spectrum_selection_duration(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<SelectedDuration> {
        let params = self.spectrum_params(id, selection)?;
        let Some(first) = params.first() else {
            return Ok(SelectedDuration::Empty);
        };
        let value = duration(first);
        Ok(if params.iter().all(|p| duration(p) == value) {
            SelectedDuration::Uniform(value)
        } else {
            SelectedDuration::Mixed
        })
    }
    pub fn spectrum_selection_gradient(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<SelectedGradient> {
        let params = self.spectrum_params(id, selection)?;
        let Some(first) = params.first() else {
            return Ok(SelectedGradient::Empty);
        };
        let value = gradient(first)?;
        for params in params.iter().skip(1) {
            if gradient(params)? != value {
                return Ok(SelectedGradient::Mixed);
            }
        }
        Ok(SelectedGradient::Uniform(value))
    }
    pub fn set_spectrum_duration(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        milliseconds: u32,
    ) -> Result<bool> {
        if !SPECTRUM_DURATIONS.contains(&milliseconds) {
            return Err(invalid("choose a source Spectrum duration"));
        }
        let projection = self.spectrum_projection(id, selection)?; // no decoding of untouched gradient
        self.edit_regions(
            id,
            selection,
            RegionEdit::Spectrum(Edit::Duration(milliseconds), &projection),
        )
    }
    pub fn set_spectrum_gradient(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        stops: &[GradientStop],
        custom: bool,
    ) -> Result<bool> {
        if !(2..=10).contains(&stops.len()) {
            return Err(invalid("new gradient requires two to ten stops"));
        }
        for stop in stops {
            validate_stop(stop)?;
        }
        let normalize_duration =
            self.spectrum_selection_duration(id, selection)? != SelectedDuration::Mixed;
        let projection = self.spectrum_projection(id, selection)?;
        self.edit_regions(
            id,
            selection,
            RegionEdit::Spectrum(
                Edit::Gradient {
                    stops,
                    custom,
                    normalize_duration,
                },
                &projection,
            ),
        )
    }
    pub fn edit_spectrum_stop(
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
                validate_stop(&GradientStop { position, color })?
            }
            StopEdit::Delete { .. } => {}
        }
        if !matches!(
            self.spectrum_selection_gradient(id, selection)?,
            SelectedGradient::Uniform(Some(_))
        ) {
            return Err(invalid("choose one uniform gradient before editing stops"));
        }
        let normalize_duration =
            self.spectrum_selection_duration(id, selection)? != SelectedDuration::Mixed;
        let projection = self.spectrum_projection(id, selection)?;
        self.edit_regions(
            id,
            selection,
            RegionEdit::Spectrum(
                Edit::Stop {
                    edit,
                    normalize_duration,
                },
                &projection,
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = r#"{"id":"p","title":"Spectrum","devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2},{"row":0,"col":3},{"row":0,"col":4}]}],"layers":[{"id":"s","type":"layer","name":"spectrum","value":4,"title":"Spectrum","params":{"opaque":1.2300e-7},"regions":[{"params":{"duration":1111,"future":9007199254740993123456789,"colorStops":[{"Stop":20,"Color":1,"stopFuture":9.9900e+100},{"Stop":80,"Color":2}]},"devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2}]}]},{"params":{"duration":18870,"colorStopsCustom":true,"colorStops":[{"Stop":0,"Color":3},{"Stop":100}]},"devices":[{"id":"k","leds":[{"row":0,"col":3}]}]}]}]}"#;
    fn keys(cols: &[u16]) -> Vec<LedSelection> {
        cols.iter()
            .map(|&col| LedSelection {
                device: "k".into(),
                row: 0,
                col,
            })
            .collect()
    }
    fn params(draft: &Draft, col: u16) -> Object {
        draft.spectrum_params("s", &keys(&[col])).unwrap().remove(0)
    }
    #[test]
    fn duration_only_preserves_raw_gradients_and_missing_custom_but_seeds_unassigned_custom_false()
    {
        let mut draft = Draft::open(PROFILE).unwrap();
        let before = params(&draft, 1)
            .get("colorStops")
            .unwrap()
            .get()
            .to_owned();
        draft
            .set_spectrum_duration("s", &keys(&[4, 1, 3]), 75480)
            .unwrap();
        assert_eq!(params(&draft, 1).get("colorStops").unwrap().get(), before);
        assert!(params(&draft, 1).get("colorStopsCustom").is_none());
        assert_eq!(
            params(&draft, 4).get("colorStopsCustom").unwrap().get(),
            "false"
        );
        assert!(params(&draft, 4).get("colorStops").is_none());
        assert_eq!(
            draft
                .spectrum_selection_duration("s", &keys(&[1, 3, 4]))
                .unwrap(),
            SelectedDuration::Uniform(75480)
        );
        assert_eq!(params(&draft, 2).get("duration").unwrap().get(), "1111");
        draft.undo().unwrap();
        assert!(!draft.is_dirty());
        draft.redo().unwrap();
        assert!(draft.is_dirty());
    }
    #[test]
    fn gradient_patches_preserve_mixed_durations_normalize_uniform_projection_and_keep_unknown_params(
    ) {
        let mut draft = Draft::open(PROFILE).unwrap();
        let stops = [
            GradientStop {
                position: 80,
                color: None,
            },
            GradientStop {
                position: 20,
                color: Some(0),
            },
            GradientStop {
                position: 20,
                color: Some(255),
            },
        ];
        draft
            .set_spectrum_gradient("s", &keys(&[1, 3, 4]), &stops, false)
            .unwrap();
        assert_eq!(params(&draft, 1).get("duration").unwrap().get(), "1111");
        assert_eq!(params(&draft, 3).get("duration").unwrap().get(), "18870");
        assert_eq!(params(&draft, 4).get("duration").unwrap().get(), "37740");
        assert_eq!(
            params(&draft, 1).get("future").unwrap().get(),
            "9007199254740993123456789"
        );
        let expected = vec![stops[1].clone(), stops[2].clone(), stops[0].clone()];
        assert_eq!(
            draft
                .spectrum_selection_gradient("s", &keys(&[1, 3, 4]))
                .unwrap(),
            SelectedGradient::Uniform(Some(expected))
        );
        draft.undo().unwrap();
        assert!(!draft.is_dirty());
        draft
            .set_spectrum_gradient("s", &keys(&[1]), &stops, true)
            .unwrap();
        assert_eq!(params(&draft, 1).get("duration").unwrap().get(), "37740");
        assert_eq!(
            params(&draft, 1).get("colorStopsCustom").unwrap().get(),
            "true"
        );
        assert!(draft.json().contains(r#""params":{"opaque":1.2300e-7}"#));
    }
    #[test]
    fn stop_edit_none_duplicate_positions_and_endpoint_deletion_preserve_other_raw_fields() {
        let mut draft = Draft::open(PROFILE).unwrap();
        draft
            .edit_spectrum_stop(
                "s",
                &keys(&[1]),
                StopEdit::Insert {
                    position: 20,
                    color: Some(4),
                },
            )
            .unwrap();
        draft
            .edit_spectrum_stop(
                "s",
                &keys(&[1]),
                StopEdit::Set {
                    index: 0,
                    position: 5,
                    color: None,
                },
            )
            .unwrap();
        let p = params(&draft, 1);
        let raw = p.get("colorStops").unwrap().get();
        assert!(raw.contains("9.9900e+100"));
        assert!(raw.contains(r#""Stop":5,"stopFuture"#));
        assert!(!raw.contains("null"));
        draft
            .edit_spectrum_stop("s", &keys(&[1]), StopEdit::Delete { index: 0 })
            .unwrap();
        let before = draft.json().to_owned();
        assert!(draft
            .edit_spectrum_stop("s", &keys(&[1]), StopEdit::Delete { index: 0 })
            .is_err());
        assert_eq!(draft.json(), before);
        assert_eq!(
            draft.spectrum_selection_gradient("s", &keys(&[1])).unwrap(),
            SelectedGradient::Uniform(Some(vec![
                GradientStop {
                    position: 20,
                    color: Some(4)
                },
                GradientStop {
                    position: 80,
                    color: Some(2)
                }
            ]))
        );
    }
    #[test]
    fn invalid_authored_values_mixed_stop_edits_and_wrong_effect_are_atomic() {
        let mut draft = Draft::open(PROFILE).unwrap();
        assert!(draft.set_spectrum_duration("s", &keys(&[1]), 1).is_err());
        assert!(draft
            .edit_spectrum_stop("s", &keys(&[1, 3]), StopEdit::Delete { index: 0 })
            .is_err());
        assert!(draft
            .set_spectrum_gradient(
                "s",
                &keys(&[1]),
                &[GradientStop {
                    position: 0,
                    color: None
                }],
                true
            )
            .is_err());
        assert!(draft
            .edit_spectrum_stop(
                "s",
                &keys(&[1]),
                StopEdit::Set {
                    index: 0,
                    position: 101,
                    color: None
                }
            )
            .is_err());
        assert_eq!(draft.json(), PROFILE);
        assert_eq!(draft.undo_len(), 0);
        let raw = PROFILE.replace("\"value\":4", "\"value\":8");
        let mut draft = Draft::open(&raw).unwrap();
        assert!(draft
            .set_spectrum_duration("s", &keys(&[1]), 18870)
            .is_err());
        assert_eq!(draft.json(), raw);
    }
    #[test]
    fn unsupported_imported_gradient_is_retained_during_duration_edit() {
        let raw = PROFILE.replace(r#""Color":1"#, r#""Color":null"#);
        let mut draft = Draft::open(&raw).unwrap();
        assert!(draft.spectrum_selection_gradient("s", &keys(&[1])).is_err());
        draft
            .set_spectrum_duration("s", &keys(&[1]), 18870)
            .unwrap();
        assert!(draft.json().contains(r#""Color":null"#));
        draft.undo().unwrap();
        assert!(!draft.is_dirty());
    }

    #[test]
    fn imported_twelve_stops_are_not_truncated_and_authoring_limits_are_separate() {
        let mut draft = Draft::open(PROFILE).unwrap();
        let twelve: Vec<_> = (0..12)
            .map(|i| GradientStop {
                position: i * 8,
                color: Some(u32::from(i)),
            })
            .collect();
        // Manufacture an imported twelve-stop record without using the
        // deliberately narrower two-to-ten-stop replacement authoring API.
        let raw = PROFILE.replace(
            r#"[{"Stop":20,"Color":1,"stopFuture":9.9900e+100},{"Stop":80,"Color":2}]"#,
            &serde_json::to_string(&twelve).unwrap(),
        );
        assert!(draft
            .set_spectrum_gradient("s", &keys(&[1]), &twelve, true)
            .is_err());
        assert_eq!(draft.json(), PROFILE);
        let mut draft = Draft::open(&raw).unwrap();
        assert_eq!(
            draft.spectrum_selection_gradient("s", &keys(&[1])).unwrap(),
            SelectedGradient::Uniform(Some(twelve.clone()))
        );
        draft
            .set_spectrum_duration("s", &keys(&[1]), 75480)
            .unwrap();
        assert_eq!(
            draft.spectrum_selection_gradient("s", &keys(&[1])).unwrap(),
            SelectedGradient::Uniform(Some(twelve))
        );
        let before = draft.json().to_owned();
        assert!(draft
            .edit_spectrum_stop(
                "s",
                &keys(&[1]),
                StopEdit::Insert {
                    position: 50,
                    color: None
                }
            )
            .is_err());
        assert_eq!(draft.json(), before);
        draft
            .edit_spectrum_stop("s", &keys(&[1]), StopEdit::Delete { index: 0 })
            .unwrap();
        assert!(
            matches!(draft.spectrum_selection_gradient("s",&keys(&[1])).unwrap(),SelectedGradient::Uniform(Some(stops)) if stops.len()==11)
        );
    }

    #[test]
    fn mixed_unassigned_leds_inherit_first_selected_projection_instead_of_file_region_order() {
        let mut draft = Draft::open(PROFILE).unwrap();
        let original = params(&draft, 3)
            .get("colorStops")
            .unwrap()
            .get()
            .to_owned();
        draft
            .set_spectrum_duration("s", &keys(&[3, 4, 1]), 75480)
            .unwrap();
        assert_eq!(params(&draft, 4).get("colorStops").unwrap().get(), original);
        assert_eq!(
            params(&draft, 4).get("colorStopsCustom").unwrap().get(),
            "true"
        );
        draft.undo().unwrap();
        let stops = [
            GradientStop {
                position: 20,
                color: None,
            },
            GradientStop {
                position: 80,
                color: Some(0),
            },
        ];
        draft
            .set_spectrum_gradient("s", &keys(&[3, 4]), &stops, true)
            .unwrap();
        assert_eq!(params(&draft, 4).get("duration").unwrap().get(), "18870");
        draft.undo().unwrap();
        draft
            .set_spectrum_gradient("s", &keys(&[4, 3]), &stops, true)
            .unwrap();
        assert_eq!(params(&draft, 4).get("duration").unwrap().get(), "37740");
    }
}
