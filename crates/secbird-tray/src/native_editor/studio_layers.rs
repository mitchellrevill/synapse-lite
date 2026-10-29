//! Layer actions over source-shaped, bounded local draft operations.
use super::*;
use secbird_core::studio::document::NewEffect;
use windows_sys::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};

#[derive(Clone)]
pub enum Control { Menu, Dismiss, Add(NewEffect), Group, Duplicate, Delete, Move(i8) }

fn identity() -> Result<String, String> {
    let mut bytes = [0u8; 6];
    let status = unsafe { BCryptGenRandom(null_mut(), bytes.as_mut_ptr(), 6, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status < 0 { return Err(format!("Could not generate a layer identity ({status})")); }
    Ok(bytes.iter().map(|byte|format!("{byte:02x}")).collect())
}

fn unique_title(rows: &[secbird_core::studio::document::LayerRow], selected: &secbird_core::studio::document::LayerRow) -> String {
    let defaults = [("ambient","AMBIENT"),("audio","AUDIO_METER"),("breathing","BREATHING"),
        ("chroma-generate","CHROMA_GENERATE"),("fire","FIRE"),("reactive","REACTIVE"),("ripple","RIPPLE"),
        ("spectrum","SPECTRUM_CYCLING"),("starlight","STARLIGHT"),("static","STATIC"),
        ("tidal","TIDAL"),("wave","WAVE"),("wheel","WHEEL")];
    if selected.kind == Kind::Layer && defaults.iter().any(|(name,title)|
        selected.effect.as_deref() == Some(name) && selected.title == *title) {
        return selected.title.clone();
    }
    let mut base = selected.title.as_str();
    if let Some((prefix, suffix)) = base.rsplit_once(" (") {
        if suffix.strip_suffix(')').is_some_and(|digits|!digits.is_empty() && digits.bytes().all(|c|c.is_ascii_digit())) {
            base = prefix;
        }
    }
    for number in 1..=513 {
        let title = format!("{base} ({number})");
        if !rows.iter().any(|row|row.parent == selected.parent && row.kind == selected.kind && row.title == title) { return title; }
    }
    unreachable!("editor admits at most512 rows")
}

pub(super) fn display_title(row: &secbird_core::studio::document::LayerRow) -> &str {
    match (row.effect.as_deref(), row.title.as_str()) {
        (Some("static"),"STATIC") => "Static", (Some("fire"),"FIRE") => "Fire",
        (Some("spectrum"),"SPECTRUM_CYCLING") => "Spectrum Cycling",
        (Some("ambient"),"AMBIENT") => "Ambient Awareness", (Some("audio"),"AUDIO_METER") => "Audio Meter",
        (Some("breathing"),"BREATHING") => "Breathing", (Some("chroma-generate"),"CHROMA_GENERATE") => "Chroma Generate",
        (Some("reactive"),"REACTIVE") => "Reactive", (Some("ripple"),"RIPPLE") => "Ripple",
        (Some("starlight"),"STARLIGHT") => "Starlight", (Some("tidal"),"TIDAL") => "Tidal",
        (Some("wave"),"WAVE") => "Wave", (Some("wheel"),"WHEEL") => "Wheel",
        _ => &row.title,
    }
}

fn first_layer(draft: &Draft, visible: bool) -> Option<String> {
    draft.layers().iter().find(|row|row.kind == Kind::Layer && (!visible ||
        (row.visible == Some(true) && row.parent.as_ref().is_none_or(|parent|
            draft.layers().iter().any(|group|group.id == *parent && group.visible == Some(true))))))
        .map(|row|row.id.clone())
}

pub(super) fn reconcile_selection(state: &mut State) {
    let view = &mut state.studio;
    if let Some(draft) = &view.draft {
        if view.selected.as_ref().is_some_and(|id|!draft.layers().iter().any(|row|row.id == *id)) {
            view.selected = first_layer(draft, false);
            view.canvas.clear_selection();
        }
    }
}

pub(super) fn action(state: &mut State, control: Control) {
    if matches!(control, Control::Menu) {
        state.studio.layer_menu = !state.studio.layer_menu;
        if state.studio.layer_menu { state.studio.dropdown = false; }
        return;
    }
    state.studio.layer_menu = false;
    if matches!(control, Control::Dismiss) { return; }
    let result = (|| -> Result<(), String> {
        let view = &mut state.studio;
        let draft = view.draft.as_mut().ok_or("Open a composition first")?;
        let mut clear_selection = false;
        match control {
            Control::Add(effect) => {
                let id = identity()?;
                draft.add_effect(&id, effect).map_err(|error|error.to_string())?;
                view.selected = Some(id); clear_selection = true; view.scroll = 0;
            }
            Control::Group => {
                let title = (1..=513).map(|n|format!("New group {n}"))
                    .find(|title|!draft.layers().iter().any(|row|row.kind == Kind::Group && row.title == *title))
                    .ok_or("No available group name")?;
                draft.add_group(&identity()?, &title).map_err(|error|error.to_string())?;
                view.scroll = 0;
            }
            Control::Duplicate => {
                let selected = view.selected.as_deref().ok_or("Select a layer or group")?;
                let row = draft.layers().iter().find(|row|row.id == selected).ok_or("Selected layer disappeared")?;
                let title = unique_title(draft.layers(), row);
                let count = 1 + if row.kind == Kind::Group {
                    draft.layers().iter().filter(|row|row.parent.as_deref() == Some(selected)).count()
                } else { 0 };
                let ids = (0..count).map(|_|identity()).collect::<Result<Vec<_>,_>>()?;
                draft.duplicate_item(selected, &ids, &title).map_err(|error|error.to_string())?;
                let next = first_layer(draft, true).or_else(||first_layer(draft, false));
                clear_selection = next != view.selected; view.selected = next;
            }
            Control::Delete => {
                let selected = view.selected.as_deref().ok_or("Select a layer or group")?;
                draft.delete_item(selected).map_err(|error|error.to_string())?;
                view.selected = first_layer(draft, false); clear_selection = true;
            }
            Control::Move(direction) => {
                draft.move_item(view.selected.as_deref().ok_or("Select a layer or group")?, direction)
                    .map_err(|error|error.to_string())?;
            }
            Control::Menu | Control::Dismiss => {}
        }
        if clear_selection { view.canvas.clear_selection(); }
        view.renaming = false;
        view.status = "Layer changes are local until you save".into();
        view.failed = false;
        Ok(())
    })();
    if let Err(message) = result { error(state, message); }
    selection_changed(state);
}

pub(super) unsafe fn paint(state: &State, p: &mut Painter) {
    let view = &state.studio;
    button(p,"LAYER ACTIONS",710,87,125,super::Control::Layers(Control::Menu),view.editable());
    if !view.layer_menu { return; }
    let selected = view.selected.as_deref();
    let entries = [
        ("Add Static",Control::Add(NewEffect::Static),true),
        ("Add Spectrum",Control::Add(NewEffect::Spectrum),true),
        ("Add Fire",Control::Add(NewEffect::Fire),true),
        ("New Group",Control::Group,true),
        ("Duplicate",Control::Duplicate,selected.is_some()),
        ("Move Up",Control::Move(-1),selected.is_some()),
        ("Move Down",Control::Move(1),selected.is_some()),
        ("Delete",Control::Delete,view.draft.as_ref().is_some_and(|draft|
            selected.is_some_and(|id|draft.can_delete_item(id)))),
        ("Add Breathing",Control::Add(NewEffect::Breathing),true),
        ("Add Wave",Control::Add(NewEffect::Wave),true),
    ];
    p.fill(710,116,180,entries.len() as i32*28+4,PANEL);
    // Disabled rows still block the canvas beneath the popup.
    p.hits.push(Hit{rect:p.rect(710,116,180,entries.len() as i32*28+4),
        action:Action::Studio(super::Control::Layers(Control::Dismiss))});
    for (index,(title,control,enabled)) in entries.into_iter().enumerate() {
        button(p,title,714,118+index as i32*28,172,super::Control::Layers(control),enabled && view.editable());
    }
}

pub(super) unsafe fn pointer(state: &mut State, message: u32, lparam: isize) -> bool {
    if !state.studio.open || !state.studio.layer_menu { return false; }
    match message {
        WM_LBUTTONDOWN => true,
        WM_LBUTTONUP => {
            let point = POINT{x:(lparam as u32 & 0xffff) as i16 as i32,
                y:((lparam as u32 >> 16)&0xffff) as i16 as i32};
            let action = state.hits.iter().rev().find(|hit|PtInRect(&hit.rect,point)!=0)
                .and_then(|hit|match &hit.action {Action::Studio(super::Control::Layers(control))=>Some(control.clone()),_=>None})
                .unwrap_or(Control::Dismiss);
            state.action(Action::Studio(super::Control::Layers(action))); true
        }
        WM_CANCELMODE => { state.studio.layer_menu=false; InvalidateRect(state.hwnd,null(),0); true }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secbird_core::studio::document::LayerRow;
    fn row(title: &str, effect: &str) -> LayerRow {
        LayerRow{id:"a".into(),title:title.into(),kind:Kind::Layer,parent:None,
            visible:Some(true),opened:None,effect:Some(effect.into())}
    }
    #[test]
    fn duplicate_titles_match_effect_identity_and_sibling_scope() {
        let static_layer=row("STATIC","static");
        assert_eq!(unique_title(&[static_layer.clone()],&static_layer),"STATIC");
        let renamed_fire=row("STATIC","fire");
        assert_eq!(unique_title(&[renamed_fire.clone()],&renamed_fire),"STATIC (1)");
        assert_eq!(display_title(&static_layer),"Static");
        assert_eq!(display_title(&renamed_fire),"STATIC");
        let current=row("Desk (7)","static");let mut sibling=row("Desk (1)","static");
        assert_eq!(unique_title(&[current.clone(),sibling.clone()],&current),"Desk (2)");
        sibling.parent=Some("other".into());
        assert_eq!(unique_title(&[current.clone(),sibling],&current),"Desk (1)");
    }
    #[test]
    fn secure_layer_ids_have_the_source_twelve_lowercase_hex_shape() {
        let mut ids=std::collections::BTreeSet::new();
        for _ in 0..256 {
            let id=identity().unwrap();assert_eq!(id.len(),12);
            assert!(id.bytes().all(|c|c.is_ascii_digit() || (b'a'..=b'f').contains(&c)));
            assert!(ids.insert(id));
        }
    }
}
