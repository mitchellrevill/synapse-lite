//! Whole-sync preflight, including conservative bounds for partial success.
use super::*;
use serde_json::Value;

pub(super) enum Change {
    Color(Rgb),
    Effect(crate::effects::Effect),
}
impl Change {
    fn project(&self, store: &mut Store, dev: &Device, name: &str) {
        store.for_device(dev).active_mut().keys.clear();
        let target = zone(store, dev, name);
        match self {
            Self::Color(color) => {
                target.color = Some(*color);
                target.effect = None;
            }
            Self::Effect(effect) => target.effect = Some(effect.clone()),
        }
        target.off = false;
    }
    fn apply(&self, dev: &Device, name: &str) -> Result<()> {
        check_firmware_lighting(dev)?;
        let led = dev.led_id(name)?;
        match self {
            Self::Color(color) => dev.set_static_color(led, *color),
            Self::Effect(effect) => dev.set_effect(led, effect),
        }
    }
}
fn nodes(value: &Value) -> usize {
    1 + match value {
        Value::Object(map) => map.len() + map.values().map(nodes).sum::<usize>(),
        Value::Array(values) => values.iter().map(nodes).sum(),
        _ => 0,
    }
}
fn cost(value: &Value, by_nodes: bool) -> usize {
    if by_nodes {
        nodes(value)
    } else {
        serde_json::to_vec(value)
            .expect("JSON Value serializes")
            .len()
    }
}
fn upper_bound(old: &Value, new: &Value, by_nodes: bool) -> Value {
    if old == new {
        return old.clone();
    }
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => {
            let mut result = a.clone();
            for (key, value) in b {
                result.insert(
                    key.clone(),
                    match a.get(key) {
                        Some(old) => upper_bound(old, value, by_nodes),
                        None => value.clone(),
                    },
                );
            }
            Value::Object(result)
        }
        // Arrays are whole values here: keys are retained or cleared, never
        // independently edited by a sync operation. Scalar types are atomic too.
        _ => {
            if cost(new, by_nodes) > cost(old, by_nodes) {
                new.clone()
            } else {
                old.clone()
            }
        }
    }
}
fn validate_partial_bounds(base: &Store, complete: &Store) -> Result<()> {
    base.validate_save()?;
    complete.validate_save()?;
    let old = serde_json::to_value(base).map_err(|e| crate::Error::Db(e.to_string()))?;
    let new = serde_json::to_value(complete).map_err(|e| crate::Error::Db(e.to_string()))?;
    for by_nodes in [false, true] {
        let bound: Store = serde_json::from_value(upper_bound(&old, &new, by_nodes))
            .map_err(|e| crate::Error::Db(e.to_string()))?;
        bound.validate_save()?;
    }
    Ok(())
}
pub(super) fn run(devices: &[Device], store: &mut Store, change: Change) -> Vec<Synced> {
    let mut base = store.clone();
    let mut plans = Vec::new();
    for (index, dev) in devices.iter().enumerate() {
        if base.for_device(dev).lighting_off {
            continue;
        }
        for led in dev.spec.iter().flat_map(|spec| &spec.leds) {
            plans.push((index, led.name.clone()));
        }
    }
    let mut complete = base.clone();
    for (index, name) in &plans {
        change.project(&mut complete, &devices[*index], name);
    }
    // Include the caller's unresolved keys: a failed device must keep its
    // original alias while another device can successfully migrate its key.
    let validation = base
        .validate_save()
        .and_then(|_| validate_partial_bounds(store, &complete));
    if let Err(error) = validation {
        return plans
            .into_iter()
            .map(|(index, zone)| {
                let dev = &devices[index];
                Synced {
                    pid: dev.pid,
                    key: dev.key(),
                    device: dev.name(),
                    zone,
                    result: Err(crate::Error::InvalidArg(format!(
                        "sync resource preflight: {error}"
                    ))),
                }
            })
            .collect();
    }
    let mut candidate = store.clone();
    let mut results = Vec::new();
    for (index, name) in plans {
        let dev = &devices[index];
        let result = change.apply(dev, &name);
        if result.is_ok() {
            change.project(&mut candidate, dev, &name);
        }
        results.push(Synced {
            pid: dev.pid,
            key: dev.key(),
            device: dev.name(),
            zone: name,
            result,
        });
    }
    if results.iter().any(|outcome| outcome.result.is_ok()) {
        match candidate.save() {
            Ok(()) => *store = candidate,
            Err(error) => {
                for outcome in results.iter_mut().filter(|outcome| outcome.result.is_ok()) {
                    outcome.result =
                        Err(crate::Error::System(format!("set, but not saved: {error}")));
                }
            }
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_and_node_bounds_choose_different_atomic_values() {
        let old = serde_json::json!({"opaque": "x".repeat(2000)});
        let new = serde_json::json!({"opaque": vec![Value::Null; 300]});
        let bytes = upper_bound(&old, &new, false);
        let structure = upper_bound(&old, &new, true);
        assert_eq!(bytes, old);
        assert_eq!(structure, new);
        for candidate in [&old, &new] {
            assert!(cost(&bytes, false) >= cost(candidate, false));
            assert!(nodes(&structure) >= nodes(candidate));
        }
    }
}
