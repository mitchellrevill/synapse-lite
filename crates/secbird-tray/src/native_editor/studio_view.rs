//! Local composition editor. File work bypasses the device engine entirely.
use super::*;
use secbird_core::studio::{
    document::{ColorProperty, Draft, Kind, Preview, ReadSnapshot, SaveCheckpoint, SelectedColor},
    Archive,
};
use std::{collections::BTreeMap, fs::File, os::windows::ffi::OsStringExt};
use windows_sys::Win32::UI::Controls::Dialogs::*;

#[path = "studio_preview.rs"]
mod playback;
#[path = "studio_spectrum.rs"]
mod spectrum;
#[path = "studio_layers.rs"]
mod layer_tools;
pub(super) unsafe fn layer_pointer(state: &mut State, message: u32, lparam: isize) -> bool {
    layer_tools::pointer(state, message, lparam)
}
pub(super) const PREVIEW_TIMER: usize = 79;
pub(super) unsafe fn preview_tick(state: &mut State) {
    playback::tick(state);
}
pub(super) fn preview_color(state: &State, device: &str, key: &str) -> Option<u32> {
    playback::color(state, device, key)
}

pub(super) const DIALOG: u32 = WM_APP + 112;
pub(super) const EXIT_REQUEST: u32 = WM_APP + 114;
pub(super) const NAME: u16 = 2100;
pub(super) const COLOR: u16 = 2101;
pub(super) const COLD: u16 = 2102;
pub(super) const POSITION: u16 = 2103;
pub(super) const STOP_COLOR: u16 = 2104;

#[derive(Clone)]
pub(super) enum Control {
    Enter,
    Open,
    Save,
    Profiles,
    Profile(usize),
    Select(String),
    Visibility(String),
    Expand(String),
    Rename,
    CommitRename,
    CancelRename,
    Undo,
    Redo,
    Zoom(i32),
    ApplyColor,
    ApplyCold,
    ClearColor(bool),
    Spectrum(spectrum::Control),
    Layers(layer_tools::Control),
    Preview,
    Erase,
}
#[derive(Clone, Copy)]
enum Next {
    Open,
    Profile(usize),
    Close,
}

#[derive(Default)]
pub(super) struct View {
    pub(super) canvas: studio_canvas::Canvas,
    pub open: bool,
    archive: Option<Arc<Archive>>,
    titles: Vec<String>,
    draft: Option<Draft>,
    path: Option<PathBuf>,
    index: usize,
    selected: Option<String>,
    expanded: BTreeMap<String, bool>,
    renaming: bool,
    name: String,
    color: String,
    selected_color: Option<SelectedColor>,
    cold: String,
    selected_cold: Option<SelectedColor>,
    spectrum: spectrum::Properties,
    playback: playback::Playback,
    dropdown: bool,
    layer_menu: bool,
    profile_scroll: usize,
    scroll: usize,
    epoch: u64,
    busy: bool,
    dialog: bool,
    next: Option<Next>,
    status: String,
    failed: bool,
    #[cfg(test)]
    pub test_decision: Option<i32>,
}
pub(super) enum Request {
    Preview {
        epoch: u64,
        generation: u64,
        snapshot: ReadSnapshot,
        random_state: u32,
    },
    Read {
        epoch: u64,
        path: PathBuf,
    },
    Save {
        epoch: u64,
        path: PathBuf,
        index: usize,
        archive: Arc<Archive>,
        checkpoint: SaveCheckpoint,
    },
}
pub(super) enum Response {
    Preview {
        epoch: u64,
        generation: u64,
        snapshot: ReadSnapshot,
        result: Result<Box<Preview>, String>,
    },
    Read {
        epoch: u64,
        path: PathBuf,
        result: Result<Arc<Archive>, String>,
    },
    Save {
        epoch: u64,
        path: PathBuf,
        result: Result<(Arc<Archive>, SaveCheckpoint), String>,
    },
}

pub(super) fn work(request: Request) -> Response {
    match request {
        Request::Preview {
            epoch,
            generation,
            snapshot,
            random_state,
        } => {
            let result = Preview::from_snapshot(&snapshot, random_state)
                .map(Box::new)
                .map_err(|e| e.to_string());
            Response::Preview {
                epoch,
                generation,
                snapshot,
                result,
            }
        }
        Request::Read { epoch, path } => {
            let result = File::open(&path)
                .map_err(|error| error.to_string())
                .and_then(|file| {
                    Archive::read(file)
                        .map(Arc::new)
                        .map_err(|error| error.to_string())
                });
            Response::Read {
                epoch,
                path,
                result,
            }
        }
        Request::Save {
            epoch,
            path,
            index,
            archive,
            checkpoint,
        } => {
            let result = (|| {
                let bytes = archive
                    .replace_profile_bytes(index, checkpoint.json())
                    .map_err(|error| error.to_string())?;
                let replacement = Archive::parse(&bytes).map_err(|error| error.to_string())?;
                profile_files::write_studio(&path, &bytes)?;
                Ok((Arc::new(replacement), checkpoint))
            })();
            Response::Save {
                epoch,
                path,
                result,
            }
        }
    }
}

impl View {
    fn dirty(&self) -> bool {
        self.draft.as_ref().is_some_and(Draft::is_dirty)
    }
    fn editable(&self) -> bool {
        self.draft.is_some() && !self.busy && !self.dialog
    }
    fn profile(&mut self, index: usize) -> Result<(), String> {
        self.playback.clear_frame();
        let archive = self
            .archive
            .as_ref()
            .ok_or("Open a Studio composition file first")?;
        let raw = archive
            .profile_json(index)
            .ok_or("Composition is no longer available")?;
        self.index = index;
        self.epoch = self
            .epoch
            .checked_add(1)
            .ok_or("Studio session limit reached")?;
        self.draft = None;
        self.selected = None;
        self.expanded.clear();
        self.scroll = 0;
        self.dropdown = false;
        self.layer_menu = false;
        self.renaming = false;
        self.canvas = studio_canvas::Canvas::load(raw, &assets_dir());
        match Draft::open(raw) {
            Ok(draft) => {
                self.selected = draft.layers().first().map(|row| row.id.clone());
                self.draft = Some(draft);
                self.status = "Choose Local Preview to animate the canvas".into();
                self.failed = false;
            }
            Err(error) => {
                self.status = error.to_string();
                self.failed = true;
            }
        }
        Ok(())
    }
    fn visible_ids(&self) -> Vec<String> {
        let Some(draft) = &self.draft else {
            return vec![];
        };
        draft
            .layers()
            .iter()
            .filter(|row| {
                row.parent.as_ref().is_none_or(|parent| {
                    self.expanded.get(parent).copied().unwrap_or_else(|| {
                        draft
                            .layers()
                            .iter()
                            .find(|group| group.id == *parent)
                            .and_then(|group| group.opened)
                            .unwrap_or(false)
                    })
                })
            })
            .map(|row| row.id.clone())
            .collect()
    }
    fn title(&self) -> &str {
        self.titles
            .get(self.index)
            .map(String::as_str)
            .unwrap_or("Chroma Studio")
    }
    fn is_static(&self) -> bool {
        self.effect() == Some("static")
    }
    fn is_fire(&self) -> bool {
        self.effect() == Some("fire")
    }
    fn effect(&self) -> Option<&str> {
        self.draft
            .as_ref()?
            .layers()
            .iter()
            .find(|row| Some(&row.id) == self.selected.as_ref() && row.kind == Kind::Layer)?
            .effect
            .as_deref()
    }
    fn color_property(&self, cold: bool) -> Option<ColorProperty> {
        match (self.effect()?, cold) {
            ("static", false) => Some(ColorProperty::Static),
            ("fire", false) => Some(ColorProperty::FireHot),
            ("fire", true) => Some(ColorProperty::FireCold),
            _ => None,
        }
    }
}

pub(super) fn selection_changed(state: &mut State) {
    let view = &mut state.studio;
    spectrum::refresh(view);
    view.selected_color = None;
    view.selected_cold = None;
    let Some(property) = view.color_property(false) else {
        return;
    };
    let result = view.draft.as_ref().unwrap().selection_color(
        view.selected.as_deref().unwrap(),
        &view.canvas.selection(),
        property,
    );
    match result {
        Ok(value) => {
            view.selected_color = Some(value);
            view.color = match value {
                SelectedColor::Uniform(Some(color)) => format!("{color:06X}"),
                _ if property == ColorProperty::Static => "00FF00".into(),
                _ => String::new(),
            };
        }
        Err(error) => {
            view.status = error.to_string();
            view.failed = true;
        }
    }
    if view.is_fire() {
        match view.draft.as_ref().unwrap().selection_color(
            view.selected.as_deref().unwrap(),
            &view.canvas.selection(),
            ColorProperty::FireCold,
        ) {
            Ok(value) => {
                view.selected_cold = Some(value);
                view.cold = match value {
                    SelectedColor::Uniform(Some(color)) => format!("{color:06X}"),
                    _ => String::new(),
                };
            }
            Err(error) => {
                view.status = error.to_string();
                view.failed = true;
            }
        }
    }
}
fn error(state: &mut State, message: impl Into<String>) {
    state.studio.status = message.into();
    state.studio.failed = true;
}
unsafe fn redraw(state: &mut State) {
    state.rebuild_fields();
    InvalidateRect(state.hwnd, null(), 0);
}
fn cancel_exit(state: &mut State) {
    state.exit_tray = None;
    state.closing = false;
}

pub(super) unsafe fn read_path(state: &mut State, path: PathBuf) {
    let Some(epoch) = state.studio.epoch.checked_add(1) else {
        error(state, "Studio session limit reached");
        return;
    };
    state.studio.epoch = epoch;
    state.studio.busy = true;
    state.studio.status = "Opening composition file…".into();
    let before = (state.pending, state.pending_jobs.len());
    state.queue(Job::Studio(Request::Read { epoch, path }));
    if before == (state.pending, state.pending_jobs.len()) {
        state.studio.busy = false;
        state.studio.next = None;
        cancel_exit(state);
        error(state, "Studio file operation was not accepted");
    }
}
pub(super) unsafe fn save_path(state: &mut State, path: PathBuf) {
    let Some(archive) = state.studio.archive.clone() else {
        return;
    };
    let Some(draft) = &state.studio.draft else {
        return;
    };
    let checkpoint = draft.checkpoint();
    let request = Request::Save {
        epoch: state.studio.epoch,
        path,
        index: state.studio.index,
        archive,
        checkpoint,
    };
    state.studio.busy = true;
    state.studio.status = "Saving composition file…".into();
    let before = (state.pending, state.pending_jobs.len());
    state.queue(Job::Studio(request));
    if before == (state.pending, state.pending_jobs.len()) {
        state.studio.busy = false;
        state.studio.next = None;
        cancel_exit(state);
        error(state, "Studio save was not accepted");
    }
}

unsafe fn proceed(state: &mut State, next: Next) {
    match next {
        Next::Open => {
            PostMessageW(state.hwnd, DIALOG, 0, 0);
        }
        Next::Profile(index) => {
            if let Err(message) = state.studio.profile(index) {
                error(state, message);
            }
            selection_changed(state);
        }
        Next::Close => {
            PostMessageW(state.hwnd, WM_CLOSE, 0, 0);
        }
    }
}
unsafe fn transition(state: &mut State, next: Next) {
    if state.studio.dirty() {
        state.studio.next = Some(next);
        PostMessageW(state.hwnd, DIALOG, 2, 0);
    } else {
        proceed(state, next);
    }
}
fn finish_name(state: &mut State) -> bool {
    if !state.studio.renaming {
        return true;
    }
    let result = match (&mut state.studio.draft, &state.studio.selected) {
        (Some(draft), Some(id)) => draft
            .rename_layer(id, &state.studio.name)
            .map_err(|error| error.to_string()),
        _ => Err("Selected layer is no longer available".into()),
    };
    match result {
        Ok(_) => {
            state.studio.renaming = false;
            true
        }
        Err(message) => {
            error(state, message);
            false
        }
    }
}
pub(super) unsafe fn block_close(state: &mut State) -> bool {
    if !finish_name(state) {
        state.studio.open = true;
        state.selected = None;
        redraw(state);
        cancel_exit(state);
        return true;
    }
    if state.studio.busy || state.studio.dialog {
        state.studio.next = Some(Next::Close);
        return true;
    }
    if state.studio.dirty() {
        transition(state, Next::Close);
        return true;
    }
    false
}

pub(super) unsafe fn action(state: &mut State, control: Control) {
    if matches!(control, Control::Enter) {
        state.studio.open = true;
        state.selected = None;
        state.profile_open = false;
        state.profile_actions = false;
        mouse_feedback::stop(state);
        state.sync_performance(true);
        redraw(state);
        return;
    }
    if state.studio.busy || state.studio.dialog {
        return;
    }
    if !matches!(
        control,
        Control::Rename | Control::CommitRename | Control::CancelRename
    ) && !finish_name(state)
    {
        redraw(state);
        return;
    }
    let update_color = matches!(
        &control,
        Control::Select(_)
            | Control::Undo
            | Control::Redo
            | Control::ApplyColor
            | Control::ApplyCold
            | Control::ClearColor(_)
            | Control::Erase
    );
    if !matches!(control, Control::Layers(_)) { state.studio.layer_menu = false; }
    match control {
        Control::Enter => {}
        Control::Open => transition(state, Next::Open),
        Control::Save => {
            if finish_name(state) && state.studio.draft.is_some() {
                PostMessageW(state.hwnd, DIALOG, 1, 0);
            }
        }
        Control::Profiles => state.studio.dropdown = !state.studio.dropdown,
        Control::Profile(index) => transition(state, Next::Profile(index)),
        Control::Select(id) => {
            if state.studio.selected.as_ref() != Some(&id) {
                state.studio.canvas.clear_selection();
                ReleaseCapture();
            }
            state.studio.selected = Some(id);
            state.studio.renaming = false;
        }
        Control::Expand(id) => {
            let old = state
                .studio
                .expanded
                .get(&id)
                .copied()
                .or_else(|| {
                    state
                        .studio
                        .draft
                        .as_ref()
                        .and_then(|draft| draft.layers().iter().find(|row| row.id == id))
                        .and_then(|row| row.opened)
                })
                .unwrap_or(false);
            state.studio.expanded.insert(id, !old);
        }
        Control::Rename => {
            if let Some(row) = state.studio.draft.as_ref().and_then(|draft| {
                draft
                    .layers()
                    .iter()
                    .find(|row| Some(&row.id) == state.studio.selected.as_ref())
            }) {
                state.studio.name = row.title.clone();
                state.studio.renaming = true;
            }
        }
        Control::CommitRename => {
            finish_name(state);
        }
        Control::CancelRename => state.studio.renaming = false,
        Control::Preview => playback::toggle(state),
        Control::Spectrum(control) => spectrum::action(state, control),
        Control::Layers(control) => {
            layer_tools::action(state, control);
            ReleaseCapture();
        }
        Control::ApplyColor | Control::ApplyCold | Control::ClearColor(_) | Control::Erase => {
            let erase = matches!(control, Control::Erase);
            let cold = matches!(control, Control::ApplyCold | Control::ClearColor(true));
            let clear = matches!(control, Control::ClearColor(_));
            let property = state.studio.color_property(cold);
            let selection = state.studio.canvas.selection();
            let result = (|| -> Result<bool, String> {
                let id = state
                    .studio
                    .selected
                    .as_deref()
                    .ok_or("Select an effect layer")?;
                let draft = state
                    .studio
                    .draft
                    .as_mut()
                    .ok_or("Open a composition first")?;
                if erase {
                    draft.erase_leds(id, &selection).map_err(|e| e.to_string())
                } else {
                    let text = if cold {
                        &state.studio.cold
                    } else {
                        &state.studio.color
                    };
                    let text = text.trim().trim_start_matches('#');
                    let color = if clear {
                        None
                    } else {
                        if text.len() != 6 {
                            return Err("Enter a six-digit RGB colour".into());
                        }
                        Some(
                            u32::from_str_radix(text, 16)
                                .map_err(|_| "Enter a six-digit RGB colour".to_string())?,
                        )
                    };
                    draft
                        .set_selection_color(
                            id,
                            &selection,
                            property.ok_or("Select a supported colour property")?,
                            color,
                        )
                        .map_err(|e| e.to_string())
                }
            })();
            match result {
                Ok(changed) => {
                    if changed {
                        state.studio.status =
                            "Selected LEDs updated · save the composition to keep changes".into();
                    }
                    state.studio.failed = false;
                }
                Err(message) => error(state, message),
            }
        }
        Control::Zoom(steps) => {
            state.studio.canvas.zoom(
                steps,
                f64::from(state.width) / f64::from(state.scale),
                f64::from(state.height) / f64::from(state.scale),
            );
            ReleaseCapture();
        }
        control => {
            let result = if let Some(draft) = &mut state.studio.draft {
                match control {
                    Control::Visibility(id) => {
                        let on = draft
                            .layers()
                            .iter()
                            .find(|row| row.id == id)
                            .and_then(|row| row.visible)
                            .unwrap_or(false);
                        draft.set_visible(&id, !on)
                    }
                    Control::Undo => draft.undo(),
                    Control::Redo => draft.redo(),
                    _ => Ok(false),
                }
                .map_err(|error| error.to_string())
            } else {
                Ok(false)
            };
            if let Err(message) = result {
                error(state, message);
            }
            layer_tools::reconcile_selection(state);
        }
    }
    if update_color {
        selection_changed(state);
    }
    redraw(state);
}

pub(super) unsafe fn receive(state: &mut State, response: Response) {
    let response = match response {
        Response::Preview {
            epoch,
            generation,
            snapshot,
            result,
        } => {
            playback::receive(state, epoch, generation, snapshot, result);
            return;
        }
        other => other,
    };
    let epoch = match &response {
        Response::Read { epoch, .. } | Response::Save { epoch, .. } => *epoch,
        Response::Preview { .. } => unreachable!(),
    };
    if epoch != state.studio.epoch {
        return;
    }
    state.studio.busy = false;
    let result = match response {
        Response::Read { path, result, .. } => result.and_then(|archive| {
            state.studio.titles = (0..archive.profile_count())
                .map(|index| {
                    archive
                        .profile_header(index)
                        .ok()
                        .and_then(|header| header.title)
                        .unwrap_or_else(|| format!("Composition {}", index + 1))
                })
                .collect();
            state.studio.archive = Some(archive);
            state.studio.path = Some(path);
            if state.studio.archive.as_ref().unwrap().profile_count() == 0 {
                state.studio.canvas = studio_canvas::Canvas::default();
                state.studio.draft = None;
                state.studio.selected = None;
                state.studio.status = "This file contains no compositions".into();
                Ok(())
            } else {
                state.studio.profile(0)
            }
        }),
        Response::Save { path, result, .. } => result.and_then(|(archive, checkpoint)| {
            state
                .studio
                .draft
                .as_mut()
                .ok_or("Composition changed before save completed")?
                .accept_saved(checkpoint)
                .map_err(|error| error.to_string())?;
            state.studio.archive = Some(archive);
            state.studio.path = Some(path);
            state.studio.status = if state.studio.playback.active() {
                "Composition saved · local preview only"
            } else {
                "Composition file saved · local preview is stopped"
            }
            .into();
            state.studio.failed = false;
            Ok(())
        }),
        Response::Preview { .. } => unreachable!(),
    };
    match result {
        Ok(()) => {
            selection_changed(state);
            if let Some(next) = state.studio.next.take() {
                proceed(state, next);
            }
        }
        Err(message) => {
            state.studio.open = true;
            state.selected = None;
            error(state, message);
            state.studio.next = None;
            cancel_exit(state);
        }
    }
    redraw(state);
}

unsafe fn choose(hwnd: HWND, save: bool, initial: &str) -> Result<Option<PathBuf>, String> {
    let filter = wide("Chroma Studio compositions\0*.ChromaEffects\0All files\0*.*\0");
    let extension = wide("ChromaEffects");
    let title = wide(if save {
        "Save composition file"
    } else {
        "Open composition file"
    });
    let mut buffer = vec![0u16; 32768];
    let initial = wide(initial);
    if initial.len() > buffer.len() {
        return Err("Studio file name is too long".into());
    }
    buffer[..initial.len()].copy_from_slice(&initial);
    let mut options: OPENFILENAMEW = std::mem::zeroed();
    options.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    options.hwndOwner = hwnd;
    options.lpstrFilter = filter.as_ptr();
    options.nFilterIndex = 1;
    options.lpstrFile = buffer.as_mut_ptr();
    options.nMaxFile = buffer.len() as u32;
    options.lpstrTitle = title.as_ptr();
    options.lpstrDefExt = extension.as_ptr();
    options.Flags = OFN_EXPLORER
        | OFN_NOCHANGEDIR
        | OFN_PATHMUSTEXIST
        | if save {
            OFN_OVERWRITEPROMPT
        } else {
            OFN_FILEMUSTEXIST
        };
    let accepted = if save {
        GetSaveFileNameW(&mut options)
    } else {
        GetOpenFileNameW(&mut options)
    };
    if accepted == 0 {
        let code = CommDlgExtendedError();
        return if code == 0 {
            Ok(None)
        } else {
            Err(format!("Windows file dialog failed: {code:#x}"))
        };
    }
    let end = buffer
        .iter()
        .position(|c| *c == 0)
        .ok_or("Unterminated Studio file path")?;
    Ok(Some(std::ffi::OsString::from_wide(&buffer[..end]).into()))
}

pub(super) unsafe fn dialog(hwnd: HWND, pointer: *mut RefCell<State>, kind: usize) {
    let epoch = {
        let Ok(mut state) = (*pointer).try_borrow_mut() else {
            return;
        };
        if state.restricted_actions || state.studio.busy || state.studio.dialog {
            return;
        }
        state.studio.dialog = true;
        state.studio.epoch
    };
    if kind == 2 {
        #[cfg(test)]
        let decision = (*pointer).borrow_mut().studio.test_decision.take();
        #[cfg(not(test))]
        let decision: Option<i32> = None;
        let choice = decision.unwrap_or_else(|| {
            MessageBoxW(
                hwnd,
                wide("Save changes to this Studio composition?").as_ptr(),
                wide("Chroma Studio").as_ptr(),
                MB_YESNOCANCEL | MB_ICONQUESTION,
            )
        });
        let mut state = (*pointer).borrow_mut();
        state.studio.dialog = false;
        if state.studio.epoch != epoch {
            cancel_exit(&mut state);
            return;
        }
        match choice {
            IDYES => {
                PostMessageW(hwnd, DIALOG, 1, 0);
            }
            IDNO => {
                if let Some(draft) = &mut state.studio.draft {
                    draft.discard();
                }
                selection_changed(&mut state);
                if let Some(next) = state.studio.next.take() {
                    proceed(&mut state, next);
                }
            }
            _ => {
                state.studio.next = None;
                cancel_exit(&mut state);
            }
        }
        redraw(&mut state);
        return;
    }
    let initial = (*pointer)
        .borrow()
        .studio
        .path
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            if kind == 1 {
                "Chroma Studio.ChromaEffects".into()
            } else {
                String::new()
            }
        });
    let chosen = choose(hwnd, kind == 1, &initial);
    let mut state = (*pointer).borrow_mut();
    state.studio.dialog = false;
    if state.studio.epoch != epoch {
        cancel_exit(&mut state);
        return;
    }
    match chosen {
        Ok(Some(path)) => {
            if kind == 1 {
                save_path(&mut state, path)
            } else {
                read_path(&mut state, path)
            }
        }
        Ok(None) => {
            state.studio.next = None;
            cancel_exit(&mut state);
        }
        Err(message) => {
            state.studio.next = None;
            cancel_exit(&mut state);
            error(&mut state, message);
        }
    }
    redraw(&mut state);
}

pub(super) unsafe fn rebuild_fields(state: &mut State) {
    if !state.studio.open {
        return;
    }
    spectrum::fields(state);
    if state.studio.color_property(false).is_some() {
        let text = state.studio.color.clone();
        let right = (state.width as f32 / state.scale) as i32 - 250;
        state.retained_fields.remove(&COLOR);
        state.field(COLOR, &text, right + 10, 285, 130);
        if let Some(&edit) = state.fields.get(&COLOR) {
            SendMessageW(edit, EM_SETLIMITTEXT, 7, 0);
            SendMessageW(edit, WM_SETFONT, state.fonts.normal as usize, 1);
        }
        if state.studio.is_fire() {
            let text = state.studio.cold.clone();
            state.retained_fields.remove(&COLD);
            state.field(COLD, &text, right + 10, 388, 130);
            if let Some(&edit) = state.fields.get(&COLD) {
                SendMessageW(edit, EM_SETLIMITTEXT, 7, 0);
                SendMessageW(edit, WM_SETFONT, state.fonts.normal as usize, 1);
            }
        }
    }
    if !state.studio.renaming {
        return;
    }
    let ids = state.studio.visible_ids();
    let Some(index) = ids
        .iter()
        .position(|id| Some(id) == state.studio.selected.as_ref())
    else {
        return;
    };
    if index < state.studio.scroll {
        return;
    }
    let y = 168 + (index - state.studio.scroll) as i32 * 40;
    if y + 32 > state.content_bottom_css() as i32 {
        return;
    }
    let text = state.studio.name.clone();
    state.retained_fields.remove(&NAME);
    state.field(NAME, &text, 38, y + 6, 165);
    if let Some(&edit) = state.fields.get(&NAME) {
        SendMessageW(edit, EM_SETLIMITTEXT, 32, 0);
        SendMessageW(edit, WM_SETFONT, state.fonts.normal as usize, 1);
    }
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if spectrum::command(state, id, notification) {
        return true;
    }
    if matches!(id, NAME | COLOR | COLD) && notification == EN_CHANGE as u16 {
        let text = state.raw_text(id);
        if id == NAME {
            state.studio.name = text;
        } else if id == COLD {
            state.studio.cold = text;
        } else {
            state.studio.color = text;
        }
        true
    } else {
        false
    }
}
pub(super) fn key(state: &State, key: usize, control: bool, edit: bool) -> Option<Control> {
    if !state.studio.open {
        return None;
    }
    match (key, control) {
        (27, _) if state.studio.layer_menu => Some(Control::Layers(layer_tools::Control::Dismiss)),
        (0x44, true) if !edit => Some(Control::Layers(layer_tools::Control::Duplicate)),
        (0x2e, false) if !edit => Some(Control::Layers(layer_tools::Control::Delete)),
        (13, _) if state.studio.renaming => Some(Control::CommitRename),
        (13, _) if edit && spectrum::has_focus(state) => {
            Some(Control::Spectrum(spectrum::Control::ApplyStop(false)))
        }
        (13, _) if edit => Some(
            if unsafe { GetFocus() } == state.fields.get(&COLD).copied().unwrap_or(null_mut()) {
                Control::ApplyCold
            } else {
                Control::ApplyColor
            },
        ),
        (27, _) if state.studio.renaming => Some(Control::CancelRename),
        (0x53, true) => Some(Control::Save),
        (0x4f, true) => Some(Control::Open),
        (0x5a, true) if !edit => Some(Control::Undo),
        (0x59, true) if !edit => Some(Control::Redo),
        _ => None,
    }
}
pub(super) unsafe fn wheel(state: &mut State, delta: i32, mut point: POINT) -> bool {
    if !state.studio.open {
        return false;
    }
    ScreenToClient(state.hwnd, &mut point);
    if state.studio.dropdown {
        let count = state
            .studio
            .archive
            .as_ref()
            .map_or(0, |archive| archive.profile_count());
        state.studio.profile_scroll = (state.studio.profile_scroll as i32
            - if delta > 0 { 1 } else { -1 })
        .clamp(0, count.saturating_sub(7) as i32) as usize;
    } else if (point.x as f32 / state.scale) < 250.0 {
        let count = state.studio.visible_ids().len();
        let visible = ((state.content_bottom_css() - 168.0) / 40.0).max(1.0) as usize;
        state.studio.scroll = (state.studio.scroll as i32 - if delta > 0 { 3 } else { -3 })
            .clamp(0, count.saturating_sub(visible) as i32) as usize;
    } else {
        if GetKeyState(17) < 0 {
            state.studio.canvas.zoom(
                if delta > 0 { 1 } else { -1 },
                f64::from(state.width) / f64::from(state.scale),
                f64::from(state.height) / f64::from(state.scale),
            );
            ReleaseCapture();
        } else {
            return false;
        }
    }
    redraw(state);
    true
}

unsafe fn button(
    p: &mut Painter,
    label: &str,
    x: i32,
    y: i32,
    width: i32,
    control: Control,
    enabled: bool,
) {
    p.fill(x, y, width, 27, if enabled { 0x303030 } else { 0x252525 });
    p.text(
        label,
        x + 4,
        y,
        width - 8,
        27,
        p.fonts.small,
        if enabled { TEXT } else { 0x555555 },
        DT_CENTER,
    );
    if enabled {
        p.hits.push(Hit {
            rect: p.rect(x, y, width, 27),
            action: Action::Studio(control),
        });
    }
}
unsafe fn paragraph(p: &Painter, text: &str, x: i32, y: i32, width: i32, height: i32, color: u32) {
    let previous = SelectObject(p.dc, p.fonts.normal);
    SetBkMode(p.dc, TRANSPARENT as i32);
    SetTextColor(p.dc, rgb(color));
    let mut rect = p.rect(x, y, width, height);
    DrawTextW(
        p.dc,
        wide(text).as_ptr(),
        -1,
        &mut rect,
        DT_LEFT | DT_WORDBREAK | DT_NOPREFIX,
    );
    SelectObject(p.dc, previous);
}

pub(super) unsafe fn paint(state: &State, p: &mut Painter, width: i32, height: i32) {
    let view = &state.studio;
    let right = width - 250;
    p.fill(0, 80, width, height - 80, BG);
    p.fill(0, 128, 250, height - 128, 0x181818);
    p.fill(right, 128, 250, height - 128, PANEL);
    p.fill(0, 127, width, 1, 0);
    p.fill(250, 128, 1, height - 128, 0);
    p.fill(right, 128, 1, height - 128, 0);
    studio_canvas::paint(state, p, width, height);
    p.text(
        &format!("{}{}", view.title(), if view.dirty() { " *" } else { "" }),
        46,
        87,
        222,
        28,
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    if view.archive.is_some() {
        p.text("▾", 254, 87, 20, 28, p.fonts.normal, TEXT, DT_CENTER);
        p.hits.push(Hit {
            rect: p.rect(46, 87, 230, 28),
            action: Action::Studio(Control::Profiles),
        });
    }
    button(
        p,
        "OPEN FILE",
        300,
        87,
        95,
        Control::Open,
        !view.busy && !view.dialog,
    );
    button(
        p,
        "UNDO",
        410,
        87,
        65,
        Control::Undo,
        view.editable()
            && view
                .draft
                .as_ref()
                .is_some_and(|draft| draft.undo_len() > 0),
    );
    button(
        p,
        "REDO",
        485,
        87,
        65,
        Control::Redo,
        view.editable()
            && view
                .draft
                .as_ref()
                .is_some_and(|draft| draft.redo_len() > 0),
    );
    button(
        p,
        if view.playback.active() {
            "STOP PREVIEW"
        } else {
            "LOCAL PREVIEW"
        },
        565,
        87,
        130,
        Control::Preview,
        view.editable(),
    );
    p.text(
        "EFFECT LAYERS",
        10,
        128,
        230,
        40,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    p.text(
        "STUDIO",
        270,
        128,
        (right - 285).max(1),
        40,
        p.fonts.normal,
        GREEN,
        DT_LEFT,
    );
    for (label, x, width, step) in [
        ("−", right - 170, 35, -1),
        ("FIT", right - 130, 65, 0),
        ("+", right - 60, 35, 1),
    ] {
        button(
            p,
            label,
            x,
            135,
            width,
            Control::Zoom(step),
            !view.busy && !view.dialog,
        );
    }
    let clip = SaveDC(p.dc);
    IntersectClipRect(p.dc, 0, p.px(168), p.px(250), state.height);
    let ids = view.visible_ids();
    if let Some(draft) = &view.draft {
        for (index, id) in ids.iter().enumerate().skip(view.scroll) {
            let y = 168 + (index - view.scroll) as i32 * 40;
            if y >= height {
                break;
            }
            let row = draft.layers().iter().find(|row| row.id == *id).unwrap();
            let selected = view.selected.as_deref() == Some(&row.id);
            if selected {
                p.fill(0, y, 250, 40, 0x353535);
            }
            if row.kind == Kind::Group {
                let opened = view
                    .expanded
                    .get(id)
                    .copied()
                    .or(row.opened)
                    .unwrap_or(false);
                p.text(
                    if opened { "−" } else { "+" },
                    4,
                    y,
                    24,
                    40,
                    p.fonts.normal,
                    MUTED,
                    DT_CENTER,
                );
                p.hits.push(Hit {
                    rect: p.rect(0, y, 27, 40),
                    action: Action::Studio(Control::Expand(id.clone())),
                });
            }
            let x = if row.parent.is_some() { 35 } else { 28 };
            if !(selected && view.renaming) {
                p.text(layer_tools::display_title(row), x, y, 180 - x, 40, p.fonts.normal, TEXT, DT_LEFT);
            }
            p.hits.push(Hit {
                rect: p.rect(28, y, 152, 40),
                action: Action::Studio(Control::Select(id.clone())),
            });
            button(
                p,
                if row.visible == Some(true) {
                    "HIDE"
                } else {
                    "SHOW"
                },
                188,
                y + 6,
                54,
                Control::Visibility(id.clone()),
                view.editable(),
            );
        }
    }
    RestoreDC(p.dc, clip);
    let selected = view.draft.as_ref().and_then(|draft| {
        draft
            .layers()
            .iter()
            .find(|row| Some(&row.id) == view.selected.as_ref())
    });
    p.text(
        selected
            .map(layer_tools::display_title)
            .unwrap_or("PROPERTIES"),
        right + 10,
        128,
        230,
        40,
        p.fonts.normal,
        GREEN,
        DT_LEFT,
    );
    button(
        p,
        if view.renaming {
            "APPLY NAME"
        } else {
            "RENAME LAYER"
        },
        right + 10,
        180,
        150,
        if view.renaming {
            Control::CommitRename
        } else {
            Control::Rename
        },
        view.editable() && selected.is_some(),
    );
    if view.renaming {
        button(
            p,
            "CANCEL",
            right + 10,
            220,
            100,
            Control::CancelRename,
            true,
        );
    }
    if view.is_static() {
        p.text(
            match view.selected_color {
                Some(SelectedColor::Mixed) => "COLOUR · MIXED",
                Some(SelectedColor::Uniform(None)) => "COLOUR · UNASSIGNED",
                Some(SelectedColor::Empty) => "SELECT LEDS TO COLOUR",
                _ => "COLOUR",
            },
            right + 10,
            246,
            230,
            32,
            p.fonts.small,
            TEXT,
            DT_LEFT,
        );
        if let Ok(color) = u32::from_str_radix(view.color.trim().trim_start_matches('#'), 16) {
            p.fill(right + 160, 285, 55, 27, color & 0xffffff);
        }
        let enabled = view.editable() && view.canvas.count() > 0 && view.selected_color.is_some();
        button(
            p,
            "APPLY TO SELECTION",
            right + 10,
            330,
            200,
            Control::ApplyColor,
            enabled,
        );
        button(
            p,
            "ERASE SELECTION",
            right + 10,
            368,
            200,
            Control::Erase,
            enabled,
        );
    } else if view.is_fire() {
        for (cold, title, value, text, y) in [
            (false, "HOT", view.selected_color, &view.color, 246),
            (true, "COLD", view.selected_cold, &view.cold, 349),
        ] {
            let label = match value {
                Some(SelectedColor::Mixed) => format!("{title} · MIXED"),
                Some(SelectedColor::Uniform(None)) => format!("{title} · NONE / UNASSIGNED"),
                Some(SelectedColor::Empty) => format!("{title} · SELECT LEDS"),
                _ => title.into(),
            };
            p.text(&label, right + 10, y, 230, 32, p.fonts.small, TEXT, DT_LEFT);
            if let Ok(color) = u32::from_str_radix(text.trim().trim_start_matches('#'), 16) {
                p.fill(right + 160, y + 39, 55, 27, color & 0xffffff);
            }
            let enabled = view.editable() && view.canvas.count() > 0 && value.is_some();
            button(
                p,
                if cold { "APPLY COLD" } else { "APPLY HOT" },
                right + 10,
                y + 74,
                130,
                if cold {
                    Control::ApplyCold
                } else {
                    Control::ApplyColor
                },
                enabled,
            );
            button(
                p,
                "NONE",
                right + 150,
                y + 74,
                80,
                Control::ClearColor(cold),
                enabled,
            );
        }
        button(
            p,
            "ERASE SELECTION",
            right + 10,
            466,
            200,
            Control::Erase,
            view.editable() && view.canvas.count() > 0,
        );
    } else if view.effect() == Some("spectrum") {
        spectrum::paint(p, view, right);
    } else {
        paragraph(
            p,
            "Effect properties and lighting preview are not available yet.",
            right + 10,
            274,
            230,
            88,
            MUTED,
        );
    }
    button(
        p,
        "SAVE",
        right + 75,
        height - 40,
        100,
        Control::Save,
        view.editable() && (view.dirty() || view.renaming),
    );
    let message = if view.status.is_empty() {
        "Open a .ChromaEffects file to edit its layers."
    } else {
        &view.status
    };
    paragraph(
        p,
        message,
        right + 10,
        if view.effect() == Some("spectrum") {
            590
        } else if view.is_fire() {
            506
        } else {
            400
        },
        230,
        130,
        if view.failed { 0xfd4949 } else { MUTED },
    );
    layer_tools::paint(state, p);
    if view.dropdown {
        let count = view
            .archive
            .as_ref()
            .map_or(0, |archive| archive.profile_count());
        let visible = count.saturating_sub(view.profile_scroll).min(7);
        p.fill(46, 116, 270, visible as i32 * 28, 0);
        for index in view.profile_scroll..view.profile_scroll + visible {
            let title = &view.titles[index];
            let y = 116 + (index - view.profile_scroll) as i32 * 28;
            p.text(
                &title,
                52,
                y,
                254,
                28,
                p.fonts.normal,
                if view.index == index { GREEN } else { TEXT },
                DT_LEFT,
            );
            p.hits.push(Hit {
                rect: p.rect(46, y, 270, 28),
                action: Action::Studio(Control::Profile(index)),
            });
        }
    }
}

#[cfg(test)]
pub(super) fn diagnostics(state: &State) -> Value {
    let view = &state.studio;
    json!({"open":view.open,"busy":view.busy,"dialog":view.dialog,"dirty":view.dirty(),"renaming":view.renaming,"status":view.status,"failed":view.failed,"pending":state.pending,"selected_leds":view.canvas.count(),"canvas_notes":view.canvas.notes,"canvas_devices":view.canvas.device_count(),"canvas_selectable_leds":view.canvas.selectable_count(),"color":view.color,
        "preview":playback::diagnostics(state),"index":view.index,"profiles":view.archive.as_ref().map_or(0,|archive|archive.profile_count()),"selected":view.selected,
        "undo":view.draft.as_ref().map_or(0,Draft::undo_len),"redo":view.draft.as_ref().map_or(0,Draft::redo_len),
        "rows":view.draft.as_ref().map(|draft|draft.layers().iter().map(|row|json!({"id":row.id,"title":row.title,"visible":row.visible})).collect::<Vec<_>>()).unwrap_or_default()})
}

/// Render our own window state for review, without reading another app's pixels.
#[cfg(test)]
pub(super) unsafe fn capture_frame(state: &mut State, path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Graphics::GdiPlus as gp;
    let source = GetDC(state.hwnd);
    if source.is_null() {
        return false;
    }
    let dc = CreateCompatibleDC(source);
    let bitmap = CreateCompatibleBitmap(source, state.width, state.height);
    ReleaseDC(state.hwnd, source);
    if dc.is_null() || bitmap.is_null() {
        if !dc.is_null() {
            DeleteDC(dc);
        }
        if !bitmap.is_null() {
            DeleteObject(bitmap);
        }
        return false;
    }
    let old = SelectObject(dc, bitmap);
    super::paint(state, dc);
    SelectObject(dc, old);
    let mut image = null_mut();
    let created = gp::GdipCreateBitmapFromHBITMAP(bitmap, null_mut(), &mut image);
    let encoder = windows_sys::core::GUID {
        data1: 0x557cf406,
        data2: 0x1a04,
        data3: 0x11d3,
        data4: [0x9a, 0x73, 0, 0, 0xf8, 0x1e, 0xf3, 0x2e],
    };
    let filename: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let saved = created == gp::Ok
        && gp::GdipSaveImageToFile(image.cast(), filename.as_ptr(), &encoder, null()) == gp::Ok;
    if !image.is_null() {
        gp::GdipDisposeImage(image.cast());
    }
    DeleteObject(bitmap);
    DeleteDC(dc);
    saved
}

#[cfg(test)]
pub(super) fn reset_test(state: &mut State) {
    if let Some(draft) = &mut state.studio.draft {
        draft.discard();
    }
    state.studio.renaming = false;
    state.studio.name.clear();
    state.studio.next = None;
    state.studio.test_decision = Some(IDNO);
    state.exit_tray = None;
}
