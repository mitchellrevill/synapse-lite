//! Public device catalogue metadata. Presence here does not enable hardware writes.
//!
//! Product IDs and connection IDs are separate: shared receivers may identify more
//! than one product. Callers must not silently select the first matching product.

use crate::{Error, Result};
use serde::Deserialize;
use std::collections::BTreeMap;

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_RECORDS: usize = 4096;
const MAX_ALIASES_PER_ROLE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role { Product, Dongle, Bluetooth, Wired, Xbox, Playstation, MonitorPort }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    pub id: u16,
    /// `None` means the catalogue did not make a declaration.
    pub not_chroma: Option<bool>,
    pub connections: BTreeMap<u16, Vec<Role>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Unknown,
    Unique(u16),
    Ambiguous(Vec<u16>),
}

#[derive(Debug)]
pub struct Catalog { products: BTreeMap<u16, Product> }

#[derive(Deserialize)]
#[serde(untagged)]
enum Ids { One(u16), Many(Vec<u16>) }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    product_id: u16,
    #[serde(default)] dongle_id: Option<Ids>,
    #[serde(default)] ble_id: Option<Ids>,
    #[serde(default)] wired_id: Option<Ids>,
    #[serde(default)] x_box_id: Option<Ids>,
    #[serde(default)] ps_mode_ids: Option<Ids>,
    #[serde(default)] monitor_port_ids: Option<Ids>,
    #[serde(default)] is_not_chroma_device: Option<bool>,
}

impl Catalog {
    /// Parse inert metadata with a fixed input budget. Duplicate primary records
    /// merge supplemental aliases; conflicting explicit declarations are errors.
    pub fn parse(text: &str) -> Result<Self> {
        let invalid = |message: &str| Error::Db(format!("Razer device catalogue: {message}"));
        if text.len() > MAX_BYTES { return Err(invalid("exceeds 2 MiB")); }
        let entries: Vec<Entry> = serde_json::from_str(text)
            .map_err(|e| invalid(&e.to_string()))?;
        if entries.len() > MAX_RECORDS { return Err(invalid("too many records")); }
        let mut products: BTreeMap<u16, Product> = BTreeMap::new();
        for entry in entries {
            let product = products.entry(entry.product_id).or_insert_with(|| Product {
                id: entry.product_id, not_chroma: None, connections: BTreeMap::new(),
            });
            if let Some(value) = entry.is_not_chroma_device {
                if product.not_chroma.is_some_and(|previous| previous != value) {
                    return Err(invalid("conflicting Chroma declarations for a product"));
                }
                product.not_chroma = Some(value);
            }
            add_role(product, entry.product_id, Role::Product);
            for (ids, role) in [
                (entry.dongle_id, Role::Dongle), (entry.ble_id, Role::Bluetooth),
                (entry.wired_id, Role::Wired), (entry.x_box_id, Role::Xbox),
                (entry.ps_mode_ids, Role::Playstation), (entry.monitor_port_ids, Role::MonitorPort),
            ] {
                let ids = match ids { None => continue, Some(Ids::One(id)) => vec![id], Some(Ids::Many(ids)) => ids };
                if ids.len() > MAX_ALIASES_PER_ROLE { return Err(invalid("too many connection IDs in one role")); }
                for id in ids { add_role(product, id, role); }
            }
        }
        Ok(Self { products })
    }

    pub fn products(&self) -> impl Iterator<Item = &Product> { self.products.values() }

    pub fn resolve(&self, connection_id: u16) -> Resolution {
        let candidates: Vec<u16> = self.products.values()
            .filter(|product| product.connections.contains_key(&connection_id))
            .map(|product| product.id).collect();
        match candidates.as_slice() {
            [] => Resolution::Unknown,
            [id] => Resolution::Unique(*id),
            _ => Resolution::Ambiguous(candidates),
        }
    }
}

fn add_role(product: &mut Product, id: u16, role: Role) {
    let roles = product.connections.entry(id).or_default();
    if !roles.contains(&role) { roles.push(role); roles.sort(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_connection_roles_and_supplemental_records_survive() {
        let catalog = Catalog::parse(r#"[
          {"productId":691,"dongleId":[692,692],"bleId":693,"wiredId":[694],"xBoxId":695,"psModeIds":[696],"monitorPortIds":[697]},
          {"productId":691,"isNotChromaDevice":false}, {"productId":700}
        ]"#).unwrap();
        assert_eq!(catalog.products().count(), 2);
        for id in 691..=697 { assert_eq!(catalog.resolve(id), Resolution::Unique(691)); }
        let first = catalog.products().next().unwrap();
        assert_eq!(first.not_chroma, Some(false));
        assert_eq!(first.connections[&692], [Role::Dongle]);
        assert_eq!(catalog.products().last().unwrap().not_chroma, None);
        assert_eq!(catalog.resolve(999), Resolution::Unknown);
    }

    #[test]
    fn shared_receivers_and_primary_alias_collisions_are_ambiguous() {
        for text in [
            r#"[{"productId":2595,"dongleId":2593},{"productId":2594,"dongleId":2593}]"#,
            r#"[{"productId":2593},{"productId":2594,"dongleId":2593}]"#,
        ] {
            let catalog = Catalog::parse(text).unwrap();
            assert!(matches!(catalog.resolve(2593), Resolution::Ambiguous(ids) if ids.len() == 2));
        }
        let catalog = Catalog::parse(r#"[{"productId":1,"dongleId":1}]"#).unwrap();
        assert_eq!(catalog.resolve(1), Resolution::Unique(1));
    }

    #[test]
    fn malformed_or_excessive_catalogues_fail() {
        for text in [r#"[{"productId":65536}]"#, r#"[{"productId":1,"bleId":[-1]}]"#,
            r#"[{"productId":1,"dongleId":"2"}]"#,
            r#"[{"productId":1,"isNotChromaDevice":true},{"productId":1,"isNotChromaDevice":false}]"#] {
            assert!(Catalog::parse(text).is_err());
        }
        assert!(Catalog::parse(&" ".repeat(MAX_BYTES + 1)).is_err());
        let records = format!("[{}]", vec![r#"{"productId":1}"#; MAX_RECORDS + 1].join(","));
        assert!(Catalog::parse(&records).is_err());
        let aliases = format!(r#"[{{"productId":1,"dongleId":[{}]}}]"#, vec!["2"; 65].join(","));
        assert!(Catalog::parse(&aliases).is_err());
    }
}
