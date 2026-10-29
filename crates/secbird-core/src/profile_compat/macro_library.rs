//! Global source macro library. Pure store edits; no playback or device I/O.
use super::*;
use serde::{Deserialize,Serialize};
use serde_json::value::RawValue;
use std::collections::BTreeSet;
pub mod input_events;
pub mod nested;
pub mod loops;
use input_events::{Input,Gesture};

const KEY:&str="_synapseLiteMacroLibrary";
const MAX_MACROS:usize=256;
const MAX_EVENTS:usize=512;
const MAX_PAYLOADS:usize=1024*1024;

#[derive(Clone)]
pub struct Entry {
    pub guid:String,
    pub name:String,
    pub events:Vec<Value>,
    pub referenced:bool,
    pub revision:String,
    pub encoded_record:Value,
}
pub struct Library {pub entries:Vec<Entry>,pub missing:Vec<String>,pub dependency_issues:usize}
#[derive(Clone)]
pub enum Change {
    Create{guid:String,name:String},
    Rename{guid:String,name:String},
    Duplicate{guid:String,new_guid:String,name:String},
    Delete(String),
    Import(Vec<u8>),
    Replace{guid:String,expected_revision:String,record:Value},
    InsertDelay{guid:String,index:usize,milliseconds:u32},
    InsertInput{guid:String,index:usize,input:Input,gesture:Gesture},
    RebindInput{guid:String,index:usize,input:Input},
    DeleteInputGroup{guid:String,index:usize},
    InsertNested{guid:String,index:usize,target:String},
    RebindNested{guid:String,index:usize,target:String},
    WrapLoop{guid:String,index:usize,end:usize,repeat:u32},
    SetLoopRepeat{guid:String,index:usize,repeat:u32},
    DeleteEvent{guid:String,index:usize},
    MoveEvent{guid:String,index:usize,direction:i8},
    SetDelay{guid:String,index:usize,milliseconds:u32},
}
fn global(store:&Store)->Result<Vec<Value>> {
    let Some(value)=store.extra.get(KEY) else{return Ok(vec![]);};
    if value["version"]!=1 {return Err(invalid("Unsupported macro library version; no data changed"));}
    value["records"].as_array().cloned().ok_or_else(||invalid("Macro library records are malformed"))
}
fn guid(record:&Value)->Result<String> {
    let body=decode_record(record)?;
    let value=body["guid"].as_str().ok_or_else(||invalid("Macro has no GUID"))?;
    if value.is_empty()||value.len()>128{return Err(invalid("Invalid macro GUID"));}Ok(value.into())
}
fn collect(store:&Store)->Result<BTreeMap<String,Value>> {
    validate_store_budget(store)?;
    let mut out=BTreeMap::new();
    let mut add=|record:Value|->Result<()> {
        let id=guid(&record)?;
        if let Some(old)=out.get(&id) {if old!=&record{return Err(invalid("Conflicting source macro definitions; no winner selected"));}}
        else {out.insert(id,record);}
        Ok(())
    };
    for record in global(store)? {add(record)?;}
    for device in store.devices.values(){for (_,record) in gui::macro_records(device)? {add(record)?;}}
    if out.len()>MAX_MACROS{return Err(invalid("Macro library exceeds256 definitions"));}
    let total:usize=out.values().map(|r|r["payload"].as_str().map_or(0,str::len)).sum();
    if total>MAX_PAYLOADS{return Err(invalid("Macro library encoded payloads exceed1MiB"));}
    Ok(out)
}
fn references(store:&Store)->BTreeSet<String> {
    let mut refs=BTreeMap::new();
    for device in store.devices.values(){for profile in device.profiles.values(){for value in profile.extra.values(){gui::refs(value,&mut refs);}}}
    refs.into_keys().collect()
}
fn contains_id(value:&Value,id:&str)->bool {
    match value {Value::String(s)=>s==id,Value::Array(a)=>a.iter().any(|v|contains_id(v,id)),Value::Object(o)=>o.values().any(|v|contains_id(v,id)),_=>false}
}
fn referenced(store:&Store,records:&BTreeMap<String,Value>,id:&str)->Result<bool> {
    if references(store).contains(id){return Ok(true);}
    for device in store.devices.values(){for profile in device.profiles.values(){if profile.extra.values().any(|v|contains_id(v,id)){return Ok(true);}}}
    for (key,value) in &store.extra {if key!=KEY&&contains_id(value,id){return Ok(true);}}
    for (other,record) in records {if other!=id&&contains_id(&decode_record(record)?,id){return Ok(true);}}
    Ok(false)
}
pub fn read(store:&Store)->Result<Library> {
    let records=collect(store)?;let mut refs=references(store);let mut entries=vec![];
    let graph=nested::Graph::new(&records)?;refs.extend(graph.references());let dependency_issues=graph.issues();
    for (id,record) in &records {
        let body=decode_record(record)?;let events=body["macroEvents"].as_array().ok_or_else(||invalid("Macro has no event array"))?;
        if events.len()>MAX_EVENTS{return Err(invalid("Macro exceeds512 event rows"));}
        let name=record["name"].as_str().ok_or_else(||invalid("Macro has no name"))?;
        entries.push(Entry{guid:id.clone(),name:name.into(),events:events.clone(),referenced:referenced(store,&records,id)?,revision:revision(record)?,encoded_record:record.clone()});
    }
    entries.sort_by_key(|r|(r.name.to_lowercase(),r.guid.clone()));
    Ok(Library{missing:refs.into_iter().filter(|id|!records.contains_key(id)).collect(),entries,dependency_issues})
}
fn name(value:&str)->Result<()> {
    if value.trim().is_empty()||value.encode_utf16().count()>128||value.chars().any(char::is_control){return Err(invalid("Macro names need1..128 characters without control characters"));}Ok(())
}
fn fresh(store:&Store,records:&BTreeMap<String,Value>,id:&str)->Result<()> {
    if id.is_empty()||id.len()>128||id.chars().any(char::is_control)||records.contains_key(id)||referenced(store,records,id)?{return Err(invalid("Macro GUID is invalid, already present or referenced"));}Ok(())
}
fn record(body:&Value,title:&str)->Result<Value> {
    Ok(json!({"name":title,"payload":STANDARD.encode(serde_json::to_vec(body).map_err(|e|invalid(e.to_string()))?),"hash":synapse_hash(body)?}))
}
// Root leaves remain raw while a known field is changed. A duplicate root key
// cannot be silently collapsed during an authoring operation.
struct Fields(Vec<(String,Box<RawValue>)>);
impl<'de> Deserialize<'de> for Fields {
    fn deserialize<D:serde::Deserializer<'de>>(d:D)->std::result::Result<Self,D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value=Fields;
            fn expecting(&self,f:&mut std::fmt::Formatter)->std::fmt::Result {f.write_str("unique bounded macro fields")}
            fn visit_map<A:serde::de::MapAccess<'de>>(self,mut map:A)->std::result::Result<Fields,A::Error> {
                let mut out=vec![];let mut seen=BTreeSet::new();
                while let Some(key)=map.next_key::<String>()? {if !seen.insert(key.clone())||seen.len()>128{return Err(serde::de::Error::custom("Duplicate or excessive macro fields"));}out.push((key,map.next_value()?));}Ok(Fields(out))
            }
        }
        d.deserialize_map(Visitor)
    }
}
impl Serialize for Fields {
    fn serialize<S:serde::Serializer>(&self,s:S)->std::result::Result<S::Ok,S::Error> {
        use serde::ser::SerializeMap;let mut map=s.serialize_map(Some(self.0.len()))?;for (key,value) in &self.0 {map.serialize_entry(key,value)?;}map.end()
    }
}
fn payload(record:&Value)->Result<Fields> {
    decode_record(record)?;
    let text=record["payload"].as_str().unwrap();
    let bytes=STANDARD.decode(text).or_else(|_|STANDARD_NO_PAD.decode(text)).or_else(|_|URL_SAFE.decode(text)).or_else(|_|URL_SAFE_NO_PAD.decode(text)).map_err(|e|invalid(e.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|e|invalid(e.to_string()))
}
fn patch(record:&Value,field:&str,value:Box<RawValue>)->Result<Value> {
    let mut body=payload(record)?;
    let old=body.0.iter_mut().find(|(key,_)|key==field).ok_or_else(||invalid("Macro field is missing"))?;old.1=value;
    let bytes=serde_json::to_vec(&body).map_err(|e|invalid(e.to_string()))?;let value:Value=serde_json::from_slice(&bytes).map_err(|e|invalid(e.to_string()))?;
    let mut out=record.clone();out["payload"]=json!(STANDARD.encode(bytes));out["hash"]=json!(synapse_hash(&value)?);Ok(out)
}
fn raw(value:&impl Serialize)->Result<Box<RawValue>> {RawValue::from_string(serde_json::to_string(value).map_err(|e|invalid(e.to_string()))?).map_err(|e|invalid(e.to_string()))}
fn event_rows(record:&Value)->Result<Vec<Box<RawValue>>> {
    let body=payload(record)?;
    if body.0.iter().any(|(key,_)|key=="appEngine"){return Err(invalid("Macro contains a native event cache; source cache invalidation needs verification before event edits"));}
    let events=body.0.iter().find(|(key,_)|key=="macroEvents").ok_or_else(||invalid("Macro has no event array"))?;
    let rows:Vec<Box<RawValue>>=serde_json::from_str(events.1.get()).map_err(|e|invalid(e.to_string()))?;
    if rows.len()>MAX_EVENTS{return Err(invalid("Macro exceeds512 event rows"));}Ok(rows)
}
fn delay(milliseconds:u32)->Result<Box<RawValue>> {
    if milliseconds>3_600_000{return Err(invalid("Delay exceeds one hour"));}
    let number=if milliseconds%1000==0{(milliseconds/1000).to_string()}else{format!("{}.{:03}",milliseconds/1000,milliseconds%1000).trim_end_matches('0').to_owned()};
    raw(&json!({"Type":0,"Number":number,"selected":false}))
}
fn protected(row:&RawValue)->bool {serde_json::from_str::<Value>(row.get()).ok().is_some_and(|v|v["Type"]=="actionBar")}
fn install(store:&mut Store,records:Vec<Value>)->Result<()> {
    // Preserve future metadata in this known versioned library object.
    if !store.extra.contains_key(KEY){store.extra.insert(KEY.into(),json!({"version":1,"records":[]}));}
    store.extra.get_mut(KEY).unwrap()["records"]=json!(records);
    Ok(())
}
fn propagate(store:&mut Store,record:&Value)->Result<()> {
    let id=guid(record)?;
    for device in store.devices.values_mut() {
        let mut wanted=BTreeMap::new();for p in device.profiles.values(){for v in p.extra.values(){gui::refs(v,&mut wanted);}}
        let keys:Vec<String>=device.extra.iter().filter(|(key,value)|(**key==SOURCE||key.starts_with(&format!("_synapseLiteImported:{SOURCE}")))&&value.get("profiles").is_some_and(Value::is_array)).map(|(key,_)|key.clone()).collect();
        let mut found=false;
        for key in &keys {
            let envelope=device.extra.get_mut(key).unwrap();
            if let Some(array)=envelope.get_mut("macros").and_then(Value::as_array_mut){for old in array {if guid(old)?==id{*old=record.clone();found=true;}}}
        }
        if wanted.contains_key(&id)&&!found {
            let key=keys.first().cloned().ok_or_else(||invalid("Referenced macro has no retained source envelope; no profiles changed"))?;
            let envelope=device.extra.get_mut(&key).unwrap().as_object_mut().unwrap();
            envelope.entry("macros").or_insert_with(||json!([])).as_array_mut().ok_or_else(||invalid("Retained macro list is malformed"))?.push(record.clone());
        }
    }
    Ok(())
}
pub fn edit(current:&Store,change:Change)->Result<Store> {
    let mut records=collect(current)?;let mut candidate=current.clone();
    let mut changed=None;
    let mut explicit_target=None;
    match change {
        Change::Replace{guid:id,expected_revision,record}=>{
            let old=records.get(&id).ok_or_else(||invalid("Macro disappeared; refresh before saving"))?;
            if revision(old)?!=expected_revision{return Err(invalid("Macro changed elsewhere; the draft was not saved"));}
            if guid(&record)?!=id{return Err(invalid("Edited macro GUID changed"));}
            records.insert(id,record.clone());changed=Some(record);
        }
        Change::Create{guid:id,name:title}=>{fresh(current,&records,&id)?;name(&title)?;let body=json!({"guid":id,"macroEvents":[{"Type":"actionBar","recordProfile":{},"selected":false}],"macroPlaybackOption":""});let r=record(&body,&title)?;records.insert(id,r.clone());changed=Some(r);}
        Change::Rename{guid:id,name:title}=>{name(&title)?;let r=records.get_mut(&id).ok_or_else(||invalid("Unknown macro"))?;r["name"]=json!(title);changed=Some(r.clone());}
        Change::Duplicate{guid:id,new_guid,name:title}=>{fresh(current,&records,&new_guid)?;name(&title)?;let original=records.get(&id).ok_or_else(||invalid("Unknown macro"))?;let mut r=patch(original,"guid",raw(&new_guid)?)?;r["name"]=json!(title);records.insert(new_guid,r.clone());changed=Some(r);}
        Change::Delete(id)=>{
            if referenced(current,&records,&id)?{return Err(invalid("Macro is referenced by a profile or another macro; it was not deleted"));}
            if records.remove(&id).is_none(){return Err(invalid("Unknown macro"));}
            for device in candidate.devices.values_mut(){for (key,envelope) in &mut device.extra {if key==SOURCE||key.starts_with(&format!("_synapseLiteImported:{SOURCE}")){if let Some(rows)=envelope.get_mut("macros").and_then(Value::as_array_mut){let mut kept=vec![];for row in rows.drain(..){if guid(&row)?!=id{kept.push(row);}}*rows=kept;}}}}
        }
        Change::Import(bytes)=>{
            if bytes.len()>MAX_DECODED_BYTES*2{return Err(invalid("Macro import exceeds2MiB"));}check_json_complexity(&bytes)?;
            let source:Value=serde_json::from_slice(&bytes).map_err(|e|invalid(e.to_string()))?;
            let incoming=source["macros"].as_array().ok_or_else(||invalid("Choose a Synapse4 profile export containing macro records"))?;
            if incoming.len()>MAX_MACROS{return Err(invalid("Too many imported macros"));}
            for r in incoming {let id=guid(r)?;if let Some(old)=records.get(&id){if old!=r{return Err(invalid("Import conflicts with an existing macro GUID"));}}else{records.insert(id,r.clone());}propagate(&mut candidate,r)?;}
        }
        other=>{
            let (id,index)=match &other {Change::WrapLoop{guid,index,..}|Change::SetLoopRepeat{guid,index,..}|Change::InsertDelay{guid,index,..}|Change::InsertInput{guid,index,..}|Change::RebindInput{guid,index,..}|Change::DeleteInputGroup{guid,index}|Change::InsertNested{guid,index,..}|Change::RebindNested{guid,index,..}|Change::DeleteEvent{guid,index}|Change::MoveEvent{guid,index,..}|Change::SetDelay{guid,index,..}=>(guid.clone(),*index),_=>unreachable!()};
            let original=records.get(&id).ok_or_else(||invalid("Unknown macro"))?;let mut rows=event_rows(original)?;
            loops::validate(&rows)?;
            match other {
                Change::WrapLoop{end,repeat,..}=>loops::wrap(&mut rows,index,end,repeat)?,
                Change::SetLoopRepeat{repeat,..}=>loops::set_repeat(&mut rows,index,repeat)?,
                Change::InsertDelay{milliseconds,..}=>{if rows.len()>=MAX_EVENTS||index>rows.len(){return Err(invalid("Event insertion exceeds macro bounds"));}rows.insert(index,delay(milliseconds)?);}
                Change::InsertInput{input,gesture,..}=>{
                    let parsed:Vec<Value>=rows.iter().map(|row|serde_json::from_str(row.get()).map_err(|e|invalid(e.to_string()))).collect::<Result<_>>()?;
                    let added=input_events::rows(&parsed,index,&input,gesture)?;
                    if index>rows.len()||rows.len()+added.len()>MAX_EVENTS{return Err(invalid("Event insertion exceeds macro bounds"));}
                    let added=added.iter().map(raw).collect::<Result<Vec<_>>>()?;rows.splice(index..index,added);
                }
                Change::RebindInput{input,..}=>{rows=input_events::rebind(&rows,index,&input)?;}
                Change::DeleteInputGroup{..}=>{rows=input_events::delete_group(&rows,index)?;}
                Change::InsertNested{target,..}=>{if rows.len()>=MAX_EVENTS||index>rows.len()||index==0&&rows.first().is_some_and(|row|protected(row)){return Err(invalid("Nested event insertion exceeds macro bounds"));}rows.insert(index,nested::row(&target)?);explicit_target=Some(target);}
                Change::RebindNested{target,..}=>{let old=rows.get(index).ok_or_else(||invalid("Select a nested macro event"))?;rows[index]=nested::retarget(old,&target)?;explicit_target=Some(target);}
                Change::SetDelay{milliseconds,..}=>{let old=rows.get(index).ok_or_else(||invalid("Unknown event"))?;let value:Value=serde_json::from_str(old.get()).map_err(|e|invalid(e.to_string()))?;if value["Type"]!=0{return Err(invalid("Select a delay event"));}
                    if value["Number"].is_object()||value["Number"].is_array(){return Err(invalid("Random/structured delay editing is not implemented; its source data was preserved"));}
                    // Preserve unknown leaves in an explicitly edited delay row.
                    let mut fields:Fields=serde_json::from_str(old.get()).map_err(|e|invalid(e.to_string()))?;let new:Value=serde_json::from_str(delay(milliseconds)?.get()).unwrap();let field=fields.0.iter_mut().find(|(key,_)|key=="Number").ok_or_else(||invalid("Delay has no Number field"))?;field.1=raw(&new["Number"])?;rows[index]=raw(&fields)?;}
                Change::DeleteEvent{..}=>{if index>=rows.len()||protected(&rows[index]){return Err(invalid("This event cannot be deleted"));}if serde_json::from_str::<Value>(rows[index].get()).map_err(|e|invalid(e.to_string()))?["Type"]==6{loops::delete(&mut rows,index)?;}else{rows.remove(index);}}
                Change::MoveEvent{direction,..}=>{if index>=rows.len(){return Err(invalid("Unknown event"));}let next=index.checked_add_signed(direction as isize).filter(|n|*n<rows.len()).ok_or_else(||invalid("Event move exceeds macro bounds"))?;if ![-1,1].contains(&direction)||protected(&rows[index])||protected(&rows[next]){return Err(invalid("This event cannot be moved"));}rows.swap(index,next);}
                _=>unreachable!(),
            }
            loops::validate(&rows)?;
            let r=patch(original,"macroEvents",raw(&rows)?)?;records.insert(id,r.clone());changed=Some(r);
        }
    }
    if let Some(record)=changed {
        let id=guid(&record)?;let before=collect(current)?;nested::validate_change(before.get(&id),&record,&records,explicit_target.as_deref())?;
        propagate(&mut candidate,&record)?;
    }
    nested::retain_required(&mut candidate,&records)?;
    install(&mut candidate,records.into_values().collect())?;
    read(&candidate)?;validate_store_budget(&candidate)?;Ok(candidate)
}

/// Optimistic record identity, not authentication or proof of file ownership.
pub fn revision(record:&Value)->Result<String> {decode_record(record)?;Ok(format!("{:x}",md5::compute(serde_json::to_vec(record).map_err(|e|invalid(e.to_string()))?)))}

/// A local editor uses the same pure mutations on a one-record workspace. The
/// final Replace checks the original revision in a freshly loaded real store,
/// and propagates that definition without replacing profiles from a stale copy.
pub fn edit_definition(record:&Value,change:Change)->Result<Value> {
    edit_definition_with_library(record,change,&[])
}
pub(super) fn definition_store(record:&Value,context:&[Value])->Result<Store> {
    let id=guid(record)?;let mut rows=vec![];
    for other in context {if guid(other)?!=id{rows.push(other.clone());}}
    rows.push(record.clone());let mut store=Store::default();store.extra.insert(KEY.into(),json!({"version":1,"records":rows}));collect(&store)?;Ok(store)
}
pub fn edit_definition_with_library(record:&Value,change:Change,context:&[Value])->Result<Value> {
    let id=guid(record)?;
    match &change {
        Change::WrapLoop{guid,..}|Change::SetLoopRepeat{guid,..}|Change::Rename{guid,..}|Change::InsertDelay{guid,..}|Change::InsertInput{guid,..}|Change::RebindInput{guid,..}|Change::DeleteInputGroup{guid,..}|Change::InsertNested{guid,..}|Change::RebindNested{guid,..}|Change::DeleteEvent{guid,..}|Change::MoveEvent{guid,..}|Change::SetDelay{guid,..} if guid==&id=>{},
        _=>return Err(invalid("This operation is not a local definition edit")),
    }
    let isolated=definition_store(record,context)?;
    let edited=edit(&isolated,change)?;collect(&edited)?.remove(&id).ok_or_else(||invalid("Edited macro disappeared"))
}

pub fn describe(record:&Value)->Result<Entry> {
    let mut isolated=Store::default();isolated.extra.insert(KEY.into(),json!({"version":1,"records":[record]}));
    read(&isolated)?.entries.pop().ok_or_else(||invalid("Macro is unavailable"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture()->Store {import_document(include_bytes!("../../tests/fixtures/synthetic.synapse4")).unwrap().store}
    fn created()->Store {edit(&Store::default(),Change::Create{guid:"new".into(),name:"New macro".into()}).unwrap()}
    #[test]
    fn rename_preserves_encoded_body_profiles_settings_and_source_export() {
        let before=fixture();let entry=read(&before).unwrap().entries.remove(0);let device=before.devices.keys().next().unwrap().clone();
        let after=edit(&before,Change::Rename{guid:entry.guid.clone(),name:"Renamed".into()}).unwrap();
        assert_eq!(after.devices[&device].profiles,before.devices[&device].profiles);assert_eq!(after.devices[&device].active,before.devices[&device].active);
        let renamed=read(&after).unwrap().entries.remove(0);assert_eq!(renamed.encoded_record["payload"],entry.encoded_record["payload"]);assert_eq!(renamed.encoded_record["hash"],entry.encoded_record["hash"]);
        let old:Value=serde_json::from_slice(&export_synapse4(&before,&device).unwrap().bytes).unwrap();let new:Value=serde_json::from_slice(&export_synapse4(&after,&device).unwrap().bytes).unwrap();assert_eq!(old["profiles"],new["profiles"]);assert_eq!(new["macros"][0]["name"],"Renamed");assert_eq!(new["macros"][0]["payload"],old["macros"][0]["payload"]);
    }
    #[test]
    fn imported_definition_recovers_reference_without_profile_reimport() {
        let original=fixture();let entry=read(&original).unwrap().entries.remove(0);let mut missing=original.clone();
        for device in missing.devices.values_mut(){for envelope in device.extra.values_mut(){if envelope.get("macros").is_some(){envelope["macros"]=json!([]);}}}
        let before:Vec<_>=missing.devices.values().map(|d|d.profiles.clone()).collect();assert!(read(&missing).unwrap().entries.is_empty());
        let document=serde_json::to_vec(&json!({"macros":[entry.encoded_record]})).unwrap();let recovered=edit(&missing,Change::Import(document.clone())).unwrap();
        assert_eq!(recovered.devices.values().map(|d|d.profiles.clone()).collect::<Vec<_>>(),before);assert_eq!(read(&recovered).unwrap().entries.len(),1);
        let again=edit(&recovered,Change::Import(document)).unwrap();assert_eq!(serde_json::to_value(&again).unwrap(),serde_json::to_value(&recovered).unwrap());
    }
    #[test]
    fn duplicate_changes_only_guid_and_name_delete_keeps_referenced_definitions() {
        let before=fixture();let entry=read(&before).unwrap().entries.remove(0);
        let copy=edit(&before,Change::Duplicate{guid:entry.guid.clone(),new_guid:"copy".into(),name:"Copy".into()}).unwrap();
        assert_eq!(read(&copy).unwrap().entries.len(),2);let duplicated=read(&copy).unwrap().entries.into_iter().find(|e|e.guid=="copy").unwrap();assert_eq!(duplicated.events,entry.events);
        let mut referenced=copy.clone();let key=referenced.devices.keys().next().unwrap().clone();let profile=referenced.devices.get_mut(&key).unwrap().profiles.values_mut().next().unwrap();profile.extra.insert("test-ref".into(),json!({"macroGroup":{"guid":"copy"}}));
        assert!(edit(&referenced,Change::Delete("copy".into())).is_err());let deleted=edit(&copy,Change::Delete("copy".into())).unwrap();assert_eq!(read(&deleted).unwrap().entries.len(),1);
    }
    #[test]
    fn local_delay_edits_keep_opaque_payload_and_other_rows_raw() {
        let raw=r#"{"guid":"raw","macroEvents":[{"Type":"actionBar","recordProfile":{},"selected":false},{"Type":0,"Number":"0.01","opaque":9.9900e+100},{"future":9007199254740993123456789}],"future":1.2300e-7}"#;
        let value:Value=serde_json::from_str(raw).unwrap();let original=json!({"name":"Raw","payload":STANDARD.encode(raw),"hash":synapse_hash(&value).unwrap()});
        let updated=edit_definition(&original,Change::SetDelay{guid:"raw".into(),index:1,milliseconds:1234}).unwrap();let decoded=String::from_utf8(STANDARD.decode(updated["payload"].as_str().unwrap()).unwrap()).unwrap();
        assert!(decoded.contains("9.9900e+100")&&decoded.contains("1.2300e-7")&&decoded.contains("9007199254740993123456789"));assert!(decoded.contains("1.234"));assert_eq!(describe(&updated).unwrap().events[2],describe(&original).unwrap().events[2]);
        assert!(edit_definition(&original,Change::MoveEvent{guid:"raw".into(),index:3,direction:-1}).is_err());assert!(edit_definition(&original,Change::DeleteEvent{guid:"raw".into(),index:0}).is_err());
    }
    #[test]
    fn stale_save_conflicting_import_and_future_schema_refuse() {
        let original=created();let entry=read(&original).unwrap().entries.remove(0);let renamed=edit(&original,Change::Rename{guid:"new".into(),name:"Other".into()}).unwrap();
        assert!(edit(&renamed,Change::Replace{guid:"new".into(),expected_revision:entry.revision.clone(),record:entry.encoded_record.clone()}).is_err());
        let incoming=serde_json::to_vec(&json!({"macros":[entry.encoded_record]})).unwrap();assert!(edit(&renamed,Change::Import(incoming)).is_err());
        let mut future=original.clone();future.extra.get_mut(KEY).unwrap()["version"]=json!(2);assert!(edit(&future,Change::Rename{guid:"new".into(),name:"Changed".into()}).is_err());
    }
    #[test]
    fn local_edit_is_draft_until_explicit_guarded_replace() {
        let original=created();let entry=read(&original).unwrap().entries.remove(0);let changed=edit_definition(&entry.encoded_record,Change::InsertDelay{guid:"new".into(),index:1,milliseconds:1}).unwrap();
        assert_eq!(read(&original).unwrap().entries[0].events.len(),1);
        let saved=edit(&original,Change::Replace{guid:"new".into(),expected_revision:entry.revision,record:changed}).unwrap();assert_eq!(read(&saved).unwrap().entries[0].events[1]["Number"],"0.001");
    }
    #[test]
    fn structured_delays_native_caches_and_corrupt_imports_are_preserved() {
        for body in [json!({"guid":"guarded","macroEvents":[{"Type":0,"Number":{"min":0.1,"max":0.2}}]}),json!({"guid":"guarded","macroEvents":[{"Type":0,"Number":"0.1"}],"appEngine":{"events":[]}})] {
            let encoded=record(&body,"Guarded").unwrap();let before=encoded.clone();
            assert!(edit_definition(&encoded,Change::SetDelay{guid:"guarded".into(),index:0,milliseconds:1000}).is_err());assert_eq!(encoded,before);
        }
        let before=created();let mut corrupt=read(&before).unwrap().entries.remove(0).encoded_record;corrupt["hash"]=json!("wrong");
        assert!(edit(&before,Change::Import(serde_json::to_vec(&json!({"macros":[corrupt]})).unwrap())).is_err());
        let mut referenced=before.clone();referenced.extra.insert("futureConsumer".into(),json!({"definition":"new"}));
        assert!(read(&referenced).unwrap().entries[0].referenced);assert!(edit(&referenced,Change::Delete("new".into())).is_err());
    }
}
