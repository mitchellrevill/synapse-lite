//! Source action authoring only. No clipboard, process, browser or input APIs.
use super::*;

pub const TEXT_UNITS: usize = 250;
pub const ACTION_UNITS: usize = 32767;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Text(String),
    Command(String),
    Launch {
        website: bool,
        file: String,
        url: String,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Command,
    Program,
    Website,
}
impl Action {
    pub fn blank(kind: Kind) -> Self {
        match kind {
            Kind::Text => Self::Text(String::new()),
            Kind::Command => Self::Command(String::new()),
            Kind::Program | Kind::Website => Self::Launch {
                website: kind == Kind::Website,
                file: String::new(),
                url: String::new(),
            },
        }
    }
    pub fn kind(&self) -> Kind {
        match self {
            Self::Text(_) => Kind::Text,
            Self::Command(_) => Kind::Command,
            Self::Launch { website, .. } => {
                if *website {
                    Kind::Website
                } else {
                    Kind::Program
                }
            }
        }
    }
    fn event_type(&self) -> u8 {
        match self {
            Self::Text(_) => 4,
            Self::Command(_) => 5,
            Self::Launch { .. } => 10,
        }
    }
}
pub fn normalize_text(value: &str) -> String {
    value.replace("\r\n", "\n")
}
fn string(value: &str, limit: usize, multiline: bool, required: bool) -> Result<()> {
    if value.encode_utf16().count() > limit {
        return Err(invalid(format!("The value is too long (limit: {limit})")));
    }
    if value.contains('\0') {
        return Err(invalid("The value contains an unsupported character"));
    }
    if !multiline && (value.contains('\r') || value.contains('\n')) {
        return Err(invalid("Use one line for a command, program or website"));
    }
    if required && value.is_empty() {
        return Err(invalid("Enter an action value"));
    }
    Ok(())
}
fn fields(action: &Action, required: bool) -> Result<Vec<(&'static str, Value)>> {
    Ok(match action {
        Action::Text(text) => {
            let text = normalize_text(text);
            string(&text, TEXT_UNITS, true, required)?;
            vec![("Text", json!(text))]
        }
        Action::Command(command) => {
            string(command, ACTION_UNITS, false, required)?;
            vec![("TextCommand", json!(command))]
        }
        Action::Launch { website, file, url } => {
            string(file, ACTION_UNITS, false, required && !*website)?;
            string(url, ACTION_UNITS, false, required && *website)?;
            vec![
                ("RadioIndex", json!(u8::from(*website))),
                ("Content0", json!(file)),
                ("Content1", json!(url)),
            ]
        }
    })
}
/// Refuse retained values that native controls cannot represent losslessly.
/// Empty source placeholders can open, but Apply still requires an active value.
pub fn validate_display(action: &Action) -> Result<()> {
    fields(action, false).map(|_| ())
}
pub fn row(action: &Action) -> Result<Box<RawValue>> {
    let mut value = json!({"Type":action.event_type(),"selected":false});
    for (key, data) in fields(action, true)? {
        value[key] = data;
    }
    raw(&value)
}
pub fn from_row(row: &Value) -> Result<Option<Action>> {
    let read = |name: &str| {
        row[name]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| invalid("Retained action field is not text"))
    };
    Ok(match row["Type"].as_u64() {
        Some(4) => Some(Action::Text(read("Text")?)),
        Some(5) => Some(Action::Command(read("TextCommand")?)),
        Some(10) => {
            let website = match &row["RadioIndex"] {
                Value::Null => false,
                value if value == 0 => false,
                value if value == 1 => true,
                _ => return Err(invalid("This retained launch option is not supported")),
            };
            Some(Action::Launch {
                website,
                file: read("Content0")?,
                url: read("Content1")?,
            })
        }
        _ => None,
    })
}
pub(super) fn update(old: &RawValue, action: &Action) -> Result<Box<RawValue>> {
    let value: Value =
        serde_json::from_str(old.get()).map_err(|error| invalid(error.to_string()))?;
    if value["Type"] != action.event_type() {
        return Err(invalid(
            "Action kind cannot change while editing an existing row",
        ));
    }
    let replacements = fields(action, true)?;
    let mut data: Fields =
        serde_json::from_str(old.get()).map_err(|error| invalid(error.to_string()))?;
    for (key, value) in replacements {
        if let Some((_, old)) = data.0.iter_mut().find(|(name, _)| name == key) {
            *old = raw(&value)?;
        } else {
            if data.0.len() >= 128 {
                return Err(invalid("Action fields exceed editing bounds"));
            }
            data.0.push((key.into(), raw(&value)?));
        }
    }
    raw(&data)
}
pub fn label(row: &Value) -> Option<String> {
    let action = from_row(row).ok()??;
    let (name, value) = match &action {
        Action::Text(value) => ("Text", value),
        Action::Command(value) => ("Command", value),
        Action::Launch { website, file, url } => {
            if *website {
                ("Website", url)
            } else {
                ("Program", file)
            }
        }
    };
    Some(format!(
        "{name} · {}",
        value
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(64)
            .collect::<String>()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authoring_matches_independent_source_template_fixtures() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/macro-host-source.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let action = from_row(&case["row"]).unwrap().unwrap();
            let made: Value = serde_json::from_str(row(&action).unwrap().get()).unwrap();
            assert_eq!(made, case["row"]);
        }
    }
    #[test]
    fn text_limit_counts_utf16_and_normalizes_only_windows_line_endings() {
        assert!(row(&Action::Text("x".repeat(250))).is_ok());
        assert!(row(&Action::Text("x".repeat(251))).is_err());
        assert!(row(&Action::Text("😃".repeat(125))).is_ok());
        assert!(row(&Action::Text("😃".repeat(126))).is_err());
        let value: Value =
            serde_json::from_str(row(&Action::Text("a\r\nb\t😃".into())).unwrap().get()).unwrap();
        assert_eq!(value["Text"], "a\nb\t😃");
        assert!(row(&Action::Command("one\ntwo".into())).is_err());
        assert!(row(&Action::Text("nul\0text".into())).is_err());
        assert!(validate_display(&Action::Text(String::new())).is_ok());
        assert!(validate_display(&Action::Command("retained\0tail".into())).is_err());
        assert!(validate_display(&Action::Command("x".repeat(ACTION_UNITS + 1))).is_err());
    }
    #[test]
    fn launch_keeps_inactive_values_and_raw_metadata_without_execution() {
        let old=RawValue::from_string(r#"{"Type":10,"RadioIndex":0,"Content0":"C:\\Old.exe","Content1":"example.com","future":9.9900e+100,"phase":3,"Id":47}"#.into()).unwrap();
        let action = Action::Launch {
            website: true,
            file: "C:\\Old.exe".into(),
            url: "https://example.invalid/\" & echo literal".into(),
        };
        let changed = update(&old, &action).unwrap();
        assert!(changed.get().contains("9.9900e+100"));
        let value: Value = serde_json::from_str(changed.get()).unwrap();
        assert_eq!(value["RadioIndex"], 1);
        assert_eq!(value["phase"], 3);
        assert_eq!(value["Id"], 47);
        assert_eq!(from_row(&value).unwrap(), Some(action));
        assert!(update(&old, &Action::Command("do not execute".into())).is_err());
        assert!(row(&Action::Launch {
            website: false,
            file: String::new(),
            url: "retained".into()
        })
        .is_err());
    }
    #[test]
    fn draft_action_insert_update_and_cache_refusal_preserve_store() {
        let store = edit(
            &Store::default(),
            Change::Create {
                guid: "a".into(),
                name: "A".into(),
            },
        )
        .unwrap();
        let before = read(&store).unwrap().entries.remove(0);
        let inserted = edit_definition(
            &before.encoded_record,
            Change::InsertAction {
                guid: "a".into(),
                index: 1,
                action: Action::Text("Hello".into()),
            },
        )
        .unwrap();
        assert_eq!(read(&store).unwrap().entries[0].events.len(), 1);
        let changed = edit_definition(
            &inserted,
            Change::SetAction {
                guid: "a".into(),
                index: 1,
                action: Action::Text("Edited".into()),
            },
        )
        .unwrap();
        assert_eq!(describe(&changed).unwrap().events[1]["Text"], "Edited");
        let mut body = decode_record(&before.encoded_record).unwrap();
        body["appEngine"] = json!({"events":[]});
        let cached = record(&body, "A").unwrap();
        assert!(edit_definition(
            &cached,
            Change::InsertAction {
                guid: "a".into(),
                index: 1,
                action: Action::Command("never executed".into())
            }
        )
        .is_err());
        assert!(edit_definition(
            &before.encoded_record,
            Change::InsertAction {
                guid: "a".into(),
                index: 0,
                action: Action::Text("No".into())
            }
        )
        .is_err());
    }
    #[test]
    fn action_profile_export_reopens_without_changing_profile_or_foreign_rows() {
        let before = crate::profile_compat::import_document(include_bytes!(
            "../../../tests/fixtures/synthetic.synapse4"
        ))
        .unwrap()
        .store;
        let entry = read(&before).unwrap().entries.remove(0);
        let key = before.devices.keys().next().unwrap().clone();
        let original = entry.events.clone();
        let changed = edit(
            &before,
            Change::InsertAction {
                guid: entry.guid.clone(),
                index: original.len(),
                action: Action::Text("Line one\nUnicode 😃".into()),
            },
        )
        .unwrap();
        assert_eq!(
            before.devices[&key].profiles,
            changed.devices[&key].profiles
        );
        assert_eq!(before.devices[&key].active, changed.devices[&key].active);
        let old: Value = serde_json::from_slice(
            &crate::profile_compat::export_synapse4(&before, &key)
                .unwrap()
                .bytes,
        )
        .unwrap();
        let exported = crate::profile_compat::export_synapse4(&changed, &key).unwrap();
        let new: Value = serde_json::from_slice(&exported.bytes).unwrap();
        assert_eq!(old["profiles"], new["profiles"]);
        let body = decode_record(&new["macros"][0]).unwrap();
        assert_eq!(body["guid"], entry.guid);
        assert_eq!(body["macroEvents"][0], original[0]);
        let reopened = crate::profile_compat::import_document(&exported.bytes)
            .unwrap()
            .store;
        assert_eq!(
            read(&reopened).unwrap().entries[0].events[original.len()]["Text"],
            "Line one\nUnicode 😃"
        );
    }
}
