//! Local action form. No action execution occurs here.
use super::*;
use host_actions::{Action as HostAction, Kind};
pub(super) const VALUE: u16 = 2310;
const URL: u16 = 2311;
#[derive(Clone)]
pub enum Control {
    Apply,
    Cancel,
    Program,
    Website,
    Browse,
}
pub struct Form {
    pub(super) action: HostAction,
    index: Option<usize>,
    owner: String,
    base: String,
    pub(super) token: u64,
    error: String,
}
pub(super) fn explain_leave(view: &mut View) {
    if let Some(form) = &mut view.form {
        form.error = "Apply or cancel this action editor before leaving.".into();
    }
}
pub(super) unsafe fn open(
    state: &mut State,
    kind: Kind,
    index: Option<usize>,
) -> Result<(), String> {
    let draft = state
        .macros
        .draft
        .as_ref()
        .ok_or("Create or select a macro first")?;
    let action = if let Some(index) = index {
        host_actions::from_row(draft.entry.events.get(index).ok_or("Event disappeared")?)
            .map_err(|error| error.to_string())?
            .ok_or("Select a text, command or launch action")?
    } else {
        HostAction::blank(kind)
    };
    host_actions::validate_display(&action).map_err(|error| error.to_string())?;
    state.macros.form_sequence = state.macros.form_sequence.wrapping_add(1);
    state.macros.form = Some(Form {
        action,
        index,
        owner: draft.entry.guid.clone(),
        base: draft.entry.revision.clone(),
        token: state.macros.form_sequence,
        error: String::new(),
    });
    Ok(())
}
fn title(action: &HostAction) -> &'static str {
    match action {
        HostAction::Text(_) => "TEXT FUNCTION",
        HostAction::Command(_) => "RUN COMMAND",
        HostAction::Launch { .. } => "LAUNCH",
    }
}
pub(super) unsafe fn fields(state: &mut State) {
    let Some(form) = &state.macros.form else {
        return;
    };
    let width = (state.width as f32 / state.scale) as i32;
    let height = (state.height as f32 / state.scale) as i32;
    let x = (width - 620) / 2;
    let y = (height - 360) / 2;
    let action = form.action.clone();
    let readonly = state.read_only_smoke || state.restricted_actions;
    let values = match &action {
        HostAction::Text(text) | HostAction::Command(text) => vec![(VALUE, text.clone(), y + 90)],
        HostAction::Launch { file, url, .. } => {
            vec![(VALUE, file.clone(), y + 105), (URL, url.clone(), y + 185)]
        }
    };
    for (id, text, top) in values {
        state.retained_fields.remove(&id);
        if matches!(action, HostAction::Text(_)) {
            let text = host_actions::normalize_text(&text).replace('\n', "\r\n");
            let handle = CreateWindowExW(
                0,
                wide("EDIT").as_ptr(),
                wide(&text).as_ptr(),
                WS_VISIBLE
                    | WS_CHILD
                    | WS_TABSTOP
                    | WS_VSCROLL
                    | ES_MULTILINE as u32
                    | ES_WANTRETURN as u32
                    | ES_AUTOVSCROLL as u32,
                state.px(x + 24),
                state.px(top),
                state.px(572),
                state.px(130),
                state.hwnd,
                id as usize as HMENU,
                GetModuleHandleW(null()),
                null(),
            );
            if !handle.is_null() {
                SendMessageW(handle, WM_SETFONT, state.fonts.normal as usize, 1);
                SendMessageW(handle, EM_SETLIMITTEXT, 500, 0);
                SendMessageW(handle, EM_SETREADONLY, readonly as usize, 0);
                state.fields.insert(id, handle);
            }
        } else {
            state.field(id, &text, x + 24, top, 572);
            if let Some(handle) = state.fields.get(&id) {
                SendMessageW(*handle, EM_SETLIMITTEXT, host_actions::ACTION_UNITS, 0);
                let inactive = matches!(action, HostAction::Launch { website: true, .. })
                    && id == VALUE
                    || matches!(action, HostAction::Launch { website: false, .. }) && id == URL;
                SendMessageW(*handle, EM_SETREADONLY, (readonly || inactive) as usize, 0);
            }
        }
    }
}
fn read_field(state: &State, id: u16) -> Result<String, String> {
    let handle = *state.fields.get(&id).ok_or("Action field is unavailable")?;
    unsafe {
        let length = usize::try_from(GetWindowTextLengthW(handle))
            .map_err(|_| "Could not read the action field")?;
        // Form controls have larger, explicit bounds than ordinary page fields.
        // Read the whole Unicode value or refuse it; never truncate at a surrogate.
        let limit = if state
            .macros
            .form
            .as_ref()
            .is_some_and(|form| matches!(form.action, HostAction::Text(_)))
        {
            500
        } else {
            host_actions::ACTION_UNITS
        };
        if length > limit {
            return Err(format!("The value is too long (limit: {limit})"));
        }
        let mut buffer = vec![0u16; length + 1];
        let copied = GetWindowTextW(handle, buffer.as_mut_ptr(), buffer.len() as i32);
        if usize::try_from(copied).ok() != Some(length) {
            return Err("The action field changed while being read; try again".into());
        }
        String::from_utf16(&buffer[..length])
            .map_err(|_| "The value contains unsupported Unicode".into())
    }
}
fn read_active_field(state: &mut State, id: u16) -> Result<(), String> {
    let text = read_field(state, id)?;
    if let Some(form) = &mut state.macros.form {
        match &mut form.action {
            HostAction::Text(value) => *value = host_actions::normalize_text(&text),
            HostAction::Command(value) => *value = text,
            HostAction::Launch { website, file, url } => {
                if *website && id == URL {
                    *url = text;
                } else if !*website && id == VALUE {
                    *file = text;
                }
            }
        }
        form.error.clear();
    }
    Ok(())
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if !matches!(id, VALUE | URL) || notification != EN_CHANGE as u16 {
        return false;
    }
    if state.read_only_smoke || state.restricted_actions || !state.fields.contains_key(&id) {
        return true;
    }
    if let Err(error) = read_active_field(state, id) {
        if let Some(form) = &mut state.macros.form {
            form.error = error;
        }
    }
    unsafe {
        InvalidateRect(state.hwnd, null(), 0);
    }
    true
}
pub(super) unsafe fn action(state: &mut State, control: Control) {
    if matches!(control, Control::Cancel) {
        state.macros.form = None;
        state.macros.status = "Action edit cancelled".into();
        state.macros.failed = false;
        state.rebuild_fields();
        InvalidateRect(state.hwnd, null(), 0);
        return;
    }
    if state.read_only_smoke || state.restricted_actions {
        return;
    }
    let result = (|| -> Result<(), String> {
        if matches!(control, Control::Apply) {
            let id =
                if state.macros.form.as_ref().is_some_and(|form| {
                    matches!(form.action, HostAction::Launch { website: true, .. })
                }) {
                    URL
                } else {
                    VALUE
                };
            read_active_field(state, id)?;
        }
        match control {
            Control::Apply => {
                let form = state
                    .macros
                    .form
                    .as_ref()
                    .ok_or("No action editor is open")?;
                host_actions::row(&form.action).map_err(|error| error.to_string())?;
                let draft = state.macros.draft.as_ref().ok_or("Macro disappeared")?;
                if draft.entry.guid != form.owner || draft.entry.revision != form.base {
                    return Err(
                        "Macro changed while this action was open; cancel and reopen the editor"
                            .into(),
                    );
                }
                let guid = form.owner.clone();
                let action = form.action.clone();
                let index = form.index.unwrap_or_else(|| {
                    state.macros.event.map_or(draft.entry.events.len(), |i| {
                        (i + 1).min(draft.entry.events.len())
                    })
                });
                let change = if form.index.is_some() {
                    Change::SetAction {
                        guid,
                        index,
                        action,
                    }
                } else {
                    Change::InsertAction {
                        guid,
                        index,
                        action,
                    }
                };
                super::local(state, change)?;
                state.macros.form = None;
                state.macros.event = Some(index);
            }
            Control::Program | Control::Website => {
                if let Some(Form {
                    action: HostAction::Launch { website, .. },
                    ..
                }) = &mut state.macros.form
                {
                    *website = matches!(control, Control::Website);
                }
            }
            Control::Browse => {
                PostMessageW(state.hwnd, HOST_BROWSE, 0, 0);
            }
            Control::Cancel => {}
        }
        Ok(())
    })();
    if let Err(error) = result {
        if let Some(form) = &mut state.macros.form {
            form.error = error;
        }
    }
    state.rebuild_fields();
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn paint(state: &State, p: &mut Painter, width: i32, height: i32) {
    let Some(form) = &state.macros.form else {
        return;
    };
    let x = (width - 620) / 2;
    let y = (height - 360) / 2;
    let mutable = !state.read_only_smoke && !state.restricted_actions;
    p.hits.clear();
    p.fill(0, 80, width, height - 80, 0x202020);
    p.fill(x, y, 620, 360, PANEL);
    p.text(
        title(&form.action),
        x + 24,
        y + 15,
        570,
        36,
        p.fonts.heading,
        GREEN,
        DT_LEFT,
    );
    match &form.action {
        HostAction::Text(text) => {
            p.text(
                "Enter text",
                x + 24,
                y + 58,
                570,
                28,
                p.fonts.normal,
                TEXT,
                DT_LEFT,
            );
            let count = host_actions::normalize_text(text).encode_utf16().count();
            p.text(
                &format!("{count}/250"),
                x + 24,
                y + 230,
                570,
                24,
                p.fonts.small,
                if count > 250 { 0xfd4949 } else { MUTED },
                DT_RIGHT,
            );
        }
        HostAction::Command(_) => {
            p.text(
                "Command",
                x + 24,
                y + 58,
                570,
                28,
                p.fonts.normal,
                TEXT,
                DT_LEFT,
            );
        }
        HostAction::Launch { website, .. } => {
            button(
                p,
                if *website { "Program" } else { "● Program" },
                x + 24,
                y + 62,
                160,
                super::Control::Host(Control::Program),
                mutable,
            );
            button(
                p,
                if *website { "● Website" } else { "Website" },
                x + 24,
                y + 142,
                160,
                super::Control::Host(Control::Website),
                mutable,
            );
            button(
                p,
                "BROWSE",
                x + 466,
                y + 62,
                130,
                super::Control::Host(Control::Browse),
                mutable && !*website,
            );
        }
    }
    let text = if form.error.is_empty() {
        "Changes stay in the macro draft until you save the macro."
    } else {
        &form.error
    };
    p.text(
        text,
        x + 24,
        y + 262,
        572,
        45,
        p.fonts.small,
        if form.error.is_empty() {
            MUTED
        } else {
            0xfd4949
        },
        DT_LEFT | DT_WORDBREAK,
    );
    button(
        p,
        "CANCEL",
        x + 346,
        y + 312,
        120,
        super::Control::Host(Control::Cancel),
        true,
    );
    button(
        p,
        "APPLY",
        x + 476,
        y + 312,
        120,
        super::Control::Host(Control::Apply),
        mutable,
    );
}
pub(super) unsafe fn browse(hwnd: HWND, pointer: *mut RefCell<State>) {
    let token = (*pointer)
        .try_borrow()
        .ok()
        .filter(|state| active(state) && !state.read_only_smoke && !state.restricted_actions)
        .and_then(|state| {
            state
                .macros
                .form
                .as_ref()
                .filter(|form| matches!(form.action, HostAction::Launch { website: false, .. }))
                .map(|form| form.token)
        });
    let Some(token) = token else {
        return;
    };
    let mut path = vec![0u16; 32768];
    let filter = wide("Programs and files\0*.*\0\0");
    let mut options: OPENFILENAMEW = std::mem::zeroed();
    options.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    options.hwndOwner = hwnd;
    options.lpstrFile = path.as_mut_ptr();
    options.nMaxFile = path.len() as u32;
    options.lpstrFilter = filter.as_ptr();
    options.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    let result = GetOpenFileNameW(&mut options);
    let error = if result == 0 {
        CommDlgExtendedError()
    } else {
        0
    };
    if let Ok(mut state) = (*pointer).try_borrow_mut() {
        if !state.read_only_smoke && !state.restricted_actions {
            if let Some(form) = state
                .macros
                .form
                .as_mut()
                .filter(|form| form.token == token)
            {
                if result != 0 {
                    if let Some(end) = path.iter().position(|c| *c == 0) {
                        match String::from_utf16(&path[..end]) {
                            Ok(value) => {
                                if let HostAction::Launch {
                                    website: false,
                                    file,
                                    ..
                                } = &mut form.action
                                {
                                    *file = value;
                                    form.error.clear();
                                }
                            }
                            Err(_) => form.error = "File name has unsupported Unicode".into(),
                        }
                    }
                } else if error != 0 {
                    form.error = format!("File chooser failed ({error})");
                }
            }
        }
        state.rebuild_fields();
        InvalidateRect(hwnd, null(), 0);
    }
}
#[cfg(test)]
pub(super) fn diagnostics(view: &View) -> Value {
    view.form.as_ref().map_or(Value::Null, |form| {
        let data = match &form.action {
            HostAction::Text(text) => json!({"kind":"text","value":text}),
            HostAction::Command(command) => json!({"kind":"command","value":command}),
            HostAction::Launch { website, file, url } => {
                json!({"kind":if *website{"website"}else{"program"},"file":file,"url":url})
            }
        };
        json!({"data":data,"index":form.index,"error":form.error})
    })
}
