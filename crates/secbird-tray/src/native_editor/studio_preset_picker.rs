//! On-demand offline device presets. Native menu presentation is interim.
use super::*;
use serde::Deserialize;
use std::io::Read;
use windows_sys::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};

#[derive(Clone, Deserialize)]
struct Record {
    product_id:u16, edition_id:u16, layout_id:u16,
    #[serde(default)] product_name:String,
    file:String, bytes:usize, sha256:String,
}
#[derive(Deserialize)]
struct Index { format:String, records:Vec<Record> }

pub(super) fn same_devices(left:&str,right:&str)->bool {
    #[derive(Deserialize)]
    struct Devices<'a> {#[serde(borrow)] devices:&'a serde_json::value::RawValue}
    match (serde_json::from_str::<Devices>(left),serde_json::from_str::<Devices>(right)) {
        (Ok(a),Ok(b))=>a.devices.get()==b.devices.get(), _=>false,
    }
}

fn read(path:&Path,max:usize)->Result<Vec<u8>,String> {
    let file=File::open(path).map_err(|e|e.to_string())?;
    if !file.metadata().map_err(|e|e.to_string())?.is_file() {return Err("Preset resource must be a regular file".into());}
    let mut bytes=Vec::new();file.take(max as u64+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    if bytes.len()>max {return Err("Preset resource exceeds its size budget".into());} Ok(bytes)
}
fn records(assets:&Path)->Result<Vec<Record>,String> {
    let index:Index=serde_json::from_slice(&read(&assets.join("studio-presets/index.json"),2*1024*1024)?).map_err(|e|e.to_string())?;
    if index.format!="synapse-lite-studio-presets-v1" || index.records.is_empty() || index.records.len()>4096 {
        return Err("Preset index is unavailable or unsupported".into());
    }
    let mut seen=std::collections::BTreeSet::new();
    for record in &index.records {
        if record.file!=format!("studio-{}-{}-{}.json",record.product_id,record.edition_id,record.layout_id)
            || record.bytes==0 || record.bytes>2*1024*1024
            || record.sha256.len()!=64 || !record.sha256.bytes().all(|c|c.is_ascii_hexdigit())
            || record.product_name.len()>256 || record.product_name.chars().any(char::is_control)
            || !seen.insert((record.product_id,record.edition_id,record.layout_id)) {
            return Err("Preset index has invalid or ambiguous records".into());
        }
    }
    Ok(index.records)
}
fn geometry(assets:&Path,record:&Record)->Result<source_layout::SourceLayout,String> {
    let bytes=read(&assets.join("studio-presets").join(&record.file),2*1024*1024)?;
    let mut hash=[0u8;32];
    let status=unsafe {BCryptHash(BCRYPT_SHA256_ALG_HANDLE,null(),0,bytes.as_ptr(),bytes.len() as u32,hash.as_mut_ptr(),32)};
    let digest:String=hash.iter().map(|b|format!("{b:02x}")).collect();
    if status<0 || bytes.len()!=record.bytes || !digest.eq_ignore_ascii_case(&record.sha256) {return Err("Preset differs from its packaged index".into());}
    let g=source_layout::SourceLayout::from_slice(&bytes)?;
    if (g.product_id,g.edition_id,g.layout_id)!=(record.product_id,record.edition_id,record.layout_id) {return Err("Preset identity differs from its index".into());}
    Ok(g)
}

struct Menu(HMENU);
impl Drop for Menu {fn drop(&mut self) {unsafe {DestroyMenu(self.0);}}}
unsafe fn menu()->Result<Menu,String> {let handle=CreatePopupMenu();if handle.is_null(){Err("Could not open device presets".into())}else{Ok(Menu(handle))}}
unsafe fn choose(hwnd:HWND,items:&[Record])->Result<Option<Record>,String> {
    let root=menu()?;
    let mut groups:BTreeMap<u16,Vec<usize>>=BTreeMap::new();
    for (i,record) in items.iter().enumerate(){groups.entry(record.product_id).or_default().push(i);}
    for (product,indices) in groups {
        let child=menu()?;
        for i in &indices {
            let r=&items[*i]; let title=format!("Edition {} / layout {}",r.edition_id,r.layout_id);
            if AppendMenuW(child.0,MF_STRING,*i+1,wide(&title).as_ptr())==0 {return Err("Could not populate preset layouts".into());}
        }
        let name=&items[indices[0]].product_name;
        let title=if name.is_empty(){format!("Razer device {product}")}else{format!("{} ({product})",name.replace('&',"&&"))};
        if AppendMenuW(root.0,MF_POPUP,child.0 as usize,wide(&title).as_ptr())==0{return Err("Could not populate device presets".into());}
        std::mem::forget(child); // Root now owns the submenu.
    }
    let mut point:POINT=std::mem::zeroed();if GetCursorPos(&mut point)==0{return Err("Could not position device presets".into());}
    let selected=TrackPopupMenu(root.0,TPM_RETURNCMD|TPM_NONOTIFY,point.x,point.y,0,hwnd,null()) as usize;
    Ok(selected.checked_sub(1).and_then(|index|items.get(index)).cloned())
}

fn placement(draft:&Draft,g:&source_layout::SourceLayout,id:&str)->Result<String,String> {
    #[derive(Deserialize)] struct Extent {x:f64,width:f64}
    #[derive(Deserialize)] struct Existing {devices:Vec<Extent>}
    let current:Existing=serde_json::from_str(draft.json()).map_err(|e|e.to_string())?;
    if current.devices.len()>=32 {return Err("Canvas supports at most 32 placements".into());}
    // Place to the right without guessing a physical device's source binding.
    let mut x=0f64;
    for device in &current.devices {
        if !device.x.is_finite()||!device.width.is_finite()||device.width<=0. {return Err("Existing device geometry is invalid".into());}
        x=x.max(device.x+device.width+24.);
    }
    let mut leds=Vec::new();
    for key in &g.keys {
        let (row,col)=key.input_id.split_once(':').ok_or("Preset LED identity is invalid")?;
        leds.push(json!({"row":row.parse::<u16>().map_err(|e|e.to_string())?,"col":col.parse::<u16>().map_err(|e|e.to_string())?}));
    }
    Ok(json!({"id":id,"productId":g.product_id,"editionId":g.edition_id,"layout":g.layout_id,
        "x":x,"y":0,"width":g.view_box[2],"height":g.view_box[3],"orientation":0,
        "editorPreset":true,"leds":leds}).to_string())
}

unsafe fn add(state:&mut State,record:&Record)->Result<(),String> {
    if state.read_only_smoke || state.restricted_actions || !state.studio.editable() || state.studio.output.busy {return Err("Device presets are unavailable in the current editor state".into());}
    let g=geometry(&assets_dir(),record)?;
    let id=layer_tools::identity()?;
    let draft=state.studio.draft.as_ref().ok_or("Open a composition first")?;
    let raw=placement(draft,&g,&id)?;
    // Validate the full candidate canvas before admitting the reversible edit.
    let mut trial=Draft::open(draft.json()).map_err(|e|e.to_string())?;
    trial.add_device(&raw).map_err(|e|e.to_string())?;
    let canvas=studio_canvas::Canvas::load(trial.json(),&assets_dir());
    if !canvas.notes.is_empty() {return Err(format!("Cannot add this preset: {}",canvas.notes.join("; ")));}
    state.studio.draft.as_mut().unwrap().add_device(&raw).map_err(|e|e.to_string())?;
    state.studio.canvas=canvas;playback::stop(state);
    state.studio.canvas.set_zoom(100.,None,f64::from(state.width)/f64::from(state.scale),f64::from(state.height)/f64::from(state.scale));
    selection_changed(state);
    state.studio.status="Device preset added for offline editing. Save to keep it; physical playback is unavailable.".into();
    state.studio.failed=false;Ok(())
}

/// Carry the Lighting page's known layout into the current local composition.
/// Existing placements and layers stay intact; presets grant no hardware authority.
pub(super) unsafe fn ensure_device(state: &mut State, key: (u16, u16, u16)) -> Result<(), String> {
    if state.read_only_smoke || state.restricted_actions {
        return Err("Device presets are unavailable in the current editor state".into());
    }
    if state.studio.output.busy || state.studio.busy || state.studio.dialog {
        return Err("The Studio editor is busy; open it again when the current operation finishes".into());
    }
    if let Some(draft) = &state.studio.draft {
        #[derive(Deserialize)]
        struct Identity {
            #[serde(rename = "productId")] product: u16,
            #[serde(rename = "editionId")] edition: u16,
            layout: u16,
        }
        #[derive(Deserialize)]
        struct Devices { devices: Vec<Identity> }
        let existing: Devices = serde_json::from_str(draft.json()).map_err(|e| e.to_string())?;
        if existing.devices.iter().any(|device| (device.product, device.edition, device.layout) == key) {
            return Ok(());
        }
    }
    let record = records(&assets_dir())?.into_iter()
        .find(|record| (record.product_id, record.edition_id, record.layout_id) == key)
        .ok_or("No verified Studio layout is packaged for this device")?;
    // Validate assets before creating a draft, so a missing resource leaves the library alone.
    geometry(&assets_dir(), &record)?;
    if state.studio.draft.is_none() {
        library::proceed(state, library::Next::Create);
        if state.studio.draft.is_none() { return Err(state.studio.status.clone()); }
    }
    add(state, &record)
}
pub(super) unsafe fn dialog(hwnd:HWND,pointer:*mut RefCell<State>) {
    let epoch={let Ok(mut state)=(*pointer).try_borrow_mut() else{return;};
        if state.read_only_smoke||state.restricted_actions||!state.studio.editable()||state.studio.output.busy{return;}
        state.studio.dialog=true;state.studio.epoch};
    let chosen=records(&assets_dir()).and_then(|items|choose(hwnd,&items));
    let Ok(mut state)=(*pointer).try_borrow_mut() else{return;};state.studio.dialog=false;
    if state.studio.epoch!=epoch{return;}
    match chosen {Ok(Some(record))=>if let Err(message)=add(&mut state,&record){error(&mut state,message)},Ok(None)=>{},Err(message)=>error(&mut state,message)}
    redraw(&mut state);
}

#[cfg(test)]
pub(super) unsafe fn add_test(state:&mut State,key:(u16,u16,u16)) {
    let result=records(&assets_dir()).and_then(|items|items.into_iter().find(|r|(r.product_id,r.edition_id,r.layout_id)==key).ok_or("Preset absent".into())).and_then(|record|add(state,&record));
    if let Err(message)=result{error(state,message);}redraw(state);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ambiguous_and_unsafe_index_entries_refuse_before_reading_geometry() {
        let root=std::env::temp_dir().join(format!("studio-picker-index-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(root.join("studio-presets")).unwrap();
        let record=json!({"product_id":100,"edition_id":0,"layout_id":0,"product_name":"Razer Example","file":"studio-100-0-0.json","bytes":100,"sha256":"a".repeat(64)});
        let write=|rows:Vec<Value>|std::fs::write(root.join("studio-presets/index.json"),serde_json::to_vec(&json!({"format":"synapse-lite-studio-presets-v1","records":rows})).unwrap()).unwrap();
        write(vec![record.clone()]);assert_eq!(records(&root).unwrap().len(),1);
        write(vec![record.clone(),record.clone()]);assert!(records(&root).is_err());
        for (field,value) in [("file",json!("../outside.json")),("sha256",json!("not-a-digest")),("product_name",json!("Razer\u{0000}Example")),("bytes",json!(2097153))] {
            let mut bad=record.clone();bad[field]=value;write(vec![bad]);assert!(records(&root).is_err());
        }
        write(vec![record]);let item=records(&root).unwrap().remove(0);
        std::fs::write(root.join("studio-presets/studio-100-0-0.json"),b"tampered").unwrap();assert!(geometry(&root,&item).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn layer_history_does_not_reset_canvas_selection_but_device_history_does() {
        let a=r#"{"devices":[{"id":"one","future":9.9900e+100}],"layers":[]}"#;
        let b=a.replace("\"layers\":[]","\"layers\":[{\"opaque\":true}]");assert!(same_devices(a,&b));
        let c=a.replace("\"one\"","\"two\"");assert!(!same_devices(a,&c));
        assert!(!same_devices(a,"{}"));
    }
}
