//! Foreground switching uses only an isolated store and simulated devices.
use super::*;
use secbird_core::sim::Sim;

#[test]
fn exact_game_paths_switch_restore_and_reconcile_saved_links_without_repeat_writes() {
    let folder = std::env::temp_dir().join(format!(
        "application-switch-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = folder.join("profiles.json");
    let sim = Sim::mouse();
    let mut engine = Engine::with_simulated_mouse(&sim).unwrap();
    engine.runtime_store_path = Some(path.clone());
    let mut store = Store::load(&path).unwrap();
    let key = engine.devices()[0].key();
    let profiles = store.for_device(&engine.devices()[0]);
    profiles.active_mut().dpi = Some((800, 800));
    profiles.create("One").unwrap();
    profiles.create("Two").unwrap();
    profiles.profiles.get_mut("One").unwrap().dpi = Some((1600, 1600));
    profiles.profiles.get_mut("Two").unwrap().dpi = Some((3200, 3200));
    profiles
        .set_app_paths("One", &[r"C:\One\Game.exe".into()])
        .unwrap();
    profiles
        .set_app_paths("Two", &[r"D:\Two\Game.exe".into()])
        .unwrap();
    store.save().unwrap();
    let baseline = std::fs::read(&path).unwrap();
    sim.state().log.clear();
    assert!(engine
        .set_foreground(Some(r"E:\Other\Game.exe".into()))
        .is_empty());
    assert!(sim.state().log.is_empty());
    let reports = engine.set_foreground(Some(r"C:/ONE/Game.EXE".into()));
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].profile, "One");
    assert!(reports[0].failures.is_empty());
    assert_eq!(sim.state().dpi, Some((1600, 1600)));
    sim.state().log.clear();
    assert!(engine
        .set_foreground(Some(r"c:\one\game.exe".into()))
        .is_empty());
    assert!(sim.state().log.is_empty());
    let reports = engine.set_foreground(Some(r"D:\Two\Game.exe".into()));
    assert_eq!(reports[0].profile, "Two");
    assert_eq!(sim.state().dpi, Some((3200, 3200)));
    let reports = engine.set_foreground(None);
    assert_eq!(reports[0].profile, "Default");
    assert_eq!(sim.state().dpi, Some((800, 800)));
    assert_eq!(std::fs::read(&path).unwrap(), baseline);
    engine.set_foreground(Some(r"C:\One\Game.exe".into()));
    let mut changed = Store::load(&path).unwrap();
    changed
        .devices
        .get_mut(&key)
        .unwrap()
        .set_app_paths("Two", &[r"C:\One\Game.exe".into()])
        .unwrap();
    changed.save().unwrap();
    // Force a fresh stamp observation, independently of filesystem clock granularity.
    engine.apps_read = None;
    let reports = engine.set_foreground(Some(r"C:\One\Game.exe".into()));
    assert_eq!(reports[0].profile, "Two");
    assert_eq!(sim.state().dpi, Some((3200, 3200)));
    assert_eq!(Store::load(&path).unwrap().devices[&key].active, "Default");
    let valid = std::fs::read(&path).unwrap();
    let observed = engine.foreground.clone();
    std::fs::write(&path, b"not json").unwrap();
    engine.apps_read = None;
    sim.state().log.clear();
    assert!(engine.set_foreground(None).is_empty());
    assert_eq!(engine.foreground, observed);
    assert!(engine.apps_read.is_none());
    assert!(sim.state().log.is_empty());
    std::fs::write(&path, &valid).unwrap();
    let reports = engine.set_foreground(None);
    assert_eq!(reports[0].profile, "Default");
    assert_eq!(sim.state().dpi, Some((800, 800)));
    engine.enforce_measurement_read_only().unwrap();
    sim.state().log.clear();
    let before = std::fs::read(&path).unwrap();
    assert!(engine.set_foreground(None).is_empty());
    assert!(sim.state().log.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(folder).unwrap();
}
