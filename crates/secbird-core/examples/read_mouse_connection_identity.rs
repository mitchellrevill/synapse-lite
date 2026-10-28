//! Compare the two known connection identities using GETs only. No identity text,
//! paths, profiles or device SETs are emitted or changed.
use secbird_core::{hidapi::HidApi, Db, Device};
use serde_json::json;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api = HidApi::new()?;
    let db = Db::builtin()?;
    let mut rows = Vec::new();
    let mut identities = Vec::new();
    for pid in [196, 197] {
        let candidates: Vec<_> = api
            .device_list()
            .filter(|c| {
                c.vendor_id() == 0x1532
                    && c.product_id() == pid
                    && c.interface_number() == 0
                    && c.usage_page() == 1
                    && c.usage() == 2
            })
            .collect();
        if candidates.len() != 1 {
            rows.push(json!({"pid":format!("{pid:04X}"),"eligible_collections":candidates.len(),"serial_get_answered":false}));
            continue;
        }
        let info = candidates[0];
        let opened = api.open_path(info.path()).ok().and_then(|handle| {
            Device::open(
                Box::new(handle),
                pid,
                0,
                info.path().to_owned(),
                &[31],
                db.get(pid).cloned(),
            )
        });
        if let Some(mut device) = opened {
            device.enforce_query_only();
            let usable = !device.serial.is_empty();
            rows.push(json!({"pid":format!("{pid:04X}"),"eligible_collections":1,"serial_get_answered":true,"nonempty_serial":usable,"query_only":device.is_query_only()}));
            if usable {
                identities.push(device.serial.clone());
            }
        } else {
            rows.push(json!({"pid":format!("{pid:04X}"),"eligible_collections":1,"serial_get_answered":false}));
        }
    }
    let matched = (identities.len() == 2).then(|| identities[0] == identities[1]);
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"connections":rows,"two_nonempty_protocol_serials_match":matched,"set_count":0,"profile_changes":false,"serials_or_paths_emitted":false})
        )?
    );
    Ok(())
}
