use super::*;
#[test]
#[ignore="opens two isolated native windows sequentially using simulated hardware"]
fn native_studio_session_reopens_exact_profile_after_restart_and_reorder_without_output() {
    let folder=std::env::temp_dir().join(format!("native-studio-session-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::create_dir_all(&folder).unwrap();
    let source=folder.join("session.ChromaEffects");let metadata=folder.join("studio-editor.json");
    let mut first:Value=serde_json::from_str(include_str!("../../../secbird-core/tests/fixtures/studio-preview-profile.json")).unwrap();first["id"]=json!("first");first["title"]=json!("First");let mut second=first.clone();second["id"]=json!("second");second["title"]=json!("Second");
    let mut saved_bookmark=vec![];let mut profile_baseline=vec![];
    for phase in 0..2 {
        let bytes=serde_json::to_vec(&json!({"version":1,"profiles":if phase==0 {vec![first.clone(),second.clone()]}else{vec![second.clone(),first.clone()]}})).unwrap();std::fs::write(&source,&bytes).unwrap();
        let frame=folder.join(format!("frame-{phase}.json"));let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&folder),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("studio"))]);
        let sim=Sim::mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();
        if phase==0 {let mut store=Store::load(&folder.join("profiles.json")).unwrap();store.for_device(&engine.devices()[0]);store.save().unwrap();profile_baseline=std::fs::read(folder.join("profiles.json")).unwrap();}
        let output=folder.clone();let monitor=sim.clone();let input=source.clone();
        let controller=std::thread::spawn(move||{
            let result=(||->Result<Value,String>{unsafe {
                wait_for(||frame.exists(),"Studio session window")?;let hwnd=WINDOW.load(Ordering::Acquire) as HWND;let query=||{let mut v=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut v as *mut Value as isize);v};wait_for(||query()["pending"]==0,"initial reads")?;monitor.state().log.clear();
                SendMessageW(hwnd,STUDIO_TEST,0,0);wait_for(||query()["pending"]==0&&query()["busy"]==false,"session restore")?;
                if phase==0 {
                    let path=json!(input.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,1,&path as *const Value as isize);wait_for(||query()["profiles"]==2&&query()["pending"]==0,"manual composition open")?;
                    let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;lighting_click(hwnd,150.,100.,scale);lighting_click(hwnd,150.,158.,scale);
                    wait_for(||query()["profile_id"]=="second"&&query()["pending"]==0,"profile bookmark")?;
                    if query()["index"]!=1 {return Err("Second profile was not selected".into());}
                } else {
                    if query()["profile_id"]!="second"||query()["index"]!=0||query()["profiles"]!=2{return Err(format!("Restore did not follow exact profile identity: {}",query()));}
                    let capture=json!(output.join("restored.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
                }
                if query()["dirty"]!=false||query()["preview"]["current"]==true{return Err("Restoring a bookmark started preview or dirtied the draft".into());}
                if !monitor.state().log.is_empty(){return Err("Editor persistence issued device requests".into());}
                Ok(json!({"phase":phase,"profile_id":"second","restored_index":if phase==0{1}else{0},"device_requests":0,"preview_started":false,"dirty":false}))
            }})();
            unsafe{let hwnd=WINDOW.load(Ordering::Acquire) as HWND;SendMessageW(hwnd,STUDIO_TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}
            std::fs::write(output.join(format!("interaction-{phase}.json")),serde_json::to_vec_pretty(&match &result{Ok(v)=>v.clone(),Err(e)=>json!({"error":e})}).unwrap()).unwrap();result
        });
        let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(30)));let result=controller.join().unwrap();assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");assert_eq!(std::fs::read(&source).unwrap(),bytes);assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(),profile_baseline);
        if phase==0 {saved_bookmark=std::fs::read(&metadata).unwrap();}else{assert_eq!(std::fs::read(&metadata).unwrap(),saved_bookmark);}
    }
    eprintln!("Studio session native evidence: {}",folder.display());
}
