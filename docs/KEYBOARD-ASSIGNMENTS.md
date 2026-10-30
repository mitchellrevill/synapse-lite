# Native Keyboard Function assignments

Customize now authors a local desired Keyboard Function binding using the existing
popup, input capability list and Standard/Hypershift context. It offers the 125
previously recovered source key descriptors, the six source dropdown groups, a
manual All keys chooser, left/right Shift/Ctrl/Alt and left Windows, and conditional
Turbo (1â€“20, default 7). Key Recording is deliberately replaced by manual selection;
this feature does not intercept or inject input or implement a recorder.

The displayed scan/down/up values describe the recovered key descriptor. A binding
follows its assigned input's down/up lifetime; no invented standalone-down or
standalone-up assignment field is written. Saved source records contain key and
modifier IDs, keyGroup and Turbo fields, as the source saveMapping helper does.

Save validates the complete draft and rechecks the connected device, active profile
name and complete expected profile under the worker's engine lock. Failed saves
keep the draft; Cancel writes nothing. Dirty close/navigation uses the existing
Save/Don't Save alert. No assignment action can mutate a pending save. A successful
save closes the draft and follows a pending transition. Normal snapshot invalidation
continues to discard stale owner contexts.

The pure profile update changes only the selected input/layer. It retains unrelated
records, opposite-layer records, old inactive groups, root metadata, unknown nested
keyboard/Turbo fields and profile identity. Duplicate records, non-boolean layers,
secondary/inter-device/combined mappings and malformed groups refuse rather than
guessing a translation. Native profiles without a source payload acquire one only
at Save; malformed existing payloads do not get replaced.

## Delivery boundary

Save means **saved locally**, not applied. The selected legacy native button
projection is removed, and a `localKeyboardAssignments` ownership list inside the
existing private source metadata makes the exporter retain the source record.
The new endpoint calls no device setter. This avoids activating authored bindings
through existing button/profile routes while installed mapping readback and
delivery remain unverified. The UI and completion message state this limitation.
The rest of the profile's existing projections are unchanged.

Native backups preserve this local ownership metadata. Synapse exports contain the
ordinary source assignment fields and preserve opaque record leaves. Importing a
Synapse export follows the pre-existing importer's projection rules; the private
local ownership marker is not a Synapse format field and is not exported. No live
re-import into stock or physical output was tested. A later conflicting native
button projection refuses source export rather than producing duplicate mappings.

## Source authority

Cached Ornata source `MapKeyboard.7a4bbcfc.chunk.js`, module 96223, supplies the
seven action types, side-specific modifier rules, saveMapping, getMappingData,
conditional Turbo, group reload and key/modifier ID serialization. SHA-256:
`b016b7eb1802e5c8996aab1924e432efb1519fabde610a08dbb0e3de9df365bc`.
A controlled replay of the recovered pure `saveMapping` body passed 500 key/layer/Turbo cases plus empty-key refusal. Context construction and `getMappingData` were stubbed; this verifies serialization only, not native delivery or the full vendor form. Serializer SHA-256: `6ae5cb7d00d22aba41946da4cc571d32c72519940074c4999e5401bd56638c10` (UTF-8 byte offset 22216).

Existing `macro_keys.json` supplies the 125 checked descriptors and transition
prefixes; no vendor bootstrap or host input listener is executed.

Reused historical REA summary:
`ev_37cca753633b7f0d8a50089d497e9ff17bad7a0620fbd1204acc26df9e03a683`,
parent `ev_df300d5c0bdd26501a642441e78f64cf4ce1a16adaabb2da1855be302dc22acf`.
Its JavaScript semantic coverage is partial. Focused recovered-source inspection
supports the behavior above; this is not evidence of current native execution.
The source cache belongs to the archived inventory, not a fresh claim about the
installed AppEngine version.

Archived visual references: `stock-ornata-assignment-keyboard-0.jpg` and
`stock-ornata-keyboard-mode-menu-0.jpg`, inventory capture archive, 27 October 2026.
The first is 2048Ã—1104 and was captured at 18:45:01.963 UTC. The new native fixture
requests that same physical window size and records its actual DPI/geometry.

## Remaining work

- Prove mapping readback, delivery, ownership and release recovery before enabling
  hardware output. Common driver runtime and recorder are separate work.
- Recover the six descriptor-less/zero-descriptor keys and any additional source
  symbol aliases before offering them; do not invent scan codes.
- Match the exact stock Include Modifier reveal, source button styling, all focus
  and keyboard navigation states, tooltip and animation behavior. The current
  manual form exposes side choices directly and explains local-only persistence.
- Verify the mouse chooser at matched geometry as well as the keyboard fixture;
  no physical mouse write is implied by the shared form.
- Run the full product's final memory gates. This feature's build and screenshots
  do not establish the complete-build budget below 100 MB.

## Verification

The published-base check passes 519 core tests (six new assignment checks),
307 serial tray tests (including explicit unknown-key repair versus combined-form
refusal), four CLI tests and one doc test. One inherited Dynamic Sensitivity test
is explicitly excluded after stalling in the initial integration-base run:
`dynamic_sensitivity::tests::presets_custom_chunk_readback_rollback_and_profile_scope`.
That interrupted broad run is not a passing suite. No unrelated test was edited.

The owned HWND test passes chooser/modifier selection, invalid empty selection,
Cancel with byte-identical storage, dirty-close Save and pending transition,
reopen, opposite-layer/opaque retention, stale external-profile refusal, read-only
POST refusal and zero added simulated device SETs. Startup waits up to 30 seconds
for the initial font/asset presentation; the ordinary interactions retain bounded
waits. The first attempt's four-second startup timeout and the next attempt's
unacknowledged expected error dialog are retained in local logs; the final test
uses the existing owner-checked failure acknowledgement helper.

Desktop capture was attempted but its app-access approval timed out. The visual
artifact instead calls the same native paint function into a new memory DIB. It
reads no desktop/window pixels and is labelled an **offscreen fixture render**.
The artifact is 2048×1104, at native scale 1.25 (CSS 1638.4×883.2). The archived
stock image is also 2048×1104; stock DPI was not recorded, so matched-DPI or pixel
identity is not claimed. Stock selects a different key and has a different scroll
state. Direct modifier rows, manual All keys rather than recording, the descriptor
readout and local-only message remain visible differences. Approved tabs are
unchanged.

Reproduce with a private asset pack in `SYNAPSE_LITE_ASSETS` and a task-owned
`CARGO_TARGET_DIR`:

```text
cargo test --workspace -j 1 -- --test-threads=1 --skip dynamic_sensitivity::tests::presets_custom_chunk_readback_rollback_and_profile_scope
cargo test -p secbird-tray -j 1 native_keyboard_assignment_save_cancel_layers_roundtrip_and_conflict -- --ignored --test-threads=1 --nocapture
cargo build --release --workspace -j 1
```

Optional `SYNAPSE_LITE_KEYBOARD_RENDER_PATH` names a new BMP file for the owned
memory-render artifact. The test refuses an existing destination. Run native/GDI
checks serially. No normal profile store, clipboard, live input, installed driver,
stock service or another running Lite instance is used.

Validation results and reproduction commands are recorded in
`validation/keyboard-assignments.json` after the final build.
