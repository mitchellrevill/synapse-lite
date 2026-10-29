use super::*;

#[test]
#[ignore="requires private native assets; opens an isolated simulated Linked Games window"]
fn native_linked_games_add_bind_unlink_remove_and_tab_lifecycle_preserve_settings() {
    let folder=std::env::temp_dir().join(format!("native-linked-games-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&folder).unwrap();let frame=folder.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&folder),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("dashboard"))]);
    assert!(super::super::linked_games::work(super::super::linked_games::Request::Add(r"C:\Blocked.exe".into()), vec![], true).is_err());
    assert!(!folder.join("profiles.json").exists(), "read-only worker created a store");
    let sim=Sim::mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();let key=engine.devices()[0].key();let path=folder.join("profiles.json");
    let mut store=Store::load(&path).unwrap();let profiles=store.for_device(&engine.devices()[0]);profiles.active_mut().dpi=Some((800,800));profiles.create("Gaming").unwrap();profiles.profiles.get_mut("Gaming").unwrap().dpi=Some((1600,1600));store.save().unwrap();let baseline=std::fs::read(&path).unwrap();
    let output=folder.clone();let monitor=sim.clone();let controller=std::thread::spawn(move||{
        let result=(||->Result<Value,String>{unsafe{
            wait_for(||frame.exists(),"Linked Games window")?;let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,super::super::linked_games::TEST,3,&mut value as *mut Value as isize);value};
            SendMessageW(hwnd,super::super::linked_games::TEST,1,0);wait_for(||query()["active"]==true && query()["busy"]==false && query()["devices"].as_array().is_some_and(|d|!d.is_empty()),"library read")?;
            monitor.state().log.clear();let previous_dpi=monitor.state().dpi;
            let game=json!(r"C:\Games\Synthetic.exe");SendMessageW(hwnd,super::super::linked_games::TEST,2,&game as *const Value as isize);wait_for(||query()["games"].as_array().is_some_and(|g|g.len()==1)&&query()["busy"]==false,"game add")?;
            let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;let mut client:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut client);let width=f64::from(client.right)/scale;let height=f64::from(client.bottom)/scale;let x=(width-730.)/2.;
            let capture=json!(output.join("games.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            lighting_click(hwnd,175.,240.,scale);if query()["dialog"]!=true{return Err("Game tile did not open profile linking".into());}
            lighting_click(hwnd,x+570.,274.,scale);if query()["picker"]!=true{return Err("Profile selector did not open".into());}lighting_click(hwnd,x+560.,317.,scale);
            wait_for(||query()["busy"]==false,"profile link")?;
            let saved=Store::load(&output.join("profiles.json")).map_err(|e|e.to_string())?;if saved.devices[&key].profiles["Gaming"].app_paths!=[r"c:\games\synthetic.exe"]||saved.devices[&key].active!="Default"{return Err("Profile link or saved selection is wrong".into());}
            let capture=json!(output.join("game-profiles.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            let bytes=std::fs::read(output.join("profiles.json")).unwrap();SendMessageW(hwnd,READ_ONLY_TEST_SET,1,0);lighting_click(hwnd,x+570.,274.,scale);let extra=json!(r"C:\Other\Blocked.exe");SendMessageW(hwnd,super::super::linked_games::TEST,2,&extra as *const Value as isize);if std::fs::read(output.join("profiles.json")).unwrap()!=bytes{return Err("Read-only view changed links".into());}SendMessageW(hwnd,READ_ONLY_TEST_SET,0,0);
            lighting_click(hwnd,x+703.,172.,scale);lighting_click(hwnd,width/2.+40.,105.,scale);if query()["page"]!="devices"{return Err("Devices view did not open".into());}
            let capture=json!(output.join("devices.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            lighting_click(hwnd,175.,240.,scale);lighting_click(hwnd,x+570.,274.,scale);lighting_click(hwnd,x+560.,259.,scale);wait_for(||query()["busy"]==false,"unlink")?;
            let saved=Store::load(&output.join("profiles.json")).map_err(|e|e.to_string())?;if !saved.devices[&key].profiles["Gaming"].app_paths.is_empty(){return Err("Unlink did not remove the selected game".into());}
            lighting_click(hwnd,x+703.,172.,scale);lighting_click(hwnd,width/2.-40.,105.,scale);lighting_click(hwnd,175.,240.,scale);lighting_click(hwnd,x+90.,height-88.,scale);wait_for(||query()["games"].as_array().is_some_and(Vec::is_empty)&&query()["busy"]==false,"game removal")?;
            if std::fs::read(output.join("profiles.json")).unwrap()!=baseline{return Err("Roundtrip changed original device profiles".into());}
            if monitor.state().dpi!=previous_dpi||monitor.state().log.iter().any(|c|c.id&0x80==0){return Err("Library editing issued a device SET".into());}
            // The module uses the existing tab lifecycle: closing removes only the view.
            let mut tabs=Value::Null;SendMessageW(hwnd,super::super::linked_games::TEST,4,&mut tabs as *mut Value as isize);
            let tab=tabs["tabs"].as_array().unwrap().iter().find(|tab|tab["id"]==super::super::linked_games::ID).ok_or("Linked Games module tab missing")?;
            let tx=(tab["x"].as_i64().unwrap()+tab["width"].as_i64().unwrap()/2) as u32;let ty=(tab["y"].as_i64().unwrap()+tab["height"].as_i64().unwrap()/2) as u32;
            SendMessageW(hwnd,WM_MBUTTONDOWN,0,((ty<<16)|tx) as isize);SendMessageW(hwnd,WM_MBUTTONUP,0,((ty<<16)|tx) as isize);if query()["active"]!=false{return Err("Module middle-close failed".into());}
            SendMessageW(hwnd,super::super::linked_games::TEST,1,0);wait_for(||query()["active"]==true&&query()["busy"]==false,"module reopen")?;
            let tabs_path=output.join("tabs.json");if !tabs_path.exists(){return Err("Module tab was not persisted".into());}
            Ok(json!({"games_devices_views":true,"add_bind_unlink_remove":true,"saved_profiles_byte_equal":true,"read_only_inert":true,"worker_read_only_refuses_before_store_creation":true,"tab_close_reopen":true,"profile_choice":"Gaming","hardware_sets":0,"native_file_picker_bypassed":true,"physical_discovery_disabled":true}))
        }})();
        unsafe{let hwnd=WINDOW.load(Ordering::Acquire) as HWND;PostMessageW(hwnd,WM_CLOSE,0,0);}
        std::fs::write(output.join("interaction.json"),serde_json::to_vec_pretty(&match &result{Ok(v)=>v.clone(),Err(e)=>json!({"error":e})}).unwrap()).unwrap();result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(35)));let result=controller.join().unwrap();assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");eprintln!("Linked Games native evidence: {}",folder.display());
}
