//! Global Shortcuts cards, bounded local drafts, and explicit input choices.
use super::*;
use secbird_core::global_shortcuts::{self as model, Library};
use std::io::Read;
use windows_sys::Win32::{
    Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG},
    UI::Controls::{Dialogs::*, EM_SETREADONLY},
};
pub(super) const ID: &str = "module:global-shortcuts";
pub(super) const MENU: u32 = WM_APP + 181;
pub(super) const FILE: u32 = WM_APP + 182;
#[cfg(test)]
pub(super) const TEST: u32 = WM_APP + 183;
const VALUE: u16 = 2400;
#[derive(Clone)]
pub enum Control {
    Open,
    Add,
    Select(usize),
    Duplicate,
    Delete,
    ConfirmDelete,
    Enabled,
    Scope,
    Hyper,
    Modifier(&'static str),
    Menu(bool),
    Key(String),
    Media(String),
    Category(usize),
    Save,
    Discard,
    Cancel,
    Import,
    Export,
    Next(i32),
}
#[derive(Clone)]
pub enum Request {
    Read,
    Save { base: Library, next: Library },
    Import(PathBuf),
    Export(PathBuf, Library),
}
impl Request {
    pub fn mutates(&self) -> bool {
        !matches!(self, Self::Read)
    }
}
pub struct Data {
    library: Option<Library>,
    saved: bool,
    message: String,
}
#[derive(Default)]
pub(super) struct View {
    pub used: bool,
    pub busy: bool,
    base: Option<Library>,
    draft: Option<Library>,
    selected: usize,
    offset: usize,
    status: String,
    failed: bool,
    delete: bool,
    pending: Option<Box<Action>>,
    pub popup: bool,
}
impl View {
    fn dirty(&self) -> bool {
        self.base != self.draft
    }
    fn row(&self) -> Option<&Value> {
        self.draft.as_ref()?.records().get(self.selected)
    }
    fn row_mut(&mut self) -> Option<&mut Value> {
        self.draft.as_mut()?.envelope["mappings"]
            .as_array_mut()?
            .get_mut(self.selected)
    }
}
pub(super) fn active(state: &State) -> bool {
    state.selected.as_deref() == Some(ID)
}
pub(super) fn modal(state: &State) -> bool {
    active(state) && (state.shortcuts.pending.is_some() || state.shortcuts.delete)
}
impl State {
    pub(super) unsafe fn shortcuts_export(&self, path: PathBuf) -> Option<Request> {
        self.shortcuts
            .draft
            .clone()
            .map(|library| Request::Export(path, library))
    }
}
fn identity() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    if unsafe {
        BCryptGenRandom(
            null_mut(),
            bytes.as_mut_ptr(),
            16,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } < 0
    {
        return Err("Could not create shortcut GUID".into());
    }
    bytes[6] = (bytes[6] & 15) | 64;
    bytes[8] = (bytes[8] & 63) | 128;
    let s: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &s[..8],
        &s[8..12],
        &s[12..16],
        &s[16..20],
        &s[20..]
    ))
}
pub(super) fn work(request: Request, read_only: bool) -> Result<Data, String> {
    if read_only && request.mutates() {
        return Err("Read-only mode refuses shortcut changes".into());
    }
    let path = secbird_core::config_dir().join("global-shortcuts.json");
    let mut saved = false;
    let mut message = String::new();
    let library = match request {
        Request::Read => {
            saved = true;
            Some(model::load(&path).map_err(|e| e.to_string())?)
        }
        Request::Save { base, next } => {
            model::save(&path, &base, &next).map_err(|e| e.to_string())?;
            crate::global_shortcuts::changed();
            saved = true;
            message = "Saved. Enabled supported chords run while the tray is open.".into();
            Some(next)
        }
        Request::Import(path) => {
            let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
            if !file.metadata().map_err(|e| e.to_string())?.is_file() {
                return Err("Choose a regular shortcuts JSON file".into());
            }
            let mut bytes = vec![];
            file.take(model::MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            message =
                "Imported as a draft with all host triggers disabled. Save to keep it.".into();
            Some(Library::import(&bytes).map_err(|e| e.to_string())?)
        }
        Request::Export(target, library) => {
            // Export cannot overwrite an app's live configuration accidentally.
            let parent = target.parent().and_then(|p| std::fs::canonicalize(p).ok());
            let config = std::fs::canonicalize(secbird_core::config_dir()).ok();
            if parent.is_some() && parent == config {
                return Err("Export outside the live configuration folder".into());
            }
            let bytes = library.encode().map_err(|e| e.to_string())?;
            std::fs::write(target, bytes).map_err(|e| e.to_string())?;
            message = "Shortcut draft exported. Import always disables host triggers.".into();
            None
        }
    };
    Ok(Data {
        library,
        saved,
        message,
    })
}
pub(super) unsafe fn request(state: &mut State, request: Request) {
    if state.shortcuts.busy
        || ((state.read_only_smoke || state.restricted_actions) && request.mutates())
    {
        return;
    }
    let before = (state.pending, state.pending_jobs.len());
    state.queue(Job::Shortcuts(request));
    state.shortcuts.busy = before != (state.pending, state.pending_jobs.len());
}
pub(super) unsafe fn receive(state: &mut State, result: Result<Data, String>) {
    let view = &mut state.shortcuts;
    view.busy = false;
    match result {
        Ok(data) => {
            if let Some(library) = data.library {
                if data.saved || view.base.is_none() {
                    view.base = Some(library.clone());
                }
                view.draft = Some(library);
                view.selected = view.selected.min(
                    view.draft
                        .as_ref()
                        .unwrap()
                        .records()
                        .len()
                        .saturating_sub(1),
                );
            }
            view.status = data.message;
            view.failed = false;
            if data.saved {
                if let Some(action) = view.pending.take() {
                    state.action(*action);
                    return;
                }
            }
        }
        Err(error) => {
            view.status = error;
            view.failed = true;
        }
    }
    if active(state) {
        state.rebuild_fields();
        InvalidateRect(state.hwnd, null(), 0);
    }
}
/// Inline Save / Don't Save / Cancel gate also protects window/tray close.
pub(super) fn can_leave(state: &mut State, next: Action) -> bool {
    if !active(state) {
        return true;
    }
    if state.shortcuts.busy || state.shortcuts.popup {
        state.shortcuts.status = "Wait for the shortcut operation to finish".into();
        return false;
    }
    if !state.shortcuts.dirty() {
        return true;
    }
    state.shortcuts.pending = Some(Box::new(next));
    unsafe {
        state.rebuild_fields();
        InvalidateRect(state.hwnd, null(), 0);
    }
    false
}
pub(super) unsafe fn action(state: &mut State, control: Control) {
    if matches!(control, Control::Open) {
        state.shortcuts.used = true;
        state.action(Action::Device(ID.into()));
        return;
    }
    if !active(state) || state.shortcuts.busy {
        return;
    }
    if state.read_only_smoke || state.restricted_actions {
        return;
    }
    let before = state.shortcuts.draft.clone();
    let result = (|| -> Result<(), String> {
        if state.shortcuts.pending.is_some()
            && !matches!(
                control,
                Control::Save | Control::Discard | Control::Cancel | Control::Export
            )
        {
            return Ok(());
        }
        match control {
            Control::Add => {
                let id = identity()?;
                state
                    .shortcuts
                    .draft
                    .as_mut()
                    .ok_or("Reload shortcuts first")?
                    .push(model::create(&id))
                    .map_err(|e| e.to_string())?;
                state.shortcuts.selected =
                    state.shortcuts.draft.as_ref().unwrap().records().len() - 1;
            }
            Control::Select(index) => {
                state.shortcuts.selected = index;
                state.shortcuts.delete = false;
            }
            Control::Duplicate => {
                let id = state
                    .shortcuts
                    .row()
                    .and_then(|r| r["guid"].as_str())
                    .ok_or("Select a shortcut")?
                    .to_owned();
                let guid = identity()?;
                state
                    .shortcuts
                    .draft
                    .as_mut()
                    .unwrap()
                    .duplicate(&id, &guid)
                    .map_err(|e| e.to_string())?;
                state.shortcuts.selected =
                    state.shortcuts.draft.as_ref().unwrap().records().len() - 1;
            }
            Control::Delete => state.shortcuts.delete = true,
            Control::ConfirmDelete => {
                let id = state
                    .shortcuts
                    .row()
                    .and_then(|r| r["guid"].as_str())
                    .ok_or("Select a shortcut")?
                    .to_owned();
                state.shortcuts.draft.as_mut().unwrap().remove(&id);
                state.shortcuts.selected = state.shortcuts.selected.saturating_sub(1);
                state.shortcuts.delete = false;
            }
            Control::Enabled => {
                let row = state.shortcuts.row_mut().ok_or("Select a shortcut")?;
                let next = !model::enabled(row);
                if next {
                    if let Some(reason) = model::blocked(row) {
                        return Err(reason);
                    }
                }
                model::set_enabled(row, next).map_err(|e| e.to_string())?;
            }
            Control::Scope => {
                let row = state.shortcuts.row_mut().ok_or("Select a shortcut")?;
                if row.get(model::HOST).is_none() {
                    row[model::HOST] = json!({"version":1,"enabled":false});
                }
                if row[model::HOST]["version"] != 1 {
                    return Err("Unknown local host preferences retained".into());
                }
                row[model::HOST]["scope"] = json!("windows-chord");
                model::set_enabled(row, false).map_err(|e| e.to_string())?;
            }
            Control::Hyper => {
                let row = state.shortcuts.row_mut().ok_or("Select a shortcut")?;
                row["isHyperShift"] = json!(row["isHyperShift"] != true);
                model::set_enabled(row, false).map_err(|e| e.to_string())?;
            }
            Control::Modifier(modifier) => {
                let row = state.shortcuts.row_mut().ok_or("Select a shortcut")?;
                let mut mods = row["inputModifiers"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                if mods.iter().any(|v| modifier_matches(v, modifier)) {
                    mods.retain(|v| !modifier_matches(v, modifier));
                } else {
                    mods.push(json!(modifier));
                }
                row["inputModifiers"] = json!(mods);
                model::set_enabled(row, false).map_err(|e| e.to_string())?;
            }
            Control::Key(key) => {
                let row = state.shortcuts.row_mut().ok_or("Select a shortcut")?;
                row["inputType"] = json!(if model::key_vk(&key).is_some() {
                    "KeyInput"
                } else {
                    "MouseInput"
                });
                row["inputID"] = json!(key);
                for key in ["keyInput", "mouseInput", "keyValue", "hasWarning"] {
                    row.as_object_mut().unwrap().remove(key);
                }
                model::set_enabled(row, false).map_err(|e| e.to_string())?;
            }
            Control::Media(media) => {
                let row = state.shortcuts.row_mut().ok_or("Select a shortcut")?;
                row["outputType"] = json!("multimediaGroup");
                if !row["multimediaGroup"].is_object() {
                    row["multimediaGroup"] = json!({});
                }
                row["multimediaGroup"]["multimediaAssignment"] = json!(media);
                row["multimediaGroup"]["useTurbo"] = json!(false);
                model::set_enabled(row, false).map_err(|e| e.to_string())?;
            }
            Control::Category(index) => {
                if index < 2 {
                    return Err("Inter-device assignments are retained through import; editing needs proved target eligibility and routing".into());
                }
                let (category, group) = model::CATEGORIES
                    .get(index)
                    .ok_or("Unsupported shortcut category")?;
                let row = state.shortcuts.row_mut().ok_or("Select a shortcut")?;
                row["outputType"] = json!(group);
                if !row[group].is_object() {
                    row[group] = match *category {
                        "SWITCH_LIGHTING" => json!({"ID":""}),
                        "MACRO" => json!({"guid":""}),
                        "TEXT_FUNCTION" => json!({"text":""}),
                        "LAUNCH_PROGRAM" => json!({"path":""}),
                        "MULTIMEDIA" => json!({"multimediaAssignment":"Play"}),
                        "WINDOWS_SHORTCUT" => json!({"windowsShortcutAssignment":""}),
                        _ => unreachable!(),
                    };
                }
                // Profile/sensitivity share an outer group; do not reclassify a
                // retained inter-device assignment using guessed device selectors.
                model::set_enabled(row, false).map_err(|e| e.to_string())?;
            }
            Control::Menu(assignment) => {
                PostMessageW(state.hwnd, MENU, assignment as usize, 0);
            }
            Control::Save => {
                let base = state
                    .shortcuts
                    .base
                    .clone()
                    .ok_or("Reload shortcuts first")?;
                let next = state
                    .shortcuts
                    .draft
                    .clone()
                    .ok_or("Reload shortcuts first")?;
                next.encode().map_err(|e| e.to_string())?;
                request(state, Request::Save { base, next });
            }
            Control::Discard => {
                state.shortcuts.draft = state.shortcuts.base.clone();
                state.shortcuts.delete = false;
                state.shortcuts.status = "Unsaved shortcut changes discarded".into();
                state.shortcuts.failed = false;
                if let Some(next) = state.shortcuts.pending.take() {
                    state.action(*next);
                    return Ok(());
                }
                request(state, Request::Read);
            }
            Control::Cancel => {
                state.shortcuts.pending = None;
                state.shortcuts.delete = false;
                state.closing = false;
                state.exit_tray = None;
            }
            Control::Import => {
                if state.shortcuts.dirty() {
                    state.shortcuts.status =
                        "Save or discard changes before importing another shortcut collection"
                            .into();
                    return Ok(());
                }
                PostMessageW(state.hwnd, FILE, 0, 0);
            }
            Control::Export => {
                PostMessageW(state.hwnd, FILE, 1, 0);
            }
            Control::Next(delta) => {
                let count = state
                    .shortcuts
                    .draft
                    .as_ref()
                    .map_or(0, |l| l.records().len());
                state.shortcuts.offset = (state.shortcuts.offset as i32 + delta).max(0) as usize;
                state.shortcuts.offset = state.shortcuts.offset.min(count.saturating_sub(1));
            }
            Control::Open => {}
        }
        if let Some(draft) = &state.shortcuts.draft {
            draft.encode().map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        state.shortcuts.draft = before;
        state.shortcuts.status = error;
        state.shortcuts.failed = true;
    }
    state.rebuild_fields();
    InvalidateRect(state.hwnd, null(), 0);
}
fn editable(row: &Value) -> Option<(&'static str, &'static str)> {
    match row["outputType"].as_str()? {
        "textBlockGroup" => Some(("textBlockGroup", "text")),
        "launchGroup" => Some((
            "launchGroup",
            if row["launchGroup"].get("url").is_some() {
                "url"
            } else {
                "path"
            },
        )),
        "macroGroup" => Some(("macroGroup", "guid")),
        "lightPacGroup" => Some(("lightPacGroup", "ID")),
        "win8ShortcutsGroup" => Some(("win8ShortcutsGroup", "windowsShortcutAssignment")),
        _ => None,
    }
}
fn modifier_matches(value: &Value, modifier: &str) -> bool {
    value == modifier
        || value.as_str() == Some(modifier.replace("LEFT", "RIGHT").as_str())
        || (modifier == "KEY_LEFT_GUI" && value == "KEY_WINDOWS")
}
pub(super) unsafe fn fields(state: &mut State) {
    if state.shortcuts.pending.is_some() || state.shortcuts.delete {
        return;
    }
    let value = state.shortcuts.row().and_then(|r| {
        let (g, k) = editable(r)?;
        Some(r[g][k].as_str().unwrap_or_default().to_owned())
    });
    if let Some(value) = value {
        state.retained_fields.remove(&VALUE);
        state.field(VALUE, &value, 485, 443, 445);
        if let Some(&hwnd) = state.fields.get(&VALUE) {
            SendMessageW(
                hwnd,
                windows_sys::Win32::UI::Controls::EM_SETLIMITTEXT,
                16 * 1024,
                0,
            );
            SendMessageW(
                hwnd,
                EM_SETREADONLY,
                (state.read_only_smoke || state.restricted_actions || state.shortcuts.busy)
                    as usize,
                0,
            );
        }
    }
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if !active(state) || id != VALUE || notification != EN_CHANGE as u16 {
        return false;
    }
    if state.shortcuts.busy || state.read_only_smoke || state.restricted_actions {
        return true;
    }
    let text = unsafe {
        let hwnd = state.fields.get(&id).copied().unwrap_or(null_mut());
        let length = GetWindowTextLengthW(hwnd).clamp(0, 16 * 1024) as usize;
        let mut buffer = vec![0u16; length + 1];
        let count = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..count.max(0) as usize])
    };
    let before = state.shortcuts.draft.clone();
    let mut error = None;
    if let Some(row) = state.shortcuts.row_mut() {
        if let Some((g, k)) = editable(row) {
            row[g][k] = json!(text);
            error = model::set_enabled(row, false).err().map(|e| e.to_string());
        }
    }
    error = error.or_else(|| {
        state
            .shortcuts
            .draft
            .as_ref()
            .and_then(|library| library.encode().err())
            .map(|e| e.to_string())
    });
    if let Some(error) = error {
        state.shortcuts.draft = before;
        state.shortcuts.status = error;
        state.shortcuts.failed = true;
        unsafe {
            state.rebuild_fields();
        }
    }
    unsafe {
        InvalidateRect(state.hwnd, null(), 0);
    }
    true
}
unsafe fn button(
    p: &mut Painter,
    text: &str,
    x: i32,
    y: i32,
    width: i32,
    control: Control,
    enabled: bool,
) {
    if enabled {
        p.button(text, x, y, width, Action::Shortcuts(control), false);
    } else {
        p.fill(x, y, width, 28, 0x242424);
        p.text(text, x, y, width, 28, p.fonts.normal, MUTED, DT_CENTER);
    }
}
// The shared button-label painter deliberately uses DT_SINGLELINE. Explanations
// on this page need wrapping without changing other source-derived pages.
unsafe fn paragraph(
    p: &Painter,
    text: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    font: HFONT,
    color: u32,
    align: u32,
) {
    let previous = SelectObject(p.dc, font);
    SetBkMode(p.dc, TRANSPARENT as i32);
    SetTextColor(p.dc, rgb(color));
    let mut bounds = p.rect(x, y, width, height);
    DrawTextW(
        p.dc,
        wide(text).as_ptr(),
        -1,
        &mut bounds,
        align | DT_WORDBREAK | DT_NOPREFIX,
    );
    SelectObject(p.dc, previous);
}
pub(super) unsafe fn paint(state: &State, p: &mut Painter, width: i32, height: i32) {
    let view = &state.shortcuts;
    let can = !view.busy && !state.read_only_smoke && !state.restricted_actions;
    let ready = view.draft.is_some();
    if view
        .draft
        .as_ref()
        .is_some_and(|library| library.records().is_empty())
        && view.pending.is_none()
    {
        let x = (width - 600) / 2;
        p.fill(x, 148, 600, 240, PANEL);
        p.text(
            "SHORTCUTS",
            x + 20,
            168,
            560,
            30,
            p.fonts.normal,
            GREEN,
            DT_LEFT,
        );
        paragraph(p,"Global shortcuts work across device profiles. Supported Windows chords use all keyboards; device-specific and Hypershift inputs remain unavailable.",x+20,207,560,60,p.fonts.normal,TEXT,DT_LEFT);
        button(p, "ADD A SHORTCUT", x + 20, 295, 560, Control::Add, can);
        button(p, "IMPORT", x + 20, 342, 120, Control::Import, can);
        button(p, "EXPORT", x + 155, 342, 120, Control::Export, can);
        paragraph(
            p,
            &view.status,
            30,
            height - 45,
            width - 60,
            40,
            p.fonts.small,
            if view.failed { 0xfd4949 } else { MUTED },
            DT_LEFT | DT_WORDBREAK,
        );
        return;
    }
    p.text(
        "GLOBAL SHORTCUTS",
        30,
        77,
        350,
        42,
        p.fonts.heading,
        GREEN,
        DT_LEFT,
    );
    button(p, "ADD", 30, 135, 90, Control::Add, can && ready);
    button(p, "IMPORT", 130, 135, 100, Control::Import, can);
    button(p, "EXPORT", 240, 135, 100, Control::Export, can && ready);
    button(
        p,
        "SAVE",
        width - 130,
        135,
        100,
        Control::Save,
        can && view.dirty(),
    );
    button(
        p,
        "DON'T SAVE",
        width - 270,
        135,
        130,
        Control::Discard,
        can && ready,
    );
    let count = ((height - 275).max(0) / 87) as usize;
    if let Some(library) = &view.draft {
        for (i, row) in library
            .records()
            .iter()
            .enumerate()
            .skip(view.offset)
            .take(count)
        {
            let y = 180 + (i - view.offset) as i32 * 87;
            let selected = i == view.selected;
            p.fill(30, y, 400, 79, if selected { 0x343434 } else { PANEL });
            p.text(
                &model::assignment_label(row),
                42,
                y + 4,
                375,
                27,
                p.fonts.normal,
                if selected { GREEN } else { TEXT },
                DT_LEFT,
            );
            p.text(
                &model::trigger_label(row),
                42,
                y + 31,
                375,
                25,
                p.fonts.small,
                TEXT,
                DT_LEFT,
            );
            let conflict = library
                .records()
                .iter()
                .any(|other| other["guid"] != row["guid"] && model::source_conflict(row, other));
            p.text(
                if conflict {
                    "Trigger conflict"
                } else if model::enabled(row) {
                    "Enabled"
                } else {
                    "Disabled"
                },
                42,
                y + 55,
                375,
                20,
                p.fonts.small,
                if conflict { 0xfd4949 } else { MUTED },
                DT_LEFT,
            );
            if can {
                p.hits.push(Hit {
                    rect: p.rect(30, y, 400, 79),
                    action: Action::Shortcuts(Control::Select(i)),
                });
            }
        }
    }
    button(
        p,
        "PREVIOUS",
        30,
        height - 85,
        120,
        Control::Next(-1),
        can && view.offset > 0,
    );
    button(
        p,
        "NEXT",
        160,
        height - 85,
        90,
        Control::Next(1),
        can && view
            .draft
            .as_ref()
            .is_some_and(|l| view.offset + count < l.records().len()),
    );
    if let Some(row) = view.row() {
        p.text(
            "EDIT SHORTCUT",
            485,
            182,
            width - 515,
            32,
            p.fonts.heading,
            TEXT,
            DT_LEFT,
        );
        button(
            p,
            if model::enabled(row) {
                "ENABLED"
            } else {
                "DISABLED"
            },
            485,
            225,
            130,
            Control::Enabled,
            can,
        );
        button(p, "DUPLICATE", 625, 225, 135, Control::Duplicate, can);
        button(p, "DELETE", 770, 225, 100, Control::Delete, can);
        button(
            p,
            &format!("TRIGGER: {}", row["inputID"].as_str().unwrap_or("Choose")),
            485,
            271,
            385,
            Control::Menu(false),
            can,
        );
        for (offset, (label, id)) in [
            ("CTRL", "KEY_LEFT_CTRL"),
            ("SHIFT", "KEY_LEFT_SHIFT"),
            ("ALT", "KEY_LEFT_ALT"),
            ("WIN", "KEY_LEFT_GUI"),
        ]
        .into_iter()
        .enumerate()
        {
            let checked = row["inputModifiers"]
                .as_array()
                .is_some_and(|m| m.iter().any(|v| modifier_matches(v, id)));
            button(
                p,
                &format!("{} {label}", if checked { "[x]" } else { "[ ]" }),
                485 + offset as i32 * 96,
                309,
                88,
                Control::Modifier(id),
                can,
            );
        }
        button(
            p,
            if row["isHyperShift"] == true {
                "[x] HYPERSHIFT"
            } else {
                "[ ] HYPERSHIFT"
            },
            485,
            347,
            190,
            Control::Hyper,
            can,
        );
        button(p, "ASSIGNMENT...", 485, 390, 250, Control::Menu(true), can);
        if let Some((group, key)) = editable(row) {
            let label = match (group, key) {
                ("textBlockGroup", _) => "TEXT (RETAINED)",
                ("launchGroup", "url") => "WEBSITE URL (RETAINED)",
                ("launchGroup", _) => "PROGRAM PATH (RETAINED)",
                ("macroGroup", _) => "MACRO REFERENCE ID (RETAINED)",
                ("lightPacGroup", _) => "LIGHTING REFERENCE ID (RETAINED)",
                _ => "ASSIGNMENT ID (RETAINED)",
            };
            p.text(
                label,
                485,
                422,
                width - 515,
                18,
                p.fonts.small,
                MUTED,
                DT_LEFT,
            );
        }
        paragraph(
            p,
            &model::assignment_label(row),
            485,
            478,
            width - 515,
            48,
            p.fonts.normal,
            TEXT,
            DT_LEFT | DT_WORDBREAK,
        );
        button(p, "USE WINDOWS CHORD", 485, 523, 250, Control::Scope, can);
        paragraph(p,"Windows chords use either side of each modifier and all keyboards. Editing a trigger or assignment disables it until you enable it again. Save applies registrations while the tray is running.",485,562,width-515,62,p.fonts.small,MUTED,DT_LEFT);
        let runtime = crate::global_shortcuts::status();
        let id = row["guid"].as_str().unwrap_or_default();
        let status = if view.dirty() {
            "Unsaved draft; saved registrations remain active until Save.".into()
        } else {
            model::blocked(row).unwrap_or_else(|| {
                runtime.get(id).cloned().unwrap_or_else(|| {
                    "Supported multimedia command. Start the tray to register enabled chords."
                        .into()
                })
            })
        };
        paragraph(
            p,
            &status,
            485,
            630,
            width - 515,
            60,
            p.fonts.small,
            MUTED,
            DT_LEFT | DT_WORDBREAK,
        );
    } else {
        paragraph(
            p,
            "Add a shortcut or import a source synapseGlobalShortcuts JSON envelope.",
            485,
            240,
            width - 515,
            80,
            p.fonts.normal,
            MUTED,
            DT_LEFT | DT_WORDBREAK,
        );
    }
    paragraph(
        p,
        &view.status,
        30,
        height - 45,
        width - 60,
        40,
        p.fonts.small,
        if view.failed { 0xfd4949 } else { MUTED },
        DT_LEFT | DT_WORDBREAK,
    );
    if view.pending.is_some() || view.delete {
        let x = (width - 520) / 2;
        let y = (height - 200) / 2;
        p.fill(x, y, 520, 200, PANEL);
        p.text(
            if view.delete {
                "Delete this shortcut?"
            } else {
                "Save Changes?"
            },
            x + 20,
            y + 12,
            480,
            42,
            p.fonts.heading,
            TEXT,
            DT_LEFT,
        );
        p.text(
            if view.delete {
                "Deletion removes this assignment when you save."
            } else {
                "Your shortcut changes have not been saved."
            },
            x + 20,
            y + 62,
            480,
            45,
            p.fonts.normal,
            TEXT,
            DT_LEFT,
        );
        // Underlying controls are fully occluded for input, including child edits.
        p.hits.clear();
        button(p, "CANCEL", x + 20, y + 140, 130, Control::Cancel, can);
        button(
            p,
            if view.delete { "DELETE" } else { "DON'T SAVE" },
            x + 170,
            y + 140,
            150,
            if view.delete {
                Control::ConfirmDelete
            } else {
                Control::Discard
            },
            can,
        );
        if !view.delete {
            button(p, "SAVE", x + 340, y + 140, 150, Control::Save, can);
        }
    }
}
struct Menu(HMENU);
impl Drop for Menu {
    fn drop(&mut self) {
        unsafe {
            DestroyMenu(self.0);
        }
    }
}
pub(super) unsafe fn menu(hwnd: HWND, assignment: bool) -> Result<Option<Control>, String> {
    let root = Menu(CreatePopupMenu());
    if root.0.is_null() {
        return Err("Could not open shortcut choices".into());
    }
    let mut choices = vec![];
    let mut append = |label: &str, control: Control| -> Result<(), String> {
        choices.push(control);
        if AppendMenuW(root.0, MF_STRING, choices.len(), wide(label).as_ptr()) == 0 {
            Err("Could not populate shortcut choices".into())
        } else {
            Ok(())
        }
    };
    if assignment {
        for (id, label, _) in model::MEDIA {
            append(label, Control::Media((*id).into()))?;
        }
        for (index, (category, _)) in model::CATEGORIES.iter().enumerate() {
            append(&category.replace('_', " "), Control::Category(index))?;
        }
    } else {
        // Compact native submenus avoid requiring a 70-row screen-height popup.
        let groups: Vec<(&str, Vec<String>)> = vec![
            ("Letters", ('A'..='Z').map(|v| format!("KEY_{v}")).collect()),
            ("Numbers", ('0'..='9').map(|v| format!("KEY_{v}")).collect()),
            (
                "Function keys",
                (1..=24)
                    .filter(|v| *v != 12)
                    .map(|v| format!("KEY_F{v}"))
                    .collect(),
            ),
            (
                "Navigation",
                [
                    "SPACEBAR",
                    "ENTER",
                    "TAB",
                    "ESC",
                    "UP_ARROW",
                    "DOWN_ARROW",
                    "LEFT_ARROW",
                    "RIGHT_ARROW",
                    "HOME",
                    "END",
                    "PAGE_UP",
                    "PAGE_DOWN",
                    "INSERT",
                    "DELETE",
                ]
                .map(|k| format!("KEY_{k}"))
                .to_vec(),
            ),
            (
                "Mouse (stored only)",
                ["ScrollButton", "RightClick", "Button4", "Button5"]
                    .map(str::to_owned)
                    .to_vec(),
            ),
        ];
        for (name, keys) in groups {
            let child = Menu(CreatePopupMenu());
            if child.0.is_null() {
                return Err("Could not open trigger choices".into());
            }
            for key in keys {
                choices.push(Control::Key(key.clone()));
                if AppendMenuW(
                    child.0,
                    MF_STRING,
                    choices.len(),
                    wide(key.trim_start_matches("KEY_")).as_ptr(),
                ) == 0
                {
                    return Err("Could not populate trigger choices".into());
                }
            }
            if AppendMenuW(root.0, MF_POPUP, child.0 as usize, wide(name).as_ptr()) == 0 {
                return Err("Could not open trigger category".into());
            }
            std::mem::forget(child);
        }
    }
    let mut point: POINT = std::mem::zeroed();
    if GetCursorPos(&mut point) == 0 {
        return Err("Could not position shortcut choices".into());
    }
    let selected = TrackPopupMenu(
        root.0,
        TPM_RETURNCMD | TPM_NONOTIFY,
        point.x,
        point.y,
        0,
        hwnd,
        null(),
    ) as usize;
    Ok(selected
        .checked_sub(1)
        .and_then(|i| choices.get(i))
        .cloned())
}
pub(super) unsafe fn file(hwnd: HWND, export: bool) -> Result<Option<PathBuf>, String> {
    let mut buffer = vec![0u16; 32768];
    let filter = wide("Global Shortcuts JSON\0*.json\0\0");
    let title = wide(if export {
        "Export shortcut draft"
    } else {
        "Import Global Shortcuts (all triggers disabled)"
    });
    let extension = wide("json");
    let mut options: OPENFILENAMEW = std::mem::zeroed();
    options.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    options.hwndOwner = hwnd;
    options.lpstrFile = buffer.as_mut_ptr();
    options.nMaxFile = buffer.len() as u32;
    options.lpstrTitle = title.as_ptr();
    options.lpstrFilter = filter.as_ptr();
    options.lpstrDefExt = extension.as_ptr();
    options.Flags = OFN_NOCHANGEDIR
        | OFN_PATHMUSTEXIST
        | if export {
            OFN_OVERWRITEPROMPT
        } else {
            OFN_FILEMUSTEXIST
        };
    if (if export {
        GetSaveFileNameW(&mut options)
    } else {
        GetOpenFileNameW(&mut options)
    }) == 0
    {
        let error = CommDlgExtendedError();
        return if error == 0 {
            Ok(None)
        } else {
            Err(format!("Shortcut file picker failed ({error})"))
        };
    }
    let length = buffer
        .iter()
        .position(|v| *v == 0)
        .ok_or("Unterminated shortcut path")?;
    Ok(Some(PathBuf::from(
        String::from_utf16(&buffer[..length]).map_err(|e| e.to_string())?,
    )))
}
#[cfg(test)]
pub(super) fn diagnostics(state: &State) -> Value {
    let view = &state.shortcuts;
    json!({"open":active(state),"busy":view.busy,"dirty":view.dirty(),"records":view.draft.as_ref().map(|l|l.records()),"selected":view.selected,"pending":view.pending.is_some(),"delete":view.delete,"status":view.status,"failed":view.failed,"popup":view.popup})
}
