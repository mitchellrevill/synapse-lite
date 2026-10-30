//! Owned process fixture. No file writes, devices, environment writes or network.
#![cfg_attr(windows, windows_subsystem = "windows")]
fn main() {
    let code = if std::env::args().any(|arg| arg == "--expect-temp-working-directory") {
        match (
            std::env::current_dir().and_then(|p| p.canonicalize()),
            std::env::temp_dir().canonicalize(),
        ) {
            (Ok(actual), Ok(expected)) if actual == expected => 42,
            _ => 11,
        }
    } else {
        0
    };
    std::thread::sleep(std::time::Duration::from_millis(100));
    std::process::exit(code);
}
