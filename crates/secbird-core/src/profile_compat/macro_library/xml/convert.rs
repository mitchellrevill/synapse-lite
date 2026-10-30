//! Typed projection of the recovered v4 XML import constructor.
use super::*;

fn text(node: &Node, name: &str) -> Result<String> {
    Ok(node.one(name)?.scalar()?.to_owned())
}
fn optional_text(node: &Node, name: &str) -> Result<String> {
    node.optional(name)?
        .map(Node::scalar)
        .transpose()
        .map(|s| s.unwrap_or("").to_owned())
}
fn integer(node: &Node, name: &str, max: u64) -> Result<u64> {
    let value = text(node, name)?;
    let n = value
        .parse::<u64>()
        .map_err(|_| invalid(format!("Invalid XML integer {name}")))?;
    if n > max {
        return Err(invalid(format!("XML integer {name} exceeds its range")));
    }
    Ok(n)
}
fn finite(node: &Node, name: &str) -> Result<f64> {
    let n = text(node, name)?
        .parse::<f64>()
        .map_err(|_| invalid(format!("Invalid XML number {name}")))?;
    if !n.is_finite() {
        return Err(invalid("Nonfinite XML number"));
    }
    Ok(n)
}
fn state(node: &Node, name: &str, max: u64) -> Result<Value> {
    match node.optional(name)? {
        None => Ok(Value::Null),
        Some(n) if n.scalar()?.is_empty() => Ok(Value::Null),
        Some(_) => integer(node, name, max).map(|v| json!(v)),
    }
}
fn delay(seconds: f64) -> Result<String> {
    if !seconds.is_finite() || !(0.0..=3600.0).contains(&seconds) {
        return Err(invalid("XML delay must be within one hour"));
    }
    // Exact positive binary64 scaling implements source Number.toFixed(3).
    // Multiplying a float by1000 first would incorrectly round near ties.
    let bits = seconds.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let significand = (bits & ((1u64 << 52) - 1)) | if exponent == 0 { 0 } else { 1u64 << 52 };
    let shift = if exponent == 0 { 1074 } else { 1075 - exponent } as u32;
    let scaled = u128::from(significand) * 1000;
    let rounded = if shift >= 128 {
        0
    } else {
        (scaled + (1u128 << (shift - 1))) >> shift
    };
    Ok(format!("{}.{:03}", rounded / 1000, rounded % 1000))
}

pub(super) fn definition(root: &Node) -> Result<(String, Value)> {
    root.fields(&[
        "Name",
        "Guid",
        "Version",
        "DelaySetting",
        "MouseMoveType",
        "MacroEvents",
    ])?;
    if text(root, "Version")? != "4" {
        return Err(invalid(
            "Only standalone Synapse4 Macro XML version4 is supported",
        ));
    }
    let title = text(root, "Name")?;
    name(&title)?;
    let id = text(root, "Guid")?;
    if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
        return Err(invalid("Invalid XML macro GUID"));
    }
    let setting = integer(root, "DelaySetting", u32::MAX as u64)?;
    let movement = if root.optional("MouseMoveType")?.is_some() {
        text(root, "MouseMoveType")?
    } else {
        "none".into()
    };
    if !["none", "absolute", "foreground", "relative"].contains(&movement.as_str()) {
        return Err(invalid("Unknown XML mouse movement mode"));
    }
    let events = root.one("MacroEvents")?;
    if !events.text.trim().is_empty()
        || events.children.iter().any(|n| n.name != "MacroEvent")
        || events.children.len() > MAX_EVENTS
    {
        return Err(invalid("Invalid/bounded XML event list"));
    }
    let mut rows = vec![];
    for event in &events.children {
        if event.optional("Delay")?.is_some() {
            return Err(invalid(
                "Inline legacy XML delays remain unsupported; no macro was imported",
            ));
        }
        rows.push(row(event)?);
        if rows.len() > MAX_EVENTS {
            return Err(invalid("Macro exceeds512 event rows"));
        }
    }
    // Source assigns editor pair identities after conversion. Preserve its
    // v4 ordering rule rather than converting key codes into HID usages.
    let mut next = 0;
    for i in 0..rows.len() {
        let group = match rows[i]["Type"].as_u64() {
            Some(1) => "KeyEvent",
            Some(2) => "MouseEvent",
            Some(3) => {
                rows[i]["Id"] = json!(next);
                next += 1;
                continue;
            }
            _ => continue,
        };
        if rows[i][group]["State"] != 0 {
            continue;
        }
        rows[i]["Id"] = json!(next);
        let kind = rows[i]["Type"].clone();
        for row in rows.iter_mut().skip(i) {
            let release = if group == "KeyEvent" {
                row[group]["State"].as_u64().is_some_and(|v| v != 0)
            } else {
                row[group]["State"] == 1
            };
            if row["Type"] == kind && release {
                row["Id"] = json!(next);
                break;
            }
        }
        next += 1;
    }
    Ok((
        title.clone(),
        json!({"name":title,"guid":id,"macroEvents":rows,"delaySetting":setting,
        "actionLoad":{"previousAction":[],"nextAction":[]},"deviceList":[],"appEngine":{"mouseMoveType":movement}}),
    ))
}

fn row(event: &Node) -> Result<Value> {
    let kind = text(event, "Type")?;
    let mut fields = vec![
        "Type",
        "Id",
        "selected",
        "isPairing",
        "Delay",
        "phase",
        "ScanCode",
        "flag",
    ];
    fields.extend_from_slice(match kind.as_str() {
        "actionBar" => &["recordProfile"][..],
        "0" => &["Number"],
        "1" => &["KeyEvent"],
        "2" | "3" => &["MouseEvent", "Number"],
        "4" => &["Text"],
        "5" => &["TextCommand"],
        "6" => &["Number", "LoopEvent"],
        "7" => &["MPIndex", "guid"],
        "10" => &["RadioIndex", "Content0", "Content1"],
        _ => &[],
    });
    event.fields(&fields)?;
    if event.optional("phase")?.is_some() {
        return Err(invalid("Phased macro XML conversion remains unsupported"));
    }
    match kind.as_str() {
        "actionBar" => {
            let mut profile = json!({});
            if let Some(record) = event.optional("recordProfile")? {
                record.fields(&["mmtSetting"])?;
                if record.optional("mmtSetting")?.is_some() {
                    let n = integer(record, "mmtSetting", 3)?;
                    if n > 0 {
                        profile = json!({"mmtSetting":n});
                    }
                }
            }
            Ok(json!({"Type":"actionBar","recordProfile":profile,"selected":false}))
        }
        "0" => Ok(json!({"Type":0,"Number":delay(finite(event,"Number")?)?,"selected":false})),
        "1" => {
            let key = event.one("KeyEvent")?;
            key.fields(&["Makecode", "State", "Type"])?;
            Ok(
                json!({"Type":1,"Id":null,"KeyEvent":{"Makecode":integer(key,"Makecode",u16::MAX as u64)?,"State":state(key,"State",5)?},"selected":false,"isPairing":false}),
            )
        }
        "2" => {
            let mouse = event.one("MouseEvent")?;
            mouse.fields(&["MouseButton", "State"])?;
            Ok(
                json!({"Type":2,"Id":null,"MouseEvent":{"MouseButton":integer(mouse,"MouseButton",9)?,"State":state(mouse,"State",1)?},"selected":false,"isPairing":false}),
            )
        }
        "3" => {
            let mouse = event.one("MouseEvent")?;
            if !mouse.text.trim().is_empty()
                || mouse.children.is_empty()
                || mouse.children.iter().any(|n| n.name != "Buffer")
            {
                return Err(invalid("Unsupported XML movement buffer"));
            }
            let mut buffer = vec![];
            let mut sum = 0.0;
            for point in &mouse.children {
                point.fields(&["time", "x", "y"])?;
                let time = finite(point, "time")?;
                if time < 0.0 {
                    return Err(invalid("Negative XML movement delay"));
                }
                sum += time;
                buffer.push(json!({"time":time,"x":finite(point,"x")?,"y":finite(point,"y")?}));
            }
            Ok(
                json!({"Type":3,"Id":null,"Number":delay(sum/1000.)?,"MouseEvent":{"Buffer":buffer},"selected":false}),
            )
        }
        "4" => Ok(json!({"Type":4,"Text":optional_text(event,"Text")?,"selected":false})),
        "5" => {
            Ok(json!({"Type":5,"TextCommand":optional_text(event,"TextCommand")?,"selected":false}))
        }
        "6" => {
            let item = event.one("LoopEvent")?;
            item.fields(&["State"])?;
            Ok(
                json!({"Type":6,"Id":null,"Number":integer(event,"Number",u32::MAX as u64)?,"LoopEvent":{"State":integer(item,"State",1)?},"selected":false,"isPairing":false}),
            )
        }
        "7" => Ok(
            json!({"Type":7,"MPIndex":text(event,"MPIndex")?,"guid":text(event,"guid")?,"selected":false}),
        ),
        "10" => Ok(
            json!({"Type":10,"RadioIndex":integer(event,"RadioIndex",1)?,"Content0":text(event,"Content0")?,"Content1":text(event,"Content1")?,"selected":false}),
        ),
        _ => Err(invalid(format!("Unsupported XML macro event type {kind}"))),
    }
}
