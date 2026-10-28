# Studio composition file interchange

The core library and CLI now read and export selections from the recovered
JSON-v1 `.ChromaEffects` container. This is the file-handling foundation for
Studio, not a canvas editor, installed composition store or lighting runtime.

The later [layer draft model](STUDIO-DRAFT.md) adds local rename/visibility and
saved/working/undo/discard semantics plus archive-replacement preparation. It is
not yet connected to native Studio controls or actual composition persistence.

```text
secbird studio inspect Desk.ChromaEffects
secbird studio export Desk.ChromaEffects Selected.ChromaEffects 1 3
secbird studio export Desk.ChromaEffects Backup.ChromaEffects all
```

Numbers are one-based and refer to the inspection list. Selected profiles retain
source order. The output must be a new file; an existing file, including the
input itself, is refused. Argument checks, input validation and complete encoding
precede output creation. A write/sync failure reports that the newly created
export may be incomplete; it never replaces an existing document.

Inspection shows only bounded id/title metadata. These commands run before
device discovery and do not load the device profile store, invoke vendor code,
apply lighting, remap LEDs or contact a service. Bad/oversized display metadata
produces a notice while the raw composition remains available for export.

## Preservation and limits

The envelope is `{version:1,profiles:[...]}`. Profile JSON values and unknown
envelope values are stored as `RawValue`, preserving their original numeric
literals, whitespace, unknown parameters, group/layer/region/device objects and
LED identity metadata. Numbers beyond binary64/integer precision are not
converted through a numeric tree. Root keys may be re-escaped and surrounding
envelope whitespace changes; whole-file byte identity is not promised.

The codec accepts UTF8 JSON with an optional UTF8 BOM, numeric version1 and
object-valued profile entries. It refuses malformed/trailing JSON, duplicate
envelope keys (including equivalent Unicode escapes), unsupported versions,
wrong envelope shapes, legacy ZIP/XML, empty explicit selections and invalid or
duplicate selection numbers. Opaque profile content is preserved, including
duplicates inside it; future editor/runtime admission must independently validate
that content rather than interpreting archival acceptance as schema approval.

Resource bounds are8MiB per input/output document,128 profiles,64 envelope fields,
nesting depth64 and262,144 structural markers outside strings. These are the
replacement's limits, not recovered stock limits. Output is bounded during
serialization. No device, edition, layout, layer/group or effect compatibility
is certified by this container codec. It preserves already exported LED metadata
and does not fabricate metadata for disconnected devices.

## Source evidence

Retained cached Studio main SHA256
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`,
ModalExport SHA256
`5f3388d89bd49da3d4c7bce701676c23552e4e75c07b9f19188774583e24ccee`,
ModalImport SHA256
`d6d00e7c8dfa505cea636c564cc9f5a6938494022bc3adca784e41383e590b4d`.

ModalExport1663 serializes a JSON object with version1/profiles. Main178436
declares the version. Export1966–2536 removes UI `checked` and enriches existing
LED records from runtime configuration; main19602 preserves LED coordinates and
adds region/input identity fields, with inputData itself a JSON string. The
replacement container exporter preserves supplied records and performs no such
live enrichment or UI-profile generation.

ModalImport1850–4370 reads text, JSON-parses first and falls back to a legacy
ZIP/XML converter only when reading/parsing fails. The reviewed handler does
not check the root version. It regenerates profile IDs and remaps placements/
LEDs before committing imported compositions. That is a separate, potentially
lossy action, and is not performed by these file commands. Legacy conversion
and actual installed-import workflows remain unfinished.

The exporter passes a JavaScript string to the native save bridge. The bridge's
actual byte encoding was not traced; this checkpoint chooses UTF8 output and
does not claim all stock encodings are supported. Cached source identity is not
proven to be the current live Studio revision.

REA parent `ev_e345e5d4b8d9ca33632805d1422597658056bf01b3beb750c41adbe9cee5a09d`
and summary `ev_ddef81e345ea069abbfa43c2fb574046955cc4dd616f12de2d02b71ce85cf9c3`
parsed40 JS files with zero parse failures. Application and semantic coverage
are partial/resource-limited. Focused source contracts retain exact hashes,
offsets and search bounds; missing graph edges are not evidence of absence.

## Verification and remaining work

Six new core tests cover exact raw profile/numeric preservation, selection order,
unknown fields, metadata failure without data loss, malformed/duplicate keys,
BOM/escaping, legacy/version refusal and byte/count/depth/structural budgets.
Two new CLI tests cover actual file selection, existing-target/input protection
and rejection before output creation. Full checks pass:373 core tests (six
intentionally ignored), one doc test, three CLI tests and229 serial tray tests
(24 intentionally ignored). Optimized CLI and tray builds pass.

The [release-command check](validation/studio-file-interchange.json) used a
source-shaped **synthetic** grouped Static composition, not a captured stock
export. The optimized CLI inspected two compositions and exported one/all;
an independent Node JSON parser confirmed complete group, region, LED and
unknown-envelope structure. Selected profile bytes and large/float literals
remained exact; input and normal device-profile-store hashes stayed unchanged,
and existing output was refused. This is not a successful import into stock.

The actual Studio canvas, selection tools, layer/group editing, gradients,
save/discard/history, local composition installation, device remapping, real
stock round trips, effect runtime/preview and hardware acceptance remain open.
New source profiles default to Spectrum Cycling; the Static fixture is an
explicit test choice, not a claim about that default. Studio and complete-build
working-set/private-memory acceptance below100MB remain unachieved.
