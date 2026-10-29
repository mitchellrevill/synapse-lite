//! Native local game/profile management. No program launching or device writes.
use super::*;
use secbird_core::linked_games::{Catalog, Change, Game};
use secbird_core::Store;
use std::{cell::Cell, collections::BTreeMap};
use windows_sys::Win32::{
    Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG},
    UI::Controls::Dialogs::*,
};
pub(super) const ID: &str = "module:linked-games";
pub(super) const BROWSE: u32 = WM_APP + 170;
#[cfg(test)]
pub(super) const TEST: u32 = WM_APP + 171;
const SEARCH: u16 = 2200;
#[derive(Clone, Copy, Default, PartialEq)]
pub enum Page {
    #[default]
    Games,
    Devices,
}
#[derive(Clone)]
pub enum Control {
    Open,
    Page(Page),
    Browse,
    Game(String),
    Device(String),
    Dismiss,
    Choose(String, String),
    Bind(String, String, Option<String>),
    Remove(String),
    ViewFilter,
    Order,
}
#[derive(Clone)]
pub enum Request {
    Read,
    Add(String),
    Bind(String, String, Option<String>),
    Remove(String),
}
impl Request {
    pub fn mutates(&self) -> bool {
        !matches!(self, Self::Read)
    }
}
#[derive(Clone)]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    pub pid: u16,
    pub profiles: Vec<String>,
}
pub struct Data {
    pub games: Vec<Game>,
    pub devices: Vec<DeviceRow>,
    pub bindings: BTreeMap<(String, String), Vec<String>>,
    pub legacy: BTreeMap<(String, String), Vec<String>>,
}
#[derive(Default)]
pub(super) struct View {
    pub used: bool,
    pub busy: bool,
    page: Page,
    data: Option<Data>,
    search: String,
    linked_only: bool,
    descending: bool,
    scroll: i32,
    dialog: Option<Control>,
    picker: Option<(String, String)>,
    dialog_offset: usize,
    picker_offset: usize,
    maximum_scroll: Cell<i32>,
    status: String,
    failed: bool,
}
pub(super) fn active(state: &State) -> bool {
    state.selected.as_deref() == Some(ID)
}
fn identity() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    let status = unsafe {
        BCryptGenRandom(
            null_mut(),
            bytes.as_mut_ptr(),
            16,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status < 0 {
        return Err("Could not create a game identity".into());
    }
    Ok(format!(
        "local-{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ))
}
pub(super) fn work(
    request: Request,
    mut devices: Vec<DeviceRow>,
    read_only: bool,
) -> Result<Data, String> {
    if read_only && request.mutates() {
        return Err("Read-only mode refuses game/profile changes".into());
    }
    let mut store = Store::load(&secbird_core::config_dir().join("profiles.json"))
        .map_err(|e| e.to_string())?;
    let change = match request {
        Request::Read => None,
        Request::Add(path) => Some(Change::Add {
            id: identity()?,
            path,
        }),
        Request::Bind(game, device, profile) => {
            let row = devices
                .iter()
                .find(|d| d.id == device)
                .ok_or("Selected device is no longer connected")?;
            store.profiles_for(&row.id, row.pid);
            Some(Change::Bind {
                game,
                device,
                profile,
            })
        }
        Request::Remove(game) => Some(Change::Remove { game }),
    };
    if let Some(change) = change {
        store = secbird_core::linked_games::edit(&store, change).map_err(|e| e.to_string())?;
        store.save().map_err(|e| e.to_string())?;
    }
    let catalog = Catalog::read(&store).map_err(|e| e.to_string())?;
    let mut bindings = BTreeMap::new();
    let mut legacy = BTreeMap::new();
    for device in &mut devices {
        let profiles = store.profiles_for(&device.id, device.pid);
        device.profiles = profiles.profiles.keys().cloned().collect();
        for game in &catalog.games {
            let (exact, names) = secbird_core::linked_games::bindings(&store, game, &device.id)
                .map_err(|e| e.to_string())?;
            if !exact.is_empty() {
                bindings.insert((game.id.clone(), device.id.clone()), exact);
            }
            if !names.is_empty() {
                legacy.insert((game.id.clone(), device.id.clone()), names);
            }
        }
    }
    Ok(Data {
        games: catalog.games,
        devices,
        bindings,
        legacy,
    })
}
pub(super) unsafe fn request(state: &mut State, request: Request) {
    if state.linked_games.busy {
        return;
    }
    if request.mutates() && (state.read_only_smoke || state.restricted_actions) {
        return;
    }
    let before = (state.pending, state.pending_jobs.len());
    state.linked_games.busy = true;
    state.queue(Job::LinkedGames(request));
    if state.pending <= before.0 && state.pending_jobs.len() <= before.1 {
        state.linked_games.busy = false;
    }
}
pub(super) unsafe fn receive(state: &mut State, result: Result<Data, String>) {
    state.linked_games.busy = false;
    match result {
        Ok(data) => {
            state.linked_games.data = Some(data);
            state.linked_games.status = "".into();
            state.linked_games.failed = false;
        }
        Err(error) => {
            state.linked_games.status = error.clone();
            state.linked_games.failed = true;
            state.error(&error);
        }
    }
    if active(state) {
        state.rebuild_fields();
        InvalidateRect(state.hwnd, null(), 0);
    }
}
pub(super) unsafe fn action(state: &mut State, control: Control) {
    if matches!(control, Control::Open) {
        state.linked_games.used = true;
        state.action(Action::Device(ID.into()));
        return;
    }
    if !active(state) {
        return;
    }
    match control {
        Control::Page(page) => {
            state.linked_games.page = page;
            state.linked_games.scroll = 0;
            state.linked_games.dialog = None;
            state.linked_games.picker = None;
            state.linked_games.dialog_offset = 0;
        }
        Control::Browse if !state.linked_games.busy => {
            PostMessageW(state.hwnd, BROWSE, 0, 0);
        }
        Control::Game(id) => {
            state.linked_games.dialog = Some(Control::Game(id));
            state.linked_games.picker = None;
            state.linked_games.dialog_offset = 0;
        }
        Control::Device(id) => {
            state.linked_games.dialog = Some(Control::Device(id));
            state.linked_games.picker = None;
            state.linked_games.dialog_offset = 0;
        }
        Control::Dismiss => {
            state.linked_games.dialog = None;
            state.linked_games.picker = None;
        }
        Control::Choose(game, device) => {
            state.linked_games.picker = Some((game, device));
            state.linked_games.picker_offset = 0;
        }
        Control::Bind(game, device, profile) => {
            state.linked_games.picker = None;
            request(state, Request::Bind(game, device, profile));
        }
        Control::Remove(game) => {
            state.linked_games.dialog = None;
            state.linked_games.picker = None;
            request(state, Request::Remove(game));
        }
        Control::ViewFilter => {
            state.linked_games.linked_only = !state.linked_games.linked_only;
            state.linked_games.scroll = 0;
        }
        Control::Order => state.linked_games.descending = !state.linked_games.descending,
        _ => {}
    }
    state.rebuild_fields();
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn fields(state: &mut State) {
    if !active(state)
        || state.linked_games.page != Page::Games
        || state.linked_games.dialog.is_some()
    {
        return;
    }
    let text = state.linked_games.search.clone();
    state.retained_fields.remove(&SEARCH);
    state.field(SEARCH, &text, 78, 91, 230);
    if let Some(&field) = state.fields.get(&SEARCH) {
        SendMessageW(field, EM_SETLIMITTEXT, 128, 0);
        SendMessageW(field, WM_SETFONT, state.fonts.normal as usize, 1);
    }
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if id != SEARCH || notification != EN_CHANGE as u16 {
        return false;
    }
    state.linked_games.search = state.raw_text(id);
    state.linked_games.scroll = 0;
    unsafe { InvalidateRect(state.hwnd, null(), 0) };
    true
}
pub(super) unsafe fn wheel(state: &mut State, delta: i16) {
    let view = &mut state.linked_games;
    if view.picker.is_some() {
        view.picker_offset = if delta < 0 {
            view.picker_offset.saturating_add(1)
        } else {
            view.picker_offset.saturating_sub(1)
        };
    } else if view.dialog.is_some() {
        view.dialog_offset = if delta < 0 {
            view.dialog_offset.saturating_add(1)
        } else {
            view.dialog_offset.saturating_sub(1)
        };
    } else {
        view.scroll =
            (view.scroll + if delta < 0 { 80 } else { -80 }).clamp(0, view.maximum_scroll.get());
    }
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn browse(hwnd: HWND) -> Result<Option<String>, String> {
    let mut file = vec![0u16; 32768];
    let filter = wide("Applications (*.exe)\0*.exe\0\0");
    let title = wide("Add a game or application");
    let mut options: OPENFILENAMEW = std::mem::zeroed();
    options.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    options.hwndOwner = hwnd;
    options.lpstrFilter = filter.as_ptr();
    options.lpstrFile = file.as_mut_ptr();
    options.nMaxFile = file.len() as u32;
    options.lpstrTitle = title.as_ptr();
    options.Flags = OFN_EXPLORER | OFN_NOCHANGEDIR | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST;
    if GetOpenFileNameW(&mut options) == 0 {
        let error = CommDlgExtendedError();
        return if error == 0 {
            Ok(None)
        } else {
            Err(format!("Could not choose the application ({error})"))
        };
    }
    let end = file
        .iter()
        .position(|&c| c == 0)
        .ok_or("Invalid application path")?;
    String::from_utf16(&file[..end])
        .map(Some)
        .map_err(|_| "Unsupported application path".into())
}
unsafe fn button(
    p: &mut Painter,
    label: &str,
    x: i32,
    y: i32,
    w: i32,
    control: Control,
    enabled: bool,
) {
    p.fill(x, y, w, 28, if enabled { 0x303030 } else { 0x222222 });
    p.text(
        label,
        x + 5,
        y,
        w - 10,
        28,
        p.fonts.small,
        if enabled { TEXT } else { 0x555555 },
        DT_CENTER,
    );
    if enabled {
        p.hits.push(Hit {
            rect: p.rect(x, y, w, 28),
            action: Action::LinkedGames(control),
        });
    }
}
unsafe fn tile(p: &Painter, x: i32, y: i32, w: i32, h: i32) {
    let r = p.rect(x, y, w, h);
    let brush = CreateSolidBrush(rgb(PANEL));
    let pen = CreatePen(PS_SOLID, 0, rgb(PANEL));
    let old_b = SelectObject(p.dc, brush);
    let old_p = SelectObject(p.dc, pen);
    RoundRect(p.dc, r.left, r.top, r.right, r.bottom, p.px(10), p.px(10));
    SelectObject(p.dc, old_b);
    SelectObject(p.dc, old_p);
    DeleteObject(brush);
    DeleteObject(pen);
}
pub(super) unsafe fn paint(state: &State, p: &mut Painter, width: i32, height: i32) {
    let view = &state.linked_games;
    let center = width / 2;
    for (page, label, x) in [
        (Page::Games, "GAMES", center - 75),
        (Page::Devices, "DEVICES", center + 5),
    ] {
        p.button(
            label,
            x,
            91,
            70,
            Action::LinkedGames(Control::Page(page)),
            view.page == page,
        );
    }
    p.line(0, 130, width, 0);
    if view.page == Page::Games {
        button(
            p,
            "+",
            10,
            91,
            32,
            Control::Browse,
            !view.busy && !state.read_only_smoke,
        );
        p.text("Search", 44, 91, 40, 28, p.fonts.small, MUTED, DT_LEFT);
        button(
            p,
            if view.linked_only {
                "Linked games"
            } else {
                "All games"
            },
            width - 360,
            91,
            160,
            Control::ViewFilter,
            true,
        );
        button(
            p,
            if view.descending {
                "Name (Z - A)"
            } else {
                "Name (A - Z)"
            },
            width - 180,
            91,
            160,
            Control::Order,
            true,
        );
    }
    let Some(data) = &view.data else {
        p.text(
            if view.busy {
                "Loading…"
            } else {
                &view.status
            },
            30,
            160,
            width - 60,
            60,
            p.fonts.normal,
            MUTED,
            DT_LEFT,
        );
        return;
    };
    let columns = ((width - 40) / 310).max(1);
    let clip = SaveDC(p.dc);
    IntersectClipRect(p.dc, p.px(0), p.px(132), p.px(width), p.px(height - 42));
    if view.page == Page::Games {
        let query = view.search.to_lowercase();
        let mut games: Vec<_> = data
            .games
            .iter()
            .filter(|g| {
                (g.name.to_lowercase().contains(&query) || g.path.to_lowercase().contains(&query))
                    && (!view.linked_only
                        || data
                            .bindings
                            .iter()
                            .any(|((game, _), p)| game == &g.id && !p.is_empty()))
            })
            .collect();
        games.sort_by_key(|g| g.name.to_lowercase());
        if view.descending {
            games.reverse();
        }
        if games.is_empty() {
            p.text(
                if data.games.is_empty() {
                    "Add a game or application to link its device profiles."
                } else {
                    "No games match this view."
                },
                30,
                170,
                width - 60,
                60,
                p.fonts.normal,
                MUTED,
                DT_LEFT,
            );
        }
        let maximum =
            (((games.len() as i32 + columns - 1) / columns) * 240 - (height - 184)).max(0);
        view.maximum_scroll.set(maximum);
        let scroll = view.scroll.min(maximum);
        for (index, game) in games.iter().enumerate() {
            let x = 30 + index as i32 % columns * 310;
            let y = 142 + index as i32 / columns * 240 - scroll;
            if y + 220 < 132 || y >= height - 42 {
                continue;
            }
            tile(p, x, y, 290, 220);
            p.image(
                state.icons.get("host-game-placeholder"),
                x + 115,
                y + 45,
                60,
                60,
            );
            p.text(
                &game.name,
                x + 10,
                y + 160,
                270,
                24,
                p.fonts.normal,
                TEXT,
                DT_CENTER,
            );
            let count = data
                .bindings
                .iter()
                .filter(|((id, _), profiles)| id == &game.id && !profiles.is_empty())
                .count();
            if count > 0 {
                p.text(
                    &format!("{count} linked device(s)"),
                    x + 10,
                    y + 188,
                    270,
                    22,
                    p.fonts.small,
                    GREEN,
                    DT_CENTER,
                );
            }
            p.hits.push(Hit {
                rect: p.rect(x, y.max(132), 290, (y + 220).min(height - 42) - y.max(132)),
                action: Action::LinkedGames(Control::Game(game.id.clone())),
            });
        }
    } else {
        let maximum =
            (((data.devices.len() as i32 + columns - 1) / columns) * 240 - (height - 184)).max(0);
        view.maximum_scroll.set(maximum);
        let scroll = view.scroll.min(maximum);
        for (index, device) in data.devices.iter().enumerate() {
            let x = 30 + index as i32 % columns * 310;
            let y = 142 + index as i32 / columns * 240 - scroll;
            if y + 220 < 132 || y >= height - 42 {
                continue;
            }
            tile(p, x, y, 290, 220);
            if let Some(raw) = state.devices().iter().find(|d| d["id"] == device.id) {
                state
                    .device_artwork
                    .borrow_mut()
                    .draw(p, raw, x + 15, y + 10, 260, 140);
            }
            p.text(
                &device.name.to_uppercase(),
                x + 15,
                y + 160,
                260,
                48,
                p.fonts.normal,
                TEXT,
                DT_CENTER | DT_WORDBREAK,
            );
            p.hits.push(Hit {
                rect: p.rect(x, y.max(132), 290, (y + 220).min(height - 42) - y.max(132)),
                action: Action::LinkedGames(Control::Device(device.id.clone())),
            });
        }
    }
    RestoreDC(p.dc, clip);
    p.text(
        &view.status,
        30,
        height - 36,
        width - 60,
        30,
        p.fonts.small,
        if view.failed { 0xfd4949 } else { MUTED },
        DT_LEFT,
    );
    if let Some(dialog) = &view.dialog {
        paint_dialog(state, p, data, dialog, width, height);
    }
}
unsafe fn paint_dialog(
    state: &State,
    p: &mut Painter,
    data: &Data,
    dialog: &Control,
    width: i32,
    height: i32,
) {
    let view = &state.linked_games;
    let x = (width - 730) / 2;
    let y = 150;
    let h = (height - 210).max(250);
    p.fill(x - 1, y - 1, 732, h + 2, BORDER);
    p.fill(x, y, 730, h, 0x222222);
    p.hits.push(Hit {
        rect: p.rect(0, 132, width, height - 132),
        action: Action::LinkedGames(Control::Dismiss),
    });
    p.hits.push(Hit {
        rect: p.rect(x, y, 730, h),
        action: Action::LinkedGames(Control::Choose("".into(), "".into())),
    });
    button(p, "×", x + 688, y + 8, 30, Control::Dismiss, true);
    let rows: Vec<(String, String, String)> = match dialog {
        Control::Game(id) => {
            let Some(game) = data.games.iter().find(|g| g.id == *id) else {
                return;
            };
            p.text(
                &game.name,
                x + 25,
                y + 20,
                640,
                30,
                p.fonts.heading,
                GREEN,
                DT_LEFT,
            );
            p.text(
                &game.path,
                x + 25,
                y + 60,
                640,
                28,
                p.fonts.small,
                MUTED,
                DT_LEFT,
            );
            button(
                p,
                "Remove game",
                x + 25,
                y + h - 42,
                140,
                Control::Remove(id.clone()),
                !view.busy && !state.read_only_smoke,
            );
            data.devices
                .iter()
                .map(|d| (game.id.clone(), d.id.clone(), d.name.clone()))
                .collect()
        }
        Control::Device(id) => {
            let Some(device) = data.devices.iter().find(|d| d.id == *id) else {
                return;
            };
            p.text(
                &device.name,
                x + 25,
                y + 20,
                640,
                48,
                p.fonts.heading,
                GREEN,
                DT_LEFT,
            );
            data.games
                .iter()
                .map(|g| (g.id.clone(), device.id.clone(), g.name.clone()))
                .collect()
        }
        _ => return,
    };
    let visible = ((h - 170) / 52).max(1) as usize;
    let offset = view.dialog_offset.min(rows.len().saturating_sub(visible));
    for (index, (game, device, label)) in rows.iter().skip(offset).take(visible).enumerate() {
        let row_y = y + 110 + index as i32 * 52;
        p.text(label, x + 25, row_y, 390, 35, p.fonts.normal, TEXT, DT_LEFT);
        let binding = data
            .bindings
            .get(&(game.clone(), device.clone()))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let name = if binding.len() == 1 {
            binding[0].as_str()
        } else if binding.len() > 1 {
            "Choose profile"
        } else {
            "Not linked"
        };
        button(
            p,
            name,
            x + 440,
            row_y,
            260,
            Control::Choose(game.clone(), device.clone()),
            !view.busy && !state.read_only_smoke,
        );
        if data
            .legacy
            .get(&(game.clone(), device.clone()))
            .is_some_and(|p| !p.is_empty())
        {
            p.text(
                "A filename rule also applies",
                x + 25,
                row_y + 25,
                390,
                20,
                p.fonts.small,
                MUTED,
                DT_LEFT,
            );
        }
    }
    if let Some((game, device)) = &view.picker {
        if let Some(row) = data.devices.iter().find(|d| d.id == *device) {
            let top = y + 95;
            let left = x + 435;
            let visible = ((h - 130) / 29).max(1) as usize;
            let count = row.profiles.len() + 1;
            let offset = view.picker_offset.min(count.saturating_sub(visible));
            p.fill(
                left - 1,
                top - 1,
                267,
                (count.min(visible)) as i32 * 29 + 2,
                BORDER,
            );
            for (i, index) in (offset..count).take(visible).enumerate() {
                let profile = if index == 0 {
                    None
                } else {
                    Some(row.profiles[index - 1].clone())
                };
                button(
                    p,
                    profile.as_deref().unwrap_or("Not linked"),
                    left,
                    top + i as i32 * 29,
                    265,
                    Control::Bind(game.clone(), device.clone(), profile.clone()),
                    !view.busy && !state.read_only_smoke,
                );
            }
        }
    }
}
#[cfg(test)]
pub(super) fn diagnostics(state: &State) -> Value {
    let v = &state.linked_games;
    json!({"active":active(state),"busy":v.busy,"status":v.status,"failed":v.failed,"devices":v.data.as_ref().map(|d|d.devices.iter().map(|r|json!({"id":r.id,"profiles":r.profiles})).collect::<Vec<_>>()).unwrap_or_default(),"games":v.data.as_ref().map(|d|d.games.clone()).unwrap_or_default(),"dialog":v.dialog.is_some(),"picker":v.picker.is_some(),"page":if v.page==Page::Games{"games"}else{"devices"}})
}
