//! Explicit file-backed target choices; previews never change the draft.
use super::*;
unsafe fn select(hwnd: HWND, labels: &[String]) -> Result<Option<usize>, String> {
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return Err("Could not open remap choices".into());
    }
    for (i, label) in labels.iter().enumerate() {
        AppendMenuW(
            menu,
            MF_STRING,
            i + 1,
            wide(&label.replace('&', "&&")).as_ptr(),
        );
    }
    let mut pos = std::mem::zeroed();
    GetCursorPos(&mut pos);
    let selected = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_NONOTIFY,
        pos.x,
        pos.y,
        0,
        hwnd,
        null(),
    ) as usize;
    DestroyMenu(menu);
    Ok(selected.checked_sub(1))
}
pub(super) unsafe fn dialog(hwnd: HWND, pointer: *mut RefCell<State>) {
    let (epoch, choices) = {
        let Ok(mut state) = (*pointer).try_borrow_mut() else {
            return;
        };
        if state.read_only_smoke
            || state.restricted_actions
            || !state.studio.editable()
            || state.studio.output.busy
        {
            return;
        }
        let choices = match state.studio.draft.as_ref().unwrap().remap_choices() {
            Ok(c) => c,
            Err(e) => {
                error(&mut state, e.to_string());
                return;
            }
        };
        cancel_gestures(&mut state);
        state.studio.dialog = true;
        (state.studio.epoch, choices)
    };
    let result = (|| -> Result<Option<_>, String> {
        let Some(source) = select(
            hwnd,
            &choices
                .iter()
                .map(|(_, label)| format!("SOURCE: {label}"))
                .collect::<Vec<_>>(),
        )?
        else {
            return Ok(None);
        };
        let Some(path) = choose(hwnd, false, "")? else {
            return Ok(None);
        };
        let archive = Archive::read(File::open(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        #[derive(serde::Deserialize)]
        struct Placements {
            devices: Vec<Box<serde_json::value::RawValue>>,
        }
        let mut targets = Vec::new();
        let mut labels = Vec::new();
        for i in 0..archive.profile_count() {
            let raw = archive.profile_json(i).unwrap();
            let draft = Draft::open(raw).map_err(|e| e.to_string())?;
            let choices = draft.remap_choices().map_err(|e| e.to_string())?;
            let placements: Placements = serde_json::from_str(raw).map_err(|e| e.to_string())?;
            for ((_, label), target) in choices.into_iter().zip(placements.devices) {
                labels.push(format!("TARGET {}: {label}", i + 1));
                targets.push(target);
                if targets.len() > 512 {
                    return Err("Target file exceeds512 placement choices".into());
                }
            }
        }
        let Some(target) = select(hwnd, &labels)? else {
            return Ok(None);
        };
        let preview = {
            let state = (*pointer).borrow();
            if state.studio.epoch != epoch {
                return Err("Composition changed".into());
            }
            state
                .studio
                .draft
                .as_ref()
                .ok_or("Composition closed")?
                .preview_remap(&choices[source].0, targets[target].get())
                .map_err(|e| e.to_string())?
        };
        let summary=format!("{} LED assignments matched. {} missing.\n{}\n\nCommit this layout remap? Save the composition to keep it.",preview.matched,preview.missing.len(),preview.missing.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
        if !preview.missing.is_empty() {
            MessageBoxW(
                hwnd,
                wide(&summary).as_ptr(),
                wide("Remap blocked: missing LEDs").as_ptr(),
                MB_OK | MB_ICONWARNING,
            );
            return Ok(None);
        }
        if MessageBoxW(
            hwnd,
            wide(&summary).as_ptr(),
            wide("Layout remap preview").as_ptr(),
            MB_OKCANCEL | MB_ICONQUESTION,
        ) != IDOK
        {
            return Ok(None);
        }
        Ok(Some(preview))
    })();
    let Ok(mut state) = (*pointer).try_borrow_mut() else {
        return;
    };
    state.studio.dialog = false;
    if state.studio.epoch != epoch {
        return;
    }
    if state.read_only_smoke
        || state.restricted_actions
        || !state.studio.editable()
        || state.studio.output.busy
    {
        error(
            &mut state,
            "Remapping is unavailable in the current editor state",
        );
        redraw(&mut state);
        return;
    }
    match result {
        Ok(Some(preview)) => {
            match state
                .studio
                .draft
                .as_mut()
                .ok_or("Composition closed".to_string())
                .and_then(|draft| draft.commit_remap(preview).map_err(|e| e.to_string()))
            {
                Ok(_) => {
                    playback::stop(&mut state);
                    state.studio.canvas = studio_canvas::Canvas::load(
                        state.studio.draft.as_ref().unwrap().json(),
                        &assets_dir(),
                    );
                    selection_changed(&mut state);
                    state.studio.status="Layout remapped locally. Undo or Save to keep it. Hardware output remains separately gated.".into();
                    state.studio.failed = false;
                }
                Err(message) => error(&mut state, message),
            }
        }
        Ok(None) => {}
        Err(message) => error(&mut state, message),
    }
    redraw(&mut state);
}

#[cfg(test)]
pub(super) unsafe fn fixture(state: &mut State) {
    let result = (|| -> Result<(), String> {
        let draft = state.studio.draft.as_mut().ok_or("No draft")?;
        let target = include_str!("../../../secbird-core/tests/fixtures/remap-target.json");
        let original = draft.json().to_owned();
        drop(
            draft
                .preview_remap("old", target)
                .map_err(|e| e.to_string())?,
        );
        if draft.json() != original {
            return Err("Preview changed draft".into());
        }
        draft
            .commit_remap(
                draft
                    .preview_remap("old", target)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(e) = result {
        error(state, e);
    } else {
        state.studio.canvas =
            studio_canvas::Canvas::load(state.studio.draft.as_ref().unwrap().json(), &assets_dir());
        selection_changed(state);
    }
    redraw(state);
}
