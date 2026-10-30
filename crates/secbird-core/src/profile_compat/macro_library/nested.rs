//! Source nested macro references. Graph inspection never expands or executes events.
use super::*;

#[derive(Clone)]
pub struct Target {pub guid:String,pub name:String,pub reason:Option<String>}
struct Node {name:String,mode:Option<Value>,targets:BTreeSet<String>,invalid:Vec<Value>}
pub(super) struct Graph {nodes:BTreeMap<String,Node>}
fn valid_id(id:&str)->bool {!id.is_empty()&&id.len()<=128&&!id.chars().any(char::is_control)}
fn info(body:&Value)->Result<(BTreeSet<String>,Vec<Value>)> {
    let rows=body["macroEvents"].as_array().ok_or_else(||invalid("Macro has no event array"))?;
    if rows.len()>MAX_EVENTS{return Err(invalid("Macro exceeds512 event rows"));}
    let mut targets=BTreeSet::new();let mut bad=vec![];
    for row in rows.iter().filter(|row|row["Type"]==7) {
        if let Some(id)=row["guid"].as_str().filter(|id|valid_id(id)){targets.insert(id.into());}else{bad.push(row.clone());}
    }
    Ok((targets,bad))
}
fn same_mode(a:Option<&Value>,b:Option<&Value>)->bool {
    match (a,b) {
        (None,None)|(Some(Value::Null),Some(Value::Null))=>true,
        (Some(Value::String(a)),Some(Value::String(b)))=>a==b,
        (Some(Value::Bool(a)),Some(Value::Bool(b)))=>a==b,
        (Some(Value::Number(a)),Some(Value::Number(b)))=>match(a.as_f64(),b.as_f64()){(Some(a),Some(b))if a.is_finite()&&b.is_finite()=>a==b,_=>false},
        _=>false, // Separate object/array options do not satisfy source strict equality.
    }
}
impl Graph {
    pub(super) fn new(records:&BTreeMap<String,Value>)->Result<Self> {
        if records.len()>MAX_MACROS||records.values().map(|record|record["payload"].as_str().map_or(0,str::len)).sum::<usize>()>MAX_PAYLOADS{return Err(invalid("Macro dependency graph exceeds library bounds"));}
        let mut nodes=BTreeMap::new();
        for (id,record) in records {let body=decode_record(record)?;let (targets,bad)=info(&body)?;nodes.insert(id.clone(),Node{name:record["name"].as_str().unwrap_or("Macro").into(),mode:body.get("macroPlaybackOption").cloned(),targets,invalid:bad});}
        Ok(Self{nodes})
    }
    fn walk(&self,id:&str,grey:&mut BTreeSet<String>,done:&mut BTreeSet<String>)->Result<()> {
        if grey.contains(id){return Err(invalid("Nested macro dependency cycle; no reference saved"));}
        if done.contains(id){return Ok(());}
        let node=self.nodes.get(id).ok_or_else(||invalid("Nested macro target is missing; no reference saved"))?;
        if !node.invalid.is_empty(){return Err(invalid("Nested macro chain contains an unresolved event"));}
        grey.insert(id.into());
        for child in &node.targets {let target=self.nodes.get(child).ok_or_else(||invalid("Nested macro target is missing; no reference saved"))?;
            if !same_mode(node.mode.as_ref(),target.mode.as_ref()){return Err(invalid("Nested macros require the same verified playback option"));}
            self.walk(child,grey,done)?;
        }
        grey.remove(id);done.insert(id.into());Ok(())
    }
    fn edge(&self,owner:&str,target:&str)->Result<()> {
        if !valid_id(target)||owner==target{return Err(invalid("A macro cannot reference itself or an invalid target"));}
        let parent=self.nodes.get(owner).ok_or_else(||invalid("Current macro is missing"))?;
        let child=self.nodes.get(target).ok_or_else(||invalid("Nested macro target is missing; no reference saved"))?;
        if !same_mode(parent.mode.as_ref(),child.mode.as_ref()){return Err(invalid("Nested macros require the same verified playback option"));}
        let mut grey=BTreeSet::from([owner.into()]);self.walk(target,&mut grey,&mut BTreeSet::new())
    }
    pub(super) fn references(&self)->BTreeSet<String> {self.nodes.values().flat_map(|node|node.targets.iter().cloned()).collect()}
    pub(super) fn issues(&self)->usize {self.nodes.keys().filter(|id|self.walk(id,&mut BTreeSet::new(),&mut BTreeSet::new()).is_err()).count()}
    fn closure(&self,roots:&BTreeSet<String>)->BTreeSet<String> {
        let mut found=BTreeSet::new();let mut pending:Vec<_>=roots.iter().cloned().collect();
        while let Some(id)=pending.pop(){if !found.insert(id.clone()){continue;}if let Some(node)=self.nodes.get(&id){pending.extend(node.targets.iter().cloned());}}
        found
    }
}

pub fn targets(record:&Value,context:&[Value])->Result<Vec<Target>> {
    let store=definition_store(record,context)?;let graph=Graph::new(&collect(&store)?)?;let owner=guid(record)?;
    let mut targets:Vec<_>=graph.nodes.iter().map(|(id,node)|Target{guid:id.clone(),name:node.name.clone(),reason:graph.edge(&owner,id).err().map(|error|error.to_string())}).collect();
    targets.sort_by_key(|target|(target.name.to_lowercase(),target.guid.clone()));Ok(targets)
}
pub(crate) fn required(records:&BTreeMap<String,Value>,roots:&BTreeSet<String>)->Result<BTreeSet<String>> {
    let mut selected=BTreeMap::new();let mut pending:Vec<_>=roots.iter().cloned().collect();
    while let Some(id)=pending.pop(){if selected.contains_key(&id){continue;}if selected.len()>=MAX_MACROS{return Err(invalid("Selected macro dependencies exceed library bounds"));}let record=records.get(&id).ok_or_else(||invalid("Selected nested macro definition is missing"))?;let (children,_)=info(&decode_record(record)?)?;pending.extend(children);selected.insert(id,record.clone());}
    let graph=Graph::new(&selected)?;for root in roots {graph.walk(root,&mut BTreeSet::new(),&mut BTreeSet::new())?;}Ok(graph.closure(roots))
}
pub(crate) fn rewrite_guids(body:&mut Value,renames:&BTreeMap<String,(String,String)>)->Result<()> {
    let changed=body["macroEvents"].as_array().is_some_and(|rows|rows.iter().filter(|row|row["Type"]==7).any(|row|row["guid"].as_str().and_then(|id|renames.get(id)).is_some_and(|(id,_)|row["guid"].as_str()!=Some(id.as_str()))));
    if changed&&body.get("appEngine").is_some(){return Err(invalid("Nested GUID cloning with a native event cache needs cache verification; use a full native backup"));}
    if let Some(rows)=body.get_mut("macroEvents").and_then(Value::as_array_mut){for row in rows.iter_mut().filter(|row|row["Type"]==7){if let Some((id,_))=row["guid"].as_str().and_then(|id|renames.get(id)).filter(|(id,_)|row["guid"].as_str()!=Some(id.as_str())){row["guid"]=json!(id);if row.get("MPIndex").is_some(){row["MPIndex"]=Value::Null;}}}}Ok(())
}

pub(super) fn row(target:&str)->Result<Box<RawValue>> {
    if !valid_id(target){return Err(invalid("Nested macro GUID is invalid"));}
    raw(&json!({"Type":7,"MPIndex":null,"guid":target,"selected":false}))
}
pub(super) fn retarget(row:&RawValue,target:&str)->Result<Box<RawValue>> {
    if !valid_id(target){return Err(invalid("Nested macro GUID is invalid"));}
    let value:Value=serde_json::from_str(row.get()).map_err(|error|invalid(error.to_string()))?;
    if value["Type"]!=7{return Err(invalid("Select a nested macro event"));}
    let mut fields:Fields=serde_json::from_str(row.get()).map_err(|error|invalid(error.to_string()))?;
    if let Some((_,old))=fields.0.iter_mut().find(|(key,_)|key=="guid"){*old=raw(&target)?;}else{if fields.0.len()>=128{return Err(invalid("Nested event fields exceed editing bounds"));}fields.0.push(("guid".into(),raw(&target)?));}
    if let Some((_,old))=fields.0.iter_mut().find(|(key,_)|key=="MPIndex"){*old=raw(&Value::Null)?;}
    raw(&fields)
}

pub(super) fn validate_change(old:Option<&Value>,new:&Value,records:&BTreeMap<String,Value>,forced:Option<&str>)->Result<()> {
    let new_body=decode_record(new)?;let (new_targets,bad)=info(&new_body)?;
    let old_body=old.map(decode_record).transpose()?;
    let (old_targets,mut old_bad)=old_body.as_ref().map(info).transpose()?.unwrap_or_default();
    let cached=|body:&Value|body.get("appEngine").is_some_and(|engine|!movement_only_hint(engine));
    if old_body.as_ref().is_some_and(|old|cached(old)||cached(&new_body))&&old_body.as_ref().is_some_and(|old|old["macroEvents"]!=new_body["macroEvents"]){return Err(invalid("Native event cache invalidation needs verification before event edits"));}
    for row in bad {let Some(index)=old_bad.iter().position(|old|old==&row) else{return Err(invalid("New nested event has no verified target"));};old_bad.remove(index);}
    let mut check:BTreeSet<_>=new_targets.difference(&old_targets).cloned().collect();
    if old_body.as_ref().is_some_and(|old|old.get("macroPlaybackOption")!=new_body.get("macroPlaybackOption")){check.extend(new_targets);}
    if let Some(target)=forced{check.insert(target.into());}
    if !check.is_empty(){let graph=Graph::new(records)?;let owner=guid(new)?;for target in check {graph.edge(&owner,&target)?;}}
    Ok(())
}

// Retained source exports need the entire reachable GUI definition closure,
// not just a parent's newly edited encoded record. Existing bad chains are
// retained/reported; this projection neither executes nor repairs them.
pub(super) fn retain_required(store:&mut Store,records:&BTreeMap<String,Value>)->Result<()> {
    let graph=Graph::new(records)?;
    for device in store.devices.values_mut() {
        let keys:Vec<_>=device.extra.iter().filter(|(key,value)|(**key==SOURCE||key.starts_with(&format!("_synapseLiteImported:{SOURCE}")))&&value.get("profiles").is_some_and(Value::is_array)).map(|(key,_)|key.clone()).collect();
        if keys.is_empty(){continue;}
        let mut present=BTreeSet::new();for key in &keys {if let Some(rows)=device.extra[key].get("macros").and_then(Value::as_array){for row in rows {present.insert(guid(row)?);}}}
        let mut wanted=BTreeMap::new();for profile in device.profiles.values(){for value in profile.extra.values(){gui::refs(value,&mut wanted);}}
        for (i,key) in keys.iter().enumerate() {
            let mut roots=BTreeSet::new();if let Some(rows)=device.extra[key].get("macros").and_then(Value::as_array){for row in rows {roots.insert(guid(row)?);}}
            let existing=roots.clone();
            if i==0{roots.extend(wanted.keys().filter(|id|!present.contains(*id)).cloned());}
            let added:Vec<_>=graph.closure(&roots).into_iter().filter(|id|!existing.contains(id)).filter_map(|id|records.get(&id).cloned()).collect();
            if added.is_empty(){continue;}
            let envelope=device.extra.get_mut(key).unwrap().as_object_mut().ok_or_else(||invalid("Retained source envelope is malformed"))?;
            let rows=envelope.entry("macros").or_insert_with(||json!([])).as_array_mut().ok_or_else(||invalid("Retained macro list is malformed"))?;rows.extend(added);
        }
    }
    Ok(())
}

#[cfg(test)] mod tests {
    use super::*;
    fn library()->Store {let mut store=Store::default();for id in ["a","b","c"] {store=edit(&store,Change::Create{guid:id.into(),name:id.into()}).unwrap();}store}
    fn link(store:&Store,owner:&str,target:&str)->Result<Store>{edit(store,Change::InsertNested{guid:owner.into(),index:1,target:target.into()})}
    fn entry(store:&Store,id:&str)->Entry {read(store).unwrap().entries.into_iter().find(|entry|entry.guid==id).unwrap()}
    fn replace_raw(store:&Store,id:&str,body:Value)->Store {let mut store=store.clone();let records=store.extra.get_mut(KEY).unwrap()["records"].as_array_mut().unwrap();let row=records.iter_mut().find(|row|guid(row).unwrap()==id).unwrap();*row=record(&body,id).unwrap();store}

    #[test] fn nested_row_matches_source_and_graph_guards_cycles_modes_and_missing_targets() {
        let fixture:Value=serde_json::from_str(include_str!("../../../tests/fixtures/macro-nested-source.json")).unwrap();assert_eq!(serde_json::from_str::<Value>(row("synthetic-child").unwrap().get()).unwrap(),fixture["row"]);
        let base=library();let linked=link(&base,"a","b").unwrap();let linked=link(&linked,"b","c").unwrap();assert!(link(&linked,"c","a").is_err());assert!(link(&base,"a","a").is_err());assert!(link(&base,"a","missing").is_err());assert!(entry(&linked,"b").referenced);assert!(edit(&linked,Change::Delete("b".into())).is_err());
        let mut body=decode_record(&entry(&base,"b").encoded_record).unwrap();body["macroPlaybackOption"]=json!("Sequence");let mismatch=replace_raw(&base,"b",body);assert!(link(&mismatch,"a","b").is_err());
        let mut body=decode_record(&entry(&base,"a").encoded_record).unwrap();body["macroPlaybackOption"]=json!({"future":1});let object=replace_raw(&base,"a",body.clone());body["guid"]=json!("b");let object=replace_raw(&object,"b",body);assert!(link(&object,"a","b").is_err());
    }
    #[test] fn local_nested_draft_requires_context_and_save_revalidates_current_dependencies() {
        let base=library();let selected=entry(&base,"a");let context:Vec<_>=read(&base).unwrap().entries.into_iter().map(|entry|entry.encoded_record).collect();let change=Change::InsertNested{guid:"a".into(),index:1,target:"b".into()};assert!(edit_definition(&selected.encoded_record,change.clone()).is_err());let draft=edit_definition_with_library(&selected.encoded_record,change,&context).unwrap();assert_eq!(entry(&base,"a").events.len(),1);
        let missing=edit(&base,Change::Delete("b".into())).unwrap();assert!(edit(&missing,Change::Replace{guid:"a".into(),expected_revision:selected.revision.clone(),record:draft.clone()}).is_err());
        let cycle=link(&base,"b","a").unwrap();assert!(edit(&cycle,Change::Replace{guid:"a".into(),expected_revision:selected.revision,record:draft}).is_err());
    }
    #[test] fn retarget_preserves_raw_fields_and_deletion_releases_target_reference() {
        let base=library();let mut body=decode_record(&entry(&base,"a").encoded_record).unwrap();body["macroEvents"]=json!([{"Type":"actionBar"},{"Type":7,"guid":"b","MPIndex":12,"future":9}]);let linked=replace_raw(&base,"a",body);
        let changed=edit(&linked,Change::RebindNested{guid:"a".into(),index:1,target:"c".into()}).unwrap();assert_eq!(entry(&changed,"a").events[1]["guid"],"c");assert!(entry(&changed,"a").events[1]["MPIndex"].is_null());assert_eq!(entry(&changed,"a").events[1]["future"],9);assert!(!entry(&changed,"b").referenced);assert!(entry(&changed,"c").referenced);
        let deleted=edit(&changed,Change::DeleteEvent{guid:"a".into(),index:1}).unwrap();assert!(!entry(&deleted,"c").referenced);
        let raw=RawValue::from_string(r#"{"Type":7,"guid":"b","future":1.2300e-7}"#.into()).unwrap();assert!(retarget(&raw,"c").unwrap().get().contains("1.2300e-7"));
    }
    #[test] fn imported_bad_chains_are_retained_reported_and_recoverable() {
        let base=library();let mut a=decode_record(&entry(&base,"a").encoded_record).unwrap();a["macroEvents"]=json!([{"Type":7,"guid":"missing"}]);let bad=replace_raw(&base,"a",a);let view=read(&bad).unwrap();assert_eq!(view.missing,["missing"]);assert_eq!(view.dependency_issues,1);assert!(link(&bad,"c","a").is_err());
        let fixed=edit(&bad,Change::RebindNested{guid:"a".into(),index:0,target:"b".into()}).unwrap();assert!(read(&fixed).unwrap().missing.is_empty());assert_eq!(read(&fixed).unwrap().dependency_issues,0);
        let cycle=link(&base,"a","b").unwrap();let mut b=decode_record(&entry(&cycle,"b").encoded_record).unwrap();b["macroEvents"]=json!([{"Type":7,"guid":"a"}]);let cycle=replace_raw(&cycle,"b",b);assert_eq!(read(&cycle).unwrap().dependency_issues,2);assert!(link(&cycle,"c","a").is_err());
    }
    #[test] fn source_export_includes_transitive_definitions_without_profile_reimport() {
        let mut source:Value=serde_json::from_slice(include_bytes!("../../../tests/fixtures/synthetic.synapse4")).unwrap();let mut body=decode_record(&source["macros"][0]).unwrap();body["macroPlaybackOption"]=json!("");source["macros"][0]=record(&body,"Root").unwrap();let mut profile=decode_record(&source["profiles"][0]).unwrap();profile["mappings"]=json!([{"inputID":"DKM_F1","outputType":"future-macro","macroGroup":{"guid":body["guid"],"name":"Root"}}]);let profile_name=source["profiles"][0]["name"].as_str().unwrap().to_owned();source["profiles"][0]=record(&profile,&profile_name).unwrap();let before=crate::profile_compat::import_document(&serde_json::to_vec(&source).unwrap()).unwrap().store;let root=read(&before).unwrap().entries.remove(0);let key=before.devices.keys().next().unwrap().clone();
        let mut store=before.clone();for id in ["b","c"] {store=edit(&store,Change::Create{guid:id.into(),name:id.into()}).unwrap();}store=link(&store,"b","c").unwrap();store=link(&store,&root.guid,"b").unwrap();assert_eq!(store.devices[&key].profiles,before.devices[&key].profiles);assert_eq!(store.devices[&key].active,before.devices[&key].active);
        let exported=crate::profile_compat::export_synapse4(&store,&key).unwrap();let document:Value=serde_json::from_slice(&exported.bytes).unwrap();assert_eq!(document["profiles"],source["profiles"]);let records=document["macros"].as_array().unwrap();let ids:BTreeSet<_>=records.iter().map(|record|guid(record).unwrap()).collect();assert!(ids.contains(&root.guid)&&ids.contains("b")&&ids.contains("c"));assert_eq!(ids.len(),3);
        let reopened=crate::profile_compat::import_document(&exported.bytes).unwrap().store;assert!(read(&reopened).unwrap().missing.is_empty());assert_eq!(read(&reopened).unwrap().dependency_issues,0);
        let preview=crate::profile_compat::gui::preview_import(&exported.bytes).unwrap();let choices=preview.initial_choices(None);let cloned=crate::profile_compat::gui::selected_import(&preview,&choices,&store,&key).unwrap();let cloned:Value=serde_json::from_slice(&cloned.bytes).unwrap();let mut cloned_records=BTreeMap::new();for record in cloned["macros"].as_array().unwrap(){cloned_records.insert(guid(record).unwrap(),record.clone());}assert_eq!(cloned_records.len(),3);assert!(!cloned_records.contains_key("b")&&!cloned_records.contains_key("c")&&!cloned_records.contains_key(&root.guid));let roots:BTreeSet<_>=cloned_records.keys().cloned().collect();assert_eq!(required(&cloned_records,&roots).unwrap(),roots);
        let preview=crate::profile_compat::gui::preview_export(&serde_json::to_vec(&store).unwrap(),&key,&store.devices[&key].active).unwrap();let selected=crate::profile_compat::gui::selected_export(&preview,&preview.initial_choices(None)).unwrap();let selected:Value=serde_json::from_slice(&selected.bytes).unwrap();assert_eq!(selected["macros"].as_array().unwrap().len(),3);
    }
    #[test] fn invalid_targets_bounds_and_cache_edits_refuse_atomically() {
        let base=library();let mut body=decode_record(&entry(&base,"a").encoded_record).unwrap();body["appEngine"]=json!({"events":[]});let cached=replace_raw(&base,"a",body);assert!(link(&cached,"a","b").is_err());
        let before=entry(&cached,"a");let mut changed=decode_record(&before.encoded_record).unwrap();changed["macroEvents"]=json!([{"Type":7,"guid":"b"}]);let record=record(&changed,"a").unwrap();assert!(edit(&cached,Change::Replace{guid:"a".into(),expected_revision:before.revision,record}).is_err());
        assert!(row("").is_err());assert!(row("bad\n").is_err());assert!(edit(&base,Change::InsertNested{guid:"a".into(),index:0,target:"b".into()}).is_err());
        let mut body=json!({"macroEvents":[{"Type":7,"guid":"b"}],"appEngine":{"events":[{"type":"macro","guid":"b"}]}});let original=body.clone();let renames=BTreeMap::from([("b".into(),("new-b".into(),"B".into()))]);assert!(rewrite_guids(&mut body,&renames).is_err());assert_eq!(body,original);
    }
}
