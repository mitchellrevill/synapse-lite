//! Literal projection of the recovered GUI converter, with project data bounds.
use super::{invalid, json, source_uint, Result, Value};

fn read<'a>(row: &'a Value, key: &str, limit: usize, multiline: bool) -> Result<&'a str> {
    row[key]
        .as_str()
        .filter(|s| crate::source_macro::host::text(s, limit, multiline))
        .ok_or_else(|| invalid(format!("Invalid host action field {key}")))
}

pub(super) fn convert(row: &Value) -> Result<Vec<Value>> {
    match source_uint(&row["Type"], 10) {
        Some(4) => {
            let text = read(row, "Text", 250, true)?;
            Ok(if text.is_empty() {
                vec![]
            } else {
                vec![
                    json!({"type":"clipboard","id":"text","text":text}),
                    json!({"type":"delay","ms":500}),
                ]
            })
        }
        Some(5) => {
            let text = read(row, "TextCommand", 32767, false)?;
            if text.is_empty() {
                return Ok(vec![]);
            }
            let path = if text.contains("taskmgr") {
                "cmd /c start taskmgr"
            } else {
                text
            };
            Ok(vec![
                json!({"type":"launch","path":path}),
                json!({"type":"delay","ms":50}),
            ])
        }
        Some(10) => {
            if row["RadioIndex"].is_null() {
                return Ok(vec![]);
            }
            let website = match source_uint(&row["RadioIndex"], 1) {
                Some(0) => false,
                Some(1) => true,
                _ => return Err(invalid("Invalid launch option")),
            };
            let value = read(
                row,
                if website { "Content1" } else { "Content0" },
                32767,
                false,
            )?;
            if value.is_empty() {
                return Ok(vec![]);
            }
            let (launch, delay) = if website {
                let url = if ["http://", "https://", "chrome://", "www."]
                    .iter()
                    .any(|s| value.contains(s))
                {
                    value.to_owned()
                } else {
                    format!("https://www.{value}")
                };
                (
                    json!({"type":"launch","path":format!("cmd /c start \"\" \"{url}\""),"startHidden":true}),
                    120,
                )
            } else {
                let exe = value
                    .rsplit('.')
                    .next()
                    .is_some_and(|s| s.eq_ignore_ascii_case("exe"));
                let path = if exe {
                    value.to_owned()
                } else {
                    format!("cmd /c \"{value}\"")
                };
                (
                    json!({"type":"launch","path":path,"startHidden":!exe,"pathToCheck":value}),
                    50,
                )
            };
            // Wrapping can exceed the native command-line bound even when the
            // original field fits. Refuse before returning a partial plan.
            if !crate::source_macro::host::text(launch["path"].as_str().unwrap(), 32767, false) {
                return Err(invalid(
                    "Converted launch exceeds native command-line bound",
                ));
            }
            Ok(vec![launch, json!({"type":"delay","ms":delay})])
        }
        _ => Err(invalid("Not a host action")),
    }
}
