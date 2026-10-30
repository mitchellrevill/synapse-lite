use super::*;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tests/fixtures/standalone-macro-xml.json"
    ))
    .unwrap()
}
fn simple() -> Vec<u8> {
    fixture()["cases"][1]["xml"]
        .as_str()
        .unwrap()
        .as_bytes()
        .to_vec()
}

#[test]
fn standalone_xml_source_constructor_vectors_and_verbatim_roundtrip() {
    let data = fixture();
    let cases = data["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 31);
    for case in cases {
        let bytes = case["xml"].as_str().unwrap().as_bytes();
        let encoded = import(bytes).unwrap_or_else(|e| panic!("{}: {e}", case["name"]));
        let body = decode_record(&encoded).unwrap();
        let mut expected = case["imported"].clone();
        if case["intentional_difference"].is_string() {
            expected["macroEvents"][1]["Text"] = case["input"]["macroEvents"][1]["Text"].clone();
        }
        assert_eq!(
            stable_json(&body).unwrap(),
            stable_json(&expected).unwrap(),
            "{}",
            case["name"]
        );
        assert_eq!(export(&encoded).unwrap(), bytes, "{}", case["name"]);
        assert_eq!(encoded[ORIGINAL]["bytes"], STANDARD.encode(bytes));
    }
}

#[test]
fn standalone_xml_export_matches_source_envelope_without_mutating_record() {
    for case in fixture()["cases"].as_array().unwrap() {
        let encoded = record(&case["input"], case["input"]["name"].as_str().unwrap()).unwrap();
        let before = encoded.clone();
        let output = export(&encoded).unwrap();
        assert_eq!(encoded, before);
        let root = parse(&output).unwrap();
        assert_eq!(text_field(&root, "Name"), case["envelope"]["Macro"]["Name"]);
        assert_eq!(text_field(&root, "Guid"), case["envelope"]["Macro"]["Guid"]);
        assert_eq!(text_field(&root, "Version"), json!("4"));
        let projected = import(&output).unwrap();
        let expected = import(case["xml"].as_str().unwrap().as_bytes()).unwrap();
        assert_eq!(
            stable_json(&decode_record(&projected).unwrap()).unwrap(),
            stable_json(&decode_record(&expected).unwrap()).unwrap(),
            "{}",
            case["name"]
        );
    }
}
fn text_field(node: &Node, name: &str) -> Value {
    json!(node.one(name).unwrap().scalar().unwrap())
}

#[test]
fn standalone_xml_import_integrates_without_profiles_or_active_selection_changes() {
    let initial = import_document(include_bytes!(
        "../../../../tests/fixtures/synthetic.synapse4"
    ))
    .unwrap()
    .store;
    let before = serde_json::to_value(&initial).unwrap();
    let bytes = simple();
    let changed = edit(&initial, Change::Import(bytes.clone())).unwrap();
    assert_eq!(serde_json::to_value(&initial).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&changed.devices).unwrap(),
        serde_json::to_value(&initial.devices).unwrap()
    );
    let encoded = import(&bytes).unwrap();
    let id = guid(&encoded).unwrap();
    let entry = read(&changed)
        .unwrap()
        .entries
        .into_iter()
        .find(|e| e.guid == id)
        .unwrap();
    assert_eq!(entry.encoded_record, encoded);
    assert_eq!(
        serde_json::to_value(edit(&changed, Change::Import(bytes)).unwrap()).unwrap(),
        serde_json::to_value(&changed).unwrap()
    );
    let revised = edit_definition(
        &encoded,
        Change::SetDelay {
            guid: id.clone(),
            index: 1,
            milliseconds: 250,
        },
    )
    .unwrap();
    assert_eq!(revised[ORIGINAL], encoded[ORIGINAL]);
    assert_eq!(describe(&revised).unwrap().events[1]["Number"], "0.25");
    assert_eq!(
        decode_record(&import(&export(&revised).unwrap()).unwrap()).unwrap()["macroEvents"][1]
            ["Number"],
        "0.250"
    );
    let renamed = edit_definition(
        &encoded,
        Change::Rename {
            guid: id,
            name: "Renamed & portable".into(),
        },
    )
    .unwrap();
    assert_eq!(
        import(&export(&renamed).unwrap()).unwrap()["name"],
        "Renamed & portable"
    );
}

#[test]
fn standalone_xml_conflicts_and_malformed_data_refuse_atomically() {
    let bytes = simple();
    let initial = edit(&Store::default(), Change::Import(bytes.clone())).unwrap();
    let before = serde_json::to_value(&initial).unwrap();
    let changed = String::from_utf8(bytes)
        .unwrap()
        .replace("Portable &amp;", "Different &amp;");
    assert!(edit(&initial, Change::Import(changed.into_bytes())).is_err());
    assert_eq!(serde_json::to_value(&initial).unwrap(), before);
    let source = String::from_utf8(simple()).unwrap();
    for bad in [
        source.replace("<Version>4</Version>", "<Version>3</Version>"),
        source.replace("<Guid>", "<Guid>x</Guid><Guid>"),
        source.replace("<Number>", "<Future/><Number>"),
        source.replace("<Macro>", "<Macro attr='x'>"),
        source.replace("<Macro>", "<Macro xmlns='urn:unknown'>"),
        source.replace("<Type>0</Type>", "<Type>9</Type>"),
        source.replace("<Type>0</Type>", "<Type>0</Type><phase>0</phase>"),
        source.replace("<Type>0</Type>", "<Type>0</Type><Delay>100</Delay>"),
        source.replace("</Macro>", "</Other>"),
        source.clone() + "<Macro/>",
        format!("<!DOCTYPE Macro [<!ENTITY secret SYSTEM 'file:///never-read'>]>{source}"),
        source.replace("Portable", "&unknown;"),
        source.replace("Portable", "&#0;"),
        source.replace("Portable", "&#xFFFE;"),
        format!("<?xml version='1.0' encoding='UTF-16'?>{source}"),
        format!("<?xml version='1.1'?>{source}"),
    ] {
        assert!(
            import(bad.as_bytes()).is_err(),
            "Invalid input was accepted: {}",
            bad.chars().take(150).collect::<String>()
        );
    }
    assert!(import(&[0xff, 0xfe, 0, 0]).is_err());
    let all = fixture()["cases"][0]["xml"].as_str().unwrap().to_owned();
    for bad in [
        all.replace("<Makecode>65</Makecode>", "<Makecode>65536</Makecode>"),
        all.replace("<State>1</State>", "<State>6</State>"),
        all.replace(
            "<MouseButton>0</MouseButton>",
            "<MouseButton>10</MouseButton>",
        ),
        all.replace("<time>5</time>", "<time>-1</time>"),
        all.replace("<x>-4</x>", "<x>NaN</x>"),
        all.replace("<Number>1.2345</Number>", "<Number>NaN</Number>"),
        source.replace("<Type>0</Type>", "<Type>3</Type><MouseEvent/>"),
    ] {
        assert!(import(bad.as_bytes()).is_err());
    }
}

#[test]
fn standalone_xml_bounds_and_cache_guards_are_not_bypassed() {
    assert!(import(&vec![b' '; MAX_BYTES + 1]).is_err());
    let deep = format!(
        "<Macro>{}x{}</Macro>",
        "<nested>".repeat(33),
        "</nested>".repeat(33)
    );
    assert!(import(deep.as_bytes()).is_err());
    let many = format!("<Macro>{}</Macro>", "<node/>".repeat(MAX_NODES));
    assert!(import(many.as_bytes()).is_err());
    let source = String::from_utf8(simple()).unwrap();
    let bar = "<MacroEvent><Type>actionBar</Type></MacroEvent>";
    let too_many = source.replace(
        "<MacroEvents>",
        &format!("<MacroEvents>{}", bar.repeat(MAX_EVENTS)),
    );
    assert!(import(too_many.as_bytes()).is_err());
    let encoded = import(&simple()).unwrap();
    let id = guid(&encoded).unwrap();
    let body = decode_record(&encoded).unwrap();
    assert!(planning::prepare(
        &edit(&Store::default(), Change::Import(simple())).unwrap(),
        &json!({"guid":id,"macroPlaybackOption":"Once"}).to_string()
    )
    .is_ok());
    for extra in [
        json!({"events":[]}),
        json!({"hash":"x"}),
        json!({"future":true}),
    ] {
        let mut changed = body.clone();
        changed["appEngine"]
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let record = record(&changed, "Guarded").unwrap();
        assert!(edit_definition(
            &record,
            Change::SetDelay {
                guid: id.clone(),
                index: 1,
                milliseconds: 250
            }
        )
        .is_err());
        let mut store = Store::default();
        store
            .extra
            .insert(KEY.into(), json!({"version":1,"records":[record]}));
        assert!(planning::prepare(
            &store,
            &json!({"guid":id,"macroPlaybackOption":"Once"}).to_string()
        )
        .is_err());
    }
    let mut bad = encoded.clone();
    bad[ORIGINAL]["version"] = json!(2);
    assert!(export(&bad).is_err());
    let mut bad = encoded;
    bad["hash"] = json!("invalid");
    assert!(export(&bad).is_err());
}

#[test]
fn standalone_xml_cdata_whitespace_and_literal_entities_remain_text() {
    let source = String::from_utf8(simple()).unwrap();
    let end = source.find("<MacroEvents>").unwrap();
    let prefix = &source[..end];
    let xml=format!("{prefix}<MacroEvents><MacroEvent><Type>4</Type><Text><![CDATA[ literal &lt; <tag> 😃 ]]></Text></MacroEvent></MacroEvents><DelaySetting>0</DelaySetting><Guid>text</Guid><Version>4</Version></Macro>");
    let encoded = import(xml.as_bytes()).unwrap();
    assert_eq!(
        describe(&encoded).unwrap().events[0]["Text"],
        " literal &lt; <tag> 😃 "
    );
    let declared = format!("\u{feff}<?xml version='1.0' encoding='UTF-8'?>{xml}");
    assert!(import(declared.as_bytes()).is_ok());
}
