//! Native host-event data only; decoding does not execute imported content.
use super::{fields, invalid, Event, PlanError};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostAction {
    Launch {
        /// Complete native command line. Do not split/requote as argv.
        path: String,
        start_hidden: bool,
        path_to_check: Option<String>,
    },
    ClipboardText(String),
}

pub(crate) fn text(value: &str, limit: usize, multiline: bool) -> bool {
    value.encode_utf16().count() <= limit
        && !value.contains('\0')
        && (multiline || (!value.contains('\r') && !value.contains('\n')))
}

pub(super) fn decode(index: usize, object: &Map<String, Value>) -> Result<Event, PlanError> {
    let at = Some(index);
    let read = |key: &str, limit, multiline| {
        object
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| text(s, limit, multiline))
            .map(str::to_owned)
            .ok_or_else(|| invalid(at, key))
    };
    let action = if object["type"] == "clipboard" {
        fields(at, object, &["type", "id", "text"])?;
        if object.get("id").and_then(Value::as_str) != Some("text") {
            return Err(invalid(at, "id"));
        }
        HostAction::ClipboardText(read("text", 250, true)?)
    } else {
        fields(at, object, &["type", "path", "startHidden", "pathToCheck"])?;
        let path = read("path", 32767, false)?;
        if path.is_empty() {
            return Err(invalid(at, "path"));
        }
        let start_hidden = object
            .get("startHidden")
            .map(|value| value.as_bool().ok_or_else(|| invalid(at, "startHidden")))
            .transpose()?
            .unwrap_or(false);
        let path_to_check = object
            .get("pathToCheck")
            .map(|_| read("pathToCheck", 32767, false))
            .transpose()?;
        HostAction::Launch {
            path,
            start_hidden,
            path_to_check,
        }
    };
    Ok(Event::Host(Box::new(action)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_macro::{EventLocation, Limits, PlaybackMetadata, SourceJson};
    use serde_json::json;

    fn plan(value: Value) -> Result<super::super::SymbolicPlan, PlanError> {
        let binding = json!({"guid":"host","macroPlaybackOption":"Once"});
        SourceJson::parse(&json!([value]).to_string(), Limits::default())?.plan(
            EventLocation::RootArray,
            &PlaybackMetadata::parse(&binding)?,
        )
    }
    #[test]
    fn host_native_fields_have_explicit_defaults_and_no_input_conversion() {
        let p = plan(json!({"type":"launch","path":"literal command"})).unwrap();
        assert_eq!(
            p.events(),
            &[Event::Host(Box::new(HostAction::Launch {
                path: "literal command".into(),
                start_hidden: false,
                path_to_check: None
            }))]
        );
        let p = plan(json!({"type":"launch","path":"literal command","startHidden":true,"pathToCheck":"C:/Path.exe"})).unwrap();
        assert!(
            matches!(&p.events()[0], Event::Host(a) if matches!(a.as_ref(), HostAction::Launch { start_hidden:true, path_to_check:Some(_), .. }))
        );
    }
    #[test]
    fn host_native_malformed_fields_and_unknown_options_refuse() {
        for value in [
            json!({"type":"launch","path":""}),
            json!({"type":"launch","path":"command","startHidden":0}),
            json!({"type":"launch","path":"command","pathToCheck":null}),
            json!({"type":"launch","path":"command","elevated":true}),
            json!({"type":"clipboard","id":"file","text":"x"}),
            json!({"type":"clipboard","id":"text","text":"x\0y"}),
            json!({"type":"clipboard","id":"text","text":"😀".repeat(126)}),
        ] {
            assert!(plan(value).is_err());
        }
    }
    #[test]
    fn host_native_text_keeps_unicode_and_line_endings_exact() {
        let text = "a\r\n😀\n\t雪";
        assert_eq!(
            plan(json!({"type":"clipboard","id":"text","text":text}))
                .unwrap()
                .events(),
            &[Event::Host(Box::new(HostAction::ClipboardText(
                text.into()
            )))]
        );
    }
}
