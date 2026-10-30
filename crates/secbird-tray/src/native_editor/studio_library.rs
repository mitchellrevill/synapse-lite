//! Local library actions and native selection menus. Device output is never invoked.
use super::*;
use secbird_core::studio::library as model;

#[derive(Clone)]
pub enum Control {
    Menu,
    Create,
    Duplicate,
    Rename,
    Delete,
    Import,
    Export,
    CommitRename,
    CancelRename,
}
#[derive(Clone, Copy)]
pub enum Next {
    Create,
    Duplicate,
    Delete,
    Import,
}

fn snapshot(view: &View) -> Result<Archive, String> {
    let archive = view
        .archive
        .as_ref()
        .ok_or("Open or create a composition first")?;
    let bytes = match &view.draft {
        Some(draft) => archive.replace_profile_bytes(view.index, draft.json()),
        None => archive.to_bytes(),
    }
    .map_err(|e| e.to_string())?;
    Archive::parse(&bytes).map_err(|e| e.to_string())
}

fn stage(view: &mut View, archive: Archive, index: usize) -> Result<(), String> {
    let archive = Arc::new(archive);
    view.titles = (0..archive.profile_count())
        .map(|i| {
            archive
                .profile_header(i)
                .ok()
                .and_then(|h| h.title)
                .unwrap_or_else(|| format!("Composition {}", i + 1))
        })
        .collect();
    view.archive = Some(archive);
    view.library_dirty = true;
    view.dropdown = false;
    view.composition_menu = false;
    view.profile_scroll = 0;
    view.composition_renaming = false;
    if view.titles.is_empty() {
        view.index = 0;
        view.draft = None;
        view.selected = None;
        view.canvas = studio_canvas::Canvas::default();
        view.playback.clear_frame();
    } else {
        view.profile(index.min(view.titles.len() - 1))?;
    }
    view.status = "Composition library changed · Save to keep changes".into();
    view.failed = false;
    Ok(())
}

pub(super) fn discard(state: &mut State) {
    discard_view(&mut state.studio);
}

fn discard_view(view: &mut View) {
    view.composition_renaming = false;
    if view.library_dirty {
        if let Some(baseline) = view.baseline.clone() {
            let index = view.baseline_index;
            // An admitted archive can always be serialized again.
            if let Ok(archive) = baseline.to_bytes().and_then(|bytes| Archive::parse(&bytes)) {
                let _ = stage(view, archive, index);
                view.library_dirty = false;
            }
        } else {
            view.archive = None;
            view.draft = None;
            view.titles.clear();
            view.selected = None;
            view.canvas = studio_canvas::Canvas::default();
            view.library_dirty = false;
        }
    } else if let Some(draft) = &mut view.draft {
        draft.discard();
    }
}

fn unique_title(view: &View, base: &str) -> String {
    for number in 1..=129 {
        let suffix = format!(" ({number})");
        let mut prefix = String::new();
        for c in base.chars() {
            if prefix.encode_utf16().count() + c.len_utf16() + suffix.len() > 32 {
                break;
            }
            prefix.push(c);
        }
        let title = format!("{prefix}{suffix}");
        if !view.titles.contains(&title) {
            return title;
        }
    }
    unreachable!("at most128 compositions")
}

pub(super) fn finish_name(state: &mut State) -> bool {
    if state.read_only_smoke || state.restricted_actions {
        return false;
    }
    if state.studio.name == state.studio.title() {
        state.studio.composition_renaming = false;
        return true;
    }
    let result = (|| {
        let archive = snapshot(&state.studio)?;
        let next = model::rename(&archive, state.studio.index, &state.studio.name)
            .map_err(|e| e.to_string())?;
        let index = state.studio.index;
        stage(&mut state.studio, next, index)
    })();
    match result {
        Ok(()) => true,
        Err(message) => {
            error(state, message);
            false
        }
    }
}

pub(super) unsafe fn action(state: &mut State, control: Control) {
    if state.read_only_smoke || state.restricted_actions {
        return;
    }
    if !matches!(control, Control::Menu) {
        state.studio.composition_menu = false;
    }
    match control {
        Control::Menu => {
            state.studio.composition_menu = !state.studio.composition_menu;
            state.studio.dropdown = false;
        }
        Control::Create => transition(state, super::Next::Library(Next::Create)),
        Control::Duplicate => transition(state, super::Next::Library(Next::Duplicate)),
        Control::Delete => transition(state, super::Next::Library(Next::Delete)),
        Control::Import => transition(state, super::Next::Library(Next::Import)),
        Control::Export => {
            PostMessageW(state.hwnd, DIALOG, 7, 0);
        }
        Control::Rename => {
            if state
                .studio
                .archive
                .as_ref()
                .is_some_and(|a| a.profile_count() > 0)
            {
                state.studio.name = state.studio.title().into();
                state.studio.composition_renaming = true;
                state.studio.dropdown = false;
                state.studio.composition_menu = false;
            }
        }
        Control::CommitRename => {
            finish_name(state);
        }
        Control::CancelRename => state.studio.composition_renaming = false,
    }
}

pub(super) unsafe fn proceed(state: &mut State, next: Next) {
    match next {
        Next::Import => {
            PostMessageW(state.hwnd, DIALOG, 5, 0);
        }
        Next::Delete => {
            PostMessageW(state.hwnd, DIALOG, 4, 0);
        }
        Next::Create | Next::Duplicate => {
            let result = (|| {
                let id = layer_tools::identity()?;
                let archive = if state.studio.archive.is_some() {
                    snapshot(&state.studio)?
                } else {
                    model::empty().map_err(|e| e.to_string())?
                };
                let index = archive.profile_count();
                let next = if matches!(next, Next::Create) {
                    let name = unique_title(&state.studio, "Chroma Effect");
                    model::create(&archive, &id, &layer_tools::identity()?, &name)
                } else {
                    let name = unique_title(&state.studio, state.studio.title());
                    model::duplicate(&archive, state.studio.index, &id, &name)
                }
                .map_err(|e| e.to_string())?;
                stage(&mut state.studio, next, index)
            })();
            if let Err(message) = result {
                error(state, message);
            }
            selection_changed(state);
        }
    }
}

pub(super) unsafe fn paint_menu(state: &State, p: &mut Painter, y: i32) {
    let available = !state.studio.busy
        && !state.studio.dialog
        && !state.read_only_smoke
        && !state.restricted_actions;
    let selected = state
        .studio
        .archive
        .as_ref()
        .is_some_and(|a| a.profile_count() > 0);
    p.fill(278, y, 180, 168, 0);
    for (offset, (label, control, enabled)) in [
        ("Add", Control::Create, available),
        ("Import…", Control::Import, available),
        ("Rename", Control::Rename, available && selected),
        ("Duplicate", Control::Duplicate, available && selected),
        ("Export…", Control::Export, available && selected),
        ("Delete…", Control::Delete, available && selected),
    ]
    .into_iter()
    .enumerate()
    {
        let row_y=y+offset as i32*28;
        p.text(label,284,row_y,168,28,p.fonts.normal,if enabled {TEXT}else{0x555555},DT_LEFT);
        if enabled {p.hits.push(Hit{rect:p.rect(278,row_y,180,28),action:Action::Studio(super::Control::Library(control))});}
    }
}

/// Library popovers sit over the LED canvas. Consume their click sequence before
/// the canvas can start a selection gesture on an underlying device.
pub(super) unsafe fn pointer(state: &mut State, message: u32, lparam: isize) -> bool {
    if !state.studio.open || !(state.studio.composition_menu || state.studio.dropdown) { return false; }
    match message {
        WM_LBUTTONDOWN | WM_MOUSEMOVE => true,
        WM_LBUTTONUP => {
            let point = POINT {x:(lparam as u32 & 0xffff) as i16 as i32,
                y:((lparam as u32 >> 16)&0xffff) as i16 as i32};
            let composition_menu = state.studio.composition_menu;
            let action = state.hits.iter().rev().find(|hit|PtInRect(&hit.rect,point)!=0)
                .and_then(|hit|match &hit.action {
                    Action::Studio(control @ (super::Control::Library(_) | super::Control::Profiles)) if composition_menu => Some(control.clone()),
                    Action::Studio(control @ (super::Control::Profile(_) | super::Control::Profiles)) if !composition_menu => Some(control.clone()),
                    _ => None,
                }).unwrap_or(if composition_menu {super::Control::Library(Control::Menu)} else {super::Control::Profiles});
            state.action(Action::Studio(action)); true
        }
        WM_RBUTTONUP | WM_CANCELMODE | WM_KILLFOCUS => {
            state.studio.composition_menu=false; state.studio.dropdown=false;
            InvalidateRect(state.hwnd,null(),0); message==WM_RBUTTONUP
        }
        _ => false,
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

/// Native checked menu: changes are provisional until Done; Escape cancels them.
unsafe fn select(
    hwnd: HWND,
    archive: &Archive,
    current: Option<usize>,
) -> Result<Option<Vec<usize>>, String> {
    let mut selected = vec![current.is_none(); archive.profile_count()];
    if let Some(index) = current {
        if let Some(value) = selected.get_mut(index) {
            *value = true;
        }
    }
    loop {
        let menu = Menu(CreatePopupMenu());
        if menu.0.is_null() {
            return Err("Could not create composition selection".into());
        }
        let add = |id: usize, label: &str, flags: u32| -> Result<(), String> {
            if AppendMenuW(menu.0, flags, id, wide(label).as_ptr()) == 0 {
                Err("Could not populate composition selection".into())
            } else {
                Ok(())
            }
        };
        add(
            1001,
            "Done",
            MF_STRING
                | if selected.iter().any(|v| *v) {
                    0
                } else {
                    MF_GRAYED
                },
        )?;
        add(1002, "Select all", MF_STRING)?;
        add(1003, "Select none", MF_STRING)?;
        for (i, on) in selected.iter().enumerate() {
            let title = archive
                .profile_header(i)
                .ok()
                .and_then(|h| h.title)
                .unwrap_or_else(|| format!("Composition {}", i + 1));
            add(
                i + 1,
                &format!("{} · {}", i + 1, title.replace('&', "&&")),
                MF_STRING | if *on { MF_CHECKED } else { 0 },
            )?;
        }
        let mut point: POINT = std::mem::zeroed();
        GetCursorPos(&mut point);
        let choice = TrackPopupMenu(
            menu.0,
            TPM_RETURNCMD | TPM_NONOTIFY,
            point.x,
            point.y,
            0,
            hwnd,
            null(),
        );
        match choice {
            0 => return Ok(None),
            1001 => {
                return Ok(Some(
                    selected
                        .iter()
                        .enumerate()
                        .filter_map(|(i, on)| on.then_some(i))
                        .collect(),
                ))
            }
            1002 => selected.fill(true),
            1003 => selected.fill(false),
            number => {
                if let Some(value) = selected.get_mut(number as usize - 1) {
                    *value = !*value;
                }
            }
        }
    }
}

unsafe fn queue(state: &mut State, request: Request) {
    let before = (state.pending, state.pending_jobs.len());
    state.studio.busy = true;
    state.queue(Job::Studio(request));
    if before == (state.pending, state.pending_jobs.len()) {
        state.studio.busy = false;
        error(state, "Studio operation was not accepted");
    }
}

pub(super) unsafe fn dialog(hwnd: HWND, pointer: *mut RefCell<State>, kind: usize) {
    let epoch = {
        let Ok(mut state) = (*pointer).try_borrow_mut() else {
            return;
        };
        if state.read_only_smoke
            || state.restricted_actions
            || state.studio.busy
            || state.studio.dialog
        {
            return;
        }
        cancel_gestures(&mut state);
        state.studio.dialog = true;
        state.studio.epoch
    };
    // No mutable State borrow spans a native modal call.
    let result = (|| -> Result<Option<(Option<PathBuf>, Option<Vec<usize>>)>, String> {
        match kind {
            4 => {
                let title = (*pointer).borrow().studio.title().to_owned();
                #[cfg(test)]
                let decision = (*pointer).borrow_mut().studio.test_decision.take();
                #[cfg(not(test))]
                let decision: Option<i32> = None;
                let accepted=decision.unwrap_or_else(||MessageBoxW(hwnd,wide(&format!("Delete composition ‘{title}’?\nThis removes it from this library when saved.")).as_ptr(),wide("Delete composition").as_ptr(),MB_YESNO|MB_ICONWARNING|MB_DEFBUTTON2));
                Ok((accepted == IDYES).then_some((None, None)))
            }
            5 => Ok(choose(hwnd, false, "")?.map(|path| (Some(path), None))),
            6 => {
                let archive = (*pointer)
                    .borrow()
                    .studio
                    .import_candidate
                    .clone()
                    .ok_or("Import selection is no longer available")?;
                Ok(select(hwnd, &archive, None)?.map(|indices| (None, Some(indices))))
            }
            7 => {
                let (archive, index) = {
                    let state = (*pointer).borrow();
                    (snapshot(&state.studio)?, state.studio.index)
                };
                let Some(indices) = select(hwnd, &archive, Some(index))? else {
                    return Ok(None);
                };
                Ok(choose(hwnd, true, "Selected.ChromaEffects")?
                    .map(|path| (Some(path), Some(indices))))
            }
            _ => Err("Unknown Studio library dialog".into()),
        }
    })();
    let mut state = (*pointer).borrow_mut();
    state.studio.dialog = false;
    if state.studio.epoch != epoch {
        return;
    }
    let result = result.and_then(|choice| {
        let Some((path, indices)) = choice else {
            state.studio.import_candidate = None;
            return Ok(());
        };
        match kind {
            4 => {
                let archive = snapshot(&state.studio)?;
                let next =
                    model::delete(&archive, state.studio.index).map_err(|e| e.to_string())?;
                let index = state.studio.index.saturating_sub(1);
                stage(&mut state.studio, next, index)?;
            }
            5 => queue(
                &mut state,
                Request::Import {
                    epoch,
                    path: path.unwrap(),
                },
            ),
            6 => {
                let incoming = state
                    .studio
                    .import_candidate
                    .take()
                    .ok_or("Import is no longer available")?;
                let archive = if state.studio.archive.is_some() {
                    snapshot(&state.studio)?
                } else {
                    model::empty().map_err(|e| e.to_string())?
                };
                let index = archive.profile_count();
                let next = model::import(&archive, &incoming, &indices.unwrap())
                    .map_err(|e| e.to_string())?;
                stage(&mut state.studio, next, index)?;
            }
            7 => {
                let path = path.unwrap();
                // Export must never overwrite the current editor's archive/baseline.
                if state
                    .studio
                    .path
                    .as_ref()
                    .is_some_and(|source| same_path(source, &path))
                {
                    return Err(
                        "Choose a different export path; use Save to update the open library"
                            .into(),
                    );
                }
                let archive = Arc::new(snapshot(&state.studio)?);
                queue(
                    &mut state,
                    Request::Export {
                        epoch,
                        path,
                        archive,
                        indices: indices.unwrap(),
                    },
                );
            }
            _ => unreachable!(),
        }
        selection_changed(&mut state);
        Ok(())
    });
    if let Err(message) = result {
        error(&mut state, message);
    }
    redraw(&mut state);
}

fn same_path(a: &Path, b: &Path) -> bool {
    let a = std::fs::canonicalize(a).unwrap_or_else(|_| a.to_owned());
    let b = std::fs::canonicalize(b).unwrap_or_else(|_| b.to_owned());
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> View {
        let archive=Arc::new(Archive::parse(br#"{"version":1,"opaque":9007199254740993123,"profiles":[{"id":"original","title":"Desk","devices":[],"layers":[]}]}"#).unwrap());
        let mut view = View::default();
        view.baseline = Some(archive.clone());
        view.archive = Some(archive);
        view.profile(0).unwrap();
        view
    }
    #[test]
    fn composition_changes_discard_as_one_library_and_saved_snapshot_clears_dirty() {
        let mut view = view();
        let before = view.baseline.as_ref().unwrap().to_bytes().unwrap();
        let next =
            model::duplicate(view.archive.as_ref().unwrap(), 0, "0123456789ab", "Copy").unwrap();
        stage(&mut view, next, 1).unwrap();
        assert!(view.dirty());
        assert_eq!(view.index, 1);
        discard_view(&mut view);
        assert!(!view.dirty());
        assert_eq!(view.index, 0);
        assert_eq!(view.archive.as_ref().unwrap().to_bytes().unwrap(), before);
        let next = model::delete(view.archive.as_ref().unwrap(), 0).unwrap();
        stage(&mut view, next, 0).unwrap();
        assert!(view.dirty());
        assert!(view.draft.is_none());
        discard_view(&mut view);
        assert!(view.draft.is_some());
        assert!(!view.dirty());
    }
    #[test]
    fn new_unsaved_library_discards_without_creating_a_file() {
        let mut view = View::default();
        stage(
            &mut view,
            model::create(
                &model::empty().unwrap(),
                "0123456789ab",
                "abcdef012345",
                "New",
            )
            .unwrap(),
            0,
        )
        .unwrap();
        assert!(view.dirty());
        discard_view(&mut view);
        assert!(!view.dirty());
        assert!(view.archive.is_none());
        assert!(view.path.is_none());
    }
    #[test]
    fn empty_library_saves_and_failed_or_readonly_workers_retain_existing_files() {
        let folder = std::env::temp_dir().join(format!(
            "studio-library-worker-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("library.ChromaEffects");
        let existing = b"retained";
        std::fs::write(&path, existing).unwrap();
        let request = |path: PathBuf| Request::Save {
            epoch: 1,
            path,
            index: 0,
            archive: Arc::new(model::empty().unwrap()),
            checkpoint: None,
        };
        assert!(matches!(
            work(request(path.clone()), true),
            Response::Save { result: Err(_), .. }
        ));
        assert_eq!(std::fs::read(&path).unwrap(), existing);
        assert!(matches!(
            work(request(folder.join("missing/library.ChromaEffects")), false),
            Response::Save { result: Err(_), .. }
        ));
        assert!(matches!(
            work(request(path.clone()), false),
            Response::Save { result: Ok(_), .. }
        ));
        assert_eq!(
            Archive::read(File::open(&path).unwrap())
                .unwrap()
                .profile_count(),
            0
        );
        let export = Request::Export {
            epoch: 1,
            path: path.clone(),
            archive: Arc::new(model::empty().unwrap()),
            indices: vec![],
        };
        let before = std::fs::read(&path).unwrap();
        assert!(matches!(
            work(export, false),
            Response::Export { result: Err(_), .. }
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
