//! Standalone Synapse4 Macro XML files. Pure conversion; no file or device I/O.
use super::*;
use quick_xml::{events::Event as XmlEvent, Reader};

mod convert;
#[cfg(test)]
mod tests;

pub const MAX_BYTES: usize = 512 * 1024;
const MAX_NODES: usize = 16_384;
const MAX_DEPTH: usize = 32;
const ORIGINAL: &str = "_synapseLiteStandaloneMacroXml";

#[derive(Default)]
struct Node {
    name: String,
    text: String,
    children: Vec<Node>,
}
impl Node {
    fn one(&self, name: &str) -> Result<&Node> {
        let mut found = self.children.iter().filter(|n| n.name == name);
        let item = found
            .next()
            .ok_or_else(|| invalid(format!("Missing XML field {name}")))?;
        if found.next().is_some() {
            return Err(invalid(format!("Duplicate XML field {name}")));
        }
        Ok(item)
    }
    fn optional(&self, name: &str) -> Result<Option<&Node>> {
        if self.children.iter().any(|n| n.name == name) {
            self.one(name).map(Some)
        } else {
            Ok(None)
        }
    }
    fn scalar(&self) -> Result<&str> {
        if !self.children.is_empty() {
            return Err(invalid("Expected a scalar XML value"));
        }
        Ok(&self.text)
    }
    fn fields(&self, names: &[&str]) -> Result<()> {
        if !self.text.trim().is_empty() {
            return Err(invalid("Mixed XML content is unsupported"));
        }
        for child in &self.children {
            if !names.contains(&child.name.as_str()) {
                return Err(invalid(format!("Unsupported XML field {}", child.name)));
            }
            self.one(&child.name)?;
        }
        Ok(())
    }
}

fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r')
        || ('\u{20}'..='\u{d7ff}').contains(&c)
        || ('\u{e000}'..='\u{fffd}').contains(&c)
        || c >= '\u{10000}'
}
fn xml_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}
fn parse(bytes: &[u8]) -> Result<Node> {
    if bytes.len() > MAX_BYTES {
        return Err(invalid("Macro XML exceeds512KiB"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid("Macro XML must be UTF-8"))?
        .trim_start_matches('\u{feff}');
    if text.chars().any(|c| !xml_char(c)) {
        return Err(invalid("Invalid XML character"));
    }
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<Node> = vec![];
    let mut root = None;
    let mut nodes = 0;
    let mut declaration = false;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| invalid(format!("Invalid macro XML: {e}")))?;
        match event {
            XmlEvent::Start(ref e) | XmlEvent::Empty(ref e) => {
                if e.attributes().next().is_some() {
                    return Err(invalid("Macro XML attributes/namespaces are unsupported"));
                }
                let name = e.name().as_ref().to_owned();
                if !xml_name(&name) || stack.len() >= MAX_DEPTH || nodes >= MAX_NODES {
                    return Err(invalid("Macro XML name/depth/node limit"));
                }
                nodes += 1;
                let node = Node {
                    name,
                    ..Node::default()
                };
                if matches!(event, XmlEvent::Start(_)) {
                    stack.push(node);
                } else {
                    attach(&mut stack, &mut root, node)?;
                }
            }
            XmlEvent::End(_) => {
                let node = stack.pop().ok_or_else(|| invalid("Unbalanced XML"))?;
                attach(&mut stack, &mut root, node)?;
            }
            XmlEvent::Text(e) => append(&mut stack, e.xml10_content().as_ref())?,
            XmlEvent::CData(e) => append(&mut stack, e.xml10_content().as_ref())?,
            XmlEvent::GeneralRef(e) => {
                let entity = e.as_ref();
                let escaped = format!("&{entity};");
                let decoded = quick_xml::escape::unescape(&escaped)
                    .map_err(|_| invalid("Unknown XML entity"))?;
                if decoded.chars().any(|c| !xml_char(c)) {
                    return Err(invalid("Invalid XML character reference"));
                }
                append(&mut stack, &decoded)?;
            }
            XmlEvent::Decl(e) => {
                if declaration || root.is_some() || !stack.is_empty() {
                    return Err(invalid("Unexpected XML declaration"));
                }
                declaration = true;
                if e.version()
                    .map_err(|_| invalid("Invalid XML declaration"))?
                    .as_ref()
                    != "1.0"
                {
                    return Err(invalid("Only XML1.0 is supported"));
                }
                if let Some(encoding) = e.encoding() {
                    if !encoding
                        .map_err(|_| invalid("Invalid XML encoding"))?
                        .eq_ignore_ascii_case("utf-8")
                    {
                        return Err(invalid("Macro XML must declare UTF-8"));
                    }
                }
            }
            XmlEvent::Comment(_) => {}
            XmlEvent::Eof => break,
            _ => {
                return Err(invalid(
                    "Macro XML DTDs, processing instructions and external entities are unsupported",
                ))
            }
        }
    }
    if !stack.is_empty() {
        return Err(invalid("Incomplete macro XML"));
    }
    root.filter(|r: &Node| r.name == "Macro")
        .ok_or_else(|| invalid("Choose a standalone Macro XML file"))
}
fn attach(stack: &mut [Node], root: &mut Option<Node>, node: Node) -> Result<()> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else if root.replace(node).is_some() {
        return Err(invalid("Multiple XML roots"));
    }
    Ok(())
}
fn append(stack: &mut [Node], text: &str) -> Result<()> {
    if let Some(node) = stack.last_mut() {
        node.text.push_str(text);
    } else if !text.trim().is_empty() {
        return Err(invalid("Text outside XML root"));
    }
    Ok(())
}

/// Decode the stock v4 standalone format. Unsupported fields/types refuse
/// atomically; original bytes are retained as provenance outside the body hash.
pub fn import(bytes: &[u8]) -> Result<Value> {
    let root = parse(bytes)?;
    let (title, body) = convert::definition(&root)?;
    let mut encoded = record(&body, &title)?;
    encoded[ORIGINAL] = json!({"version":1,"bytes":STANDARD.encode(bytes)});
    describe(&encoded)?;
    Ok(encoded)
}

/// Export the source's v4 Macro envelope. Source record/profile bytes remain
/// unchanged. Native caches and editor-only root state are not XML fields.
pub fn export(encoded: &Value) -> Result<Vec<u8>> {
    let entry = describe(encoded)?;
    let body = decode_record(encoded)?;
    if let Some(original) = encoded.get(ORIGINAL) {
        if original["version"] != 1 {
            return Err(invalid("Unknown standalone XML provenance version"));
        }
        let raw = original["bytes"]
            .as_str()
            .ok_or_else(|| invalid("Invalid XML provenance"))?;
        if raw.len() > MAX_BYTES.div_ceil(3) * 4 {
            return Err(invalid("XML provenance exceeds its limit"));
        }
        let bytes = STANDARD
            .decode(raw)
            .map_err(|_| invalid("Invalid XML provenance"))?;
        let source = import(&bytes)?;
        if source["name"] == encoded["name"] && decode_record(&source)? == body {
            return Ok(bytes);
        }
    }
    let mut rows = body["macroEvents"]
        .as_array()
        .ok_or_else(|| invalid("Macro has no events"))?
        .clone();
    let movement = body
        .pointer("/appEngine/mouseMoveType")
        .and_then(Value::as_str)
        .unwrap_or("none");
    let setting = match movement {
        "none" => 0,
        "absolute" => 1,
        "foreground" => 2,
        "relative" => 3,
        _ => return Err(invalid("Unknown mouse movement mode; XML was not exported")),
    };
    if setting > 0 {
        for row in &mut rows {
            if row["Type"] == "actionBar" {
                row["recordProfile"] = json!({"mmtSetting":setting});
            }
        }
    }
    let mut output = String::new();
    output.push_str("<Macro>\n");
    element(&mut output, "Name", &json!(entry.name), 1)?;
    output.push_str("   <MacroEvents>\n");
    for row in rows {
        element(&mut output, "MacroEvent", &row, 2)?;
    }
    output.push_str("   </MacroEvents>\n");
    element(
        &mut output,
        "DelaySetting",
        body.get("delaySetting").unwrap_or(&json!(0)),
        1,
    )?;
    element(&mut output, "Guid", &json!(entry.guid), 1)?;
    element(&mut output, "Version", &json!(4), 1)?;
    element(&mut output, "MouseMoveType", &json!(movement), 1)?;
    output.push_str("</Macro>");
    if output.len() > MAX_BYTES {
        return Err(invalid("Macro XML exceeds512KiB"));
    }
    let bytes = output.into_bytes();
    // Never offer an XML file outside our admitted v4 import domain as a
    // successful portable export. Source/store data remains unchanged.
    import(&bytes)?;
    Ok(bytes)
}
fn element(out: &mut String, name: &str, value: &Value, depth: usize) -> Result<()> {
    if depth >= MAX_DEPTH || !xml_name(name) || name.starts_with('_') {
        return Err(invalid("Unsupported XML element name/depth"));
    }
    if let Value::Array(values) = value {
        for value in values {
            element(out, name, value, depth)?;
        }
        return Ok(());
    }
    out.push_str(&"   ".repeat(depth));
    out.push('<');
    out.push_str(name);
    match value {
        Value::Null => out.push_str("/>\n"),
        Value::Object(fields) if fields.is_empty() => out.push_str("/>\n"),
        Value::Object(fields) => {
            out.push_str(">\n");
            for (key, value) in fields {
                element(out, key, value, depth + 1)?;
            }
            out.push_str(&"   ".repeat(depth));
            out.push_str("</");
            out.push_str(name);
            out.push_str(">\n");
        }
        _ => {
            let text = match value {
                Value::String(s) => s.clone(),
                _ => value.to_string(),
            };
            if text.chars().any(|c| !xml_char(c)) {
                return Err(invalid("Macro contains an invalid XML character"));
            }
            out.push('>');
            out.push_str(&quick_xml::escape::escape(&text));
            out.push_str("</");
            out.push_str(name);
            out.push_str(">\n");
        }
    }
    if out.len() > MAX_BYTES {
        return Err(invalid("Macro XML exceeds512KiB"));
    }
    Ok(())
}
