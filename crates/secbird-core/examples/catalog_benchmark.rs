//! Repeatable, metadata-only benchmark. No device discovery or protocol calls.
use secbird_core::device_catalog::{Catalog, Resolution};
use std::{collections::BTreeSet, hint::black_box, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Expected catalogue JSON path")?;
    let text = std::fs::read_to_string(path)?;
    let catalog = Catalog::parse(&text)?;
    let products = catalog.products().count();
    let connections: BTreeSet<u16> = catalog
        .products()
        .flat_map(|product| product.connections.keys().copied())
        .collect();
    let mut queries: Vec<u16> = connections.iter().copied().collect();
    queries.extend(
        (0..=u16::MAX)
            .filter(|id| !connections.contains(id))
            .take(128),
    );
    let passes = 500;
    let mut parse_ns = Vec::new();
    let mut lookup_ns = Vec::new();
    let mut checksum = 0u64;
    for _ in 0..3 {
        for id in &queries {
            black_box(catalog.resolve(black_box(*id)));
        }
    }
    for _ in 0..9 {
        let started = Instant::now();
        for _ in 0..25 {
            black_box(Catalog::parse(black_box(&text))?);
        }
        parse_ns.push(started.elapsed().as_nanos() as f64 / 25.);
        let started = Instant::now();
        for _ in 0..passes {
            for id in &queries {
                checksum =
                    checksum.wrapping_add(match black_box(catalog.resolve(black_box(*id))) {
                        Resolution::Unknown => 0,
                        Resolution::Unique(id) => u64::from(id),
                        Resolution::Ambiguous(ids) => ids.iter().map(|id| u64::from(*id)).sum(),
                    });
            }
        }
        lookup_ns.push(started.elapsed().as_nanos() as f64 / (passes * queries.len()) as f64);
    }
    println!(
        "{}",
        serde_json::json!({
            "catalogue_bytes":text.len(),"products":products,"connections":connections.len(),
            "queries_per_pass":queries.len(),"passes_per_sample":passes,"samples":9,
            "parse_ns_per_catalogue":parse_ns,"lookup_ns_per_query":lookup_ns,"checksum":checksum,
            "build":"workspace release profile; metadata only; no hardware IO"
        })
    );
    Ok(())
}
