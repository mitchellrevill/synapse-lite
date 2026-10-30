//! Pure explicit Ornata logical-to-physical RGB mapping.
//!
//! Static native30B0 builds an ordered physical->logical index table;5580
//! clears the physical RGB frame then copies mapped triples. No device access,
//! layout discovery, scheduling or transport is included. See
//! docs/architecture.md for provenance and scope.

use crate::Rgb;
use std::fmt;

pub const LOGICAL_ROWS: usize = 7;
pub const LOGICAL_COLS: usize = 23;
pub const LOGICAL_PIXELS: usize = LOGICAL_ROWS * LOGICAL_COLS;
pub const PHYSICAL_ROWS: usize = 6;
pub const PHYSICAL_COLS: usize = 22;
pub const PHYSICAL_PIXELS: usize = PHYSICAL_ROWS * PHYSICAL_COLS;
pub const MAX_SOURCE_ENTRIES: usize = 256;
pub const UNMAPPED: u16 = u16::MAX;
pub const SOURCE_MANIFEST_SHA256: &str =
    "3595e11f1d8778180d1dd6edd2463c8b3b8e446542c5020dc1fa038c67937a93";
const SOURCE_LAYOUTS: [u16; 17] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15, 16, 17, 18, 20];

/// Explicit caller identity. No locale inference or historical-observation lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutIdentity {
    pub vendor_id: u16,
    pub product_id: u16,
    pub edition_id: u16,
    pub layout_id: u16,
}

/// Source MatrixPos and DevicePos are row,column pairs, not x,y pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappingEntry {
    pub logical: [u16; 2],
    pub physical: [u16; 2],
}

/// Bounded projection supplied by a caller that resolves source assets.
///
/// The digest is a declaration checked against the recovered artifact identity.
/// This pure boundary does not read/hash the whole manifest or authenticate its
/// caller's coordinate projection. Asset verification belongs to its producer.
#[derive(Debug, Clone, Copy)]
pub struct SourceLayoutRecord<'a> {
    pub identity: LayoutIdentity,
    pub logical_dimensions: [u16; 2],
    pub physical_dimensions: [u16; 2],
    pub entries: &'a [MappingEntry],
    pub manifest_sha256: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutProvenance {
    pub identity: LayoutIdentity,
    pub declared_manifest_sha256: &'static str,
    pub source_entry_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpatialError {
    UnsupportedIdentity(LayoutIdentity),
    IdentityMismatch {
        expected: LayoutIdentity,
        actual: LayoutIdentity,
    },
    ManifestDigestMismatch,
    UnsupportedDimensions {
        logical: [u16; 2],
        physical: [u16; 2],
    },
    EntryBudget(usize),
    CoordinateOutsideGrid {
        entry: usize,
        logical: [u16; 2],
        physical: [u16; 2],
    },
    LogicalFrameSize(usize),
    PhysicalFrameSize(usize),
}
impl fmt::Display for SpatialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedIdentity(id) => write!(f, "unsupported Ornata source layout {id:?}"),
            Self::IdentityMismatch { expected, actual } => write!(
                f,
                "layout identity mismatch: expected {expected:?}, received {actual:?}"
            ),
            Self::ManifestDigestMismatch => {
                f.write_str("declared manifest digest does not match the recovered Ornata source")
            }
            Self::UnsupportedDimensions { logical, physical } => write!(
                f,
                "unsupported source dimensions {logical:?} to {physical:?}"
            ),
            Self::EntryBudget(n) => write!(
                f,
                "source mapping requires 1..={MAX_SOURCE_ENTRIES} entries, received {n}"
            ),
            Self::CoordinateOutsideGrid {
                entry,
                logical,
                physical,
            } => write!(
                f,
                "mapping entry {entry} outside grid: logical {logical:?}, physical {physical:?}"
            ),
            Self::LogicalFrameSize(n) => write!(
                f,
                "logical frame requires {LOGICAL_PIXELS} pixels, received {n}"
            ),
            Self::PhysicalFrameSize(n) => write!(
                f,
                "physical frame requires {PHYSICAL_PIXELS} pixels, received {n}"
            ),
        }
    }
}
impl std::error::Error for SpatialError {}

/// Fixed storage:132 u16 indices plus bounded identity/provenance metadata.
/// No vendor mapping table is built into production code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrnataSpatialMap {
    provenance: LayoutProvenance,
    physical_to_logical: [u16; PHYSICAL_PIXELS],
    mapped_pixels: usize,
}

impl OrnataSpatialMap {
    pub fn from_record(
        expected: LayoutIdentity,
        record: SourceLayoutRecord<'_>,
    ) -> Result<Self, SpatialError> {
        if expected.vendor_id != 0x1532
            || expected.product_id != 0x025d
            || expected.edition_id != 0
            || !SOURCE_LAYOUTS.contains(&expected.layout_id)
        {
            return Err(SpatialError::UnsupportedIdentity(expected));
        }
        if record.identity != expected {
            return Err(SpatialError::IdentityMismatch {
                expected,
                actual: record.identity,
            });
        }
        if record.manifest_sha256 != SOURCE_MANIFEST_SHA256 {
            return Err(SpatialError::ManifestDigestMismatch);
        }
        if record.logical_dimensions != [7, 23] || record.physical_dimensions != [6, 22] {
            return Err(SpatialError::UnsupportedDimensions {
                logical: record.logical_dimensions,
                physical: record.physical_dimensions,
            });
        }
        if record.entries.is_empty() || record.entries.len() > MAX_SOURCE_ENTRIES {
            return Err(SpatialError::EntryBudget(record.entries.len()));
        }
        let mut indices = [UNMAPPED; PHYSICAL_PIXELS];
        for (at, entry) in record.entries.iter().enumerate() {
            let [row, col] = entry.logical;
            let [physical_row, physical_col] = entry.physical;
            if usize::from(row) >= LOGICAL_ROWS
                || usize::from(col) >= LOGICAL_COLS
                || usize::from(physical_row) >= PHYSICAL_ROWS
                || usize::from(physical_col) >= PHYSICAL_COLS
            {
                return Err(SpatialError::CoordinateOutsideGrid {
                    entry: at,
                    logical: entry.logical,
                    physical: entry.physical,
                });
            }
            // Native30B0 assigns source array order. Duplicate physical targets
            // overwrite the preceding logical index; no blending/dedup sorting.
            indices[usize::from(physical_row) * PHYSICAL_COLS + usize::from(physical_col)] =
                row * LOGICAL_COLS as u16 + col;
        }
        let mapped_pixels = indices.iter().filter(|i| **i != UNMAPPED).count();
        Ok(Self {
            provenance: LayoutProvenance {
                identity: expected,
                declared_manifest_sha256: SOURCE_MANIFEST_SHA256,
                source_entry_count: record.entries.len(),
            },
            physical_to_logical: indices,
            mapped_pixels,
        })
    }

    pub fn provenance(&self) -> LayoutProvenance {
        self.provenance
    }
    pub fn mapped_pixel_count(&self) -> usize {
        self.mapped_pixels
    }
    pub fn unmapped_pixel_count(&self) -> usize {
        PHYSICAL_PIXELS - self.mapped_pixels
    }
    /// Read-only indices into the161-pixel logical grid; UNMAPPED marks holes.
    pub fn physical_to_logical(&self) -> &[u16; PHYSICAL_PIXELS] {
        &self.physical_to_logical
    }

    /// All frame sizes are checked before output mutation. Each successful
    /// mapping starts black and preserves RGB channels and row-major ordering.
    pub fn map_frame_into(
        &self,
        logical: &[Rgb],
        physical: &mut [Rgb],
    ) -> Result<(), SpatialError> {
        if logical.len() != LOGICAL_PIXELS {
            return Err(SpatialError::LogicalFrameSize(logical.len()));
        }
        if physical.len() != PHYSICAL_PIXELS {
            return Err(SpatialError::PhysicalFrameSize(physical.len()));
        }
        physical.fill(Rgb(0, 0, 0));
        for (destination, index) in physical.iter_mut().zip(self.physical_to_logical.iter()) {
            if *index != UNMAPPED {
                *destination = logical[usize::from(*index)];
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn identity(layout: u16) -> LayoutIdentity {
        LayoutIdentity {
            vendor_id: 0x1532,
            product_id: 0x025d,
            edition_id: 0,
            layout_id: layout,
        }
    }
    fn records() -> Value {
        serde_json::from_str(include_str!("../tests/fixtures/ornata_spatial.json")).unwrap()
    }
    fn entries(case: &Value) -> Vec<MappingEntry> {
        case["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| MappingEntry {
                logical: [v[0].as_u64().unwrap() as u16, v[1].as_u64().unwrap() as u16],
                physical: [v[2].as_u64().unwrap() as u16, v[3].as_u64().unwrap() as u16],
            })
            .collect()
    }
    fn record<'a>(layout: u16, entries: &'a [MappingEntry]) -> SourceLayoutRecord<'a> {
        SourceLayoutRecord {
            identity: identity(layout),
            logical_dimensions: [7, 23],
            physical_dimensions: [6, 22],
            entries,
            manifest_sha256: SOURCE_MANIFEST_SHA256,
        }
    }
    fn ramp() -> Vec<Rgb> {
        (0..LOGICAL_PIXELS)
            .map(|i| Rgb(i as u8, (i / 23) as u8, (i % 23) as u8))
            .collect()
    }

    #[test]
    fn all17_source_records_match_independent_colored_frame_bytes() {
        let cases = records();
        let raw = include_bytes!("../tests/fixtures/ornata_spatial.rgb");
        assert_eq!(cases["layouts"].as_array().unwrap().len(), 17);
        for c in cases["layouts"].as_array().unwrap() {
            let layout = c["layout"].as_u64().unwrap() as u16;
            let e = entries(c);
            let map = OrnataSpatialMap::from_record(identity(layout), record(layout, &e)).unwrap();
            let mut out = [Rgb(9, 8, 7); PHYSICAL_PIXELS];
            map.map_frame_into(&ramp(), &mut out).unwrap();
            let bytes: Vec<u8> = out.iter().flat_map(|p| [p.0, p.1, p.2]).collect();
            let at = c["offset"].as_u64().unwrap() as usize;
            assert_eq!(
                bytes.as_slice(),
                &raw[at..at + PHYSICAL_PIXELS * 3],
                "layout{layout}"
            );
            assert_eq!(
                format!("{:x}", md5::compute(&bytes)),
                c["md5"].as_str().unwrap()
            );
            assert_eq!(
                map.mapped_pixel_count(),
                c["mapped"].as_u64().unwrap() as usize
            );
            assert_eq!(map.provenance().identity, identity(layout));
            assert_eq!(map.provenance().source_entry_count, e.len());
            assert_eq!(
                map.provenance().declared_manifest_sha256,
                SOURCE_MANIFEST_SHA256
            );
        }
    }

    #[test]
    fn uk_has114_leds_and18_black_holes_every_frame() {
        let data = records();
        let c = data["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["layout"] == 6)
            .unwrap();
        let e = entries(c);
        let map = OrnataSpatialMap::from_record(identity(6), record(6, &e)).unwrap();
        assert_eq!(
            (map.mapped_pixel_count(), map.unmapped_pixel_count()),
            (114, 18)
        );
        assert_eq!(map.physical_to_logical()[3], 4); // logical0,4 -> physical0,3
        let holes = [
            0, 2, 58, 81, 82, 83, 87, 101, 103, 105, 114, 115, 116, 118, 119, 120, 128, 131,
        ];
        let mut out = [Rgb(99, 88, 77); PHYSICAL_PIXELS];
        map.map_frame_into(&[Rgb(16, 32, 48); LOGICAL_PIXELS], &mut out)
            .unwrap();
        for (i, pixel) in out.iter().enumerate() {
            assert_eq!(
                *pixel,
                if holes.contains(&i) {
                    Rgb(0, 0, 0)
                } else {
                    Rgb(16, 32, 48)
                }
            );
        }
        map.map_frame_into(&[Rgb(1, 2, 3); LOGICAL_PIXELS], &mut out)
            .unwrap();
        for i in holes {
            assert_eq!(out[i], Rgb(0, 0, 0));
        }
    }

    #[test]
    fn layout12_collisions_keep_last_source_destination_assignment() {
        let data = records();
        let c = data["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["layout"] == 12)
            .unwrap();
        let e = entries(c);
        let map = OrnataSpatialMap::from_record(identity(12), record(12, &e)).unwrap();
        assert_eq!(e.len(), 119);
        assert_eq!(map.mapped_pixel_count(), 117);
        // Source entries19→34 targetphysical0,21;57→79 targetphysical3,14.
        assert_eq!(e[19].logical, [0, 22]);
        assert_eq!(e[34].logical, [1, 14]);
        assert_eq!(map.physical_to_logical()[21], 37);
        assert_eq!(e[57].logical, [2, 14]);
        assert_eq!(e[79].logical, [3, 15]);
        assert_eq!(map.physical_to_logical()[3 * 22 + 14], 84);
    }

    #[test]
    fn source_horizontal_wave_composition_matches_independent_physical_cycles() {
        use crate::wave_lighting::{CardinalDirection, WaveCycle, WaveRegion};
        let data = records();
        let c = data["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["layout"] == 6)
            .unwrap();
        let entries = entries(c);
        let map = OrnataSpatialMap::from_record(identity(6), record(6, &entries)).unwrap();
        // UK has no physical destination for logical padding row6.
        assert!(map
            .physical_to_logical()
            .iter()
            .all(|i| *i == UNMAPPED || *i < 6 * 23));
        let raw = include_bytes!("../tests/fixtures/ornata_spatial_wave.rgb");
        for case in data["wave_cases"].as_array().unwrap() {
            let direction = match case["direction"].as_str().unwrap() {
                "left" => CardinalDirection::Left,
                "right" => CardinalDirection::Right,
                _ => panic!("fixture direction"),
            };
            let region = WaveRegion::new(7, 23, 0, 0, 23, 7).unwrap();
            let mut wave = WaveCycle::source_quick(region, direction).unwrap();
            assert_eq!(wave.frame_count(), 25);
            let mut logical = [Rgb(0, 0, 0); LOGICAL_PIXELS];
            let mut physical = [Rgb(8, 9, 10); PHYSICAL_PIXELS];
            let offset = case["offset"].as_u64().unwrap() as usize;
            for frame in 0..wave.frame_count() {
                wave.next_frame(&mut logical).unwrap();
                map.map_frame_into(&logical, &mut physical).unwrap();
                let bytes: Vec<u8> = physical.iter().flat_map(|p| [p.0, p.1, p.2]).collect();
                let start = offset + frame * PHYSICAL_PIXELS * 3;
                assert_eq!(
                    bytes.as_slice(),
                    &raw[start..start + PHYSICAL_PIXELS * 3],
                    "direction {direction:?}, frame {frame}"
                );
                assert_eq!(physical.iter().filter(|p| **p == Rgb(0, 0, 0)).count(), 18);
                let before = physical;
                logical[6 * 23..].fill(Rgb(255, 255, 255));
                map.map_frame_into(&logical, &mut physical).unwrap();
                assert_eq!(physical, before);
            }
            assert_eq!(wave.next_frame_index(), 0);
        }
    }

    #[test]
    fn identity_source_and_dimension_mismatches_never_fallback() {
        let e = [MappingEntry {
            logical: [0, 4],
            physical: [0, 3],
        }];
        let id = identity(6);
        for changed in [
            LayoutIdentity { layout_id: 0, ..id },
            LayoutIdentity {
                layout_id: 13,
                ..id
            },
            LayoutIdentity {
                edition_id: 1,
                ..id
            },
            LayoutIdentity {
                product_id: 0x00c5,
                ..id
            },
            LayoutIdentity { vendor_id: 0, ..id },
        ] {
            assert!(matches!(
                OrnataSpatialMap::from_record(changed, record(6, &e)),
                Err(SpatialError::UnsupportedIdentity(_))
            ));
        }
        assert!(matches!(
            OrnataSpatialMap::from_record(id, record(1, &e)),
            Err(SpatialError::IdentityMismatch { .. })
        ));
        let mut r = record(6, &e);
        r.manifest_sha256 = "other-source";
        assert_eq!(
            OrnataSpatialMap::from_record(id, r),
            Err(SpatialError::ManifestDigestMismatch)
        );
        let mut r = record(6, &e);
        r.logical_dimensions = [6, 22];
        assert!(matches!(
            OrnataSpatialMap::from_record(id, r),
            Err(SpatialError::UnsupportedDimensions { .. })
        ));
        let mut r = record(6, &e);
        r.physical_dimensions = [7, 23];
        assert!(matches!(
            OrnataSpatialMap::from_record(id, r),
            Err(SpatialError::UnsupportedDimensions { .. })
        ));
    }

    #[test]
    fn coordinate_and_entry_budgets_fail_atomically() {
        for bad in [
            MappingEntry {
                logical: [7, 0],
                physical: [0, 0],
            },
            MappingEntry {
                logical: [0, 23],
                physical: [0, 0],
            },
            MappingEntry {
                logical: [0, 0],
                physical: [6, 0],
            },
            MappingEntry {
                logical: [0, 0],
                physical: [0, 22],
            },
            MappingEntry {
                logical: [65535, 65535],
                physical: [65535, 65535],
            },
        ] {
            let e = [
                MappingEntry {
                    logical: [0, 0],
                    physical: [0, 0],
                },
                bad,
            ];
            assert!(matches!(
                OrnataSpatialMap::from_record(identity(6), record(6, &e)),
                Err(SpatialError::CoordinateOutsideGrid { entry: 1, .. })
            ));
        }
        assert_eq!(
            OrnataSpatialMap::from_record(identity(6), record(6, &[])),
            Err(SpatialError::EntryBudget(0))
        );
        let e = vec![
            MappingEntry {
                logical: [0, 0],
                physical: [0, 0]
            };
            MAX_SOURCE_ENTRIES + 1
        ];
        assert_eq!(
            OrnataSpatialMap::from_record(identity(6), record(6, &e)),
            Err(SpatialError::EntryBudget(MAX_SOURCE_ENTRIES + 1))
        );
    }

    #[test]
    fn wrong_frame_sizes_leave_caller_output_unchanged() {
        let e = [MappingEntry {
            logical: [0, 4],
            physical: [0, 3],
        }];
        let map = OrnataSpatialMap::from_record(identity(6), record(6, &e)).unwrap();
        let mut out = [Rgb(9, 8, 7); PHYSICAL_PIXELS];
        let before = out;
        assert_eq!(
            map.map_frame_into(&ramp()[..160], &mut out),
            Err(SpatialError::LogicalFrameSize(160))
        );
        assert_eq!(out, before);
        assert_eq!(
            map.map_frame_into(&ramp(), &mut out[..131]),
            Err(SpatialError::PhysicalFrameSize(131))
        );
        assert_eq!(out, before);
        assert!(std::mem::size_of::<OrnataSpatialMap>() < 400);
    }
}
