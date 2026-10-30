//! Real window routing and asynchronous file saves, with simulated hardware only.
use super::*;

#[test]
#[ignore = "requires private stock export/assets; opens isolated simulated Starlight authoring window"]
fn native_studio_starlight_properties_gradient_save_and_refusal_preserve_source() {
    let input=PathBuf::from(std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select stock export"));
    let original=std::fs::read(&input).unwrap();
    let directory=std::env::temp_dir().join(format!("native-studio-starlight-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&directory).unwrap();let frame=directory.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&directory),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("studio"))]);
    let sim=Sim::wireless_mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();
    let mut store=Store::load(&directory.join("profiles.json")).unwrap();store.for_device(&engine.devices()[0]);store.save().unwrap();
    let original_store=std::fs::read(directory.join("profiles.json")).unwrap();
    let folder=directory.clone();let monitor=sim.clone();let source=input.clone();
    let controller=std::thread::spawn(move || {
        let result=(|| -> Result<Value,String> {unsafe {
            wait_for_studio_window(||frame.exists(),"Wave window")?;
            let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut value as *mut Value as isize);value};
            wait_for(||query()["pending"]==0,"initial reads")?;
            monitor.state().log.clear();
            let path=json!(source.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,1,&path as *const Value as isize);
            wait_for(||query()["profiles"]==1 && query()["busy"]==false,"composition load")?;
            let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;
            let mut client:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut client);
            let width=f64::from(client.right)/scale;let height=f64::from(client.bottom)/scale;let right=width-250.;
            lighting_click(hwnd,775.,100.,scale);lighting_click(hwnd,800.,412.,scale);
            let id=query()["selected"].as_str().ok_or("Starlight not selected")?.to_owned();
            if !query()["rows"].as_array().unwrap().iter().any(|r|r["id"]==id && r["title"]=="STARLIGHT") {return Err("Starlight layer was not added".into());}
            let fit=((width-540.)/960.).min((height-250.)/360.).clamp(0.05,4.);
            lighting_click(hwnd,width/2.+(67.-480.)*fit,(118.+height)/2.+(76.-180.)*fit,scale);
            lighting_click(hwnd,right+195.,290.,scale); // Fast500 assigns source defaults
            let density=GetDlgItem(hwnd,2108);if density.is_null(){return Err("Starlight density field missing".into());}
            SendMessageW(density,WM_SETTEXT,0,wide("7").as_ptr() as isize);lighting_click(hwnd,right+100.,413.,scale);
            lighting_click(hwnd,right+50.,458.,scale); // random on
            lighting_click(hwnd,right+100.,503.,scale); // gradient view, controls disabled
            let history=query()["undo"].clone();lighting_click(hwnd,right+210.,360.,scale); // must not replace gradient while random
            if query()["undo"]!=history {return Err("Random mode allowed a gradient edit".into());}
            lighting_click(hwnd,right+100.,290.,scale); // properties
            lighting_click(hwnd,right+170.,458.,scale); // random off
            let capture=json!(folder.join("starlight-properties.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            lighting_click(hwnd,right+100.,503.,scale);lighting_click(hwnd,right+210.,360.,scale);
            let position=GetDlgItem(hwnd,2103);let color=GetDlgItem(hwnd,2104);if position.is_null()||color.is_null(){return Err("Starlight gradient fields missing".into());}
            SendMessageW(position,WM_SETTEXT,0,wide("25").as_ptr() as isize);SendMessageW(color,WM_SETTEXT,0,wide("112233").as_ptr() as isize);
            lighting_click(hwnd,right+70.,493.,scale);lighting_click(hwnd,right+190.,493.,scale); // None
            let capture=json!(folder.join("starlight-gradient.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            lighting_click(hwnd,630.,100.,scale);
            wait_for(||query()["failed"]==true,"unsupported Starlight preview refusal")?;
            if query()["preview"]["current"]==true {return Err("Starlight falsely presented implemented playback".into());}
            let output=folder.join("starlight.ChromaEffects");let path=json!(output.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);
            wait_for(||query()["busy"]==false && query()["dirty"]==false,"Starlight save")?;
            let bytes=std::fs::read(output).map_err(|e|e.to_string())?;let archive=secbird_core::studio::Archive::parse(&bytes).map_err(|e|e.to_string())?;
            let saved:Value=serde_json::from_str(archive.profile_json(0).unwrap()).unwrap();
            let row=saved["layers"].as_array().unwrap().iter().find(|r|r["id"]==id).ok_or("saved Starlight missing")?;let params=&row["regions"][0]["params"];
            if params["duration"]!=500 || params["density"]!=7 || params["randomColor"]!=false || params["colorStopsCustom"]!=true || params["colorStops"][0]["Stop"]!=25 || params["colorStops"][0].get("Color").is_some() || params["colorStops"][1]["Color"]!=0x00ff00 {return Err(format!("Starlight settings were not saved: {params}"));}
            lighting_click(hwnd,450.,100.,scale);if query()["dirty"]!=true{return Err("Starlight undo failed".into());}lighting_click(hwnd,520.,100.,scale);if query()["dirty"]!=false{return Err("Starlight redo failed".into());}
            lighting_click(hwnd,right+100.,290.,scale);let field=GetDlgItem(hwnd,2108);SendMessageW(field,WM_SETTEXT,0,wide("11").as_ptr() as isize);lighting_click(hwnd,right+100.,413.,scale);
            if query()["dirty"]!=false || query()["failed"]!=true{return Err("Invalid Starlight density was accepted".into());}
            SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);lighting_click(hwnd,right+20.,290.,scale);if query()["dirty"]!=false{return Err("Readonly Starlight changed".into());}SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);
            if !monitor.state().log.is_empty() {return Err("Local Wave authoring made device requests".into());}
            Ok(json!({"new_starlight_layer":true,"duration_ms":500,"density":7,"random_off":true,"gradient_none_saved":true,"save_undo_redo":true,"invalid_density_atomic":true,"readonly_inert":true,"unsupported_preview_refused":true,"sim_device_requests":0,"physical_discovery_disabled":true}))
        }})();
        unsafe {let hwnd=WINDOW.load(Ordering::Acquire) as HWND;SendMessageW(hwnd,STUDIO_TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}
        std::fs::write(folder.join("interaction.json"),serde_json::to_vec_pretty(&match &result {Ok(value)=>value.clone(),Err(error)=>json!({"error":error})}).unwrap()).unwrap();result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(30)));let result=controller.join().unwrap();
    assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");
    assert_eq!(std::fs::read(input).unwrap(),original);assert_eq!(std::fs::read(directory.join("profiles.json")).unwrap(),original_store);
    eprintln!("Starlight native evidence: {}",directory.display());
}


#[test]
#[ignore = "requires private stock export/assets; opens isolated simulated Wave authoring window"]
fn native_studio_wave_direction_presets_motion_preview_save_and_undo_preserve_source() {
    let input=PathBuf::from(std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select stock export"));
    let original=std::fs::read(&input).unwrap();
    let directory=std::env::temp_dir().join(format!("native-studio-wave-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&directory).unwrap();let frame=directory.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&directory),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("studio"))]);
    let sim=Sim::wireless_mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();
    let mut store=Store::load(&directory.join("profiles.json")).unwrap();store.for_device(&engine.devices()[0]);store.save().unwrap();
    let original_store=std::fs::read(directory.join("profiles.json")).unwrap();
    let folder=directory.clone();let monitor=sim.clone();let source=input.clone();
    let controller=std::thread::spawn(move || {
        let result=(|| -> Result<Value,String> {unsafe {
            wait_for_studio_window(||frame.exists(),"Wave window")?;
            let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut value as *mut Value as isize);value};
            wait_for(||query()["pending"]==0,"initial reads")?;
            monitor.state().log.clear();
            let path=json!(source.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,1,&path as *const Value as isize);
            wait_for(||query()["profiles"]==1 && query()["busy"]==false,"composition load")?;
            let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;
            let mut client:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut client);
            let width=f64::from(client.right)/scale;let height=f64::from(client.bottom)/scale;let right=width-250.;
            lighting_click(hwnd,775.,100.,scale);lighting_click(hwnd,800.,384.,scale);
            let id=query()["selected"].as_str().ok_or("new Wave not selected")?.to_owned();
            if !query()["rows"].as_array().unwrap().iter().any(|row|row["id"]==id && row["title"]=="WAVE") {return Err("Wave menu did not create the source-shaped layer".into());}
            let fit=((width-540.)/960.).min((height-250.)/360.).clamp(0.05,4.);
            lighting_click(hwnd,width/2.+(67.-480.)*fit,(118.+height)/2.+(76.-180.)*fit,scale);
            lighting_click(hwnd,right+211.,290.,scale); // Left270, assigns source defaults
            lighting_click(hwnd,right+210.,360.,scale); // source preset5: solid green
            for (id,text) in [(2105,"0"),(2106,"125"),(2107,"79")] {
                let field=GetDlgItem(hwnd,id);if field.is_null(){return Err("Wave motion field missing".into());}
                SendMessageW(field,WM_SETTEXT,0,wide(text).as_ptr() as isize);
            }
            lighting_click(hwnd,right+100.,500.,scale);
            lighting_click(hwnd,630.,100.,scale);
            wait_for(||query()["preview"]["current"]==true && query()["preview"]["effects"]==3,"Wave preview")?;
            wait_for(||query()["preview"]["frame"][1]==json!([0,255,0]),"frozen Wave preset colour")?;
            lighting_click(hwnd,right+100.,580.,scale); // custom gradient panel
            for (id,text) in [(2103,"0"),(2104,"112233")] {
                let field=GetDlgItem(hwnd,id);if field.is_null(){return Err("Wave gradient field missing".into());}
                SendMessageW(field,WM_SETTEXT,0,wide(text).as_ptr() as isize);
            }
            lighting_click(hwnd,right+70.,493.,scale);
            lighting_click(hwnd,right+170.,422.,scale); // next stop
            let field=GetDlgItem(hwnd,2104);SendMessageW(field,WM_SETTEXT,0,wide("112233").as_ptr() as isize);
            lighting_click(hwnd,right+70.,493.,scale);
            wait_for(||query()["preview"]["current"]==true && query()["preview"]["frame"][1]==json!([17,34,51]),"custom Wave preview")?;
            let field=GetDlgItem(hwnd,2103);SendMessageW(field,WM_SETTEXT,0,wide("101").as_ptr() as isize);
            lighting_click(hwnd,right+70.,493.,scale);
            if query()["failed"]!=true {return Err("Invalid Wave gradient position was accepted".into());}
            // Shared gradient workflow through actual native fields and mouse routing.
            let before_shared = query()["undo"].as_u64().ok_or("missing undo count")?;
            lighting_click(hwnd,right+120.,565.,scale); // RGB representation
            let field=GetDlgItem(hwnd,2104);
            SendMessageW(field,WM_SETTEXT,0,wide("255,128,0").as_ptr() as isize);
            lighting_click(hwnd,right+70.,493.,scale);
            if query()["undo"] != before_shared+1 {return Err("RGB entry did not make one edit".into());}
            lighting_click(hwnd,right+195.,565.,scale); // HSV representation
            let field=GetDlgItem(hwnd,2104);
            SendMessageW(field,WM_SETTEXT,0,wide("240,100,100").as_ptr() as isize);
            lighting_click(hwnd,right+70.,493.,scale);
            if query()["undo"] != before_shared+2 {return Err("HSV entry did not make one edit".into());}
            let field=GetDlgItem(hwnd,2104);
            SendMessageW(field,WM_SETTEXT,0,wide("NaN,0,0").as_ptr() as isize);
            lighting_click(hwnd,right+70.,493.,scale);
            if query()["failed"] != true || query()["undo"] != before_shared+2 {return Err("Invalid HSV changed history".into());}
            lighting_click(hwnd,right+40.,565.,scale); // HEX
            let at=|percent:f64|position(right+15.+percent*209./100.,399.,scale);
            SendMessageW(hwnd,WM_LBUTTONDOWN,1,at(100.));
            SendMessageW(hwnd,WM_MOUSEMOVE,1,at(25.));
            if query()["undo"] != before_shared+2 {return Err("Drag preview mutated document".into());}
            SendMessageW(hwnd,WM_LBUTTONUP,0,at(25.));
            if query()["undo"] != before_shared+3 {return Err("Completed drag did not create exactly one edit".into());}
            // Crossing the other endpoint keeps this same stop selected.
            SendMessageW(hwnd,WM_LBUTTONDOWN,1,at(25.));
            SendMessageW(hwnd,WM_MOUSEMOVE,1,at(75.));
            SendMessageW(hwnd,WM_CANCELMODE,0,0);
            if query()["undo"] != before_shared+3 {return Err("Cancelled drag changed document".into());}
            let shared_output=folder.join("shared-gradient.ChromaEffects");
            let path=json!(shared_output.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);
            wait_for(||query()["busy"]==false && query()["dirty"]==false,"shared gradient save")?;
            let bytes=std::fs::read(&shared_output).map_err(|e|e.to_string())?;
            let shared=secbird_core::studio::Archive::parse(&bytes).map_err(|e|e.to_string())?;
            let value:Value=serde_json::from_str(shared.profile_json(0).unwrap()).unwrap();
            let row=value["layers"].as_array().unwrap().iter().find(|r|r["id"]==id).ok_or("shared Wave missing")?;
            let stops=&row["regions"][0]["params"]["colorStops"];
            if stops[1]["Stop"]!=25 || stops[1]["Color"]!=0x0000ff {return Err(format!("HSV/drag saved wrong stop: {stops}"));}
            let capture=json!(folder.join("shared-gradient.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);
            SendMessageW(hwnd,WM_LBUTTONDOWN,1,at(25.));SendMessageW(hwnd,WM_LBUTTONUP,0,at(90.));
            if query()["undo"] != before_shared+3 {return Err("Read-only gradient drag changed history".into());}
            SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);
            for _ in 0..3 {lighting_click(hwnd,450.,100.,scale);}
            if query()["undo"] != before_shared {return Err("Shared gradient undo did not restore original".into());}
            // Restore the selected stop for the existing Wave motion/save checks.
            lighting_click(hwnd,right+170.,422.,scale);
            let gradient_capture=json!(folder.join("wave-gradient.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&gradient_capture as *const Value as isize);
            lighting_click(hwnd,right+100.,290.,scale); // back to motion
            let capture=json!(folder.join("wave.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            if std::env::var("SYNAPSE_LITE_STUDIO_VISUAL_HOLD").as_deref()==Ok("1") {std::thread::sleep(Duration::from_secs(20));}
            let output=folder.join("wave.ChromaEffects");let path=json!(output.to_string_lossy());
            SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);wait_for(||query()["busy"]==false && query()["dirty"]==false,"Wave save")?;
            let bytes=std::fs::read(output).map_err(|e|e.to_string())?;let archive=secbird_core::studio::Archive::parse(&bytes).map_err(|e|e.to_string())?;
            let saved:Value=serde_json::from_str(archive.profile_json(0).unwrap()).unwrap();
            let row=saved["layers"].as_array().unwrap().iter().find(|row|row["id"]==id).ok_or("saved Wave missing")?;
            let params=&row["regions"][0]["params"];
            if params["angle"]!=270 || params["speed"]!=0 || params["width"]!=125 || params["pause"]!=79 || params["cycles"]!=-1 || params["colorStops"][0]["Color"]!=0x112233 || params["colorStops"][1]["Color"]!=0x112233 || params["colorStopsCustom"]!=true {return Err(format!("Wave fields were not saved: {params}"));}
            lighting_click(hwnd,450.,100.,scale);if query()["dirty"]!=true {return Err("Wave speed undo did not restore the previous draft".into());}
            lighting_click(hwnd,520.,100.,scale);if query()["dirty"]!=false {return Err("Wave speed redo did not return to saved state".into());}
            let field=GetDlgItem(hwnd,2106);SendMessageW(field,WM_SETTEXT,0,wide("99").as_ptr() as isize);
            lighting_click(hwnd,right+100.,500.,scale);
            if query()["dirty"]!=false || query()["failed"]!=true {return Err("Invalid motion did not refuse atomically".into());}
            SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);lighting_click(hwnd,right+10.,290.,scale);
            if query()["dirty"]!=false {return Err("Read-only Wave direction mutated the saved draft".into());}
            SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);
            if !monitor.state().log.is_empty() {return Err("Local Wave authoring made device requests".into());}
            Ok(json!({"new_wave_layer":true,"source_direction":270,"source_preset":4,"motion":{"speed":0,"width":125,"pause_ms":79},"preview_rgb":[17,34,51],"custom_stops":true,"invalid_stop_refused":true,"save_undo_redo":true,"invalid_motion_atomic":true,"readonly_inert":true,"sim_device_requests":0,"physical_discovery_disabled":true}))
        }})();
        unsafe {let hwnd=WINDOW.load(Ordering::Acquire) as HWND;SendMessageW(hwnd,STUDIO_TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}
        std::fs::write(folder.join("interaction.json"),serde_json::to_vec_pretty(&match &result {Ok(value)=>value.clone(),Err(error)=>json!({"error":error})}).unwrap()).unwrap();result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(30)));let result=controller.join().unwrap();
    assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");
    assert_eq!(std::fs::read(input).unwrap(),original);assert_eq!(std::fs::read(directory.join("profiles.json")).unwrap(),original_store);
    eprintln!("Wave native evidence: {}",directory.display());
}


#[test]
#[ignore = "requires private stock export/assets; opens isolated simulated Breathing authoring window"]
fn native_studio_breathing_colors_speed_preview_save_and_undo_preserve_source() {
    let input=PathBuf::from(std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select stock export"));
    let original=std::fs::read(&input).unwrap();
    let directory=std::env::temp_dir().join(format!("native-studio-breathing-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&directory).unwrap();let frame=directory.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&directory),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("studio"))]);
    let sim=Sim::wireless_mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();
    let mut store=Store::load(&directory.join("profiles.json")).unwrap();store.for_device(&engine.devices()[0]);store.save().unwrap();
    let original_store=std::fs::read(directory.join("profiles.json")).unwrap();
    let folder=directory.clone();let monitor=sim.clone();let source=input.clone();
    let controller=std::thread::spawn(move || {
        let result=(|| -> Result<Value,String> {unsafe {
            wait_for_studio_window(||frame.exists(),"Breathing window")?;
            let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut value as *mut Value as isize);value};
            wait_for(||query()["pending"]==0,"initial reads")?;
            monitor.state().log.clear();
            let path=json!(source.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,1,&path as *const Value as isize);
            wait_for(||query()["profiles"]==1 && query()["busy"]==false,"composition load")?;
            let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;
            let mut client:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut client);
            let width=f64::from(client.right)/scale;let height=f64::from(client.bottom)/scale;let right=width-250.;
            lighting_click(hwnd,775.,100.,scale);lighting_click(hwnd,800.,356.,scale);
            let id=query()["selected"].as_str().ok_or("new Breathing not selected")?.to_owned();
            if !query()["rows"].as_array().unwrap().iter().any(|row|row["id"]==id && row["title"]=="BREATHING") {return Err("Breathing menu did not create the source-shaped layer".into());}
            let fit=((width-540.)/960.).min((height-250.)/360.).clamp(0.05,4.);
            lighting_click(hwnd,width/2.+(67.-480.)*fit,(118.+height)/2.+(76.-180.)*fit,scale);
            let first=GetDlgItem(hwnd,i32::from(studio_view::COLOR));let second=GetDlgItem(hwnd,i32::from(studio_view::COLD));
            if first.is_null() || second.is_null() {return Err("Breathing dual colour fields missing".into());}
            SendMessageW(first,WM_SETTEXT,0,wide("112233").as_ptr() as isize);lighting_click(hwnd,right+70.,333.,scale);
            let second=GetDlgItem(hwnd,i32::from(studio_view::COLD)); // applying colour1 rebuilds fields
            SendMessageW(second,WM_SETTEXT,0,wide("AABBCC").as_ptr() as isize);lighting_click(hwnd,right+70.,436.,scale);
            lighting_click(hwnd,right+200.,543.,scale); // source fast6000ms
            lighting_click(hwnd,630.,100.,scale);
            wait_for(||query()["preview"]["current"]==true && query()["preview"]["effects"]==3,"Breathing preview")?;
            wait_for(||query()["preview"]["frame"][1]==json!([17,34,51]),"first Breathing peak colour")?;
            let capture=json!(folder.join("breathing.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            let output=folder.join("breathing.ChromaEffects");let path=json!(output.to_string_lossy());
            SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);wait_for(||query()["busy"]==false && query()["dirty"]==false,"Breathing save")?;
            let bytes=std::fs::read(output).map_err(|e|e.to_string())?;let archive=secbird_core::studio::Archive::parse(&bytes).map_err(|e|e.to_string())?;
            let saved:Value=serde_json::from_str(archive.profile_json(0).unwrap()).unwrap();
            let row=saved["layers"].as_array().unwrap().iter().find(|row|row["id"]==id).ok_or("saved Breathing missing")?;
            let params=&row["regions"][0]["params"];
            if params["color"]!=0x112233 || params["color2"]!=0xaabbcc || params["duration"]!=6000 {return Err(format!("Breathing fields were not saved: {params}"));}
            lighting_click(hwnd,450.,100.,scale);if query()["dirty"]!=true {return Err("Breathing speed undo did not restore the previous draft".into());}
            lighting_click(hwnd,520.,100.,scale);if query()["dirty"]!=false {return Err("Breathing speed redo did not return to saved state".into());}
            lighting_click(hwnd,630.,100.,scale); // Stop the two-colour preview.
            lighting_click(hwnd,right+100.,234.,scale); // Open custom stop authoring.
            let position=GetDlgItem(hwnd,i32::from(studio_view::POSITION));let color=GetDlgItem(hwnd,i32::from(studio_view::STOP_COLOR));
            if position.is_null()||color.is_null(){return Err("Breathing gradient fields missing".into());}
            SendMessageW(position,WM_SETTEXT,0,wide("0").as_ptr() as isize);SendMessageW(color,WM_SETTEXT,0,wide("ABCDEF").as_ptr() as isize);lighting_click(hwnd,right+70.,493.,scale);
            // Keep the expected colour for0..12 percent, so native polling does
            // not have to catch a single40ms frame on a busy desktop.
            lighting_click(hwnd,right+175.,422.,scale);let color=GetDlgItem(hwnd,i32::from(studio_view::STOP_COLOR));SendMessageW(color,WM_SETTEXT,0,wide("ABCDEF").as_ptr() as isize);lighting_click(hwnd,right+70.,493.,scale);lighting_click(hwnd,right+50.,422.,scale);
            lighting_click(hwnd,630.,100.,scale);wait_for(||query()["preview"]["current"]==true&&query()["preview"]["frame"][1]==json!([171,205,239]),"custom Breathing first gradient pixel")?;
            lighting_click(hwnd,630.,100.,scale);
            SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);wait_for(||query()["pending"]==0&&query()["dirty"]==false,"custom Breathing save")?;
            let saved=secbird_core::studio::Archive::parse(&std::fs::read(&path.as_str().unwrap()).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            let saved:Value=serde_json::from_str(saved.profile_json(0).unwrap()).unwrap();let row=saved["layers"].as_array().unwrap().iter().find(|r|r["id"]==id).unwrap();let params=&row["regions"][0]["params"];
            if params["colorStops"][0]["Color"]!=0xabcdef||params["colorStops"][1]["Color"]!=0xabcdef||params["colorStops"].as_array().unwrap().len()!=9||params["duration"]!=6000||params["color"]!=0x112233||params["color2"]!=0xaabbcc{return Err(format!("Custom Breathing save mismatch: {params}"));}
            lighting_click(hwnd,450.,100.,scale);if query()["dirty"]!=true{return Err("Custom gradient Undo failed".into());}lighting_click(hwnd,520.,100.,scale);if query()["dirty"]!=false{return Err("Custom gradient Redo failed".into());}
            let position=GetDlgItem(hwnd,i32::from(studio_view::POSITION));SendMessageW(position,WM_SETTEXT,0,wide("101").as_ptr() as isize);let history=query()["undo"].clone();lighting_click(hwnd,right+70.,493.,scale);
            if query()["undo"]!=history||query()["dirty"]!=false{return Err("Invalid gradient stop changed the draft".into());}
            SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);lighting_click(hwnd,right+185.,493.,scale);if query()["dirty"]!=false{return Err("Read-only Breathing gradient changed".into());}SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);
            let position=GetDlgItem(hwnd,i32::from(studio_view::POSITION));SendMessageW(position,WM_SETTEXT,0,wide("0").as_ptr() as isize);lighting_click(hwnd,right+185.,493.,scale);
            SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);wait_for(||query()["pending"]==0&&query()["dirty"]==false,"Breathing None save")?;
            let saved=secbird_core::studio::Archive::parse(&std::fs::read(path.as_str().unwrap()).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;let saved:Value=serde_json::from_str(saved.profile_json(0).unwrap()).unwrap();let row=saved["layers"].as_array().unwrap().iter().find(|r|r["id"]==id).unwrap();
            if row["regions"][0]["params"]["colorStops"][0].get("Color").is_some(){return Err("Breathing None serialized a colour instead of omitting it".into());}
            let capture=json!(folder.join("breathing-gradient.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            if !monitor.state().log.is_empty() {return Err("Local Breathing authoring made device requests".into());}
            Ok(json!({"new_breathing_layer":true,"two_color_fields":true,"source_fast_ms":6000,"peak_rgb":[17,34,51],"custom_gradient_preview_rgb":[171,205,239],"custom_stops_saved":9,"none_omits_colour":true,"custom_stop_history":true,"invalid_stop_atomic":true,"read_only_inert":true,"save_undo_redo":true,"sim_device_requests":0,"physical_discovery_disabled":true}))
        }})();
        unsafe {let hwnd=WINDOW.load(Ordering::Acquire) as HWND;SendMessageW(hwnd,STUDIO_TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}
        std::fs::write(folder.join("interaction.json"),serde_json::to_vec_pretty(&match &result {Ok(value)=>value.clone(),Err(error)=>json!({"error":error})}).unwrap()).unwrap();result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(30)));let result=controller.join().unwrap();
    assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");
    assert_eq!(std::fs::read(input).unwrap(),original);assert_eq!(std::fs::read(directory.join("profiles.json")).unwrap(),original_store);
    eprintln!("Breathing native evidence: {}",directory.display());
}

#[test]
#[ignore = "requires private stock export and pinned asset; opens isolated simulated Studio playback window"]
fn native_studio_saved_apply_and_stop_use_shared_worker_without_changing_profiles() {
    let input=PathBuf::from(std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select stock export"));
    let original=std::fs::read(&input).unwrap();
    let directory=std::env::temp_dir().join(format!("native-studio-output-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&directory).unwrap();
    let frame=directory.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&directory),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("studio"))]);
    let (engine,spy)=Engine::with_studio_output_simulator(directory.join("profiles.json"));
    let key=engine.devices()[0].key();
    let mut profile=secbird_core::profile_compat::new_native_profile(605,"Default").unwrap();
    crate::host_uniform::Program::Spectrum.opt_in(&mut profile).unwrap();
    let mut store=Store::load(&directory.join("profiles.json")).unwrap();
    store.for_device(&engine.devices()[0]).profiles.insert("Default".into(),profile);
    store.save().unwrap();
    let store_before=std::fs::read(directory.join("profiles.json")).unwrap();
    let engine=Arc::new(Mutex::new(engine));
    let mut worker=crate::uniform_worker::Worker::start(engine.clone()).unwrap();
    let observed=engine.clone();
    let folder=directory.clone();
    let source=input.clone();
    let controller=std::thread::spawn(move || {
        let result=(|| -> Result<Value,String> { unsafe {
            wait_for_studio_window(||frame.exists(),"Studio output window")?;
            let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut value as *mut Value as isize);value};
            wait_for(||query()["pending"]==0 && query()["output_control_ready"]==true,"device control ready")?;
            let path=json!(source.to_string_lossy());
            SendMessageW(hwnd,STUDIO_TEST,1,&path as *const Value as isize);
            wait_for(||query()["profiles"]==1 && query()["busy"]==false,"saved composition load")?;
            redraw(hwnd);
            let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.0;
            let apply=position(914.0,100.0,scale);
            SendMessageW(hwnd,WM_LBUTTONDOWN,1,apply);
            SendMessageW(hwnd,WM_LBUTTONUP,0,apply);
            wait_for(||observed.lock().unwrap().studio_status()["accepted_frames"].as_u64().unwrap_or(0)>=3,"Studio frames accepted by shared worker")?;
            wait_for(||query()["output_busy"]==false,"apply response")?;
            PostMessageW(hwnd,WM_TIMER,CACHED_LIGHTING_TIMER,0);
            wait_for(||query()["device_output"]["accepted_frames"].as_u64().unwrap_or(0)>=1,"cached playback status")?;
            let screenshot=json!(folder.join("studio-playback.png").to_string_lossy());
            if SendMessageW(hwnd,STUDIO_TEST,6,&screenshot as *const Value as isize)!=1 {return Err("Owned playback render failed".into());}
            let before_stop=observed.lock().unwrap().studio_status();
            let stop=position(1048.0,100.0,scale);
            SendMessageW(hwnd,WM_LBUTTONDOWN,1,stop);
            SendMessageW(hwnd,WM_LBUTTONUP,0,stop);
            wait_for(||observed.lock().unwrap().studio_status()["state"]=="stopped","Studio stopped and drained")?;
            wait_for(||query()["output_busy"]==false,"stop response")?;
            wait_for(||observed.lock().unwrap().lighting_runtime_status()[&key]["uniform_host"]["accepted_frames"].as_u64().unwrap_or(0)>=1,"saved Quick resumes")?;
            if observed.lock().unwrap().studio_holds(&key) {return Err("Studio still holds Quick after stop".into());}
            // Verify that unsaved edits cannot be sent to hardware. Selecting a
            // root and duplicating it changes only the editor draft.
            let layer=position(100.0,184.0,scale);
            SendMessageW(hwnd,WM_LBUTTONDOWN,1,layer);
            SendMessageW(hwnd,WM_LBUTTONUP,0,layer);
            // The test action enters the same dispatcher as keyboard/menu use.
            SendMessageW(hwnd,STUDIO_TEST,7,0);
            if query()["dirty"]!=true || !query()["status"].as_str().unwrap_or("").contains("Save") {return Err(format!("Dirty apply was not visibly refused: {}",query()));}
            if observed.lock().unwrap().studio_status()["state"]!="stopped" {return Err("Unexpected apply after stopped output".into());}
            Ok(json!({"applied_snapshot":before_stop,"shared_worker_native_apply_and_stop":true,"saved_quick_resumed":true,"physical_device_output":false,"source_and_store_bytes_preserved":true}))
        }})();
        unsafe {let hwnd=WINDOW.load(Ordering::Acquire) as HWND;SendMessageW(hwnd,STUDIO_TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}
        std::fs::write(folder.join("interaction.json"),serde_json::to_vec_pretty(&match &result {Ok(value)=>value.clone(),Err(error)=>json!({"error":error})}).unwrap()).unwrap();
        result
    });
    let run=run_standalone(engine.clone(),Some(Duration::from_secs(30)));
    let result=controller.join().unwrap();
    assert!(worker.stop(Duration::from_secs(1)),"simulated worker must join");
    engine.lock().unwrap().set_uniform_worker(false,None,None);
    assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");
    assert_eq!(std::fs::read(input).unwrap(),original);
    assert_eq!(std::fs::read(directory.join("profiles.json")).unwrap(),store_before);
    assert_eq!(spy.state.lock().unwrap().mode,3);
    eprintln!("Studio output interaction evidence: {}",directory.display());
}

#[test]
#[ignore = "requires an explicitly selected private stock Ornata export; opens isolated simulated window"]
fn native_studio_actual_stock_export_loads_geometry_and_preserves_profiles() {
    let input = PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select private stock export"),
    );
    let original = std::fs::read(&input).unwrap();
    let source = secbird_core::studio::Archive::parse(&original).unwrap();
    assert_eq!(source.profile_count(), 1);
    let directory = std::env::temp_dir().join(format!(
        "native-stock-studio-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("studio")),
    ]);
    let sim = Sim::wireless_mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    store.for_device(&engine.devices()[0]);
    store.save().unwrap();
    let store_before = std::fs::read(directory.join("profiles.json")).unwrap();
    let folder = directory.clone();
    let controller_sim = sim.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for(
                    || {
                        std::fs::read(&frame)
                            .ok()
                            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                            .is_some_and(|v| v["presented"] == true)
                    },
                    "actual export window presented",
                )?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut value = Value::Null;
                    SendMessageW(hwnd, STUDIO_TEST, 2, &mut value as *mut Value as isize);
                    value
                };
                wait_for(|| query()["pending"] == 0, "initial simulated reads")?;
                controller_sim.state().log.clear();
                let path = json!(input.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 1, &path as *const Value as isize);
                wait_for(
                    || query()["profiles"] == 1 && query()["busy"] == false,
                    "actual export load",
                )?;
                let state = query();
                if state["dirty"] != false
                    || state["canvas_devices"] != 1
                    || state["canvas_selectable_leds"] != 109
                    || state["rows"].as_array().map(Vec::len) != Some(2)
                    || state["canvas_notes"]
                        .as_array()
                        .is_none_or(|n| !n.is_empty())
                {
                    return Err(format!("Actual export not fully admitted: {state}"));
                }
                redraw(hwnd);
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let width = f64::from(client.right) / scale;
                let height = f64::from(client.bottom) / scale;
                let fit = ((width - 540.0) / 960.0)
                    .min((height - 250.0) / 360.0)
                    .clamp(0.05, 4.0);
                lighting_click(
                    hwnd,
                    width / 2.0 + (67.0 - 480.0) * fit,
                    (118.0 + height) / 2.0 + (76.0 - 180.0) * fit,
                    scale,
                );
                if query()["selected_leds"] != 1 || query()["dirty"] != false {
                    return Err("Actual export Esc hit failed or changed the document".into());
                }
                let output = folder.join("actual-roundtrip.ChromaEffects");
                let path = json!(output.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(|| query()["busy"] == false, "actual export save")?;
                if query()["failed"] != false {
                    return Err(format!("Actual export save failed: {}", query()));
                }
                let bytes = std::fs::read(&output).map_err(|e| e.to_string())?;
                let saved =
                    secbird_core::studio::Archive::parse(&bytes).map_err(|e| e.to_string())?;
                if saved.profile_json(0) != source.profile_json(0) {
                    return Err("Actual raw profile changed during roundtrip".into());
                }
                lighting_click(hwnd, 630.0, 100.0, scale);
                wait_for(
                    || {
                        query()["preview"]["current"] == true
                            && query()["preview"]["frames"].as_u64().unwrap_or(0) >= 2
                    },
                    "actual composition preview ready",
                )?;
                if query()["preview"]["effects"] != 2
                    || query()["dirty"] != false
                    || query()["preview"]["bytes"].as_u64().unwrap_or(u64::MAX) >= 100000
                {
                    return Err("Local preview plan count, storage or read-only composition invariant failed".into());
                }
                let first_frame = query()["preview"]["frame"].clone();
                wait_for(
                    || query()["preview"]["frame"] != first_frame,
                    "local Fire animation changes",
                )?;
                let capture = json!(folder.join("actual-live-preview.png").to_string_lossy());
                if SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize) != 1 {
                    return Err("Local preview capture failed".into());
                }
                lighting_click(hwnd, 630.0, 100.0, scale);
                if query()["preview"]["requested"] != false || query()["preview"]["frames"] != 0 {
                    return Err("Local preview stop retained frame plan".into());
                }
                // Stopping/navigation during compilation must reject stale work;
                // there is never more than one retained pending compile.
                lighting_click(hwnd, 630.0, 100.0, scale);
                lighting_click(hwnd, 65.0, 20.0, scale);
                wait_for(
                    || {
                        query()["preview"]["requested"] == false
                            && query()["preview"]["pending"] == false
                    },
                    "hidden preview stops and drains compile",
                )?;
                SendMessageW(hwnd, STUDIO_TEST, 0, 0);
                if query()["preview"]["frames"] != 0 || query()["dirty"] != false {
                    return Err("Hidden preview resumed or changed composition".into());
                }
                // Exercise independent regional edits through real native edit
                // controls and production hit targets on the user's Fire layer.
                use secbird_core::studio::document::{
                    ColorProperty, Draft, LedSelection, SelectedColor,
                };
                let id = state["selected"].as_str().ok_or("No Fire layer selected")?;
                let baseline =
                    Draft::open(source.profile_json(0).unwrap()).map_err(|e| e.to_string())?;
                let device: Value = serde_json::from_str(source.profile_json(0).unwrap()).unwrap();
                let selection = [LedSelection {
                    device: device["devices"][0]["id"].as_str().unwrap().into(),
                    row: 0,
                    col: 1,
                }];
                let original_cold = baseline
                    .selection_color(id, &selection, ColorProperty::FireCold)
                    .map_err(|e| e.to_string())?;
                let right = width - 250.0;
                let hot = GetDlgItem(hwnd, i32::from(studio_view::COLOR));
                let cold = GetDlgItem(hwnd, i32::from(studio_view::COLD));
                if hot.is_null() || cold.is_null() {
                    return Err("Fire colour fields missing".into());
                }
                SendMessageW(hot, WM_SETTEXT, 0, wide("123456").as_ptr() as isize);
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                lighting_click(hwnd, right + 60.0, 333.0, scale);
                if query()["dirty"] != false {
                    return Err("Read-only Fire edit changed document".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                lighting_click(hwnd, 630.0, 100.0, scale);
                wait_for(
                    || query()["preview"]["current"] == true,
                    "preview before edit",
                )?;
                let prior_generation = query()["preview"]["generation"].as_u64().unwrap();
                lighting_click(hwnd, right + 60.0, 333.0, scale);
                if query()["dirty"] != true || query()["failed"] != false {
                    return Err("Fire Hot edit failed".into());
                }
                wait_for(
                    || {
                        query()["preview"]["generation"].as_u64().unwrap_or(0) > prior_generation
                            && query()["preview"]["current"] == true
                    },
                    "preview recompiles current draft",
                )?;
                lighting_click(hwnd, 630.0, 100.0, scale);
                let edited_path = folder.join("actual-fire-edited.ChromaEffects");
                let save_path = json!(edited_path.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 3, &save_path as *const Value as isize);
                wait_for(|| query()["busy"] == false, "Fire edited save")?;
                let edited = secbird_core::studio::Archive::parse(
                    &std::fs::read(&edited_path).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                let edited =
                    Draft::open(edited.profile_json(0).unwrap()).map_err(|e| e.to_string())?;
                if edited
                    .selection_color(id, &selection, ColorProperty::FireHot)
                    .map_err(|e| e.to_string())?
                    != SelectedColor::Uniform(Some(0x123456))
                    || edited
                        .selection_color(id, &selection, ColorProperty::FireCold)
                        .map_err(|e| e.to_string())?
                        != original_cold
                {
                    return Err("Fire saved Hot/Cold independence failed".into());
                }
                // Cold-only change, explicit None and undo/redo must leave Hot.
                let cold = GetDlgItem(hwnd, i32::from(studio_view::COLD));
                SendMessageW(cold, WM_SETTEXT, 0, wide("654321").as_ptr() as isize);
                lighting_click(hwnd, right + 60.0, 436.0, scale);
                lighting_click(hwnd, right + 180.0, 436.0, scale);
                lighting_click(hwnd, 440.0, 100.0, scale);
                SendMessageW(hwnd, STUDIO_TEST, 3, &save_path as *const Value as isize);
                wait_for(|| query()["busy"] == false, "Fire Cold save")?;
                let edited = secbird_core::studio::Archive::parse(
                    &std::fs::read(&edited_path).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                let edited =
                    Draft::open(edited.profile_json(0).unwrap()).map_err(|e| e.to_string())?;
                if edited
                    .selection_color(id, &selection, ColorProperty::FireHot)
                    .map_err(|e| e.to_string())?
                    != SelectedColor::Uniform(Some(0x123456))
                    || edited
                        .selection_color(id, &selection, ColorProperty::FireCold)
                        .map_err(|e| e.to_string())?
                        != SelectedColor::Uniform(Some(0x654321))
                {
                    return Err("Fire Cold edit or None undo failed".into());
                }
                lighting_click(hwnd, 515.0, 100.0, scale);
                if query()["dirty"] != true {
                    return Err("Fire None redo lost dirty state".into());
                }
                lighting_click(hwnd, 440.0, 100.0, scale);
                if query()["dirty"] != false {
                    return Err("Fire None undo lost saved checkpoint".into());
                }
                let spectrum_id = device["layers"][1]["id"].as_str().unwrap();
                let original_gradient = baseline
                    .spectrum_selection_gradient(spectrum_id, &selection)
                    .map_err(|e| e.to_string())?;
                lighting_click(hwnd, 60.0, 228.0, scale);
                if query()["selected_leds"] != 0 {
                    return Err("Spectrum switch retained Fire selection".into());
                }
                lighting_click(
                    hwnd,
                    width / 2.0 + (67.0 - 480.0) * fit,
                    (118.0 + height) / 2.0 + (76.0 - 180.0) * fit,
                    scale,
                );
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                lighting_click(hwnd, right + 195.0, 291.0, scale);
                if query()["dirty"] != false {
                    return Err("Read-only Spectrum duration mutated profile".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                lighting_click(hwnd, right + 195.0, 291.0, scale);
                let save_spectrum = || -> Result<Draft, String> {
                    SendMessageW(hwnd, STUDIO_TEST, 3, &save_path as *const Value as isize);
                    wait_for(|| query()["busy"] == false, "Spectrum save")?;
                    if query()["failed"] != false {
                        return Err("Spectrum save failed".into());
                    }
                    let bytes = std::fs::read(&edited_path).map_err(|e| e.to_string())?;
                    let archive =
                        secbird_core::studio::Archive::parse(&bytes).map_err(|e| e.to_string())?;
                    Draft::open(archive.profile_json(0).unwrap()).map_err(|e| e.to_string())
                };
                use secbird_core::studio::document::{
                    GradientStop, SelectedDuration, SelectedGradient,
                };
                let saved_spectrum = save_spectrum()?;
                if saved_spectrum
                    .spectrum_selection_duration(spectrum_id, &selection)
                    .map_err(|e| e.to_string())?
                    != SelectedDuration::Uniform(18870)
                    || saved_spectrum
                        .spectrum_selection_gradient(spectrum_id, &selection)
                        .map_err(|e| e.to_string())?
                        != original_gradient
                {
                    return Err(
                        "Duration-only Spectrum edit changed gradient or stored wrong milliseconds"
                            .into(),
                    );
                }
                // Fifth source preset has three stops. Edit a real native field,
                // clear its colour, then exercise add/delete and exact history.
                lighting_click(hwnd, right + 210.0, 360.0, scale);
                let position = GetDlgItem(hwnd, i32::from(studio_view::POSITION));
                let color = GetDlgItem(hwnd, i32::from(studio_view::STOP_COLOR));
                if position.is_null() || color.is_null() {
                    return Err("Spectrum stop fields missing".into());
                }
                SendMessageW(position, WM_SETTEXT, 0, wide("101").as_ptr() as isize);
                lighting_click(hwnd, right + 180.0, 493.0, scale);
                if query()["failed"] != true {
                    return Err("Invalid gradient percentage accepted".into());
                }
                let position = GetDlgItem(hwnd, i32::from(studio_view::POSITION));
                SendMessageW(position, WM_SETTEXT, 0, wide("25").as_ptr() as isize);
                lighting_click(hwnd, right + 180.0, 493.0, scale);
                lighting_click(hwnd, right + 60.0, 529.0, scale);
                lighting_click(hwnd, right + 180.0, 529.0, scale);
                lighting_click(hwnd, 440.0, 100.0, scale);
                lighting_click(hwnd, 440.0, 100.0, scale);
                let saved_spectrum = save_spectrum()?;
                let expected = SelectedGradient::Uniform(Some(vec![
                    GradientStop {
                        position: 25,
                        color: None,
                    },
                    GradientStop {
                        position: 50,
                        color: Some(0xefff00),
                    },
                    GradientStop {
                        position: 100,
                        color: Some(0xff0000),
                    },
                ]));
                if saved_spectrum
                    .spectrum_selection_gradient(spectrum_id, &selection)
                    .map_err(|e| e.to_string())?
                    != expected
                    || saved_spectrum
                        .spectrum_selection_duration(spectrum_id, &selection)
                        .map_err(|e| e.to_string())?
                        != SelectedDuration::Uniform(18870)
                {
                    return Err("Spectrum preset/None/add/delete/history did not preserve expected gradient and duration".into());
                }
                let saved_json: Value = serde_json::from_str(saved_spectrum.json()).unwrap();
                let assigned = saved_json["layers"][1]["regions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|region| {
                        region["devices"][0]["leds"].as_array().is_some_and(|leds| {
                            leds.iter().any(|led| led["row"] == 0 && led["col"] == 1)
                        })
                    })
                    .ok_or("Edited Spectrum region absent")?;
                if assigned["params"]["colorStopsCustom"] != true
                    || assigned["params"]["colorStops"][0].get("Color").is_some()
                {
                    return Err("Spectrum custom flag or omitted None encoding incorrect".into());
                }
                if std::fs::read(&input).unwrap() != original
                    || std::fs::read(folder.join("profiles.json")).unwrap() != store_before
                {
                    return Err("Source export or device-profile store changed".into());
                }
                if controller_sim
                    .state()
                    .log
                    .iter()
                    .any(|command| command.id & 0x80 == 0)
                {
                    return Err("Actual file operation submitted device SETs".into());
                }
                let capture = json!(folder.join("actual-studio-frame.png").to_string_lossy());
                if SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize) != 1 {
                    return Err("Actual export paint capture failed".into());
                }
                Ok(
                    json!({"actual_export":true,"simulated_transport_only":true,"profiles":1,"layers":2,"canvas_devices":1,"selectable_leds":109,"source_esc_hit":true,"raw_profile_roundtrip_exact":true,"whole_file_byte_identity_observed":bytes==original,"local_actual_composition_animates":true,"local_preview_does_not_dirty":true,"local_preview_stops_and_drains_when_hidden":true,"local_preview_recompiles_current_snapshot":true,"fire_native_hot_cold_independent_saved":true,"fire_none_undo_redo":true,"fire_read_only_guard":true,"spectrum_duration_preserves_gradient":true,"spectrum_preset_stop_none_save":true,"spectrum_add_delete_undo":true,"spectrum_invalid_percentage_refused":true,"spectrum_read_only_guard":true,"layer_switch_clears_selection":true,"source_file_unchanged":true,"device_profiles_unchanged":true,"no_device_sets":true}),
                )
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            if !hwnd.is_null() {
                SendMessageW(hwnd, STUDIO_TEST, 5, 0);
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
        result
    });
    run_window(
        Arc::new(Mutex::new(engine)),
        Some(Duration::from_secs(20)),
        None,
    )
    .unwrap();
    let evidence = controller.join().unwrap().unwrap();
    std::fs::write(
        directory.join("actual-interaction.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    eprintln!(
        "Actual stock Studio interaction evidence: {}",
        directory.display()
    );
}

#[test]
#[ignore = "opens a bounded native Studio window; run alone"]
fn native_studio_edits_save_failure_and_exit_recovery() {
    // Rendering source assets can take longer on a loaded development desktop.
    fn wait_for(mut predicate: impl FnMut() -> bool, reason: &str) -> Result<(), String> {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(20) {
            if predicate() { return Ok(()); }
            std::thread::sleep(Duration::from_millis(20));
        }
        Err(format!("Timed out: {reason}"))
    }
    let directory = std::env::temp_dir().join(format!(
        "native-studio-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("studio")),
    ]);
    let sim = Sim::wireless_mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    store.for_device(&engine.devices()[0]);
    store.save().unwrap();
    let original_store = std::fs::read(directory.join("profiles.json")).unwrap();
    let profile = r#"{"id":"p0","title":"Desk","devices":[{"id":"keyboard","productId":605,"editionId":0,"layout":6,"x":4950,"y":4980,"width":960,"height":360,"orientation":0,"leds":[{"row":0,"col":1},{"row":0,"col":4}]}],"unknown":9007199254740993123456789,"layers":[{"id":"g","type":"group","title":"Group","visible":true,"opened":true,"layers":[{"id":"a","groupId":"g","type":"layer","title":"STATIC","name":"static","visible":false,"regions":[],"future":1.2300e-7}]}]}"#;
    let other = r#"{"id":"p1","title":"Other","devices":[],"layers":[{"id":"b","type":"layer","title":"SECOND","name":"static","visible":true,"regions":[]}]}"#;
    let source = directory.join("input.ChromaEffects");
    let output = directory.join("saved.ChromaEffects");
    std::fs::write(
        &source,
        format!("{{\"version\":1,\"profiles\":[{profile},{other}]}}"),
    )
    .unwrap();
    let controller_sim = sim.clone();
    let folder = directory.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for(
                    || {
                        std::fs::read(&frame)
                            .ok()
                            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                            .is_some_and(|v| v["presented"] == true)
                    },
                    "Studio presented",
                )?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut value = Value::Null;
                    SendMessageW(hwnd, STUDIO_TEST, 2, &mut value as *mut Value as isize);
                    value
                };
                let file = |kind, path: &Path| {
                    let value = json!(path.to_string_lossy());
                    SendMessageW(hwnd, STUDIO_TEST, kind, &value as *const Value as isize);
                };
                wait_for(|| query()["pending"] == 0, "initial device reads")?;
                controller_sim.state().log.clear();
                file(1, &source);
                wait_for(
                    || query()["profiles"] == 2 && query()["busy"] == false,
                    "composition load",
                )?;
                redraw(hwnd);
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let right = f64::from(client.right) / scale - 250.0;
                let click = |x, y| lighting_click(hwnd, x, y, scale);
                let check = |ok: bool, message: &str| {
                    if ok {
                        Ok(())
                    } else {
                        Err(format!("{message}: {}", query()))
                    }
                };
                check(
                    query()["canvas_notes"].as_array().unwrap().is_empty(),
                    "source geometry unavailable",
                )?;
                let width = f64::from(client.right) / scale;
                let height = f64::from(client.bottom) / scale;
                let fit = ((width - 540.0) / 960.0)
                    .min((height - 250.0) / 360.0)
                    .clamp(0.05, 4.0);
                // Independent coordinates from the pinned UK SVG: Esc center67,76;
                // F1 center145,76. The imported leds inventory admits exactly those two.
                let screen = |x: f64, y: f64| {
                    position(
                        width / 2.0 + (x - 480.0) * fit,
                        (118.0 + height) / 2.0 + (y - 180.0) * fit,
                        scale,
                    )
                };
                let esc = screen(67.0, 76.0);
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, esc);
                SendMessageW(hwnd, WM_LBUTTONUP, 0, esc);
                check(
                    query()["selected_leds"] == 1 && query()["dirty"] == false,
                    "LED hit selection changed document or missed",
                )?;
                let mut navigation = Value::Null;
                SendMessageW(hwnd, STUDIO_TEST, studio_view::TEST_NAVIGATION, &mut navigation as *mut Value as isize);
                check(navigation["ok"] == true, &format!("viewport gestures: {navigation}"))?;
                let zoom_field = GetDlgItem(hwnd, studio_view::ZOOM as i32);
                check(!zoom_field.is_null(), "numeric zoom field missing")?;
                SendMessageW(zoom_field, WM_SETTEXT, 0, wide("175%").as_ptr() as isize);
                click(right - 55.0, height - 25.0);
                check(query()["viewport"]["zoom"] == 175.0 && query()["dirty"] == false && query()["selected_leds"] == 1, "numeric zoom failed or changed selection/document")?;
                click(300.0, height - 25.0);
                check(query()["viewport"]["names"] == true, "device-name toggle failed")?;
                click(380.0, height - 25.0);
                check((query()["viewport"]["zoom"].as_f64().unwrap() - fit * 100.0).abs() < 1e-7, "fit selected failed")?;
                click(right - 100.0, 148.0);
                let capture = json!(folder.join("viewport-navigation.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                let from = screen(45.0, 54.0);
                let to = screen(165.0, 98.0);
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, from);
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, to);
                SendMessageW(hwnd, WM_CANCELMODE, 0, 0);
                check(
                    query()["selected_leds"] == 1,
                    "canceled marquee lost prior selection",
                )?;
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, from);
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, to);
                SendMessageW(hwnd, WM_LBUTTONUP, 0, to);
                check(
                    query()["selected_leds"] == 2 && query()["dirty"] == false,
                    "marquee did not select admitted LED paths",
                )?;
                click(right - 40.0, 148.0);
                check(
                    query()["selected_leds"] == 2 && query()["dirty"] == false,
                    "zoom changed selection or draft",
                )?;
                click(right - 100.0, 148.0);
                redraw(hwnd);
                click(215.0, 188.0);
                check(
                    query()["rows"][0]["visible"] == false
                        && query()["rows"][1]["visible"] == false,
                    "group visibility did not cascade",
                )?;
                click(440.0, 100.0);
                check(
                    query()["dirty"] == false && query()["rows"][0]["visible"] == true,
                    "undo did not restore clean baseline",
                )?;
                click(515.0, 100.0);
                check(query()["dirty"] == true, "redo did not restore edit")?;
                click(440.0, 100.0);
                click(60.0, 228.0);
                check(query()["selected"] == "a", "child selection failed")?;
                check(
                    query()["selected_leds"] == 0,
                    "layer switch kept prior LED selection",
                )?;
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, from);
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, to);
                SendMessageW(hwnd, WM_LBUTTONUP, 0, to);
                let color_edit = || GetDlgItem(hwnd, i32::from(studio_view::COLOR));
                if color_edit().is_null() {
                    return Err("Static RGB field missing".into());
                }
                SendMessageW(
                    color_edit(),
                    WM_SETTEXT,
                    0,
                    wide("GG0000").as_ptr() as isize,
                );
                click(right + 100.0, 343.0);
                check(
                    query()["failed"] == true && query()["dirty"] == false,
                    "invalid RGB mutated composition",
                )?;
                SendMessageW(
                    color_edit(),
                    WM_SETTEXT,
                    0,
                    wide("FF0000").as_ptr() as isize,
                );
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                click(right + 100.0, 343.0);
                check(
                    query()["dirty"] == false,
                    "read-only Static edit changed composition",
                )?;
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                click(right + 100.0, 343.0);
                check(
                    query()["dirty"] == true && query()["color"] == "FF0000",
                    "native RGB did not update selected memberships",
                )?;
                click(440.0, 100.0);
                check(
                    query()["dirty"] == false,
                    "region-only undo did not return clean",
                )?;
                click(515.0, 100.0);
                check(
                    query()["dirty"] == true && query()["color"] == "FF0000",
                    "region-only redo failed",
                )?;
                click(right + 100.0, 381.0);
                check(
                    query()["dirty"] == false,
                    "erase did not return to unassigned baseline",
                )?;
                click(440.0, 100.0);
                check(
                    query()["dirty"] == true && query()["color"] == "FF0000",
                    "erase undo failed",
                )?;
                let rename = |name: &str| -> Result<(), String> {
                    click(right + 60.0, 193.0);
                    let edit = GetDlgItem(hwnd, i32::from(studio_view::NAME));
                    if edit.is_null() {
                        return Err("Rename edit missing".into());
                    }
                    SendMessageW(edit, WM_SETTEXT, 0, wide(name).as_ptr() as isize);
                    click(right + 60.0, 193.0);
                    check(query()["renaming"] == false, "rename did not commit")
                };
                rename("Desk lighting")?;
                check(
                    query()["rows"][1]["title"] == "Desk lighting",
                    "rename lost native edit text",
                )?;
                file(3, &output);
                wait_for(|| query()["busy"] == false, "first save")?;
                check(
                    query()["dirty"] == false && query()["failed"] == false,
                    "save not acknowledged",
                )?;
                let bytes = std::fs::read(&output).map_err(|e| e.to_string())?;
                let capture = json!(folder.join("studio-frame.png").to_string_lossy());
                check(
                    SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize) == 1,
                    "native frame capture failed",
                )?;
                let archive =
                    secbird_core::studio::Archive::parse(&bytes).map_err(|e| e.to_string())?;
                let saved_draft =
                    secbird_core::studio::document::Draft::open(archive.profile_json(0).unwrap())
                        .map_err(|e| e.to_string())?;
                let selected = [1, 4].map(|col| secbird_core::studio::document::LedSelection {
                    device: "keyboard".into(),
                    row: 0,
                    col,
                });
                check(
                    saved_draft
                        .static_selection_color("a", &selected)
                        .map_err(|e| e.to_string())?
                        == secbird_core::studio::document::SelectedColor::Uniform(Some(0xff0000)),
                    "saved RGB assignments lost",
                )?;
                check(
                    archive
                        .profile_json(0)
                        .unwrap()
                        .contains("9007199254740993123456789")
                        && archive.profile_json(0).unwrap().contains("1.2300e-7")
                        && archive.profile_json(1) == Some(other),
                    "unknown data or other composition changed",
                )?;
                click(70.0, 100.0);
                click(70.0, 158.0);
                check(query()["index"] == 1, "composition menu did not switch")?;
                rename("Second edited")?;
                file(3, &output);
                wait_for(|| query()["busy"] == false, "second save")?;
                let saved = std::fs::read(&output).map_err(|e| e.to_string())?;
                let archive =
                    secbird_core::studio::Archive::parse(&saved).map_err(|e| e.to_string())?;
                check(
                    archive.profile_json(0).unwrap().contains("Desk lighting")
                        && archive.profile_json(1).unwrap().contains("Second edited"),
                    "later save lost earlier composition edits",
                )?;
                rename("Unsaved work")?;
                file(3, &folder.join("missing").join("cannot-save.ChromaEffects"));
                wait_for(|| query()["busy"] == false, "failed save")?;
                check(
                    query()["dirty"] == true && query()["failed"] == true,
                    "failed save discarded draft",
                )?;
                check(
                    std::fs::read(&output).unwrap() == saved,
                    "failed save damaged prior file",
                )?;
                // The existing Home tab may be used while an unsaved draft remains open.
                click(65.0, 20.0);
                check(query()["open"] == false, "Home did not hide Studio")?;
                SendMessageW(hwnd, STUDIO_TEST, 0, 0);
                check(
                    query()["dirty"] == true && query()["rows"][0]["title"] == "Unsaved work",
                    "navigation lost Studio draft",
                )?;
                SendMessageW(hwnd, STUDIO_TEST, 4, IDCANCEL as isize);
                SendMessageW(hwnd, super::super::linked_games::TEST, 1, 0);
                std::thread::sleep(Duration::from_millis(100));
                check(query()["open"] == true && query()["dirty"] == true,
                    "cancel Linked Games navigation lost Studio draft")?;
                // An invalid in-progress rename must be recoverable even if hidden.
                click(right + 60.0, 193.0);
                let edit = GetDlgItem(hwnd, i32::from(studio_view::NAME));
                SendMessageW(edit, WM_SETTEXT, 0, wide("").as_ptr() as isize);
                click(65.0, 20.0);
                SendMessageW(hwnd, WM_CLOSE, 0, 0);
                check(
                    IsWindow(hwnd) != 0
                        && query()["open"] == true
                        && query()["renaming"] == true
                        && query()["failed"] == true,
                    "invalid hidden rename was not revealed on close",
                )?;
                click(right + 60.0, 233.0);
                check(
                    query()["renaming"] == false && query()["rows"][0]["title"] == "Unsaved work",
                    "cancel rename lost previously committed draft",
                )?;
                // Exercise the real tray-exit guard against our own message-only sentinel.
                let tray = CreateWindowExW(
                    0,
                    wide("STATIC").as_ptr(),
                    wide("Studio test tray").as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    null_mut(),
                    GetModuleHandleW(null()),
                    null(),
                );
                if tray.is_null() {
                    return Err("Tray sentinel creation failed".into());
                }
                SendMessageW(hwnd, STUDIO_TEST, 4, IDCANCEL as isize);
                check(defer_process_exit(tray), "tray exit did not defer")?;
                wait_for(|| query()["dialog"] == false, "cancel close")?;
                // A round trip to the UI thread follows the posted close/prompt messages.
                std::thread::sleep(Duration::from_millis(100));
                check(
                    IsWindow(hwnd) != 0 && query()["dirty"] == true,
                    "cancel close lost window or draft",
                )?;
                let mut message: MSG = std::mem::zeroed();
                if PeekMessageW(&mut message, tray, WM_CLOSE, WM_CLOSE, PM_REMOVE) != 0 {
                    DestroyWindow(tray);
                    return Err("Canceled close acknowledged process exit".into());
                }
                SendMessageW(hwnd, STUDIO_TEST, 4, IDNO as isize);
                check(defer_process_exit(tray), "discard exit did not defer")?;
                wait_for(
                    || PeekMessageW(&mut message, tray, WM_CLOSE, WM_CLOSE, PM_REMOVE) != 0,
                    "exit acknowledgement",
                )?;
                let ack = message.wParam == 1 && WINDOW.load(Ordering::Acquire) == 0;
                DestroyWindow(tray);
                if !ack {
                    return Err("Exit acknowledged before editor destruction".into());
                }
                if std::fs::read(folder.join("profiles.json")).unwrap() != original_store {
                    return Err("Studio file work changed device profiles".into());
                }
                if controller_sim
                    .state()
                    .log
                    .iter()
                    .any(|command| command.id & 0x80 == 0)
                {
                    return Err("Studio file work wrote device commands".into());
                }
                Ok(
                    json!({"actual_hwnd":true,"simulated_only":true,"group_visibility_undo_redo":true,
                "native_rename":true,"two_composition_saves_preserved":true,"unknown_literals_preserved":true,
                "save_failure_keeps_draft_and_file":true,"navigation_keeps_draft":true,
                "invalid_hidden_rename_recovery":true,
                "source_led_hit_marquee_cancel_zoom":true,
                "static_region_native_rgb_erase_history_save":true,"invalid_and_readonly_rgb_inert":true,
                "tray_exit_cancel_and_discard":true,"device_profiles_unchanged":true,"no_device_sets":true}),
                )
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            if !hwnd.is_null() {
                SendMessageW(hwnd, STUDIO_TEST, 5, 0);
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
        result
    });
    run_window(
        Arc::new(Mutex::new(engine)),
        Some(Duration::from_secs(60)),
        None,
    )
    .unwrap();
    let evidence = controller.join().unwrap().unwrap();
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    eprintln!(
        "Studio native interaction evidence: {}",
        directory.display()
    );
}

#[test]
#[ignore = "opens isolated native Studio with explicitly selected private stock export"]
fn native_studio_layer_authoring_preview_undo_and_save_preserve_source() {
    let input = PathBuf::from(std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select private stock export"));
    let original = std::fs::read(&input).unwrap();
    let source = secbird_core::studio::Archive::parse(&original).unwrap();
    let baseline = secbird_core::studio::document::Draft::open(source.profile_json(0).unwrap()).unwrap();
    let original_count = baseline.layers().len();
    let directory = std::env::temp_dir().join(format!("native-studio-layers-{}-{}",std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&directory),
        ("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("studio"))]);
    let sim = Sim::wireless_mouse(); let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    store.for_device(&engine.devices()[0]); store.save().unwrap();
    let store_before = std::fs::read(directory.join("profiles.json")).unwrap();
    let folder = directory.clone(); let observer = sim.clone(); let export = input.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value,String> { unsafe {
            wait_for_studio_window(||frame.exists(),"Studio layer window")?;
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            let query = || {let mut value=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut value as *mut Value as isize);value};
            wait_for(||query()["pending"]==0,"initial simulated queries")?;
            let path=json!(export.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,1,&path as *const Value as isize);
            wait_for(||query()["busy"]==false && query()["profiles"]==1,"private export loaded")?;
            observer.state().log.clear();
            let scale=GetDpiForWindow(hwnd) as f64/96.;let mut rect:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut rect);
            let width=f64::from(rect.right)/scale;let height=f64::from(rect.bottom)/scale;
            let right=width-250.;
            let menu = |index:usize| {lighting_click(hwnd,770.,100.,scale);lighting_click(hwnd,770.,131.+index as f64*28.,scale);};
            // Read-only routing must block adding, including opening its menu.
            SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);menu(0);
            if query()["rows"].as_array().unwrap().len()!=original_count || query()["dirty"]!=false {
                return Err("Read-only Studio allowed structural editing".into());
            }
            SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);
            menu(0);
            let created=query();let id=created["selected"].as_str().ok_or("new layer not selected")?.to_string();
            if id.len()!=12 || created["rows"].as_array().unwrap().len()!=original_count+1 || created["selected_leds"]!=0 {
                return Err(format!("New empty layer did not follow source click-add: {created}"));
            }
            let fit=((width-540.)/960.).min((height-250.)/360.).clamp(0.05,4.);
            lighting_click(hwnd,width/2.+(67.-480.)*fit,(118.+height)/2.+(76.-180.)*fit,scale);
            let field=GetDlgItem(hwnd,i32::from(studio_view::COLOR));if field.is_null(){return Err("New Static properties missing".into());}
            SendMessageW(field,WM_SETTEXT,0,wide("123456").as_ptr() as isize);
            lighting_click(hwnd,right+80.,343.,scale);
            lighting_click(hwnd,630.,100.,scale);
            wait_for(||query()["preview"]["current"]==true,"new layer preview")?;
            if query()["preview"]["effects"]!=3 || query()["preview"]["frame"][1]!=json!([18,52,86]) {
                return Err(format!("New Static layer did not composite above existing effects: {}",query()));
            }
            let capture=json!(folder.join("new-static-layer.png").to_string_lossy());
            if SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize)!=1{return Err("Owned layer render failed".into());}
            menu(4); // duplicate selected Static
            if query()["rows"].as_array().unwrap().len()!=original_count+2 || query()["selected_leds"]!=0 {
                return Err("Duplicate failed or retained the other layer's LED selection".into());
            }
            menu(6); // move duplicate down
            menu(7); // delete duplicate, leaving original Static
            menu(3); // empty new group, retaining current effect
            if query()["rows"].as_array().unwrap().len()!=original_count+2 {
                return Err("Delete/group count did not preserve remaining layers".into());
            }
            for _ in 0..6 {lighting_click(hwnd,440.,100.,scale);}
            if query()["dirty"]!=false || query()["rows"].as_array().unwrap().len()!=original_count {
                return Err(format!("Structural/region undo did not return to saved baseline: {}",query()));
            }
            for _ in 0..6 {lighting_click(hwnd,515.,100.,scale);}
            let output=folder.join("authored.ChromaEffects");let path=json!(output.to_string_lossy());
            SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);
            wait_for(||query()["busy"]==false,"authored file saved")?;
            if query()["failed"]!=false || query()["dirty"]!=false {return Err(format!("Authoring save failed: {}",query()));}
            let bytes=std::fs::read(&output).map_err(|error|error.to_string())?;
            let archive=secbird_core::studio::Archive::parse(&bytes).map_err(|error|error.to_string())?;
            let draft=secbird_core::studio::document::Draft::open(archive.profile_json(0).unwrap()).map_err(|error|error.to_string())?;
            if draft.layers().len()!=original_count+2 || !draft.layers().iter().any(|row|row.id==id) {
                return Err("Saved file omitted authored layers".into());
            }
            if !observer.state().log.is_empty(){return Err("Local layer editing made device calls".into());}
            Ok(json!({"new_layer_id_shape":"12 lowercase hex","new_empty_root_layer":true,
                "static_region_preview_rgb":[18,52,86],"duplicate_move_delete_group":true,
                "mixed_structural_region_undo_redo":true,"compatible_local_save":true,
                "read_only_blocks_structure":true,"sim_device_calls_added":0,"physical_discovery_disabled":true,
                "profile_bytes_unchanged":true,"original_export_unchanged":true}))
        }})();
        unsafe {SendMessageW(WINDOW.load(Ordering::Acquire) as HWND,STUDIO_TEST,5,0);PostMessageW(WINDOW.load(Ordering::Acquire) as HWND,WM_CLOSE,0,0);}
        std::fs::write(folder.join("interaction.json"),serde_json::to_vec_pretty(&match &result{Ok(value)=>value.clone(),Err(error)=>json!({"error":error})}).unwrap()).unwrap();
        result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(30)));
    let result=controller.join().unwrap();assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");
    assert_eq!(std::fs::read(&input).unwrap(),original);assert_eq!(std::fs::read(directory.join("profiles.json")).unwrap(),store_before);
    eprintln!("Studio authoring evidence: {}",directory.display());
}

#[test]
#[ignore = "opens isolated simulated Studio window; run alone"]
fn native_studio_organization_drag_groups_history_and_persistence() {
    let directory = std::env::temp_dir().join(format!("native-studio-organization-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame), ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("studio"))]);
    let sim = Sim::wireless_mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    store.for_device(&engine.devices()[0]); store.save().unwrap();
    let store_before = std::fs::read(directory.join("profiles.json")).unwrap();
    let source = directory.join("input.ChromaEffects");
    let original = br#"{"version":1,"profiles":[{"id":"p","title":"Organization","devices":[],"opaque":9.9900e+100,"layers":[{"id":"g","type":"group","title":"Group","opened":true,"visible":true,"layers":[{"id":"a","type":"layer","title":"A","name":"static","groupId":"g","visible":true,"regions":[],"future":1.2300e-7},{"id":"b","type":"layer","title":"B","name":"static","groupId":"g","visible":false,"regions":[]}]},{"id":"c","type":"layer","title":"C","name":"static","visible":true,"regions":[]},{"id":"h","type":"group","title":"Second","visible":true,"opened":false,"layers":[]}]}]}"#;
    std::fs::write(&source, original).unwrap();
    let folder = directory.clone(); let monitor = sim.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> { unsafe {
            wait_for_studio_window(|| frame.exists(), "Studio organization window")?;
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            let query = || { let mut value = Value::Null; SendMessageW(hwnd, STUDIO_TEST, 2, &mut value as *mut Value as isize); value };
            wait_for(|| query()["pending"] == 0, "initial simulated reads")?;
            let path = json!(source.to_string_lossy()); SendMessageW(hwnd, STUDIO_TEST, 1, &path as *const Value as isize);
            wait_for(|| query()["profiles"] == 1 && query()["busy"] == false, "organization fixture")?;
            monitor.state().log.clear(); redraw(hwnd);
            let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
            let pack = |x: f64, y: f64| (((y*scale).round() as u32) << 16 | (x*scale).round() as u32) as isize;
            let drag = |from: f64, to: f64| {
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, pack(100.,from));
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, pack(100.,to));
                SendMessageW(hwnd, WM_LBUTTONUP, 0, pack(100.,to)); redraw(hwnd);
            };
            let click = |x,y| lighting_click(hwnd,x,y,scale);
            SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0); drag(308.,188.);
            SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
            if query()["undo"] != 0 { return Err("read-only drag edited the tree".into()); }
            SendMessageW(hwnd, WM_LBUTTONDOWN, 1, pack(100.,308.));
            SendMessageW(hwnd, WM_MOUSEMOVE, 1, pack(100.,188.));
            SendMessageW(hwnd, WM_CANCELMODE, 0, 0);
            SendMessageW(hwnd, WM_LBUTTONUP, 0, pack(100.,188.));
            if query()["undo"] != 0 { return Err("cancelled drag edited the tree".into()); }
            drag(308.,188.); // C into G, prepend
            if query()["rows"][1]["id"] != "c" || query()["rows"][1]["parent"] != "g" { return Err(format!("root into group failed: {}",query())); }
            drag(268.,348.); // A into closed H
            click(10.,308.); // expand H, persisted
            drag(348.,170.); // A out to root, before G
            click(100.,348.); // H selected for rename
            let mut client: RECT = std::mem::zeroed(); GetClientRect(hwnd,&mut client);
            let right = f64::from(client.right)/scale - 250.;
            click(right+80.,193.);
            let field=GetDlgItem(hwnd, i32::from(studio_view::NAME));
            if field.is_null() { return Err("group rename field missing".into()); }
            SendMessageW(field, WM_SETTEXT, 0, wide("Renamed").as_ptr() as isize);
            click(right+80.,193.);
            click(210.,348.); // hide empty H
            drag(348.,170.); // H before A
            click(10.,268.); // collapse G
            if query()["undo"] != 8 || query()["failed"] == true { return Err(format!("organization operations failed: {}",query())); }
            let rows = query()["rows"].clone();
            let image=json!(folder.join("organized.png").to_string_lossy());
            if SendMessageW(hwnd,STUDIO_TEST,6,&image as *const Value as isize)!=1 { return Err("capture failed".into()); }
            for _ in 0..8 { click(440.,100.); }
            if query()["dirty"] != false { return Err(format!("organization undo failed: {}",query())); }
            for _ in 0..8 { click(515.,100.); }
            if query()["rows"] != rows { return Err("organization redo changed rows".into()); }
            let output=folder.join("organized.ChromaEffects");
            let path=json!(output.to_string_lossy()); SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);
            wait_for(||query()["busy"]==false && query()["dirty"]==false,"organization save")?;
            let saved=std::fs::read(&output).map_err(|e|e.to_string())?;
            let archive=secbird_core::studio::Archive::parse(&saved).map_err(|e|e.to_string())?;
            let raw=archive.profile_json(0).ok_or("missing saved profile")?;
            let draft=secbird_core::studio::document::Draft::open(raw).map_err(|e|e.to_string())?;
            if draft.layers()[0].id!="h" || draft.layers()[0].title!="Renamed" || draft.layers()[0].opened!=Some(true)
                || draft.layers()[0].visible!=Some(false) || draft.layers()[1].parent.is_some()
                || draft.layers()[2].opened!=Some(false) || !raw.contains("1.2300e-7") || !raw.contains("9.9900e+100") {
                return Err("saved group structure/metadata differs".into());
            }
            SendMessageW(hwnd,STUDIO_TEST,1,&path as *const Value as isize);
            wait_for(||query()["busy"]==false && query()["undo"]==0,"organization reopen")?;
            if query()["rows"]!=rows { return Err("reopened rows differ".into()); }
            if std::fs::read(&source).map_err(|e|e.to_string())? != original { return Err("source modified".into()); }
            if !monitor.state().log.is_empty() { return Err("organization made device calls".into()); }
            Ok(json!({"drag_root_into_group":true,"drag_between_groups":true,"drag_out_to_root":true,
                "group_reorder_rename_expand_visibility":true,"undo_redo_steps":8,"save_reopen":true,
                "unknown_raw_data_preserved":true,"read_only_and_cancel":true,"device_calls":0}))
        }})();
        unsafe { let hwnd=WINDOW.load(Ordering::Acquire) as HWND; SendMessageW(hwnd,STUDIO_TEST,5,0); PostMessageW(hwnd,WM_CLOSE,0,0); }
        std::fs::write(folder.join("interaction.json"),serde_json::to_vec_pretty(&match &result {Ok(v)=>v.clone(),Err(e)=>json!({"error":e})}).unwrap()).unwrap();
        result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(45)));
    let result=controller.join().unwrap();
    assert!(run.is_ok(),"{run:?}"); assert!(result.is_ok(),"{result:?}");
    assert_eq!(std::fs::read(directory.join("profiles.json")).unwrap(),store_before);
    eprintln!("Studio organization evidence: {}",directory.display());
}
