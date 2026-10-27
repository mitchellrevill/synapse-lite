//! Read-only driver diagnostics isolated from the interactive controller.
//!
//! A driver request can block in kernel mode. The parent bounds its own wait
//! and requests child termination on timeout; it does not promise that a broken
//! kernel driver will complete cancellation. No request buffers live in parent.

use secbird_core::driver_control::{self, InputHookReader, ProbeStop, MAX_HOOK_RECORDS};
use secbird_core::Error;
use serde_json::{json, Value};
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MAX_OUTPUT: u64 = 1024 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

enum Operation {
    List,
    Probe { interface: usize, limit: usize },
}

fn parse(args: &[String]) -> Result<Operation, Error> {
    let bad = || {
        Error::InvalidArg(
            "usage: driver-control list | driver-control probe <interface-index> [limit 1..64]"
                .into(),
        )
    };
    match args {
        [action] if action == "list" => Ok(Operation::List),
        [action, interface] | [action, interface, _] if action == "probe" => {
            let interface = interface.parse::<usize>().map_err(|_| bad())?;
            let limit = args
                .get(2)
                .map(|s| s.parse::<usize>())
                .transpose()
                .map_err(|_| bad())?
                .unwrap_or(1);
            if !(1..=MAX_HOOK_RECORDS).contains(&limit) {
                return Err(bad());
            }
            Ok(Operation::Probe { interface, limit })
        }
        _ => Err(bad()),
    }
}

fn system(error: impl std::fmt::Display) -> Error {
    Error::System(error.to_string())
}

pub fn run(args: &[String], child: bool) -> Result<(), Error> {
    let operation = parse(args)?;
    let report = if child {
        inspect(operation)?
    } else {
        supervise(args)?
    };
    println!("{}", serde_json::to_string_pretty(&report).map_err(system)?);
    Ok(())
}

fn supervise(args: &[String]) -> Result<Value, Error> {
    let mut child = Command::new(std::env::current_exe().map_err(system)?)
        .arg("--driver-control-child")
        .args(args)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(system)?;
    let output = child
        .stdout
        .take()
        .ok_or_else(|| system("Probe stdout was not captured"))?;
    // Drain concurrently so a complete bounded report cannot block on pipe capacity.
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        output
            .take(MAX_OUTPUT + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let bytes = reader
                    .join()
                    .map_err(|_| system("Probe output reader failed"))?
                    .map_err(system)?;
                if bytes.len() > MAX_OUTPUT as usize {
                    return Err(system("Driver probe output exceeded 1 MiB"));
                }
                if !status.success() {
                    return Err(system(format!(
                        "Read-only driver probe exited with {status}"
                    )));
                }
                return serde_json::from_slice(&bytes).map_err(system);
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                // Do not wait/join indefinitely for a kernel request to cancel.
                let termination = child.kill().err();
                return Err(system(format!(
                    "Read-only driver probe exceeded 5 seconds; child termination requested{}",
                    termination.map(|e| format!(": {e}")).unwrap_or_default()
                )));
            }
            Err(error) => {
                let _ = child.kill();
                return Err(system(error));
            }
        }
    }
}

fn inspect(operation: Operation) -> Result<Value, Error> {
    let interfaces = driver_control::enumerate_control_interfaces().map_err(system)?;
    match operation {
        Operation::List => Ok(json!({
            "mode":"read-only", "interface_count":interfaces.len(),
            "interfaces": interfaces.iter().enumerate().map(|(index, interface)| json!({
                "index":index, "product_id":format!("{:04X}", interface.product_id()),
                "interface_number":interface.interface_number(), "query_supported":interface.query_supported()
            })).collect::<Vec<_>>()
        })),
        Operation::Probe {
            interface: index,
            limit,
        } => {
            let interface = interfaces.get(index).ok_or_else(|| {
                Error::InvalidArg(
                    "Interface index is not present; run driver-control list again".into(),
                )
            })?;
            let mut reader = InputHookReader::open(interface).map_err(system)?;
            let probe = reader.read_prefix(limit).map_err(system)?;
            let records: Vec<_> = probe.records.iter().enumerate().map(|(index, record)| json!({
                "index":index, "routing_field":record.routing_field(), "kind":record.kind(),
                "keyboard":record.keyboard().map(|key| json!({ "unit_id":key.unit_id, "make_code":key.make_code,
                    "flags":key.flags, "reserved":key.reserved, "extra_information":key.extra_information })),
                "mouse":record.mouse().map(|mouse| json!({ "unit_id":mouse.unit_id, "flags":mouse.flags,
                    "button_flags":mouse.button_flags, "button_data":mouse.button_data, "raw_buttons":mouse.raw_buttons,
                    "last_x":mouse.last_x, "last_y":mouse.last_y, "extra_information":mouse.extra_information })),
                "raw_hex":record.raw().iter().map(|byte| format!("{byte:02x}")).collect::<String>()
            })).collect();
            let stop = match probe.stop {
                ProbeStop::LimitReached => json!({"reason":"limit-reached", "complete":false}),
                ProbeStop::QueryFailed { index, error } => {
                    json!({"reason":"query-failed", "index":index, "os_error":error.raw_os_error(), "message":error.to_string(), "complete":false})
                }
            };
            Ok(
                json!({"mode":"read-only", "interface_index":index, "product_id":format!("{:04X}",interface.product_id()),
                "interface_number":interface.interface_number(), "requested_limit":limit, "records":records, "stop":stop}),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_limits_and_extra_arguments_are_rejected_before_spawning() {
        for args in [
            vec!["probe", "0", "0"],
            vec!["probe", "0", "65"],
            vec!["probe", "-1"],
            vec!["list", "extra"],
            vec!["probe", "0", "1", "extra"],
        ] {
            assert!(parse(&args.into_iter().map(str::to_string).collect::<Vec<_>>()).is_err());
        }
        assert!(matches!(
            parse(&["probe".into(), "0".into()]).unwrap(),
            Operation::Probe {
                interface: 0,
                limit: 1
            }
        ));
    }
}
