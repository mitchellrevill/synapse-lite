//! Rebind proved source input groups without replacing opaque row fields.
use super::*;

pub struct Selection {
    pub input:Input,
    pub indices:Vec<usize>,
    pub gesture:Gesture,
}

fn identify(row:&Value)->Option<Input> {
    if row["Type"]==2 {
        return row["MouseEvent"]["MouseButton"].as_u64().filter(|button|*button<=9).map(|button|Input::Mouse(button as u8));
    }
    if row["Type"]!=1{return None;}
    let code=row["KeyEvent"]["Makecode"].as_u64()?;
    let scan=row.get("ScanCode").or_else(||row.get("scancode"));
    let category=row["KeyEvent"].get("Type").and_then(Value::as_str);
    let mut candidates:Vec<_>=keys().iter().filter(|key|u64::from(key.make_code)==code&&scan.is_none_or(|scan|scan.as_u64()==Some(key.scan_code.into()))&&category.is_none_or(|category|key.category==category)).collect();
    if candidates.len()>1 {
        let state=row["KeyEvent"]["State"].as_u64()?;
        candidates.retain(|key|u64::from(key.prefix)==state-state%2);
    }
    (candidates.len()==1).then(||Input::Keyboard(candidates[0].id.clone()))
}

fn state(row:&Value,input:&Input)->Result<u8> {
    let (value,max)=match input {
        Input::Keyboard(_)=>(&row["KeyEvent"]["State"],5),
        Input::Mouse(0..=4)=>(&row["MouseEvent"]["State"],1),
        Input::Mouse(_)=>(&row["MouseEvent"]["State"],0),
    };
    value.as_u64().filter(|state|*state<=max).map(|state|state as u8).ok_or_else(||invalid("This retained input state is not verified for editing; add explicit input rows instead"))
}

pub fn selection(rows:&[Value],index:usize)->Result<Selection> {
    let row=rows.get(index).ok_or_else(||invalid("Select an input event"))?;
    let input=identify(row).ok_or_else(||invalid("This input has no unambiguous source descriptor"))?;
    let id=row["Id"].as_u64().filter(|id|*id<=MAX_SOURCE_ID).ok_or_else(||invalid("This input identity is not verified for paired editing"))?;
    // A float spelling of the same JS numeric ID must not be silently ignored.
    let mut indices=vec![];
    for (i,other) in rows.iter().enumerate() {
        if other["Id"].as_f64()==Some(id as f64) {
            if other["Id"].as_u64()!=Some(id){return Err(invalid("Ambiguous numeric input identity"));}
            indices.push(i);
        }
    }
    if indices.is_empty()||indices.len()>2{return Err(invalid("Input identity is shared ambiguously; no rows changed"));}
    let mut states=vec![];
    for i in &indices {
        if identify(&rows[*i]).as_ref()!=Some(&input){return Err(invalid("Shared identity refers to different inputs"));}
        states.push(state(&rows[*i],&input)?);
    }
    let gesture=if indices.len()==2 {
        if matches!(input,Input::Mouse(5..=9))||states[0]%2!=0||states[1]!=states[0]+1{return Err(invalid("Input pair order or states are ambiguous; no rows changed"));}
        Gesture::Tap
    }else if matches!(input,Input::Mouse(5..=9)){Gesture::Tap}else if states[0]%2==0{Gesture::Down}else{Gesture::Up};
    Ok(Selection{input,indices,gesture})
}

pub fn can_rebind(from:&Input,to:&Input)->bool {
    match (from,to) {
        (Input::Keyboard(_),Input::Keyboard(_))=>true,
        (Input::Mouse(a),Input::Mouse(b))=>*a<=9&&*b<=9&&(*a<5)==(*b<5),
        _=>false,
    }
}

fn values(rows:&[Box<RawValue>])->Result<Vec<Value>> {
    rows.iter().map(|row|serde_json::from_str(row.get()).map_err(|error|invalid(error.to_string()))).collect()
}
fn fields(row:&RawValue)->Result<Fields> {serde_json::from_str(row.get()).map_err(|error|invalid(error.to_string()))}
fn nested(row:&Fields,key:&str)->Result<Fields> {
    fields(row.0.iter().find(|(name,_)|name==key).map(|(_,value)|&**value).ok_or_else(||invalid("Input field is missing"))?)
}
fn set(fields:&mut Fields,key:&str,value:Box<RawValue>)->Result<()> {
    if let Some((_,old))=fields.0.iter_mut().find(|(name,_)|name==key){*old=value;}
    else {if fields.0.len()>=128{return Err(invalid("Input fields exceed editing bounds"));}fields.0.push((key.into(),value));}
    Ok(())
}
fn validate_raw(rows:&[Box<RawValue>],selection:&Selection)->Result<()> {
    let key=if matches!(selection.input,Input::Keyboard(_)){"KeyEvent"}else{"MouseEvent"};
    for index in &selection.indices {let root=fields(&rows[*index])?;nested(&root,key)?;}
    Ok(())
}

pub fn rebind(rows:&[Box<RawValue>],index:usize,input:&Input)->Result<Vec<Box<RawValue>>> {
    let values=values(rows)?;let group=selection(&values,index)?;validate_raw(rows,&group)?;
    if !can_rebind(&group.input,input){return Err(invalid("Choose the same input kind: keyboard key, mouse button, or complete mouse action"));}
    let template=super::rows(&[],0,input,Gesture::Tap)?;
    let mut updated=rows.to_vec();
    for index in group.indices {
        let direction=usize::from(state(&values[index],&group.input)?%2);
        let template=&template[direction];let mut root=fields(&rows[index])?;
        if matches!(input,Input::Keyboard(_)) {
            let mut key=nested(&root,"KeyEvent")?;
            for field in ["Makecode","State","Type"] {set(&mut key,field,raw(&template["KeyEvent"][field])?)?;}
            set(&mut root,"KeyEvent",raw(&key)?)?;
            for field in ["ScanCode","flag"] {set(&mut root,field,raw(&template[field])?)?;}
            if root.0.iter().any(|(key,_)|key=="scancode"){set(&mut root,"scancode",raw(&template["ScanCode"])?)?;}
        }else {
            let mut mouse=nested(&root,"MouseEvent")?;
            for field in ["MouseButton","State"]{set(&mut mouse,field,raw(&template["MouseEvent"][field])?)?;}
            set(&mut root,"MouseEvent",raw(&mouse)?)?;
        }
        updated[index]=raw(&root)?;
    }
    Ok(updated)
}

pub fn delete_group(rows:&[Box<RawValue>],index:usize)->Result<Vec<Box<RawValue>>> {
    let group=selection(&values(rows)?,index)?;validate_raw(rows,&group)?;
    Ok(rows.iter().enumerate().filter(|(i,_)|!group.indices.contains(i)).map(|(_,row)|row.clone()).collect())
}

#[cfg(test)] mod tests {
    use super::*;
    fn raw_rows(values:&[Value])->Vec<Box<RawValue>>{values.iter().map(|value|raw(value).unwrap()).collect()}
    fn pair(input:Input)->Vec<Value>{super::super::rows(&[],0,&input,Gesture::Tap).unwrap()}

    #[test] fn every_rebound_key_matches_the_independent_source_fixture() {
        let fixture:Value=serde_json::from_str(include_str!("../../../../tests/fixtures/macro-authored-key-events.json")).unwrap();
        let old=raw_rows(&pair(Input::Keyboard("KEY_A".into())));
        for case in fixture["cases"].as_array().unwrap(){let input=Input::Keyboard(case["key"].as_str().unwrap().into());let changed=rebind(&old,1,&input).unwrap();assert_eq!(json!(values(&changed).unwrap()),case["events"],"{}",case["key"]);}
    }

    #[test] fn group_rebind_and_delete_preserve_raw_leaves_and_intervening_delay() {
        let down=r#"{"Type":1,"Id":77,"ScanCode":30,"scancode":30,"flag":0,"KeyEvent":{"Makecode":65,"State":0,"Type":"Alphanumeric","future":9.9900e+100},"future":1.2300e-7,"phase":2,"isPairing":true}"#;
        let up=down.replace("\"State\":0","\"State\":1");let delay=r#"{"Type":0,"Number":"0.123","future":9007199254740993123456789}"#;
        let rows:Vec<_>=[down,delay,&up].into_iter().map(|s|RawValue::from_string(s.into()).unwrap()).collect();let changed=rebind(&rows,2,&Input::Keyboard("KEY_RIGHT_SHIFT".into())).unwrap();
        assert_eq!(changed[1].get(),delay);for (i,state) in [(0,0),(2,1)] {let value:Value=serde_json::from_str(changed[i].get()).unwrap();assert_eq!(value["Id"],77);assert_eq!(value["ScanCode"],54);assert_eq!(value["scancode"],54);assert_eq!(value["KeyEvent"]["State"],state);assert_eq!(value["phase"],2);assert_eq!(value["isPairing"],true);assert!(changed[i].get().contains("9.9900e+100")&&changed[i].get().contains("1.2300e-7"));}
        let deleted=delete_group(&rows,2).unwrap();assert_eq!(deleted.len(),1);assert_eq!(deleted[0].get(),delay);assert_eq!(rows[0].get(),down);
    }

    #[test] fn legacy_missing_scan_fields_and_mouse_actions_are_explicit() {
        let rows=raw_rows(&[json!({"Type":1,"Id":1,"KeyEvent":{"Makecode":65,"State":0}}),json!({"Type":1,"Id":1,"KeyEvent":{"Makecode":65,"State":1}})]);
        let changed=rebind(&rows,0,&Input::Keyboard("KEY_RIGHT_CTRL".into())).unwrap();let changed=values(&changed).unwrap();assert_eq!(changed[0]["KeyEvent"]["State"],2);assert_eq!(changed[1]["KeyEvent"]["State"],3);assert_eq!(changed[0]["ScanCode"],29);assert_eq!(changed[0]["KeyEvent"]["Type"],"Modifiers");
        for button in 0..=4 {let old=raw_rows(&pair(Input::Mouse(0)));let changed=values(&rebind(&old,0,&Input::Mouse(button)).unwrap()).unwrap();assert_eq!(changed,pair(Input::Mouse(button)));}
        let wheel=raw_rows(&pair(Input::Mouse(8)));let changed=values(&rebind(&wheel,0,&Input::Mouse(5)).unwrap()).unwrap();assert_eq!(changed,pair(Input::Mouse(5)));assert!(rebind(&wheel,0,&Input::Mouse(0)).is_err());assert!(rebind(&rows,0,&Input::Mouse(0)).is_err());
        let single=raw_rows(&super::super::rows(&[],0,&Input::Keyboard("KEY_A".into()),Gesture::Up).unwrap());let changed=values(&rebind(&single,0,&Input::Keyboard("KEY_RIGHT_CTRL".into())).unwrap()).unwrap();assert_eq!(changed.len(),1);assert_eq!(changed[0]["KeyEvent"]["State"],3);
    }

    #[test] fn ambiguous_groups_states_and_duplicate_fields_refuse_without_guessing() {
        let mut invalid=vec![];
        let a=pair(Input::Keyboard("KEY_A".into()));let mut three=a.clone();three.push(a[0].clone());invalid.push(three);
        let mut reversed=a.clone();reversed.reverse();invalid.push(reversed);
        let mut mismatch=a.clone();mismatch[1]["KeyEvent"]["Makecode"]=json!(66);invalid.push(mismatch);
        let mut prefix=a.clone();prefix[1]["KeyEvent"]["State"]=json!(3);invalid.push(prefix);
        let mut null=a.clone();null[0]["KeyEvent"]["State"]=Value::Null;invalid.push(null);
        let mut float=a.clone();float[1]["Id"]=json!(1.0);invalid.push(float);
        for rows in invalid {let raw=raw_rows(&rows);assert!(rebind(&raw,0,&Input::Keyboard("KEY_B".into())).is_err());assert!(delete_group(&raw,0).is_err());assert_eq!(values(&raw).unwrap(),rows);}
        let duplicate=RawValue::from_string(r#"{"Type":1,"Id":1,"KeyEvent":{"Makecode":65,"Makecode":65,"State":0}}"#.into()).unwrap();assert!(rebind(&[duplicate.clone()],0,&Input::Keyboard("KEY_B".into())).is_err());assert!(delete_group(&[duplicate],0).is_err());
    }

    #[test] fn store_edit_preserves_profiles_and_source_export_references() {
        let mut source:Value=serde_json::from_slice(include_bytes!("../../../../tests/fixtures/synthetic.synapse4")).unwrap();let mut body=decode_record(&source["macros"][0]).unwrap();body["macroEvents"]=json!(pair(Input::Keyboard("KEY_A".into())));let title=source["macros"][0]["name"].as_str().unwrap().to_owned();source["macros"][0]=super::super::super::record(&body,&title).unwrap();
        let before=crate::profile_compat::import_document(&serde_json::to_vec(&source).unwrap()).unwrap().store;let entry=super::super::super::read(&before).unwrap().entries.remove(0);let key=before.devices.keys().next().unwrap().clone();
        let after=super::super::super::edit(&before,Change::RebindInput{guid:entry.guid,index:0,input:Input::Keyboard("KEY_RIGHT_CTRL".into())}).unwrap();assert_eq!(before.devices[&key].profiles,after.devices[&key].profiles);assert_eq!(before.devices[&key].active,after.devices[&key].active);
        let old:Value=serde_json::from_slice(&crate::profile_compat::export_synapse4(&before,&key).unwrap().bytes).unwrap();let new:Value=serde_json::from_slice(&crate::profile_compat::export_synapse4(&after,&key).unwrap().bytes).unwrap();assert_eq!(old["profiles"],new["profiles"]);let body=decode_record(&new["macros"][0]).unwrap();assert_eq!(body["macroEvents"][0]["KeyEvent"]["State"],2);assert_eq!(body["macroEvents"][1]["KeyEvent"]["State"],3);
    }
}
