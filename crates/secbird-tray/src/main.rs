//! Secretarybird tray app: re-applies each device's profile when it is plugged in or the
//! PC wakes, and switches profiles from the tray menu. Plain Win32, no GUI toolkit.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod apps;
#[cfg(windows)]
mod engine;
#[cfg(windows)]
mod lighting_runtime;
#[cfg(windows)]
mod host_static;
#[cfg(windows)]
mod host_uniform;
#[cfg(windows)]
mod host_wave;
#[cfg(windows)]
mod uniform_worker;
#[cfg(windows)]
mod lighting_status;
#[cfg(windows)]
mod osd;
#[cfg(windows)]
mod native_editor;
#[cfg(windows)]
mod server;
#[cfg(windows)]
mod settings;
#[cfg(windows)]
mod runtime_probe;
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = runtime_probe::duration(&args) {
        eprintln!("{error}");
        std::process::exit(2);
    }
    if let Err(error) = runtime_probe::read_only_duration(&args) {
        eprintln!("{error}");
        std::process::exit(2);
    }
    if let Err(error) = runtime_probe::wave_duration(&args) {
        eprintln!("{error}");
        std::process::exit(2);
    }
    if let Some(index) = args.iter().position(|arg| arg == "--native-smoke") {
        let seconds = args.get(index + 1).and_then(|s| s.parse::<u64>().ok()).filter(|s| (1..=600).contains(s));
        let Some(seconds) = seconds else {
            eprintln!("--native-smoke requires 1..600 seconds");
            std::process::exit(2);
        };
        let mut engine = match engine::Engine::new() {
            Ok(engine) => engine,
            Err(error) => { eprintln!("{error}"); std::process::exit(1); }
        };
        engine.rescan();
        if let Err(error) = native_editor::run_standalone(
            std::sync::Arc::new(std::sync::Mutex::new(engine)),
            Some(std::time::Duration::from_secs(seconds)),
        ) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    // Development: serve the editor only (no tray icon, no single-instance lock, nothing
    // applied), print its URL and keep running. Used to look at the editor beside a running
    // tray, e.g. with SECBIRD_SIM devices. Debug builds have a console for the URL.
    if std::env::args().any(|a| a == "--serve") {
        return serve();
    }
    win::run();
}

#[cfg(windows)]
fn serve() {
    let mut engine = match engine::Engine::new() {
        Ok(e) => e,
        Err(e) => return eprintln!("could not start HID access: {e}"),
    };
    engine.rescan();
    match server::start(std::sync::Arc::new(std::sync::Mutex::new(engine))) {
        Ok(editor) => {
            println!("{}", editor.url);
            loop {
                std::thread::park();
            }
        }
        Err(e) => eprintln!("could not start the editor: {e}"),
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("secbird-tray only runs on Windows");
}
