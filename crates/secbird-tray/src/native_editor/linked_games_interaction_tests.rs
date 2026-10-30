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
    lighting_studio_entry(false,false);
}

#[test]
#[ignore="opens an isolated native keyboard Lighting page with an existing composition"]
fn native_lighting_studio_preserves_restored_composition() {
    lighting_studio_entry(true,false);
}

#[test]
#[ignore="opens an isolated Chroma page and enters Studio through its visible button"]
fn native_chroma_opens_usable_studio_with_keyboard() {
    lighting_studio_entry(false,true);
}

fn lighting_studio_entry(restore: bool, via_chroma: bool) {
    let folder=std::env::temp_dir().join(format!("native-lighting-studio-{}-{restore}-{via_chroma}",std::process::id()));std::fs::create_dir_all(&folder).unwrap();let frame=folder.join("frame.json");
    let composition=folder.join("existing.ChromaEffects");
    let original=br#"{"version":1,"profiles":[{"id":"existing","title":"Keep my desk","devices":[],"layers":[{"id":"keep","type":"layer","title":"KEEP MY LAYER","name":"static","visible":true,"regions":[]}]}]}"#;
    if restore {
        std::fs::write(&composition,original).unwrap();
        std::fs::write(folder.join("studio-editor.json"),serde_json::to_vec(&json!({"version":1,"path":composition,"profile_id":"existing"})).unwrap()).unwrap();
    }
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&folder),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("keyboard-lighting"))]);
    let sim=Sim::keyboard();let engine=Engine::with_simulated_keyboard(&sim,folder.join("profiles.json")).unwrap();let output=folder.clone();
    let controller=std::thread::spawn(move||{
        let result=(||->Result<(),String>{unsafe{
            wait_for(||std::fs::read(&frame).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).is_some_and(|v|v["source_lighting_view"]==true),"Lighting page")?;
            let hwnd=WINDOW.load(Ordering::Acquire) as HWND;ShowWindow(hwnd,SW_MAXIMIZE);redraw(hwnd);let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;let mut client:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut client);
            let width=f64::from(client.right)/scale;let geometry=lighting_layout::geometry(width,128.,0.,1).ok_or("Lighting geometry")?;
            if via_chroma {
                SendMessageW(hwnd,chroma_view::TEST,0,0);
                let chroma=||{let mut value=Value::Null;SendMessageW(hwnd,chroma_view::TEST,1,&mut value as *mut Value as isize);value};
                wait_for(||chroma()["open"]==true&&chroma()["busy"]==false,"Chroma page ready")?;
                let left=((width-1120.)/2.).max(30.);
                lighting_click(hwnd,width-left-230.,108.,scale);redraw(hwnd);
            } else {
                lighting_click(hwnd,geometry.advanced_tab.x+30.,geometry.advanced_tab.y+14.,scale);redraw(hwnd);
                lighting_click(hwnd,geometry.effects_panel.x+120.,geometry.effects_panel.y+182.,scale);redraw(hwnd);
            }
            let mut studio=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut studio as *mut Value as isize);
            if studio["open"]!=true{return Err(format!("Lighting launch failed: {studio}"));}
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut value as *mut Value as isize);value};
            wait_for(||query()["busy"]==false,"Studio session restore")?;
            if query()["profiles"]!=1 || query()["canvas_devices"]!=1 {
                return Err(format!("Lighting entry did not automatically load the keyboard: {}",query()));
            }
            if restore && (query()["profile_id"]!="existing" || query()["rows"][0]["id"]!="keep" || std::fs::read(output.join("existing.ChromaEffects")).unwrap()!=original) {
                return Err("Loading the keyboard replaced the existing composition or saved over its file".into());
            }
            let before=query();
            lighting_click(hwnd,20.,60.,scale);redraw(hwnd);
            if query()["open"]!=false {return Err("Keyboard tab did not return to Lighting".into());}
            lighting_click(hwnd,geometry.advanced_tab.x+30.,geometry.advanced_tab.y+14.,scale);redraw(hwnd);
            lighting_click(hwnd,geometry.effects_panel.x+120.,geometry.effects_panel.y+182.,scale);redraw(hwnd);
            if query()["open"]!=true || query()["canvas_devices"]!=1 || query()["rows"]!=before["rows"] || query()["undo"]!=before["undo"] {
                return Err(format!("Reopening Studio changed the draft: before {before}, after {}",query()));
            }
            let capture=json!(output.join("studio-from-lighting.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            let artwork=assets_dir().join("studio-artwork/3def02cda8fe7b5ad33d94702950d37297d99aa9daba471c2933930356851e7c-body.bgra");
            if artwork.exists() && query()["canvas_assets"]["placements_with_artwork"]!=1 {
                return Err("The matching keyboard body and legends were not loaded".into());
            }
            if query()["workspace"]["icons"]!=85 {return Err("Recovered Studio masks were not loaded".into());}
            let height=f64::from(client.bottom)/scale;
            let top=(height as i32-410).max(208) as f64;
            let effects=["ambient","audio","breathing","chroma-generate","fire","reactive","ripple","spectrum","starlight","static","tidal","wave","wheel"];
            for (index,effect) in effects.iter().enumerate() {
                let before=query();let count=before["rows"].as_array().unwrap().len();
                // Persistent palette: no command port or popup used to add the effect.
                let x=11.+(index%3) as f64*76.+38.;let y=top+40.+(index/3) as f64*72.+36.;
                if y>=height {return Err("Fixture palette needs a larger viewport".into());}
                lighting_click(hwnd,x,y,scale);let after=query();
                let selected=after["selected"].as_str().ok_or("Palette did not select a new layer")?;
                if after["rows"].as_array().unwrap().len()!=count+1 ||
                    !after["rows"].as_array().unwrap().iter().any(|row|row["id"]==selected && row["effect"]==*effect) {
                    return Err(format!("Palette tile {effect} did not author its corresponding effect: {after}"));
                }
                // Exercise the visible undo control so the next tile starts from the same draft.
                lighting_click(hwnd,width-310.,148.,scale);
                if query()["rows"]!=before["rows"] {return Err(format!("Undo did not restore {effect}"));}
            }
            // Hover paints the eye control, then its real hit target toggles visibility.
            let rows=query()["rows"].as_array().unwrap().clone();
            SendMessageW(hwnd,WM_MOUSEMOVE,0,position(232.,188.,scale));redraw(hwnd);
            lighting_click(hwnd,232.,188.,scale);
            if query()["rows"][0]["visible"]==rows[0]["visible"] {return Err("Layer eye control did not toggle visibility".into());}
            lighting_click(hwnd,width-310.,148.,scale);
            if query()["rows"]!=json!(rows) {return Err("Undo did not restore eye toggle".into());}
            let zoom=query()["viewport"]["zoom"].as_f64().unwrap()/100.;
            let center_y=(118.+height)/2.;
            let start=position(width/2.-480.*zoom,center_y-180.*zoom,scale);
            let end=position(width/2.+480.*zoom,center_y+180.*zoom,scale);
            SendMessageW(hwnd,WM_LBUTTONDOWN,0,start);
            SendMessageW(hwnd,WM_MOUSEMOVE,1,end);
            SendMessageW(hwnd,WM_LBUTTONUP,0,end);redraw(hwnd);
            if query()["selected_leds"]!=109 {return Err("Artwork obscured editable LED hit regions".into());}
            let capture=json!(output.join("studio-palette-verified.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            // Use the visible Fire tile and both stock-positioned color selectors.
            lighting_click(hwnd,125.,top+148.,scale);redraw(hwnd);
            if !query()["rows"].as_array().unwrap().iter().any(|row|row["id"]==query()["selected"]&&row["effect"]=="fire") {return Err("Fire tile did not select its properties".into());}
            lighting_click(hwnd,width/2.-15.,148.,scale);
            lighting_click(hwnd,width/2.-420.*zoom,center_y-105.*zoom,scale);redraw(hwnd);
            if query()["selected_leds"]!=109 {return Err("Bucket tool did not select the keyboard’s LED regions".into());}
            lighting_click(hwnd,width/2.-97.,148.,scale);redraw(hwnd);
            let right=width-250.;let before_colors=query()["undo"].as_u64().unwrap();
            for (cold,x,id,color) in [(false,right+30.,studio_view::COLOR,"D07005"),(true,right+94.,studio_view::COLD,"240010")] {
                lighting_click(hwnd,x,239.,scale);redraw(hwnd);
                let edit=GetDlgItem(hwnd,i32::from(id));if edit.is_null(){return Err("Color selector did not open its input".into());}
                SetWindowTextW(edit,wide(color).as_ptr());SendMessageW(hwnd,WM_COMMAND,usize::from(id)|((EN_CHANGE as usize)<<16),edit as isize);
                lighting_click(hwnd,right+70.,402.,scale);redraw(hwnd);
                if query()["failed"]==true {return Err(format!("{} color edit failed: {}",if cold{"Cold"}else{"Hot"},query()));}
                lighting_click(hwnd,right+200.,200.,scale);redraw(hwnd);
            }
            if query()["undo"].as_u64().unwrap()!=before_colors+2 {return Err(format!("Hot/Cold did not create independent undoable edits: before {before_colors}, after {}",query()));}
            let capture=json!(output.join("studio-fire-full.png").to_string_lossy());SendMessageW(hwnd,STUDIO_TEST,6,&capture as *const Value as isize);
            std::fs::write(output.join("studio-palette.json"),serde_json::to_vec_pretty(&json!({"snapshot":query(),"palette_pointer_checks":effects,"eye_toggle_and_undo":true,"fixture":"private simulated keyboard; no hardware certification"})).unwrap()).map_err(|e|e.to_string())?;
            Ok(())
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

#[test]
#[ignore="opens a fresh native Studio with no compatible lighting device"]
fn native_studio_without_keyboard_still_has_working_effects_and_device_picker() {
    studio_without_keyboard(false);
}

#[test]
#[ignore="opens a fresh Studio with a remembered keyboard but no live keyboard interface"]
fn native_studio_retains_remembered_keyboard_layout_for_offline_editing() {
    studio_without_keyboard(true);
}

fn studio_without_keyboard(remembered: bool) {
    let folder=std::env::temp_dir().join(format!("native-empty-studio-{}-{remembered}",std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();let frame=folder.join("frame.json");
    let _environment=Environment::set(&[("SYNAPSE_LITE_CONFIG_DIR",&folder),("SYNAPSE_LITE_SMOKE_REPORT",&frame),("SYNAPSE_LITE_SMOKE_VIEW",Path::new("dashboard"))]);
    if remembered {std::fs::write(folder.join("tabs.json"),br#"{"version":1,"order":["025D:remembered"],"closed":[]}"#).unwrap();}
    let sim=Sim::mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();
    let controller=std::thread::spawn(move||{
        let result=(||->Result<(),String>{unsafe{
            wait_for(||frame.exists(),"fresh Studio window")?;let hwnd=WINDOW.load(Ordering::Acquire) as HWND;
            let query=||{let mut value=Value::Null;SendMessageW(hwnd,STUDIO_TEST,2,&mut value as *mut Value as isize);value};
            SendMessageW(hwnd,STUDIO_TEST,0,0);
            wait_for(||query()["busy"]==false,"fresh Studio initialized")?;
            if query()["profiles"]!=1||query()["canvas_devices"]!=u64::from(remembered)||query()["rows"].as_array().is_none_or(Vec::is_empty){return Err(format!("Fresh Studio has no editable composition: {}",query()));}
            let scale=f64::from(GetDpiForWindow(hwnd).max(96))/96.;
            let before=query()["rows"].as_array().unwrap().len();
            studio_effect_click(hwnd,"Fire",scale)?;
            if query()["rows"].as_array().unwrap().len()!=before+1{return Err("Effect palette remained disabled without a keyboard".into());}
            studio_click(hwnd,"undo",scale)?;
            if query()["rows"].as_array().unwrap().len()!=before{return Err("Undo did not work without a keyboard".into());}
            if remembered {studio_click(hwnd,"workspace",scale)?;}
            studio_click(hwnd,"add-device",scale)?;
            wait_for(||query()["dialog"]==true,"device picker opens")?;
            SendMessageW(hwnd,STUDIO_TEST,9,0);
            wait_for(||query()["dialog"]==false,"device picker cancels")?;
            Ok(())
        }})();unsafe{let hwnd=WINDOW.load(Ordering::Acquire) as HWND;SendMessageW(hwnd,STUDIO_TEST,9,0);SendMessageW(hwnd,STUDIO_TEST,5,0);PostMessageW(hwnd,WM_CLOSE,0,0);}result
    });
    let run=run_standalone(Arc::new(Mutex::new(engine)),Some(Duration::from_secs(30)));let result=controller.join().unwrap();assert!(run.is_ok(),"{run:?}");assert!(result.is_ok(),"{result:?}");
}
