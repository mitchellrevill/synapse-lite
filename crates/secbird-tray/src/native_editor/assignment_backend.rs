//! One local-only commit boundary for the integrated Customize categories.
use super::*;
use assignment_model::{Category, Context, Draft};
use secbird_core::profile_compat::{self, assignment_state};

pub(super) fn candidate(draft: &Draft) -> Result<secbird_core::Profile, String> {
    if !draft.can_save() {
        return Err("Choose a valid supported assignment before Save".into());
    }
    assignment_state::validate_input(
        draft.context.owner.pid,
        &draft.input.input_id,
        &draft.input.input_type,
    )
    .map_err(|e| e.to_string())?;
    let mut original: secbird_core::Profile =
        serde_json::from_value(draft.context.original_profile.clone())
            .map_err(|e| e.to_string())?;
    assignment_state::clear_sensitivity(
        &mut original,
        draft.context.owner.pid,
        &draft.context.owner.profile,
        &draft.input.input_id,
        draft.hyper,
    )
    .map_err(|e| e.to_string())?;
    let prepared = assignment_state::prepare_input(
        &original,
        draft.context.owner.pid,
        &draft.context.owner.profile,
        &draft.input.input_id,
    )
    .map_err(|e| e.to_string())?;
    let value = serde_json::to_value(&prepared).map_err(|e| e.to_string())?;
    let mappings = assignment_state::effective_mappings(&prepared, draft.context.owner.pid)
        .map_err(|e| e.to_string())?;
    let mut edit = draft.clone();
    edit.original_records = mappings
        .iter()
        .filter(|r| {
            r["inputID"] == draft.input.input_id
                && r["isHyperShift"].as_bool().unwrap_or(false) == draft.hyper
        })
        .cloned()
        .collect();
    edit.context = Arc::new(Context {
        owner: draft.context.owner.clone(),
        original_profile: value.clone(),
        mappings: Some(mappings),
        sensitivity_range: draft.context.sensitivity_range.clone(),
    });
    let pid = edit.context.owner.pid;
    let mut next = match edit.category {
        Some(Category::Keyboard) => profile_compat::keyboard_assignments::update(
            &prepared,
            pid,
            &edit.input.input_id,
            &edit.input.input_type,
            edit.hyper,
            &edit.keyboard_setting(),
        )
        .map_err(|e| e.to_string())?,
        Some(Category::Mouse) => {
            let mut next = prepared;
            profile_compat::mouse_assignment::set_pending(
                &mut next,
                pid,
                &edit.context.owner.profile,
                edit.assignment()?,
            )
            .map_err(|e| e.to_string())?;
            next
        }
        Some(Category::Media) => profile_compat::multimedia::candidate(
            &prepared,
            pid,
            &edit.media_edit().ok_or("Select a Multimedia command")?,
        )
        .map_err(|e| e.to_string())?,
        Some(Category::Windows) => profile_compat::windows_shortcuts::edit(
            &prepared,
            pid,
            &edit.input.input_id,
            &edit.input.input_type,
            edit.hyper,
            edit.options()[edit.windows_option.ok_or("Select a Windows shortcut")?].0,
        )
        .map_err(|e| e.to_string())?,
        Some(Category::Launch) => {
            serde_json::from_value(edit.launch_profile()?).map_err(|e| e.to_string())?
        }
        Some(Category::Default | Category::Disable) => {
            serde_json::from_value(edit.local_profile(&value)?).map_err(|e| e.to_string())?
        }
        _ => return Err("This assignment category has no verified local editor".into()),
    };
    assignment_state::mark_local(&mut next, pid, &edit.input.input_id, edit.hyper)
        .map_err(|e| e.to_string())?;
    Ok(next)
}

pub(super) fn save(engine: &mut Engine, draft: &Draft) -> Result<Value, String> {
    if engine.is_measurement_read_only() {
        return Err("Read-only mode refuses assignment saves".into());
    }
    let device = engine
        .devices()
        .iter()
        .find(|d| d.key() == draft.context.owner.device)
        .ok_or("Selected device disconnected")?;
    if device.pid != draft.context.owner.pid {
        return Err("Device identity changed".into());
    }
    let mut store = engine.load_store().map_err(|e| e.to_string())?;
    let key = profile_compat::gui::stored_device_key(&store, &draft.context.owner.device)
        .ok_or("Stored device disappeared")?;
    let profiles = store
        .devices
        .get_mut(&key)
        .ok_or("Stored device disappeared")?;
    if profiles.active != draft.context.owner.profile {
        return Err("Active profile changed; reopen assignment".into());
    }
    let profile = profiles
        .profiles
        .get_mut(&profiles.active)
        .ok_or("Stored profile disappeared")?;
    if serde_json::to_value(&*profile).map_err(|e| e.to_string())? != draft.context.original_profile
    {
        return Err("Profile changed; reopen assignment before saving".into());
    }
    *profile = candidate(draft)?;
    profile_compat::validate_store_budget(&store).map_err(|e| e.to_string())?;
    store.save().map_err(|e| e.to_string())?;
    server::dispatch(engine, "GET", "/api/state", &[]).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sensitivity_drafts_replace_and_are_replaced_without_promoting_source_or_shadowing() {
        let path = std::env::temp_dir().join(format!(
            "sensitivity-category-integration-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut store = secbird_core::Store::load(&path).unwrap();
        let mut profile = profile_compat::new_native_profile(196, "Fixture").unwrap();
        profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"] = json!([
            {"inputID":"RightClick","inputType":"MouseInput","isHyperShift":false,"outputType":"futureGroup","futureGroup":{"opaque":17}},
            {"inputID":"RightClick","inputType":"MouseInput","isHyperShift":true,"outputType":"futureGroup","futureGroup":{"other":19}}
        ]);
        let range = secbird_core::db::DpiRange {
            min: 100,
            max: 26000,
            step: 50,
        };
        let intent = secbird_core::sensitivity_assignment::Intent {
            input: "RightClick".into(),
            hyper: false,
            assignment: "DPI_Clutch".into(),
            x: 401,
            y: 1200,
            independent: true,
        };
        let input = assignment_model::mouse_inputs().remove(1);
        for category in [
            Category::Mouse,
            Category::Keyboard,
            Category::Media,
            Category::Windows,
            Category::Launch,
            Category::Disable,
            Category::Default,
            Category::Mouse,
        ] {
            let device = json!({"id":"00C4:INTEGRATION","pid":"00C4","active":"Fixture","profile":profile,"sensitivity_assignment":{"min":100,"max":26000,"step":50}});
            let context = Arc::new(Context::from_device(&device).unwrap());
            let mut edit = Draft::new(
                context,
                input.clone(),
                assignment_model::Anchor::Mouse(1),
                false,
            );
            edit.select(category);
            match category {
                Category::Keyboard => edit.select_keyboard_option(2),
                Category::Mouse => edit.select_mouse(3),
                Category::Media => edit.select_option(7),
                Category::Windows => edit.select_option(25),
                Category::Launch => {
                    edit.launch.website = true;
                    edit.launch.url = "category.example.test".into();
                }
                _ => {}
            }
            edit.dirty = true;
            profile = candidate(&edit).unwrap_or_else(|e| panic!("{category:?}: {e}"));
            assert!(!profile
                .extra
                .contains_key(secbird_core::sensitivity_assignment::DRAFTS));
            let source = profile.extra["_synapseLiteSynapse4"]["decoded"].clone();
            let buttons = profile.buttons.clone();
            let profiles = store.profiles_for("00C4:INTEGRATION", 196);
            profiles.profiles.clear();
            profiles.profiles.insert("Fixture".into(), profile.clone());
            profiles.active = "Fixture".into();
            let expected = serde_json::to_value(&profile).unwrap();
            secbird_core::sensitivity_assignment::save_draft(
                &mut store,
                "00C4:INTEGRATION",
                "Fixture",
                196,
                &range,
                &expected,
                &intent,
            )
            .unwrap();
            profile = store.devices["00C4:INTEGRATION"].profiles["Fixture"].clone();
            assert_eq!(profile.extra["_synapseLiteSynapse4"]["decoded"], source);
            assert_eq!(profile.buttons, buttons);
            let device = json!({"id":"00C4:INTEGRATION","pid":"00C4","active":"Fixture","profile":profile,"sensitivity_assignment":{"min":100,"max":26000,"step":50}});
            let reopened = Draft::new(
                Arc::new(Context::from_device(&device).unwrap()),
                input.clone(),
                assignment_model::Anchor::Mouse(1),
                false,
            );
            assert_eq!(reopened.category, Some(Category::Sensitivity));
            assert_eq!(reopened.sensitivity, intent);
            let effective = assignment_state::effective_mappings(&profile, 196).unwrap();
            assert_eq!(
                effective
                    .iter()
                    .find(|r| r["inputID"] == "RightClick" && r["isHyperShift"] == true)
                    .unwrap()["futureGroup"],
                json!({"other":19})
            );
        }
        std::fs::remove_file(path).unwrap();
    }
    fn draft(profile: &secbird_core::Profile, category: Category, hyper: bool) -> Draft {
        let device =
            json!({"id":"025D:INTEGRATION","pid":"025D","active":"Fixture","profile":profile});
        let context = Arc::new(Context::from_device(&device).unwrap());
        let input = assignment_model::Input {
            key: "A".into(),
            input_id: "KEY_A".into(),
            input_type: "KeyInput".into(),
            label: "A".into(),
            default_label: "A".into(),
            enabled: true,
            functions: vec![
                "DEFAULT",
                "DISABLE",
                "KEYBOARD_FUNCTION",
                "MOUSE_FUNCTION",
                "MULTIMEDIA",
                "WINDOWS_SHORTCUT",
                "LAUNCH_PROGRAM",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        };
        let mut draft = Draft::new(
            context,
            input,
            assignment_model::Anchor::Key("KEY_A".into()),
            hyper,
        );
        draft.select(category);
        match category {
            Category::Keyboard => draft.select_keyboard_option(2),
            Category::Mouse => draft.select_mouse(3),
            Category::Media => draft.select_option(7),
            Category::Windows => draft.select_option(25),
            Category::Launch => {
                draft.launch.website = true;
                draft.launch.url = "integration.example.test".into();
            }
            _ => {}
        }
        draft.dirty = true;
        draft
    }
    fn profile() -> secbird_core::Profile {
        let mut profile = profile_compat::new_native_profile(605, "Fixture").unwrap();
        profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"] = json!([
            {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"futureGroup","futureGroup":{"keep":[1,2]},"rootOpaque":17},
            {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":true,"outputType":"futureGroup","futureGroup":{"keep":[3,4]}},
            {"inputID":"KEY_B","inputType":"KeyInput","isHyperShift":false,"outputType":"futureGroup","futureGroup":{"other":true}}
        ]);
        profile
    }
    #[test]
    fn integrated_categories_replace_each_other_without_shadowing_and_roundtrip() {
        let mut profile = profile();
        let original = profile.clone();
        for (category, group) in [
            (Category::Mouse, "mouseGroup"),
            (Category::Keyboard, "keyboardGroup"),
            (Category::Media, "multimediaGroup"),
            (Category::Windows, "win8ShortcutsGroup"),
            (Category::Launch, "launchGroup"),
            (Category::Disable, "disableGroup"),
            (Category::Default, "defaultGroup"),
            (Category::Mouse, "mouseGroup"),
            (Category::Media, "multimediaGroup"),
            (Category::Keyboard, "keyboardGroup"),
        ] {
            let before = profile.clone();
            let edit = draft(&profile, category, false);
            profile = candidate(&edit).unwrap_or_else(|e| panic!("{category:?}: {e}"));
            assert_eq!(
                before.extra["_synapseLiteSynapse4"]["decoded"]["mappings"][1],
                original.extra["_synapseLiteSynapse4"]["decoded"]["mappings"][1]
            );
            let rows = assignment_state::effective_mappings(&profile, 605).unwrap();
            let row = rows
                .iter()
                .find(|r| r["inputID"] == "KEY_A" && r["isHyperShift"] == false)
                .unwrap();
            assert_eq!(row["outputType"], group, "{category:?}");
            assert_eq!(row["rootOpaque"], 17);
            assert_eq!(row["futureGroup"], json!({"keep":[1,2]}));
            assert_eq!(
                rows.iter().find(|r| r["inputID"] == "KEY_B").unwrap()["futureGroup"],
                json!({"other":true})
            );
            assert!(profile.buttons.is_empty());
            let mut store = secbird_core::Store::default();
            let profiles = store.device(605);
            profiles.profiles.clear();
            profiles.profiles.insert("Fixture".into(), profile.clone());
            profiles.active = "Fixture".into();
            let exported = profile_compat::export_synapse4(&store, "025D").unwrap();
            let imported = profile_compat::import_document(&exported.bytes).unwrap();
            let rows = &imported.store.devices["025D"].profiles["Fixture"].extra
                ["_synapseLiteSynapse4"]["decoded"]["mappings"];
            assert_eq!(
                rows.as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["inputID"] == "KEY_A" && r["isHyperShift"] == false)
                    .unwrap()["outputType"],
                group
            );
        }
    }
    #[test]
    fn integrated_layer_edits_preserve_opposite_native_projection_and_input() {
        let mut profile = profile();
        profile
            .buttons
            .insert("key-a/hypershift".into(), "key:x".into());
        profile = candidate(&draft(&profile, Category::Mouse, false)).unwrap();
        profile = candidate(&draft(&profile, Category::Keyboard, false)).unwrap();
        assert_eq!(profile.buttons["key-a/hypershift"], "key:x");
        let normal = assignment_state::effective_mappings(&profile, 605)
            .unwrap()
            .into_iter()
            .find(|r| r["inputID"] == "KEY_A" && r["isHyperShift"] == false)
            .unwrap();
        profile = candidate(&draft(&profile, Category::Windows, true)).unwrap();
        assert!(!profile.buttons.contains_key("key-a/hypershift"));
        assert_eq!(
            assignment_state::effective_mappings(&profile, 605)
                .unwrap()
                .into_iter()
                .find(|r| r["inputID"] == "KEY_A" && r["isHyperShift"] == false)
                .unwrap(),
            normal
        );
    }
}
