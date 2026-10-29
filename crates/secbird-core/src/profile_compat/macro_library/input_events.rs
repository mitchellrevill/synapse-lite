//! Source UI input rows only. This module never records, converts or sends input.
use super::*;
use std::sync::LazyLock;

#[derive(Clone,Deserialize)]
pub struct Key {
    pub id:String,
    pub name:String,
    pub category:String,
    pub make_code:u16,
    pub scan_code:u16,
    pub source_flag:u8,
    pub prefix:u8,
}
static KEYS:LazyLock<Vec<Key>>=LazyLock::new(||serde_json::from_str(include_str!("../macro_keys.json")).expect("checked source key descriptors"));
pub fn keys()->&'static [Key] {&KEYS}

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Input {Keyboard(String),Mouse(u8)}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Gesture {Tap,Down,Up}
pub const MOUSE_NAMES:[&str;10]=["Left click","Right click","Scroll click","Mouse button 4","Mouse button 5","Double click","Scroll left","Scroll right","Scroll up","Scroll down"];
const MAX_SOURCE_ID:u64=9_007_199_254_740_991;
mod editing;
pub use editing::{selection,Selection,can_rebind};
pub(super) use editing::{rebind,delete_group};

fn next_id(rows:&[Value])->Result<u64> {
    let mut highest=0;
    for row in rows {
        let Some(id)=row.get("Id").filter(|id|!id.is_null()) else{continue;};
        let id=id.as_u64().filter(|id|*id<MAX_SOURCE_ID).ok_or_else(||invalid("Unsupported macro event identity; no identity guessed"))?;
        highest=highest.max(id);
    }
    Ok(highest+1)
}
fn same_input(row:&Value,input:&Input)->bool {
    match input {
        Input::Keyboard(id)=>keys().iter().find(|key|&key.id==id).is_some_and(|key|row["Type"]==1&&row["KeyEvent"]["Makecode"]==key.make_code&&row["ScanCode"]==key.scan_code),
        Input::Mouse(button)=>row["Type"]==2&&row["MouseEvent"]["MouseButton"]==*button,
    }
}
fn open_id(rows:&[Value],input:&Input)->Option<u64> {
    let mut open=None;
    for row in rows.iter().filter(|row|same_input(row,input)) {
        let state=if matches!(input,Input::Keyboard(_)){&row["KeyEvent"]["State"]}else{&row["MouseEvent"]["State"]};
        if let Some(state)=state.as_u64(){if state%2==0 {open=row["Id"].as_u64();}else{open=None;}}
    }
    open
}

/// Inserts a key/button transition or explicit down/up pair. Wheel/double-click
/// actions are single source rows and accept Tap only. IDs follow source max+1;
/// an explicit release reuses an unmatched preceding authored down when present.
pub fn rows(existing:&[Value],index:usize,input:&Input,gesture:Gesture)->Result<Vec<Value>> {
    if index>existing.len(){return Err(invalid("Event insertion exceeds macro bounds"));}
    if index==0&&existing.first().is_some_and(|row|row["Type"]=="actionBar"){return Err(invalid("Input rows must follow the source action bar"));}
    let fresh=next_id(existing)?;
    let id=if gesture==Gesture::Up{open_id(&existing[..index],input).unwrap_or(fresh)}else{fresh};
    let states:&[u8]=match gesture{Gesture::Tap=>&[0,1],Gesture::Down=>&[0],Gesture::Up=>&[1]};
    match input {
        Input::Keyboard(input_id)=>{
            let key=keys().iter().find(|key|&key.id==input_id).ok_or_else(||invalid("This keyboard key has no verified source descriptor"))?;
            Ok(states.iter().map(|state|json!({"Type":1,"Id":id,"KeyEvent":{"Makecode":key.make_code,"State":key.prefix+state,"Type":key.category},"ScanCode":key.scan_code,"flag":key.source_flag,"selected":false,"isPairing":false})).collect())
        }
        Input::Mouse(button)=>{
            if *button>9{return Err(invalid("Unknown mouse function"));}
            let states=if *button>=5 {if gesture!=Gesture::Tap{return Err(invalid("Wheel and double-click functions are complete actions"));}&[0][..]}else{states};
            Ok(states.iter().map(|state|json!({"Type":2,"Id":id,"MouseEvent":{"MouseButton":button,"State":state},"selected":false,"isPairing":false})).collect())
        }
    }
}

pub fn label(row:&Value)->Option<String> {
    let (name,state)=if row["Type"]==1 {
        let key=keys().iter().find(|key|row["KeyEvent"]["Makecode"]==key.make_code&&row["ScanCode"]==key.scan_code)?;
        (key.name.as_str(),&row["KeyEvent"]["State"])
    }else if row["Type"]==2 {
        let button=row["MouseEvent"]["MouseButton"].as_u64()? as usize;
        let name=MOUSE_NAMES.get(button)?;
        if button>=5{return Some((*name).into());}( *name,&row["MouseEvent"]["State"])
    }else{return None;};
    Some(format!("{name} · {}",match state.as_u64(){Some(n)if n%2==0=>"Down",Some(_)=>"Up",None=>"Press"}))
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn all_authored_keys_match_independent_source_conversion_fixtures() {
        let fixture:Value=serde_json::from_str(include_str!("../../../tests/fixtures/macro-authored-key-events.json")).unwrap();
        let cases=fixture["cases"].as_array().unwrap();assert_eq!(cases.len(),125);assert_eq!(keys().len(),cases.len());
        let mut ids=BTreeSet::new();for case in cases {
            let id=case["key"].as_str().unwrap();assert!(ids.insert(id));
            let authored=rows(&[],0,&Input::Keyboard(id.into()),Gesture::Tap).unwrap();assert_eq!(json!(authored),case["events"],"{id}");
            for (row,output) in authored.iter().zip(case["output"].as_array().unwrap()) {assert_eq!(row["ScanCode"],output["scancode"],"{id}");assert_eq!(row["KeyEvent"]["State"],output["flag"],"{id}");}
        }
        let shift=rows(&[],0,&Input::Keyboard("KEY_RIGHT_SHIFT".into()),Gesture::Tap).unwrap();assert_eq!(shift[0]["KeyEvent"]["State"],0);assert_eq!(shift[0]["ScanCode"],54);
        let control=rows(&[],0,&Input::Keyboard("KEY_RIGHT_CTRL".into()),Gesture::Tap).unwrap();assert_eq!(control[0]["KeyEvent"]["State"],2);assert_eq!(control[1]["KeyEvent"]["State"],3);
    }
    #[test] fn release_reuses_open_authored_identity_and_insert_is_bounded() {
        let input=Input::Keyboard("KEY_A".into());let down=rows(&[],0,&input,Gesture::Down).unwrap();let up=rows(&down,1,&input,Gesture::Up).unwrap();assert_eq!(down[0]["Id"],up[0]["Id"]);
        let mut completed=down;completed.extend(up);let fresh=rows(&completed,2,&input,Gesture::Tap).unwrap();assert_eq!(fresh[0]["Id"],2);assert_eq!(fresh[1]["Id"],2);
        assert!(rows(&completed,3,&input,Gesture::Tap).is_err());assert!(rows(&[json!({"Id":"foreign"})],1,&input,Gesture::Down).is_err());assert!(rows(&[json!({"Id":MAX_SOURCE_ID})],1,&input,Gesture::Tap).is_err());
        assert!(rows(&[],0,&Input::Keyboard("KEY_YEN".into()),Gesture::Tap).is_err());assert!(rows(&[],0,&Input::Keyboard("KEY_UP_RIGHT_ARROW".into()),Gesture::Tap).is_err());
    }
    #[test] fn mouse_pairs_and_complete_actions_keep_source_button_numbers() {
        for button in 0..=9 {let input=Input::Mouse(button);let authored=rows(&[],0,&input,Gesture::Tap).unwrap();assert_eq!(authored.len(),if button<5{2}else{1});assert_eq!(authored[0]["MouseEvent"]["MouseButton"],button);assert_eq!(authored[0]["MouseEvent"]["State"],0);assert!(label(&authored[0]).is_some());if button<5{assert_eq!(authored[1]["MouseEvent"]["State"],1);}else{assert!(rows(&[],0,&input,Gesture::Up).is_err());}}
        assert!(rows(&[],0,&Input::Mouse(10),Gesture::Tap).is_err());
    }
    #[test] fn input_authoring_preserves_opaque_rows_profiles_and_cached_records() {
        let store=edit(&Store::default(),Change::Create{guid:"edit".into(),name:"Edit".into()}).unwrap();let record=read(&store).unwrap().entries.remove(0).encoded_record;
        let changed=edit_definition(&record,Change::InsertInput{guid:"edit".into(),index:1,input:Input::Keyboard("KEY_A".into()),gesture:Gesture::Tap}).unwrap();let events=describe(&changed).unwrap().events;assert_eq!(events.len(),3);assert_eq!(events[0],describe(&record).unwrap().events[0]);assert_eq!(events[1]["KeyEvent"]["Makecode"],65);assert_eq!(events[1]["Id"],events[2]["Id"]);
        let mut body=decode_record(&record).unwrap();body["appEngine"]=json!({"events":[]});let cached=super::super::record(&body,"Cached").unwrap();assert!(edit_definition(&cached,Change::InsertInput{guid:"edit".into(),index:1,input:Input::Mouse(0),gesture:Gesture::Tap}).is_err());
        let text=r#"{"guid":"raw","macroEvents":[{"Type":"actionBar"},{"future":9.9900e+100}],"future":1.2300e-7}"#;let body:Value=serde_json::from_str(text).unwrap();
        let raw_record=json!({"name":"Raw","payload":STANDARD.encode(text),"hash":synapse_hash(&body).unwrap()});
        let changed=edit_definition(&raw_record,Change::InsertInput{guid:"raw".into(),index:2,input:Input::Mouse(4),gesture:Gesture::Tap}).unwrap();let bytes=String::from_utf8(STANDARD.decode(changed["payload"].as_str().unwrap()).unwrap()).unwrap();assert!(bytes.contains("9.9900e+100")&&bytes.contains("1.2300e-7"));
        assert!(edit_definition(&record,Change::InsertInput{guid:"edit".into(),index:0,input:Input::Mouse(0),gesture:Gesture::Tap}).is_err());
        let full=json!({"guid":"full","macroEvents":vec![json!({"Type":0,"Number":"1"});512]});let full=super::super::record(&full,"Full").unwrap();assert!(edit_definition(&full,Change::InsertInput{guid:"full".into(),index:512,input:Input::Mouse(0),gesture:Gesture::Tap}).is_err());
    }
}
