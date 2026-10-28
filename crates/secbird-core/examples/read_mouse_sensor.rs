//! Bounded GET-only inspection of the source-declared196/197 sensor registers.
//! No profiles, drivers, input redirection or SET requests. No serial/path output.
use secbird_core::{
    device::{candidates, TXID_CANDIDATES},
    hidapi::HidApi,
    proto::Command,
    Db, Device,
};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api = HidApi::new()?;
    let db = Db::builtin()?;
    let mut candidates: Vec<_> = candidates(&api)
        .into_iter()
        .filter(|c| matches!(c.pid, 196 | 197))
        .collect();
    candidates.sort_by_key(|c| {
        db.get(c.pid)
            .and_then(|s| s.interface)
            .is_none_or(|i| i != c.interface)
    });
    let mut found = None;
    for c in candidates {
        let Ok(handle) = api.open_path(&c.path) else {
            continue;
        };
        let spec = db.get(c.pid).cloned();
        let txids = spec
            .as_ref()
            .and_then(|s| s.txid)
            .map(|t| vec![t])
            .unwrap_or_else(|| TXID_CANDIDATES.to_vec());
        if let Some(mut device) =
            Device::open(Box::new(handle), c.pid, c.interface, c.path, &txids, spec)
        {
            device.enforce_query_only();
            found = Some(device);
            break;
        }
    }
    let device = found.ok_or("No196/197 protocol interface answered the serial GET")?;
    let mut rows = Vec::new();
    for (name, command, size, args) in [
        (
            "proximity_state",
            Command::new(0x0b, 0x83),
            3,
            vec![0, 4, 0],
        ),
        (
            "lift_setting",
            Command::new(0x0b, 0x8b),
            4,
            vec![0, 4, 0, 0],
        ),
        ("dynamic_state", Command::new(0x0b, 0x90), 2, vec![1, 0]),
        ("dynamic_mode", Command::new(0x0b, 0x91), 2, vec![1, 0]),
    ] {
        let row = match device.get(command, size, &args) {
            Ok(reply) => {
                json!({"name":name,"command":command.to_string(),"size":reply.0[5],"args":&reply.args()[..size as usize],"status":"success"})
            }
            Err(error) => {
                json!({"name":name,"command":command.to_string(),"status":error.to_string()})
            }
        };
        rows.push(row);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"pid":format!("{:04X}",device.pid),"interface":device.interface,"txid":device.txid,"firmware":device.firmware_version().ok(),"query_only":device.is_query_only(),"registers":rows})
        )?
    );
    Ok(())
}
