//! Read-only, bounded installed-input capability probe. No setters or activation.
//! Usage: input_readback <new-report.json> --read-only [--mouse-only]
use secbird_core::{
    commands,
    device::{candidates, TXID_CANDIDATES},
    hidapi::HidApi,
    Db, Device,
};
use serde_json::{json, Value};
use std::{fs::OpenOptions, io::Write, time::Instant};

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(args.len() == 2 || args.len() == 3 && args[2] == "--mouse-only")
        || args[1] != "--read-only"
    {
        return Err("usage: input_readback <new-report.json> --read-only [--mouse-only]".into());
    }
    if std::env::var_os("SECBIRD_SIM").is_some() {
        return Err("Installed input probe refuses simulator configuration".into());
    }
    // Reserve the report before hardware access; never overwrite prior evidence.
    let mut report = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[0])
        .map_err(|_| "Fresh report path in an existing directory required")?;
    let api = HidApi::new().map_err(|_| "Cannot initialize real HID transport")?;
    let db = Db::builtin().map_err(|_| "Cannot load builtin device metadata")?;
    let found = candidates(&api);
    let mut devices = Vec::new();
    for (pid, interface, input, command) in [
        (605, 2, 31, commands::KEY), // Razer physical key31, A; not HID usage31.
        (196, 0, 4, commands::BUTTON), // Side-back input in the upstream table.
    ] {
        if args.len() == 3 && pid != 196 {
            continue;
        }
        let matching: Vec<_> = found
            .iter()
            .filter(|c| {
                c.pid == pid
                    && c.interface == interface
                    && api.device_list().any(|info| {
                        info.path() == c.path.as_c_str()
                            && info.usage_page() == 1
                            && info.usage() == 2
                    })
            })
            .collect();
        if matching.len() != 1 {
            devices.push(json!({"pid":pid,"interface":interface,"candidate_count":matching.len(),"status":"not-tested"}));
            continue;
        }
        let candidate = matching[0];
        let spec = db.get(pid).cloned();
        let mut txids = Vec::new();
        if let Some(txid) = spec.as_ref().and_then(|s| s.txid) {
            txids.push(txid);
        }
        for txid in TXID_CANDIDATES {
            if !txids.contains(&txid) {
                txids.push(txid);
            }
        }
        let device = api.open_path(&candidate.path).ok().and_then(|hid| {
            Device::open(
                Box::new(hid),
                pid,
                interface,
                candidate.path.clone(),
                &txids,
                spec,
            )
        });
        let Some(device) = device else {
            devices.push(json!({"pid":pid,"interface":interface,"status":"discovery-failed"}));
            continue;
        };
        let mut queries = Vec::<Value>::new();
        for slot in [0, 1] {
            for layer in [0, 1] {
                let started = Instant::now();
                let result = device.get(command, 10, &[slot, input, layer]);
                let observation = match result {
                    Ok(reply) => json!({"status":"success","declared_size":reply.0[5],
                        "declared_args":&reply.args()[..10],
                        "selectors_match":reply.args()[..3]==[slot,input,layer]}),
                    Err(secbird_core::Error::Device { status, .. }) => {
                        json!({"status":"device-rejected","device_status":format!("{status:?}")})
                    }
                    Err(_) => json!({"status":"transport-or-framing-failure"}),
                };
                queries.push(json!({"slot":slot,"input":input,"layer":layer,
                    "command_class":command.class,"get_command_id":command.get().id,
                    "elapsed_ms":started.elapsed().as_millis(),"observation":observation}));
            }
        }
        devices.push(json!({"pid":pid,"interface":interface,"status":"queried","queries":queries}));
    }
    let result = json!({"scope":"installed025D/MI02 and/or wired00C4/MI00, usage1/2; one physical input each; slots0/1 and layers0/1",
        "readonly":true,"setters_called":false,"profiles_or_drivers_changed":false,
        "remapping_or_recovery_verified":false,"device_serials_or_paths_included":false,"devices":devices});
    report
        .write_all(&serde_json::to_vec_pretty(&result).map_err(|_| "Cannot serialize report")?)
        .map_err(|_| "Cannot write input report")?;
    report.sync_all().map_err(|_| "Cannot sync input report")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
