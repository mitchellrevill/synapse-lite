use super::*;
use secbird_core::profile_compat::macro_library as library;
use windows_sys::Win32::UI::Controls::EM_REPLACESEL;

#[test]
#[ignore="opens an isolated native macro surface and exercises real pointer targets"]
fn native_macro_source_surface_selector_create_save_and_dirty_switch() {
    let folder=std::env::temp_dir().join(format!("native-macro-parity-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&folder).unwrap();let frame=folder.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&folder),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("dashboard"))]);
    let sim=Sim::mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();let path=folder.join("profiles.json");
    let mut store=Store::load(&path).unwrap();
    for index in 0..10 {store=library::edit(&store,library::Change::Create{guid:format!("fixture-{index}"),name:format!("Fixture {index}")}).unwrap();}
    store.save().unwrap();let original_devices=serde_json::to_value(&store.devices).unwrap();let output=folder.clone();let monitor=sim.clone();
    let controller=std::thread::spawn(move||{
        let result=(||->Result<(),String>{unsafe{
            wait_for(||frame.exists(),"macro parity HWND")?;let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,super::super::macro_view::TEST,2,&mut value as *mut Value as isize);value};
            SendMessageW(hwnd,super::super::macro_view::TEST,1,0);
            wait_for(||query()["open"]==true&&query()["busy"]==false,"macro parity library")?;
            let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;let mut rect:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut rect);let width=f64::from(rect.right)/scale;let height=f64::from(rect.bottom)/scale;
            let click=|x,y|{lighting_click(hwnd,x,y,scale);redraw(hwnd);};
            if query()["surface_icons_loaded"]!=true{return Err("Required macro source icons were not loaded".into());}
            if query()["legacy_editor"]!=false{return Err("Normal macro page opened the detail inspector".into());}
            let target=query()["entries"][4]["guid"].as_str().unwrap().to_string();
            click(130.,103.);if query()["picker"]!=true||!GetDlgItem(hwnd,2300).is_null(){return Err("Macro selector is covered by an edit child".into());}
            click(130.,116.+4.*28.+14.);if query()["selected"]!=target{return Err("Fifth macro dropdown row did not select".into());}
            click(360.,103.);wait_for(||query()["busy"]==false&&query()["entries"].as_array().is_some_and(|rows|rows.len()==11),"new macro persisted")?;
            let created=query()["selected"].as_str().unwrap().to_string();
            let editor=((width+20.-600.)/2.).max(290.);click(editor-160.,208.);
            if query()["dirty"]!=true{return Err("Add Delay did not edit the selected macro".into());}
            let delay=GetDlgItem(hwnd,2303);if delay.is_null()||IsWindowVisible(delay)==0{return Err("Selected delay has no inline seconds editor".into());}
            SetWindowTextW(delay,wide("1.25").as_ptr());redraw(hwnd);if query()["delay"]!="1250"{return Err("Inline seconds edit did not update its delay".into());}
            click(299.,103.);click(330.,195.);
            let name=GetDlgItem(hwnd,2300);if name.is_null(){return Err("Rename did not open the selector's edit control".into());}
            SetWindowTextW(name,wide("Source inline macro").as_ptr());redraw(hwnd);
            if query()["name"]!="Source inline macro"{return Err("Inline rename did not update the draft".into());}
            let input=json!({"key":"KEY_A","gesture":"tap"});SendMessageW(hwnd,super::super::macro_view::TEST,6,&input as *const Value as isize);
            if query()["events"].as_array().unwrap().iter().filter(|row|row["Type"]==1).count()!=2{return Err("Choosing a keyboard function did not insert its press/release pair".into());}
            let capture=json!(output.join("macro-inline-draft.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            click(130.,103.);click(130.,130.);
            if query()["switch_pending"].is_null(){return Err("Dirty macro switch did not ask Save/Discard/Cancel".into());}
            let destination=query()["switch_pending"].as_str().unwrap().to_string();
            click(width/2.-150.,height/2.+45.);if query()["selected"]!=created||query()["dirty"]!=true{return Err("Cancel lost the macro draft".into());}
            click(130.,103.);click(130.,130.);click(width/2.+150.,height/2.+45.);
            wait_for(||query()["busy"]==false&&query()["selected"]==destination,"save then switch")?;
            let saved=Store::load(&path).map_err(|e|e.to_string())?;let library=library::read(&saved).map_err(|e|e.to_string())?;
            if library.entries.iter().find(|entry|entry.guid==created).unwrap().events.is_empty(){return Err("Save and switch lost the new delay".into());}
            let capture=json!(output.join("macro-source-surface.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            if serde_json::to_value(&saved.devices).unwrap()!=original_devices{return Err("Macro editing changed device profiles".into());}
            if monitor.state().log.iter().any(|command|command.id&0x80==0){return Err("Macro editing sent a device SET".into());}
            Ok(())
        }})();
        unsafe{let hwnd=WINDOW.load(Ordering::Acquire) as HWND;PostMessageW(hwnd,WM_CLOSE,0,0);}
        std::fs::write(output.join("result.json"),serde_json::to_vec_pretty(&json!({"passed":result.is_ok(),"error":result.as_ref().err(),"private_configuration":true,"physical_discovery":false})).unwrap()).unwrap();result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(40)));let result=controller.join().unwrap();
    assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");eprintln!("Macro surface evidence: {}",folder.display());
}

#[test]
#[ignore="opens an isolated simulated macro loop editor window"]
fn native_macro_loops_range_nested_history_save_discard_and_invalid_repeat() {
    let folder=std::env::temp_dir().join(format!("native-macro-loops-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&folder).unwrap();let frame=folder.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&folder),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("dashboard"))]);
    let sim=Sim::mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();let path=folder.join("profiles.json");
    let mut original=Store::load(&path).unwrap();original.for_device(&engine.devices()[0]).active_mut().dpi=Some((800,800));original.save().unwrap();
    let profiles=serde_json::to_value(&original).unwrap()["devices"].clone();let output=folder.clone();let monitor=sim.clone();
    let controller=std::thread::spawn(move||{
        let result=(||->Result<Value,String>{unsafe{
            wait_for(||frame.exists(),"macro loop HWND")?;let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,super::super::macro_view::TEST,2,&mut value as *mut Value as isize);value};
            SendMessageW(hwnd,super::super::macro_view::TEST,1,0);SendMessageW(hwnd,super::super::macro_view::TEST,13,0);wait_for(||query()["open"]==true&&query()["busy"]==false,"loop library read")?;redraw(hwnd);
            monitor.state().log.clear();
            let edit=|id:i32,text:&str|{if matches!(id,2310|2311){let field=GetDlgItem(hwnd,id);assert!(!field.is_null(),"missing host action field {id}");SendMessageW(field,EM_SETSEL,0,-1);SendMessageW(field,EM_REPLACESEL,1,wide(text).as_ptr()as isize);}else{macro_test_field(hwnd,id as u16,text);}};
            macro_command(hwnd,super::super::macro_view::Control::New);wait_for(||query()["entries"].as_array().is_some_and(|a|a.len()==1)&&query()["busy"]==false,"new loop macro")?;
            macro_command(hwnd,super::super::macro_view::Control::WrapLoop);if query()["events"].as_array().map(Vec::len)!=Some(3){return Err("Empty loop creation failed".into());}macro_command(hwnd,super::super::macro_view::Control::Undo);
            edit(2300,"Loop authoring");edit(2301,"125");macro_command(hwnd,super::super::macro_view::Control::AddDelay);macro_command(hwnd,super::super::macro_view::Control::AddDelay);
            let before=query()["events"].clone();let disk=std::fs::read(&path).unwrap();
            macro_command(hwnd,super::super::macro_view::Control::Event(1));macro_command(hwnd,super::super::macro_view::Control::MarkLoopStart);macro_command(hwnd,super::super::macro_view::Control::Event(2));edit(2302,"3");macro_command(hwnd,super::super::macro_view::Control::WrapLoop);
            let wrapped=query();if wrapped["events"].as_array().map(Vec::len)!=Some(5)||wrapped["events"][1]["LoopEvent"]["State"]!=0||wrapped["events"][4]["LoopEvent"]["State"]!=1||wrapped["events"][1]["Id"]!=wrapped["events"][4]["Id"]||wrapped["events"][1]["Number"]!=3{return Err(format!("Native loop range incorrect: {wrapped}"));}
            if wrapped["events"][2]!=before[1]||wrapped["events"][3]!=before[2]||std::fs::read(&path).unwrap()!=disk{return Err("Wrapping changed body or saved without Save".into());}
            macro_command(hwnd,super::super::macro_view::Control::Event(4));edit(2302,"4");macro_command(hwnd,super::super::macro_view::Control::ApplyLoop);
            let updated=query();if updated["events"][1]["Number"]!=4||updated["events"][4]["Number"]!=4{return Err(format!("End-boundary edit failed: {updated}"));}
            edit(2302,"2");macro_command(hwnd,super::super::macro_view::Control::WrapLoop);let nested=query();
            if nested["events"].as_array().map(Vec::len)!=Some(7)||nested["events"][1]["Id"]==nested["events"][2]["Id"]||nested["events"][2]!=updated["events"][1]||nested["events"][5]!=updated["events"][4]||nested["events"][1]["Number"]!=2{return Err(format!("Nested loop incorrect: {nested}"));}
            macro_command(hwnd,super::super::macro_view::Control::DeleteEvent);if query()["events"]!=updated["events"]{return Err("Loop delete removed the body".into());}
            macro_command(hwnd,super::super::macro_view::Control::Undo);if query()["events"]!=nested["events"]{return Err("Loop delete Undo failed".into());}
            macro_command(hwnd,super::super::macro_view::Control::Redo);if query()["events"]!=updated["events"]{return Err("Loop delete Redo failed".into());}
            macro_command(hwnd,super::super::macro_view::Control::Undo);macro_command(hwnd,super::super::macro_view::Control::Save);wait_for(||query()["busy"]==false&&query()["dirty"]==false,"loop Save")?;
            let saved=Store::load(&path).map_err(|e|e.to_string())?;let entry=library::read(&saved).map_err(|e|e.to_string())?.entries.remove(0);
            if json!(entry.events)!=nested["events"]||library::loops::pairs(&entry.events).map_err(|e|e.to_string())?.len()!=2{return Err("Saved loop rows differ".into());}
            let saved_bytes=std::fs::read(&path).unwrap();macro_command(hwnd,super::super::macro_view::Control::Event(1));edit(2302,"100000");macro_command(hwnd,super::super::macro_view::Control::ApplyLoop);macro_command(hwnd,super::super::macro_view::Control::Save);
            if query()["failed"]!=true||query()["events"]!=nested["events"]||query()["dirty"]!=true||std::fs::read(&path).unwrap()!=saved_bytes{return Err("Invalid loop repeat mutated or saved rows".into());}
            macro_command(hwnd,super::super::macro_view::Control::Discard);if query()["dirty"]!=false||query()["events"]!=nested["events"]{return Err("Loop Discard failed".into());}
            macro_command(hwnd,super::super::macro_view::Control::Event(1));let readonly=query();SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);redraw(hwnd);macro_command(hwnd,super::super::macro_view::Control::MarkLoopStart);macro_command(hwnd,super::super::macro_view::Control::WrapLoop);macro_command(hwnd,super::super::macro_view::Control::ApplyLoop);macro_command(hwnd,super::super::macro_view::Control::DeleteEvent);
            if query()["events"]!=readonly["events"]||query()["dirty"]!=false{return Err("Read-only loop controls mutated rows".into());}SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);redraw(hwnd);
            let field=GetDlgItem(hwnd,2302);let mut text=[0u16;16];let length=GetWindowTextW(field,text.as_mut_ptr(),text.len() as i32);let mut field_rect:RECT=std::mem::zeroed();GetClientRect(field,&mut field_rect);
            if String::from_utf16_lossy(&text[..length as usize])!="2"||IsWindowVisible(field)==0||field_rect.right<30||field_rect.bottom<15{return Err("Native repeat edit HWND text/visibility/geometry failed".into());}
            let capture=json!(output.join("macro-loops.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            if serde_json::to_value(Store::load(&path).unwrap()).unwrap()["devices"]!=profiles{return Err("Loop Save changed device profiles".into());}
            if monitor.state().log.iter().any(|c|c.id&0x80==0){return Err("Loop editor issued a hardware SET".into());}
            Ok(json!({"native_command_dispatch":true,"empty_loop_authoring":true,"native_repeat_child_text_visibility_geometry":true,"range_wrap_preserves_body":true,"paired_end_repeat_update":true,"nested_distinct_ids":true,"pair_delete_preserves_body":true,"undo_redo_exact":true,"explicit_save_exact_rows":true,"discard_restores_saved_rows":true,"invalid_repeat_blocks_save":true,"read_only_inert":true,"profiles_preserved":true,"hardware_sets":0,"physical_discovery_disabled":true}))
        }})();
        unsafe{let hwnd=WINDOW.load(Ordering::Acquire) as HWND;SendMessageW(hwnd,super::super::macro_view::TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}
        std::fs::write(output.join("interaction.json"),serde_json::to_vec_pretty(&match &result{Ok(v)=>v.clone(),Err(e)=>json!({"error":e})}).unwrap()).unwrap();result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(40)));let result=controller.join().unwrap();assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");eprintln!("Macro loop native evidence: {}",folder.display());
}

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
            // Keep normal action deadlines; allow private-asset startup to finish.
            let ready_started=Instant::now();while !frame.exists()&&ready_started.elapsed()<Duration::from_secs(15){std::thread::sleep(Duration::from_millis(20));}if !frame.exists(){return Err("Timed out: macro editor window (15s startup budget)".into());}let ready_ms=ready_started.elapsed().as_millis();let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,super::super::macro_view::TEST,2,&mut value as *mut Value as isize);value};
            SendMessageW(hwnd,super::super::macro_view::TEST,1,0);SendMessageW(hwnd,super::super::macro_view::TEST,13,0);wait_for(||query()["open"]==true&&query()["busy"]==false,"macro library read")?;redraw(hwnd);
            monitor.state().log.clear();let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;let mut client:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut client);let width=f64::from(client.right)/scale;
            let click=|x,y|lighting_click(hwnd,x,y,scale);
            let edit=|id:i32,text:&str|{if matches!(id,2310|2311){let field=GetDlgItem(hwnd,id);assert!(!field.is_null(),"missing host action field {id}");SendMessageW(field,EM_SETSEL,0,-1);SendMessageW(field,EM_REPLACESEL,1,wide(text).as_ptr()as isize);}else{macro_test_field(hwnd,id as u16,text);}};
            macro_command(hwnd,super::super::macro_view::Control::New);wait_for(||query()["entries"].as_array().is_some_and(|a|a.len()==1)&&query()["busy"]==false,"new macro")?;
            let id=query()["selected"].as_str().unwrap().to_owned();let baseline=std::fs::read(&path).map_err(|e|e.to_string())?;
            macro_command(hwnd,super::super::macro_view::Control::InputMenu);wait_for(||query()["menu_open"]==true,"input chooser open")?;SendMessageW(hwnd,STUDIO_TEST,9,0);wait_for(||query()["menu_open"]==false,"input chooser cancel")?;if query()["dirty"]!=false||std::fs::read(&path).unwrap()!=baseline{return Err("Cancelled input menu changed draft or disk".into());}
            // Explicit initiation, synthetic window messages only: never physical input.
            macro_command(hwnd,if query()["recording"]==true{super::super::macro_view::Control::StopRecording}else{super::super::macro_view::Control::Record});redraw(hwnd);
            if query()["recording"]!=true||query()["playback"]["active"]!=false{return Err("Record did not enter exclusive focused capture".into());}
            // The merged Play toolbar must not shadow Record or admit playback during capture.
            macro_command(hwnd,super::super::macro_view::Control::Play);
            if query()["recording"]!=true||query()["playback"]["active"]!=false{return Err("Playback overlapped focused capture".into());}
            let capture=json!(output.join("macro-recording.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            SendMessageW(hwnd,WM_KEYDOWN,65,30<<16);SendMessageW(hwnd,WM_KEYDOWN,65,(30<<16)|(1<<30));SendMessageW(hwnd,WM_KEYUP,65,30<<16);
            macro_command(hwnd,if query()["recording"]==true{super::super::macro_view::Control::StopRecording}else{super::super::macro_view::Control::Record});
            let recorded=query();if recorded["events"].as_array().map(Vec::len)!=Some(4)||recorded["events"][1]["ScanCode"]!=30||recorded["events"][1]["Id"]!=recorded["events"][3]["Id"]{return Err(format!("Recording did not preserve source pair: {recorded}"));}
            if std::fs::read(&path).unwrap()!=baseline{return Err("Recording wrote before Save".into());}
            macro_command(hwnd,super::super::macro_view::Control::Undo);if query()["events"].as_array().map(Vec::len)!=Some(1){return Err("Recording was not one undo unit".into());}
            macro_command(hwnd,if query()["recording"]==true{super::super::macro_view::Control::StopRecording}else{super::super::macro_view::Control::Record});SendMessageW(hwnd,WM_KEYDOWN,65,30<<16);SendMessageW(hwnd,WM_KEYDOWN,27,1<<16);
            if query()["events"].as_array().map(Vec::len)!=Some(1){return Err("Cancel changed draft".into());}
            macro_command(hwnd,if query()["recording"]==true{super::super::macro_view::Control::StopRecording}else{super::super::macro_view::Control::Record});SendMessageW(hwnd,WM_KEYDOWN,65,30<<16);SendMessageW(hwnd,WM_KILLFOCUS,0,0);SendMessageW(hwnd,WM_KEYUP,65,30<<16);
            if query()["events"].as_array().map(Vec::len)!=Some(1){return Err("Focus loss retained incomplete down".into());}
            edit(2300,"Night macro");edit(2301,"1250");macro_command(hwnd,super::super::macro_view::Control::AddDelay);
            if query()["dirty"]!=true||query()["events"][1]["Number"]!="1.25"{return Err(format!("Local draft incorrect: {}",query()));}
            if std::fs::read(&path).unwrap()!=baseline{return Err("Unsaved fields reached disk".into());}
            for choice in [json!({"key":"KEY_RIGHT_CTRL"}),json!({"mouse":1}),json!({"mouse":8})] {
                SendMessageW(hwnd,super::super::macro_view::TEST,16,&choice as *const Value as isize);macro_command(hwnd,super::super::macro_view::Control::AddInput);
            }
            let initial=query();if initial["events"].as_array().map(Vec::len)!=Some(7)||initial["events"][2]["KeyEvent"]["State"]!=2||initial["events"][3]["KeyEvent"]["State"]!=3||initial["events"][4]["MouseEvent"]["MouseButton"]!=1||initial["events"][6]["MouseEvent"]["MouseButton"]!=8{return Err(format!("Native input authoring incorrect: {initial}"));}
            macro_command(hwnd,super::super::macro_view::Control::Event(2));let shift=json!({"key":"KEY_RIGHT_SHIFT"});SendMessageW(hwnd,super::super::macro_view::TEST,16,&shift as *const Value as isize);macro_command(hwnd,super::super::macro_view::Control::RebindInput);
            let rebound=query();if rebound["events"][2]["ScanCode"]!=54||rebound["events"][3]["ScanCode"]!=54||rebound["events"][2]["KeyEvent"]["State"]!=0||rebound["events"][3]["KeyEvent"]["State"]!=1||rebound["events"][2]["Id"]!=initial["events"][2]["Id"]{return Err(format!("Key pair rebind incorrect: {rebound}"));}
            macro_command(hwnd,super::super::macro_view::Control::Event(4));let button=json!({"mouse":4});SendMessageW(hwnd,super::super::macro_view::TEST,16,&button as *const Value as isize);macro_command(hwnd,super::super::macro_view::Control::RebindInput);let authored=query();if authored["events"][4]["MouseEvent"]["MouseButton"]!=4||authored["events"][5]["MouseEvent"]["MouseButton"]!=4{return Err("Mouse pair rebind did not change both rows".into());}
            macro_command(hwnd,super::super::macro_view::Control::DeleteEvent);if query()["events"].as_array().map(Vec::len)!=Some(5){return Err("Delete pair did not remove both rows".into());}macro_command(hwnd,super::super::macro_view::Control::Undo);if query()["events"]!=authored["events"]{return Err("Undo did not restore the deleted pair".into());}
            if std::fs::read(&path).unwrap()!=baseline{return Err("Input rows reached disk before Save".into());}
            SendMessageW(hwnd,super::super::macro_view::TEST,3,0);SendMessageW(hwnd,WM_CLOSE,0,0);
            if query()["open"]!=true||IsWindow(hwnd)==0{return Err("Dirty draft was closed".into());}
            macro_command(hwnd,super::super::macro_view::Control::Save);wait_for(||query()["busy"]==false&&query()["dirty"]==false,"explicit save")?;
            let saved=Store::load(&path).map_err(|e|e.to_string())?;let entries=library::read(&saved).map_err(|e|e.to_string())?.entries;
            if entries[0].name!="Night macro"||entries[0].guid!=id||entries[0].events[1]["Number"]!="1.25"||entries[0].events.len()!=7||json!(entries[0].events)!=authored["events"]{return Err("Saved definition differs from draft".into());}
            macro_command(hwnd,super::super::macro_view::Control::Undo);if query()["dirty"]!=true{return Err("Undo history lost after save".into());}macro_command(hwnd,super::super::macro_view::Control::Redo);if query()["dirty"]!=false{return Err("Redo did not return to saved definition".into());}
            let saved_bytes=std::fs::read(&path).unwrap();macro_command(hwnd,super::super::macro_view::Control::Event(1));edit(2301,"-1");macro_command(hwnd,super::super::macro_view::Control::ApplyDelay);if query()["failed"]!=true||std::fs::read(&path).unwrap()!=saved_bytes{return Err("Invalid delay was saved".into());}
            SendMessageW(hwnd,super::super::macro_view::TEST,5,0);
            macro_command(hwnd,super::super::macro_view::Control::Event(2));let blocked=json!({"key":"KEY_B"});SendMessageW(hwnd,super::super::macro_view::TEST,16,&blocked as *const Value as isize);
            SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);redraw(hwnd);macro_command(hwnd,super::super::macro_view::Control::New);macro_command(hwnd,super::super::macro_view::Control::AddDelay);macro_command(hwnd,super::super::macro_view::Control::AddInput);macro_command(hwnd,super::super::macro_view::Control::RebindInput);macro_command(hwnd,super::super::macro_view::Control::Save);if std::fs::read(&path).unwrap()!=saved_bytes||query()["dirty"]!=false{return Err("Read-only controls changed disk or draft".into());}SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);redraw(hwnd);
            macro_command(hwnd,super::super::macro_view::Control::Duplicate);wait_for(||query()["entries"].as_array().is_some_and(|a|a.len()==2)&&query()["busy"]==false,"duplicate").map_err(|error|format!("{error}: {}",query()))?;
            if query()["selected"]==id{return Err("Duplicate reused original GUID".into());}macro_command(hwnd,super::super::macro_view::Control::Delete);wait_for(||query()["entries"].as_array().is_some_and(|a|a.len()==1)&&query()["busy"]==false,"delete unused duplicate")?;
            edit(2300,"Local draft");let current=Store::load(&path).map_err(|e|e.to_string())?;let changed=library::edit(&current,library::Change::Rename{guid:id.clone(),name:"External edit".into()}).map_err(|e|e.to_string())?;changed.save().map_err(|e|e.to_string())?;let external=std::fs::read(&path).unwrap();
            macro_command(hwnd,super::super::macro_view::Control::Save);wait_for(||query()["busy"]==false&&query()["failed"]==true,"stale save refusal")?;if query()["dirty"]!=true||std::fs::read(&path).unwrap()!=external{return Err("Stale save overwrote external definition".into());}
            let capture=json!(output.join("macro-editor.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            SendMessageW(hwnd,super::super::macro_view::TEST,5,0);SendMessageW(hwnd,super::super::macro_view::TEST,3,0);if query()["open"]!=false{return Err("Discard did not allow Home".into());}
            SendMessageW(hwnd,super::super::macro_view::TEST,1,0);SendMessageW(hwnd,super::super::macro_view::TEST,13,0);wait_for(||query()["open"]==true&&query()["busy"]==false,"macro reopen")?;
            let fixture=output.join("fixture.synapse4");std::fs::write(&fixture,include_bytes!("../../../secbird-core/tests/fixtures/synthetic.synapse4")).unwrap();let import=json!(fixture.to_string_lossy());SendMessageW(hwnd,super::super::macro_view::TEST,4,&import as *const Value as isize);
            wait_for(||query()["entries"].as_array().is_some_and(|a|a.len()==2)&&query()["busy"]==false,"definition import")?;
            let mut seeded=Store::load(&path).map_err(|e|e.to_string())?;for (id,name) in [("nested-b","Nested B"),("nested-c","Nested C")] {seeded=library::edit(&seeded,library::Change::Create{guid:id.into(),name:name.into()}).map_err(|e|e.to_string())?;}seeded.save().map_err(|e|e.to_string())?;
            SendMessageW(hwnd,super::super::macro_view::TEST,3,0);SendMessageW(hwnd,super::super::macro_view::TEST,1,0);SendMessageW(hwnd,super::super::macro_view::TEST,13,0);wait_for(||query()["busy"]==false&&query()["entries"].as_array().is_some_and(|a|a.len()==4),"nested target library refresh")?;
            let owner=json!(id);SendMessageW(hwnd,super::super::macro_view::TEST,7,&owner as *const Value as isize);redraw(hwnd);
            let nested=json!({"macro":"nested-b"});SendMessageW(hwnd,super::super::macro_view::TEST,16,&nested as *const Value as isize);macro_command(hwnd,super::super::macro_view::Control::AddInput);if query()["events"][7]["guid"]!="nested-b"||query()["dirty"]!=true{return Err(format!("Nested draft not inserted: {}",query()));}macro_command(hwnd,super::super::macro_view::Control::Save);wait_for(||query()["busy"]==false&&query()["dirty"]==false,"nested save")?;
            let next=json!({"macro":"nested-c"});SendMessageW(hwnd,super::super::macro_view::TEST,16,&next as *const Value as isize);macro_command(hwnd,super::super::macro_view::Control::RebindInput);if query()["events"][7]["guid"]!="nested-c"{return Err("Nested target rebind failed".into());}macro_command(hwnd,super::super::macro_view::Control::Save);wait_for(||query()["busy"]==false&&query()["dirty"]==false,"nested retarget save")?;
            let current=Store::load(&path).map_err(|e|e.to_string())?;if library::edit(&current,library::Change::Delete("nested-c".into())).is_ok(){return Err("Referenced nested definition was deletable".into());}
            let child=json!("nested-c");SendMessageW(hwnd,super::super::macro_view::TEST,7,&child as *const Value as isize);redraw(hwnd);let mut targets=Value::Null;SendMessageW(hwnd,super::super::macro_view::TEST,8,&mut targets as *mut Value as isize);if !targets.as_array().is_some_and(|targets|targets.iter().any(|target|target["guid"]==id&&target["disabled"]==true)){return Err(format!("Cycle target was not disabled: {targets}"));}
            let baseline=std::fs::read(&path).unwrap();let cycle=json!({"macro":id});SendMessageW(hwnd,super::super::macro_view::TEST,16,&cycle as *const Value as isize);macro_command(hwnd,super::super::macro_view::Control::AddInput);if query()["failed"]!=true||query()["dirty"]!=false||std::fs::read(&path).unwrap()!=baseline{return Err("Cycle request changed the draft/store".into());}
            SendMessageW(hwnd,super::super::macro_view::TEST,7,&owner as *const Value as isize);SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);redraw(hwnd);macro_command(hwnd,super::super::macro_view::Control::AddInput);if query()["dirty"]!=false||std::fs::read(&path).unwrap()!=baseline{return Err("Read-only nested add changed data".into());}SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);redraw(hwnd);
            macro_command(hwnd,super::super::macro_view::Control::Event(7));let capture=json!(output.join("macro-nested.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            let mut client:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut client);let height=f64::from(client.bottom)/scale;let fx=(width-620.)/2.;let fy=(height-360.)/2.;let apply=||{redraw(hwnd);click(fx+536.,fy+326.);};let cancel=||{redraw(hwnd);click(fx+406.,fy+326.);};
            let stored=std::fs::read(&path).unwrap();let text=json!("text");SendMessageW(hwnd,super::super::macro_view::TEST,9,&text as *const Value as isize);redraw(hwnd);if query()["form"]["data"]["kind"]!="text"{return Err("Text action form did not open".into());}edit(2310,&"x".repeat(251));apply();if query()["form"]["error"].as_str().is_none_or(str::is_empty)||query()["events"].as_array().map(Vec::len)!=Some(8){return Err("Oversized text changed the draft".into());}
            edit(2310,"Hello\r\nworld 😃");if query()["form"]["data"]["value"]!="Hello\nworld 😃"{return Err("Multiline edit did not update the form value".into());}SendMessageW(hwnd,super::super::macro_view::TEST,3,0);SendMessageW(hwnd,WM_CLOSE,0,0);if query()["form"].is_null()||IsWindow(hwnd)==0{return Err("Action form was lost on Home/Close".into());}
            let capture=json!(output.join("macro-text-form.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);apply();if !query()["form"].is_null()||query()["events"][8]["Text"]!="Hello\nworld 😃"||std::fs::read(&path).unwrap()!=stored{return Err(format!("Text did not remain local: {}",query()));}
            macro_command(hwnd,super::super::macro_view::Control::RebindInput);edit(2310,"Cancelled edit");cancel();if query()["events"][8]["Text"]!="Hello\nworld 😃"{return Err("Cancelled text edit changed event".into());}macro_command(hwnd,super::super::macro_view::Control::RebindInput);edit(2310,"Edited 😃");apply();if query()["events"][8]["Text"]!="Edited 😃"{return Err("Text update failed".into());}
            let command=json!("command");SendMessageW(hwnd,super::super::macro_view::TEST,9,&command as *const Value as isize);redraw(hwnd);let long_command=format!("{}😃 taskmgr literal","x".repeat(4095));edit(2310,&long_command);apply();if query()["events"][9]["TextCommand"]!=long_command{return Err("Command was lost or transformed during authoring".into());}
            let program=json!("program");SendMessageW(hwnd,super::super::macro_view::TEST,9,&program as *const Value as isize);redraw(hwnd);edit(2310,r"C:\Tools\Example.exe");click(fx+104.,fy+156.);edit(2311,"https://example.invalid/kept");click(fx+104.,fy+76.);if query()["form"]["data"]["file"]!=r"C:\Tools\Example.exe"||query()["form"]["data"]["url"]!="https://example.invalid/kept"{return Err("Launch mode switch lost inactive value".into());}apply();
            macro_command(hwnd,super::super::macro_view::Control::RebindInput);click(fx+104.,fy+156.);edit(2311,"https://example.invalid/updated");let capture=json!(output.join("macro-launch-form.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);apply();if query()["events"][10]["RadioIndex"]!=1||query()["events"][10]["Content0"]!=r"C:\Tools\Example.exe"||query()["events"][10]["Content1"]!="https://example.invalid/updated"{return Err("Launch edit did not preserve both fields".into());}
            let launch_value=query()["events"][10].clone();let actions=query()["events"].clone();macro_command(hwnd,super::super::macro_view::Control::RebindInput);SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);redraw(hwnd);apply();cancel();if !query()["form"].is_null()||query()["events"]!=actions||std::fs::read(&path).unwrap()!=stored{return Err("Read-only Apply/Cancel changed action data".into());}SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);redraw(hwnd);
            macro_command(hwnd,super::super::macro_view::Control::Save);wait_for(||query()["busy"]==false&&query()["dirty"]==false,"host actions saved")?;let saved=Store::load(&path).map_err(|e|e.to_string())?;let root=library::read(&saved).map_err(|e|e.to_string())?.entries.into_iter().find(|entry|entry.guid==id).ok_or("Saved macro missing")?;if json!(root.events)!=actions{return Err("Saved action rows differ from local draft".into());}
            SendMessageW(hwnd,super::super::macro_view::TEST,3,0);SendMessageW(hwnd,super::super::macro_view::TEST,1,0);SendMessageW(hwnd,super::super::macro_view::TEST,13,0);wait_for(||query()["busy"]==false,"saved action reopen")?;SendMessageW(hwnd,super::super::macro_view::TEST,7,&owner as *const Value as isize);redraw(hwnd);if query()["events"][9]["TextCommand"]!=long_command||query()["events"][10]!=launch_value{return Err("Reopened actions changed long command or launch values".into());}let final_value=serde_json::to_value(Store::load(&path).unwrap()).unwrap();if final_value["devices"]!=original_value["devices"]{return Err("Macro library changed device profiles".into());}
            if monitor.state().log.iter().any(|c|c.id&0x80==0){return Err("Macro editor issued a hardware SET".into());}
            Ok(json!({"window_ready_elapsed_ms":ready_ms,"startup_budget_seconds":15,"ordinary_action_deadline_seconds":4,"text_form_limit_multiline_and_unicode":true,"action_form_home_close_guard":true,"text_edit_apply_and_cancel":true,"command_authoring_inert_and_exact":true,"long_command_unicode_surrogate_boundary_reopen":true,"launch_modes_preserve_inactive_values":true,"read_only_form_apply_refused_cancel_allowed":true,"action_rows_saved_exactly":true,"native_file_picker_not_exercised":true,"clipboard_process_browser_actions_executed":false,"nested_add_save_retarget":true,"nested_referenced_delete_refused":true,"nested_cycle_menu_disabled":true,"nested_cycle_draft_and_store_unchanged":true,"read_only_nested_inert":true,"keyboard_pair_rebound_both_rows":true,"mouse_pair_rebound_both_rows":true,"pair_identity_preserved":true,"pair_delete_and_undo":true,"read_only_rebind_inert":true,"input_chooser_open_cancel":true,"input_chooser_selection_bypassed":true,"keyboard_extended_down_up":true,"mouse_pair_and_wheel":true,"authored_rows_saved_exactly":true,"drafts_local_until_save":true,"dirty_home_and_close_guard":true,"save_guid_preserved":true,"history_survives_save":true,"invalid_delay_refused":true,"read_only_inert":true,"duplicate_and_delete":true,"stale_save_refused":true,"definition_import_without_profile_import":true,"profiles_preserved":true,"hardware_sets":0,"physical_discovery_disabled":true}))
        }})();
        unsafe{let hwnd=WINDOW.load(Ordering::Acquire) as HWND;if IsWindow(hwnd)!=0{let mut form=Value::Null;SendMessageW(hwnd,super::super::macro_view::TEST,2,&mut form as *mut Value as isize);if !form["form"].is_null(){let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;let mut rect:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut rect);let x=(f64::from(rect.right)/scale-620.)/2.;let y=(f64::from(rect.bottom)/scale-360.)/2.;redraw(hwnd);lighting_click(hwnd,x+406.,y+326.,scale);}SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);SendMessageW(hwnd,super::super::macro_view::TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}}
        std::fs::write(output.join("interaction.json"),serde_json::to_vec_pretty(&match &result{Ok(v)=>v.clone(),Err(e)=>json!({"error":e})}).unwrap()).unwrap();result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(90)));let result=controller.join().unwrap();assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");eprintln!("Macro native evidence: {}",folder.display());
}
