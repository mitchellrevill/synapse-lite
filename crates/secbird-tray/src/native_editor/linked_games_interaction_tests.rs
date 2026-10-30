use super::*;

#[test]
#[ignore="opens a private native window with a read-only copy of installed game metadata"]
fn native_linked_games_installed_catalogue_names_icons_and_search() {
    let root=PathBuf::from(std::env::var_os("ProgramData").unwrap()).join("Razer/GameManager3/Data");
    let games=secbird_core::linked_games::stock_catalogue::read(&root).unwrap();assert!(!games.is_empty());let count=games.len();
    let folder=std::env::temp_dir().join(format!("native-stock-games-{}",std::process::id()));std::fs::create_dir_all(&folder).unwrap();let frame=folder.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&folder),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("dashboard"))]);
    let sim=Sim::mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();
    let store=secbird_core::linked_games::stock_catalogue::merge(&Store::load(&folder.join("profiles.json")).unwrap(),games).unwrap();store.save().unwrap();let before=std::fs::read(folder.join("profiles.json")).unwrap();let output=folder.clone();
    let controller=std::thread::spawn(move||{
        let result=(||->Result<(),String>{unsafe{
            wait_for(||frame.exists(),"installed-games HWND")?;let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,super::super::linked_games::TEST,3,&mut value as *mut Value as isize);value};
            SendMessageW(hwnd,super::super::linked_games::TEST,1,0);wait_for(||query()["busy"]==false&&query()["games"].as_array().is_some_and(|rows|rows.len()==count),"installed game tiles")?;
            redraw(hwnd);let capture=json!(output.join("installed-games.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            if query()["rendered_icons"].as_u64().unwrap_or(0)+query()["rendered_covers"].as_u64().unwrap_or(0)==0{return Err("No local game icons rendered".into());}
            let mut client:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut client);let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;
            let columns=(((f64::from(client.right)/scale) as usize-40)/310).max(1);let rows=((f64::from(client.bottom)/scale) as usize).saturating_sub(184).div_ceil(240);
            let mut sorted=query()["games"].as_array().unwrap().clone();sorted.sort_by_key(|game|game["name"].as_str().unwrap_or("").to_lowercase());
            let fallback_icons=sorted.iter().take(columns*rows).filter(|game|!assets_dir().join("game-covers").join(format!("{}.bgra",game["id"].as_str().unwrap())).is_file()&&game["stockIconPath"].as_str().is_some_and(|path|Path::new(path).is_file())).count();
            if query()["rendered_icons"].as_u64().unwrap_or(0)<fallback_icons as u64{return Err("A local ICO/PNG fallback was replaced by a generic placeholder".into());}
            let search=GetDlgItem(hwnd,2200);SetWindowTextW(search,wide("___no_game_matches___").as_ptr());SendMessageW(hwnd,WM_COMMAND,2200|((EN_CHANGE as usize)<<16),search as isize);redraw(hwnd);if query()["search"]!="___no_game_matches___"{return Err("Game search did not read its native edit field".into());}
            if std::fs::read(output.join("profiles.json")).unwrap()!=before{return Err("Viewing game tiles changed profile data".into());}
            Ok(())
        }})();unsafe{PostMessageW(WINDOW.load(Ordering::Acquire) as HWND,WM_CLOSE,0,0);}
        result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(30)));let result=controller.join().unwrap();assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");eprintln!("Installed game metadata HWND: {} games, {}",count,folder.display());
}

#[test]
#[ignore="opens an isolated native keyboard Lighting page and enters Studio through its visible button"]
fn native_lighting_advanced_opens_existing_studio() {
    let folder=std::env::temp_dir().join(format!("native-lighting-studio-{}",std::process::id()));std::fs::create_dir_all(&folder).unwrap();let frame=folder.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&folder),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("keyboard-lighting"))]);
    let sim=Sim::keyboard();let engine=Engine::with_simulated_keyboard(&sim,folder.join("profiles.json")).unwrap();let output=folder.clone();
    let controller=std::thread::spawn(move||{
        let result=(||->Result<(),String>{unsafe{
            wait_for(||std::fs::read(&frame).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).is_some_and(|v|v["source_lighting_view"]==true),"Lighting page")?;
            let hwnd=WINDOW.load(Ordering::Acquire) as HWND;let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;let mut client:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut client);
            let width=f64::from(client.right)/scale;let geometry=lighting_layout::geometry(width,128.,0.,1).ok_or("Lighting geometry")?;
            lighting_click(hwnd,geometry.advanced_tab.x+30.,geometry.advanced_tab.y+14.,scale);redraw(hwnd);
            lighting_click(hwnd,geometry.effects_panel.x+120.,geometry.effects_panel.y+182.,scale);redraw(hwnd);
            let mut studio=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut studio as *mut Value as isize);
            if studio["open"]!=true{return Err(format!("Lighting launch failed: {studio}"));}
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut value as *mut Value as isize);value};
            wait_for(||query()["busy"]==false,"Studio session restore")?;
            lighting_click(hwnd,288.,101.,scale);redraw(hwnd);lighting_click(hwnd,320.,130.,scale);redraw(hwnd);
            if query()["profiles"].as_u64().unwrap_or(0)==0{return Err("Studio profile menu could not create a composition".into());}
            lighting_click(hwnd,325.,186.,scale);redraw(hwnd);
            wait_for(||query()["dialog"]==true,"Studio preset menu")?;
            SendMessageW(hwnd,STUDIO_TEST,9,0);
            wait_for(||query()["dialog"]==false,"Studio preset menu cancelled")?;
            let preset=json!([605,0,6]);SendMessageW(hwnd,STUDIO_TEST,8,&preset as *const Value as isize);redraw(hwnd);
            if query()["canvas_devices"].as_u64().unwrap_or(0)==0{return Err("Studio could not add the Ornata preset".into());}
            let capture=json!(output.join("studio-from-lighting.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);Ok(())
        }})();unsafe{let hwnd=WINDOW.load(Ordering::Acquire) as HWND;SendMessageW(hwnd,STUDIO_TEST,9,0);SendMessageW(hwnd,STUDIO_TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(30)));let result=controller.join().unwrap();assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");eprintln!("Lighting to Studio HWND: {}",folder.display());
}

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
            // A dirty Studio draft must continue to this module after Discard.
            let studio_query=||{let mut value=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut value as *mut Value as isize);value};
            SendMessageW(hwnd,STUDIO_TEST,0,0);
            wait_for(||studio_query()["busy"]==false,"Studio restore")?;
            let composition=output.join("navigation.ChromaEffects");
            std::fs::write(&composition,br#"{"version":1,"profiles":[{"id":"p","title":"Desk","devices":[],"layers":[{"id":"a","type":"layer","title":"STATIC","name":"static","visible":true,"regions":[]}]}]}"#).map_err(|e|e.to_string())?;
            let value=json!(composition.to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,1,&value as *const Value as isize);
            wait_for(||studio_query()["profiles"]==1&&studio_query()["busy"]==false,"navigation composition")?;
            lighting_click(hwnd,width-190.,193.,scale);
            let field=GetDlgItem(hwnd,i32::from(studio_view::NAME));
            SendMessageW(field,WM_SETTEXT,0,wide("Navigation draft").as_ptr() as isize);
            lighting_click(hwnd,width-190.,193.,scale);
            if studio_query()["dirty"]!=true{return Err("navigation fixture did not become dirty".into());}
            SendMessageW(hwnd,STUDIO_TEST,4,IDCANCEL as isize);
            SendMessageW(hwnd,super::super::linked_games::TEST,1,0);
            std::thread::sleep(Duration::from_millis(100));
            if studio_query()["dirty"]!=true||studio_query()["open"]!=true{return Err("navigation cancel lost draft".into());}
            SendMessageW(hwnd,STUDIO_TEST,4,IDNO as isize);
            SendMessageW(hwnd,super::super::linked_games::TEST,1,0);
            wait_for(||query()["active"]==true&&query()["busy"]==false,"discard navigation continuation")?;
            let capture=json!(output.join("after-studio-discard.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            Ok(json!({"games_devices_views":true,"add_bind_unlink_remove":true,"saved_profiles_byte_equal":true,"read_only_inert":true,"worker_read_only_refuses_before_store_creation":true,"tab_close_reopen":true,"profile_choice":"Gaming","hardware_sets":0,"native_file_picker_bypassed":true,"physical_discovery_disabled":true}))
        }})();
        unsafe{let hwnd=WINDOW.load(Ordering::Acquire) as HWND;PostMessageW(hwnd,WM_CLOSE,0,0);}
        std::fs::write(output.join("interaction.json"),serde_json::to_vec_pretty(&match &result{Ok(v)=>v.clone(),Err(e)=>json!({"error":e})}).unwrap()).unwrap();result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(35)));let result=controller.join().unwrap();assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");eprintln!("Linked Games native evidence: {}",folder.display());
}
