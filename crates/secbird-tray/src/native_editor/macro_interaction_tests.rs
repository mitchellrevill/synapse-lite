use super::*;
use secbird_core::profile_compat::macro_library as library;

#[test]
#[ignore="opens an isolated simulated macro editor window"]
fn native_macro_library_drafts_save_history_conflicts_and_lifecycle() {
    let folder=std::env::temp_dir().join(format!("native-macros-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&folder).unwrap();let frame=folder.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&folder),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("dashboard"))]);
    let sim=Sim::mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();let path=folder.join("profiles.json");
    let mut original=Store::load(&path).unwrap();original.for_device(&engine.devices()[0]).active_mut().dpi=Some((800,800));original.save().unwrap();
    let original_value=serde_json::to_value(&original).unwrap();let output=folder.clone();let monitor=sim.clone();
    let controller=std::thread::spawn(move||{
        let result=(||->Result<Value,String>{unsafe{
            wait_for(||frame.exists(),"macro editor window")?;let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,super::super::macro_view::TEST,2,&mut value as *mut Value as isize);value};
            SendMessageW(hwnd,super::super::macro_view::TEST,1,0);wait_for(||query()["open"]==true&&query()["busy"]==false,"macro library read")?;
            monitor.state().log.clear();let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;let mut client:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut client);let width=f64::from(client.right)/scale;let left=(width-900.).max(10.)/2.;let timeline=left+210.;
            let click=|x,y|lighting_click(hwnd,x,y,scale);
            let edit=|id,text:&str|{let field=GetDlgItem(hwnd,id);assert!(!field.is_null(),"missing edit field {id}");SetWindowTextW(field,wide(text).as_ptr());};
            click(350.,100.);wait_for(||query()["entries"].as_array().is_some_and(|a|a.len()==1)&&query()["busy"]==false,"new macro")?;
            let id=query()["selected"].as_str().unwrap().to_owned();let baseline=std::fs::read(&path).map_err(|e|e.to_string())?;
            edit(2300,"Night macro");edit(2301,"1250");click(left+90.,368.);
            if query()["dirty"]!=true||query()["events"][1]["Number"]!="1.25"{return Err(format!("Local draft incorrect: {}",query()));}
            if std::fs::read(&path).unwrap()!=baseline{return Err("Unsaved fields reached disk".into());}
            SendMessageW(hwnd,super::super::macro_view::TEST,3,0);SendMessageW(hwnd,WM_CLOSE,0,0);
            if query()["open"]!=true||IsWindow(hwnd)==0{return Err("Dirty draft was closed".into());}
            click(timeline+620.,174.);wait_for(||query()["busy"]==false&&query()["dirty"]==false,"explicit save")?;
            let saved=Store::load(&path).map_err(|e|e.to_string())?;let entries=library::read(&saved).map_err(|e|e.to_string())?.entries;
            if entries[0].name!="Night macro"||entries[0].guid!=id||entries[0].events[1]["Number"]!="1.25"{return Err("Saved definition differs from draft".into());}
            click(timeline+40.,174.);if query()["dirty"]!=true{return Err("Undo history lost after save".into());}click(timeline+130.,174.);if query()["dirty"]!=false{return Err("Redo did not return to saved definition".into());}
            let saved_bytes=std::fs::read(&path).unwrap();edit(2301,"-1");click(left+90.,404.);if query()["failed"]!=true||std::fs::read(&path).unwrap()!=saved_bytes{return Err("Invalid delay was saved".into());}
            SendMessageW(hwnd,super::super::macro_view::TEST,5,0);
            SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);click(350.,100.);click(left+90.,368.);click(timeline+620.,174.);if std::fs::read(&path).unwrap()!=saved_bytes{return Err("Read-only controls changed disk".into());}SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);
            click(left+45.,254.);wait_for(||query()["entries"].as_array().is_some_and(|a|a.len()==2)&&query()["busy"]==false,"duplicate")?;
            if query()["selected"]==id{return Err("Duplicate reused original GUID".into());}click(left+145.,254.);wait_for(||query()["entries"].as_array().is_some_and(|a|a.len()==1)&&query()["busy"]==false,"delete unused duplicate")?;
            edit(2300,"Local draft");let current=Store::load(&path).map_err(|e|e.to_string())?;let changed=library::edit(&current,library::Change::Rename{guid:id.clone(),name:"External edit".into()}).map_err(|e|e.to_string())?;changed.save().map_err(|e|e.to_string())?;let external=std::fs::read(&path).unwrap();
            click(timeline+620.,174.);wait_for(||query()["busy"]==false&&query()["failed"]==true,"stale save refusal")?;if query()["dirty"]!=true||std::fs::read(&path).unwrap()!=external{return Err("Stale save overwrote external definition".into());}
            let capture=json!(output.join("macro-editor.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            SendMessageW(hwnd,super::super::macro_view::TEST,5,0);SendMessageW(hwnd,super::super::macro_view::TEST,3,0);if query()["open"]!=false{return Err("Discard did not allow Home".into());}
            SendMessageW(hwnd,super::super::macro_view::TEST,1,0);wait_for(||query()["open"]==true&&query()["busy"]==false,"macro reopen")?;
            let fixture=output.join("fixture.synapse4");std::fs::write(&fixture,include_bytes!("../../../secbird-core/tests/fixtures/synthetic.synapse4")).unwrap();let import=json!(fixture.to_string_lossy());SendMessageW(hwnd,super::super::macro_view::TEST,4,&import as *const Value as isize);
            wait_for(||query()["entries"].as_array().is_some_and(|a|a.len()==2)&&query()["busy"]==false,"definition import")?;
            let final_value=serde_json::to_value(Store::load(&path).unwrap()).unwrap();if final_value["devices"]!=original_value["devices"]{return Err("Macro library changed device profiles".into());}
            if monitor.state().log.iter().any(|c|c.id&0x80==0){return Err("Macro editor issued a hardware SET".into());}
            Ok(json!({"drafts_local_until_save":true,"dirty_home_and_close_guard":true,"save_guid_preserved":true,"history_survives_save":true,"invalid_delay_refused":true,"read_only_inert":true,"duplicate_and_delete":true,"stale_save_refused":true,"definition_import_without_profile_import":true,"profiles_preserved":true,"hardware_sets":0,"physical_discovery_disabled":true}))
        }})();
        unsafe{let hwnd=WINDOW.load(Ordering::Acquire) as HWND;SendMessageW(hwnd,super::super::macro_view::TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}
        std::fs::write(output.join("interaction.json"),serde_json::to_vec_pretty(&match &result{Ok(v)=>v.clone(),Err(e)=>json!({"error":e})}).unwrap()).unwrap();result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(40)));let result=controller.join().unwrap();assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");eprintln!("Macro native evidence: {}",folder.display());
}
