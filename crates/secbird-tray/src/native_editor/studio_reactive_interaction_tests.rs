use super::*;
#[test]
#[ignore="requires private stock export/assets; opens an isolated simulated Reactive authoring window"]
fn native_studio_reactive_color_random_duration_save_and_history_preserve_source() {
    let input=PathBuf::from(std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select stock export"));
    let original=std::fs::read(&input).unwrap();
    let folder=std::env::temp_dir().join(format!("native-studio-reactive-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&folder).unwrap();let frame=folder.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&folder),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("studio"))]);
    let sim=Sim::wireless_mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();let mut store=Store::load(&folder.join("profiles.json")).unwrap();store.for_device(&engine.devices()[0]);store.save().unwrap();let baseline=std::fs::read(folder.join("profiles.json")).unwrap();
    let output=folder.clone();let monitor=sim.clone();let source=input.clone();
    let controller=std::thread::spawn(move||{
        let result=(||->Result<Value,String>{unsafe {
            wait_for(||frame.exists(),"Reactive window")?;let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut v=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut v as *mut Value as isize);v};
            wait_for(||query()["pending"]==0,"initial reads")?;monitor.state().log.clear();
            let path=json!(source.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,1,&path as *const Value as isize);wait_for(||query()["profiles"]==1&&query()["busy"]==false,"composition load")?;
            let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;let mut client:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut client);let width=f64::from(client.right)/scale;let height=f64::from(client.bottom)/scale;let right=width-250.;
            lighting_click(hwnd,775.,100.,scale);lighting_click(hwnd,800.,440.,scale);
            let id=query()["selected"].as_str().ok_or("Reactive not selected")?.to_owned();
            if !query()["rows"].as_array().unwrap().iter().any(|r|r["id"]==id&&r["title"]=="REACTIVE"){return Err("Reactive layer was not added".into());}
            let fit=((width-540.)/960.).min((height-250.)/360.).clamp(0.05,4.);lighting_click(hwnd,width/2.+(67.-480.)*fit,(118.+height)/2.+(76.-180.)*fit,scale);
            lighting_click(hwnd,right+195.,461.,scale);
            let field=GetDlgItem(hwnd,2109);if field.is_null(){return Err("Reactive colour field missing".into());}
            SendMessageW(field,WM_SETTEXT,0,wide("112233").as_ptr() as isize);lighting_click(hwnd,right+65.,338.,scale);
            lighting_click(hwnd,right+50.,383.,scale);let field=GetDlgItem(hwnd,2109);
            if windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(field)!=0 {return Err("Random Colour left colour input enabled".into());}
            let history=query()["undo"].clone();lighting_click(hwnd,right+190.,338.,scale);if query()["undo"]!=history{return Err("Random Colour permitted a clear".into());}
            lighting_click(hwnd,right+170.,383.,scale);
            let capture=json!(output.join("reactive-properties.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            let saved_path=output.join("reactive.ChromaEffects");let path=json!(saved_path.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);wait_for(||query()["busy"]==false&&query()["dirty"]==false,"Reactive save")?;
            let saved=secbird_core::studio::Archive::parse(&std::fs::read(&saved_path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;let saved:Value=serde_json::from_str(saved.profile_json(0).unwrap()).unwrap();let layer=saved["layers"].as_array().unwrap().iter().find(|r|r["id"]==id).ok_or("saved Reactive absent")?;let params=&layer["regions"][0]["params"];
            if params["color"]!=0x112233||params["duration"]!=500||params["randomColor"]!=false||params["playbackStart"]!="onPress"||params["playbackEnd"]!="after"||params["cycles"]!=1{return Err(format!("Reactive save mismatch: {params}"));}
            lighting_click(hwnd,450.,100.,scale);if query()["dirty"]!=true{return Err("Reactive Undo failed".into());}lighting_click(hwnd,520.,100.,scale);if query()["dirty"]!=false{return Err("Reactive Redo failed".into());}
            let field=GetDlgItem(hwnd,2109);SendMessageW(field,WM_SETTEXT,0,wide("GGGGGG").as_ptr() as isize);lighting_click(hwnd,right+65.,338.,scale);if query()["dirty"]!=false||query()["failed"]!=true{return Err("Invalid colour changed the draft".into());}
            lighting_click(hwnd,right+190.,338.,scale);if query()["dirty"]!=true{return Err("Reactive None did not change colour".into());}SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);wait_for(||query()["busy"]==false&&query()["dirty"]==false,"Reactive None save")?;
            let saved=secbird_core::studio::Archive::parse(&std::fs::read(saved_path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;let saved:Value=serde_json::from_str(saved.profile_json(0).unwrap()).unwrap();let layer=saved["layers"].as_array().unwrap().iter().find(|r|r["id"]==id).unwrap();if layer["regions"][0]["params"].get("color").is_some(){return Err("None saved a colour value".into());}
            SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);lighting_click(hwnd,right+20.,461.,scale);if query()["dirty"]!=false{return Err("Read-only Reactive changed the draft".into());}SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);
            lighting_click(hwnd,630.,100.,scale);wait_for(||query()["failed"]==true,"Reactive preview refusal")?;if query()["preview"]["current"]==true{return Err("Reactive falsely presented live playback".into());}
            if !monitor.state().log.is_empty(){return Err("Reactive authoring issued a device request".into());}
            Ok(json!({"add_reactive":true,"rgb_saved":0x112233,"duration_ms":500,"source_trigger_defaults_saved":true,"random_preserves_colour_and_disables_edit":true,"none_omits_colour":true,"save_undo_redo":true,"invalid_colour_atomic":true,"read_only_inert":true,"unsupported_preview_refused":true,"device_requests":0,"physical_discovery_disabled":true}))
        }})();
        unsafe{let hwnd=WINDOW.load(Ordering::Acquire) as HWND;SendMessageW(hwnd,STUDIO_TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}
        std::fs::write(output.join("interaction.json"),serde_json::to_vec_pretty(&match &result{Ok(v)=>v.clone(),Err(e)=>json!({"error":e})}).unwrap()).unwrap();result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(35)));let result=controller.join().unwrap();assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");assert_eq!(std::fs::read(input).unwrap(),original);assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(),baseline);eprintln!("Reactive native evidence: {}",folder.display());
}
