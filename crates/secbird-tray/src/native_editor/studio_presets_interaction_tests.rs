use super::*;
#[test]
#[ignore="requires private indexed preset pack; opens isolated native offline device authoring"]
fn native_studio_add_presets_menu_cancel_save_reopen_and_history_preserve_profiles() {
    let folder=std::env::temp_dir().join(format!("native-studio-picker-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&folder).unwrap();
    let source=folder.join("source.ChromaEffects");
    let source_bytes=br#"{"version":1,"profiles":[{"id":"desk","title":"Offline desk","devices":[],"layers":[{"id":"static","type":"layer","name":"static","title":"STATIC","value":6,"visible":true,"params":{},"regions":[]}],"opaque":9.9900e+100}]}"#.to_vec();
    std::fs::write(&source,&source_bytes).unwrap();let frame=folder.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&folder),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("studio"))]);
    let sim=Sim::mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();let mut store=Store::load(&folder.join("profiles.json")).unwrap();store.for_device(&engine.devices()[0]);store.save().unwrap();
    let baseline=std::fs::read(folder.join("profiles.json")).unwrap();let output=folder.clone();let monitor=sim.clone();let input=source.clone();
    let controller=std::thread::spawn(move||{
        let result=(||->Result<Value,String>{unsafe {
            wait_for(||frame.exists(),"preset editor")?;let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut v=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut v as *mut Value as isize);v};
            wait_for(||query()["pending"]==0,"initial reads")?;monitor.state().log.clear();
            let path=json!(input.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,1,&path as *const Value as isize);
            wait_for(||query()["profiles"]==1&&query()["pending"]==0,"composition open")?;
            let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;
            lighting_click(hwnd,320.,184.,scale);wait_for(||query()["dialog"]==true,"native preset menu open")?;
            SendMessageW(hwnd,STUDIO_TEST,9,0);wait_for(||query()["dialog"]==false,"native preset menu cancel")?;
            if query()["canvas_devices"]!=0||query()["dirty"]!=false{return Err("Cancelling presets changed the composition".into());}
            let mouse=json!([100,0,0]);SendMessageW(hwnd,STUDIO_TEST,8,&mouse as *const Value as isize);
            if query()["canvas_devices"]!=1||query()["canvas_selectable_leds"]!=2||query()["dirty"]!=true{return Err(format!("Mouse preset was not added: {}",query()));}
            let saved=output.join("saved.ChromaEffects");let path=json!(saved.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);
            wait_for(||query()["pending"]==0&&query()["dirty"]==false,"preset save")?;
            lighting_click(hwnd,450.,100.,scale);if query()["canvas_devices"]!=0||query()["dirty"]!=true{return Err("Placement Undo failed".into());}
            lighting_click(hwnd,520.,100.,scale);if query()["canvas_devices"]!=1||query()["dirty"]!=false{return Err("Placement Redo failed".into());}
            SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);let keyboard=json!([620,0,1]);SendMessageW(hwnd,STUDIO_TEST,8,&keyboard as *const Value as isize);
            if query()["canvas_devices"]!=1||query()["dirty"]!=false{return Err("Read-only preset addition changed state".into());}SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);
            SendMessageW(hwnd,STUDIO_TEST,8,&keyboard as *const Value as isize);
            if query()["canvas_devices"]!=2||query()["canvas_selectable_leds"]!=108||query()["canvas_can_preview"]!=false{return Err(format!("Keyboard preset was not added: {}",query()));}
            SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);wait_for(||query()["pending"]==0&&query()["dirty"]==false,"two-device save")?;
            SendMessageW(hwnd,STUDIO_TEST,1,&path as *const Value as isize);wait_for(||query()["pending"]==0&&query()["canvas_devices"]==2,"two-device reopen")?;
            let archive=secbird_core::studio::Archive::parse(&std::fs::read(&saved).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            let raw=archive.profile_json(0).unwrap();let value:Value=serde_json::from_str(raw).unwrap();
            if !raw.contains("9.9900e+100")||value["devices"][0]["editorPreset"]!=true||value["devices"][1]["editorPreset"]!=true||value["layers"][0]["regions"]!=json!([]){return Err("Preset save lost opaque or offline metadata".into());}
            lighting_click(hwnd,630.,100.,scale);lighting_click(hwnd,915.,100.,scale);
            if query()["preview"]["current"]==true||query()["output_busy"]==true||query()["pending"]!=0||!monitor.state().log.is_empty(){return Err("Offline presets issued playback or device requests".into());}
            let capture=json!(output.join("added-presets.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            Ok(json!({"native_menu_open_cancel":true,"mouse_added":100,"keyboard_added":620,"layouts":[0,1],"selectable_leds":108,"save_reopen_undo_redo":true,"read_only_inert":true,"opaque_number_preserved":true,"physical_discovery_disabled":true,"device_requests":0,"menu_selection_by_user_not_verified":true}))
        }})();unsafe{let hwnd=WINDOW.load(Ordering::Acquire) as HWND;SendMessageW(hwnd,STUDIO_TEST,9,0);SendMessageW(hwnd,STUDIO_TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}
        std::fs::write(output.join("interaction.json"),serde_json::to_vec_pretty(&match &result{Ok(v)=>v.clone(),Err(e)=>json!({"error":e})}).unwrap()).unwrap();result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(30)));let result=controller.join().unwrap();
    assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");assert_eq!(std::fs::read(source).unwrap(),source_bytes);assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(),baseline);eprintln!("Studio preset picker evidence: {}",folder.display());
}
#[test]
#[ignore="requires private preset pack; opens an isolated native mouse-preset authoring window"]
fn native_studio_foreign_preset_selection_colour_save_and_output_refusal_preserve_profiles() {
    exercise_preset("studio-100-0-0.json",100,0,0,2,false);
}
#[test]
#[ignore="requires private curved preset pack; opens an isolated native circular LED authoring window"]
fn native_studio_curved_preset_selection_colour_save_and_output_refusal_preserve_profiles() {
    exercise_preset("studio-620-0-1.json",620,0,1,106,true);
}
fn exercise_preset(filename:&str,pid:u16,edition:u16,layout:u16,expected_leds:usize,curved:bool) {
    let preset=assets_dir().join("studio-presets").join(filename);let geometry=source_layout::SourceLayout::load(&preset).expect("provision exact Basilisk preset");
    let selected_id=if curved {let raw:Value=serde_json::from_slice(&std::fs::read(&preset).unwrap()).unwrap();raw["keys"].as_array().unwrap().iter().find(|key|key.get("shape").is_some()).unwrap()["inputID"].as_str().unwrap().to_owned()}else{geometry.keys.first().unwrap().input_id.clone()};
    let first=geometry.keys.iter().find(|key|key.input_id==selected_id).unwrap();let (row,col)=first.input_id.split_once(':').unwrap();let (row,col):(u16,u16)=(row.parse().unwrap(),col.parse().unwrap());
    let b=geometry.bounds(&first.input_id,source_layout::Viewport{left:0.,top:0.,width:geometry.view_box[2],height:geometry.view_box[3]}).unwrap();let center=((b.left+b.right)/2.,(b.top+b.bottom)/2.);let dims=(geometry.view_box[2],geometry.view_box[3]);
    let leds:Vec<Value>=geometry.keys.iter().map(|key|{let (r,c)=key.input_id.split_once(':').unwrap();json!({"row":r.parse::<u16>().unwrap(),"col":c.parse::<u16>().unwrap()})}).collect();
    let profile=json!({"id":"preset","title":if curved{"Curved keyboard preset"}else{"Basilisk preset"},"devices":[{"id":"basilisk","productId":pid,"editionId":edition,"layout":layout,"x":0,"y":0,"width":dims.0,"height":dims.1,"orientation":0,"leds":leds}],"layers":[{"id":"static","type":"layer","name":"static","value":6,"title":"STATIC","params":{},"regions":[],"visible":true}]});
    let folder=std::env::temp_dir().join(format!("native-studio-presets-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::create_dir_all(&folder).unwrap();let source=folder.join("source.ChromaEffects");let source_bytes=serde_json::to_vec(&json!({"version":1,"profiles":[profile]})).unwrap();std::fs::write(&source,&source_bytes).unwrap();let frame=folder.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&folder),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("studio"))]);let sim=Sim::mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();let mut store=Store::load(&folder.join("profiles.json")).unwrap();store.for_device(&engine.devices()[0]);store.save().unwrap();let baseline=std::fs::read(folder.join("profiles.json")).unwrap();let output=folder.clone();let monitor=sim.clone();let input=source.clone();
    let controller=std::thread::spawn(move||{
        let result=(||->Result<Value,String>{unsafe {
            wait_for(||frame.exists(),"preset window")?;let hwnd=WINDOW.load(Ordering::Acquire) as HWND;let query=||{let mut v=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut v as *mut Value as isize);v};wait_for(||query()["pending"]==0,"initial reads")?;monitor.state().log.clear();
            let path=json!(input.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,1,&path as *const Value as isize);wait_for(||query()["profiles"]==1&&query()["pending"]==0,"preset open")?;
            if query()["canvas_devices"]!=1||query()["canvas_selectable_leds"]!=expected_leds||query()["canvas_notes"].as_array().is_none_or(|notes|!notes.is_empty())||query()["canvas_can_preview"]!=false{return Err(format!("Preset admission was wrong: {}",query()));}
            let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;let mut client:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut client);let width=f64::from(client.right)/scale;let height=f64::from(client.bottom)/scale;let right=width-250.;let fit=((width-540.)/dims.0).min((height-250.)/dims.1).clamp(0.05,4.);
            lighting_click(hwnd,width/2.+(center.0-dims.0/2.)*fit,(168.+height)/2.+(center.1-dims.1/2.)*fit,scale);if query()["selected_leds"]!=1{return Err("Exact source mouse LED did not select".into());}
            let color=GetDlgItem(hwnd,2101);if color.is_null(){return Err("Static colour field missing".into());}SendMessageW(color,WM_SETTEXT,0,wide("ABCDEF").as_ptr() as isize);lighting_click(hwnd,right+100.,343.,scale);
            let capture=json!(output.join("basilisk-preset.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            let saved=output.join("saved.ChromaEffects");let path=json!(saved.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,3,&path as *const Value as isize);wait_for(||query()["dirty"]==false&&query()["pending"]==0,"preset save")?;
            let archive=secbird_core::studio::Archive::parse(&std::fs::read(saved).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;let value:Value=serde_json::from_str(archive.profile_json(0).unwrap()).unwrap();let region=&value["layers"][0]["regions"][0];
            if region["params"]["color"]!=0xabcdef||region["devices"][0]["id"]!="basilisk"||region["devices"][0]["leds"][0]["row"]!=row||region["devices"][0]["leds"][0]["col"]!=col{return Err(format!("Preset LED save mismatch: {region}"));}
            lighting_click(hwnd,630.,100.,scale);lighting_click(hwnd,915.,100.,scale);if query()["preview"]["current"]==true||query()["output_busy"]==true||query()["pending"]!=0{return Err("A canvas preset incorrectly enabled unimplemented device playback".into());}
            lighting_click(hwnd,450.,100.,scale);if query()["dirty"]!=true{return Err("Preset Undo failed".into());}lighting_click(hwnd,520.,100.,scale);if query()["dirty"]!=false{return Err("Preset Redo failed".into());}
            if !monitor.state().log.is_empty(){return Err("Offline preset authoring issued device requests".into());}
            Ok(json!({"product_id":pid,"edition_id":edition,"layout_id":layout,"curved_shape":curved,"source_led_selected":true,"static_rgb_saved":0xabcdef,"save_undo_redo":true,"unimplemented_preview_output_inert":true,"device_requests":0,"physical_discovery_disabled":true}))
        }})();unsafe{let hwnd=WINDOW.load(Ordering::Acquire) as HWND;SendMessageW(hwnd,STUDIO_TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}std::fs::write(output.join("interaction.json"),serde_json::to_vec_pretty(&match &result{Ok(v)=>v.clone(),Err(e)=>json!({"error":e})}).unwrap()).unwrap();result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(30)));let result=controller.join().unwrap();assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");assert_eq!(std::fs::read(source).unwrap(),source_bytes);assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(),baseline);eprintln!("Generic Studio preset native evidence: {}",folder.display());
}
