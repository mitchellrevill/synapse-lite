//! Read-only protocol facts; never invent a snapshot of the user's held keys.
use secbird_core::{
    native_paste_plan as paste,
    source_macro::{Event, HostAction},
};
use serde_json::{json, Value};

pub(super) fn describe(events: &[Event]) -> Option<Value> {
    let indexes:Vec<_>=events.iter().enumerate().filter_map(|(i,event)|{
        matches!(event,Event::Host(action) if matches!(action.as_ref(),HostAction::ClipboardText(_))).then_some(i)
    }).collect();
    if indexes.is_empty() {
        return None;
    }
    Some(json!({
        "native_text_event_indexes_zero_based":indexes,
        "sampled_virtual_keys_in_source_order":paste::SAMPLED_VIRTUAL_KEYS,
        "source_key_state_snapshot":null,
        "requires_modifier_suspend_and_restore":true,
        "paste_chord_execution_order":paste::PASTE_CHORD.iter().map(|step|json!({
            "scan_code":step.scan_code,"flags":step.flags,"route":format!("{:?}",step.route),
            "source_delay_argument":step.delay_argument,
            "source_runner_delay_on_callback_true":step.source_runner_delay(true),
            "source_runner_delay_on_callback_false":step.source_runner_delay(false)
        })).collect::<Vec<_>>(),
        "snapshot_modifier_route":"EndpointElseOsFallback",
        "runner_duration_unit_verified":false,
        "runtime_gaps":paste::RUNTIME_GAPS.iter().map(|gap|format!("{gap:?}")).collect::<Vec<_>>(),
        "physical_state_verified":false,"snapshot_read":false,"clipboard_read":false,"clipboard_changed":false,
        "input_sent":false,"delivery_observed":false,"live_dispatch_available":false
    }))
}
