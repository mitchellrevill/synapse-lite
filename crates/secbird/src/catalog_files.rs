//! Offline catalogue coverage: deliberately runs before HID initialization.
use secbird_core::{device_catalog::{Catalog, Resolution}, Db, Error};
use std::{collections::BTreeSet, io::Read};

pub fn run(args: &[String]) -> Result<(), Error> {
    let [command, file] = args else { return Err(Error::InvalidArg("usage: catalog inspect FILE".into())); };
    if command != "inspect" { return Err(Error::InvalidArg("usage: catalog inspect FILE".into())); }
    let mut input = std::fs::File::open(file).map_err(|e| Error::Db(format!("{file}: {e}")))?;
    let mut bytes = Vec::new();
    input.by_ref().take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)
        .map_err(|e| Error::Db(format!("{file}: {e}")))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| Error::Db(e.to_string()))?;
    let catalog = Catalog::parse(text)?;
    let db = Db::builtin()?;
    let mut connections = BTreeSet::new();
    let mut matching_primary = 0;
    for product in catalog.products() {
        connections.extend(product.connections.keys().copied());
        matching_primary += usize::from(db.get(product.id).is_some());
    }
    println!("Catalogue primary products: {}", catalog.products().count());
    println!("Distinct product/connection IDs: {}", connections.len());
    println!("Primary IDs with built-in protocol declarations: {matching_primary}");
    println!("Built-in USB ID declarations (including legacy devices): {}", db.len());
    for id in connections {
        if let Resolution::Ambiguous(products) = catalog.resolve(id) {
            println!("Ambiguous connection {id} (0x{id:04X}): {products:?}");
        }
    }
    println!("Metadata only: artwork, individual features and hardware operation require separate verification.");
    Ok(())
}
