# Reactive event and pulse replay boundary

This checkpoint supplies a local diagnostic renderer and an input-decoder seam.
It does **not** enable live Reactive in the ordinary Studio preview or saved-device
output. The vendor pulse algorithm remains unresolved. No guessed fade, duration
conversion, random-colour selection or gradient is installed as a vendor default.

`studio::reactive_input::ReactiveInputMap` reads the selected composition's
explicit `leds`/`ledRef` metadata. It resolves `inputType:"kbd"` and the serialized
`inputData:[make-code,prefix]` pair to exact row/column memberships. Placement IDs,
left/right extended identities and one-to-many LED memberships stay distinct.
Unknown input types (including `dkm`) stay unmapped. Malformed known keyboard
records fail before the map is returned. The bounded inventory reader rejects
conflicting current/reference inventories and duplicate coordinates. Parsing
does not modify the source composition, its identities or opaque fields.

`studio::reactive_replay::ReactiveReplay` requires caller-supplied pixel samples
and an explicit restart/ignore-while-active policy. There is no built-in pulse.
`start_local_preview` returns an instance-specific session token. Before that
call the renderer is inert. The same token must accompany events and frames.
The renderer uses declared sample timestamps, independent per-LED pulse slots,
and no unbounded event queue. A caller can inspect synthetic overlapping keys,
sample boundaries, explicit region ordering, transparent None, opaque black and
final composition brightness. None reveals the lower frame. Colour and alpha
samples use the already recovered Studio `Pixel`/`Layer` math.

Repeated key-down records while held are suppressed in this diagnostic. A fresh
press follows the caller's selected retrigger policy. These choices are **not**
claims about the vendor's unverified repeat, overlap or retrigger behavior.
The 4MiB cap applies to pulse samples and active slots; it is not a whole-app
memory measurement. There are at most 512 render LEDs, 128 regions and 16384
samples per pulse. Existing document/LED budgets also apply.

Stop, source loss, unplug and backwards time clear held state, every pulse and
the session. Restart requires an explicit new token; old events and delayed
source-loss callbacks cannot affect it. The renderer blends into caller-owned
frames, so the caller must clear/rebuild its frame after stopping. The library
does not issue a hardware clear or claim restoration of physical lighting.

The tray's `studio_reactive_source` module decodes a borrowed Windows RAWINPUT
keyboard record only for its explicitly selected nonzero collection handle.
It checks the keyboard payload size (header plus RAWKEYBOARD, **not** the larger
RAWINPUT union size), make/break/message consistency, prefix flags and fake/overrun
records. Virtual keys never stand in for scan codes or LED coordinates. It
neither calls GetRawInputData nor registers for input. A numeric handle is not
identity/ownership proof: this module is only connected to the diagnostic harness.

## Evidence and scope

The retained authoring contract remains in [STUDIO-REACTIVE.md](STUDIO-REACTIVE.md):
effect3, colour/None, random flag, source durations2000/1500/500ms and default
onPress/after/one cycle. No new animation or hardware claim follows from those
settings or from firmware Quick Reactive, which is a different path.

The shipped Studio main bundle SHA256 is
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`.
Its `function m(e,t)` at Unicode character range19602..20250 joins each LED's
row/column to `config.LedInputMap[].MatrixPos` and copies `InputType` and
`JSON.stringify(InputData)`. An inert 648-character function projection plus
provenance was inspected with REA. Parent Evidence:
`ev_007527f7cc6422581a889aecea11ecb8b67e86da9d450d45603c318b755397de`;
summary Evidence:
`ev_83dee50f8d330206937e399b06cbbce7f1450c140d335f8715e4f6d8505e83bb`.
REA read763bytes across two files, visited225 AST nodes, reported zero parse
failures and51 semantic unknowns. This is static projection evidence, with an
unresolved external configuration binding; the vendor application was not run.
It establishes metadata construction, not physical trigger delivery.

Interpreting the two `kbd` integers as make-code/prefix candidates is an inference
for the replay seam. The source metadata constructor preserves that pair without
explaining native dispatch. Prefix0/2/4 matches the diagnostic Windows record
convention; E1 and special multi-record keys have no claimed physical proof.

The previously stalled native constructor query at0x180063680 was not repeated.
A smaller `procedure_assembly` query against lighting DLL SHA256
`cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa`
also returned `provider_timeout` at240000ms. An execution outside the restricted
sandbox tested the loopback-access hypothesis and also timed out, with an initial
`cleanup_incomplete` result. An explicit REA close then succeeded. No native
function Evidence ID, assembly, animation or successful physical trace is claimed.
Private logs remain in this chat's work folder and are excluded from Git.

## Verification

Core tests exercise identity, strict parsing, synthetic sample timing, independent
overlapping pulses, region membership, explicit retrigger policies, held-repeat
bounding, None/black composition, brightness, stop/restart, stale tokens and clock
failure. Their step samples are deliberately synthetic, not vendor reference
frames. A Windows decoder test covers foreign, fake, malformed and inconsistent
records. An owned hidden message-only HWND routes private WM_APP records through
the decoder into replay, then simulates source loss and restart. It sends no
WM_INPUT, key-down/up, SendInput or device request and makes no Raw Input
registration. It needs no private assets, normal profiles or physical devices.

Run with `cargo test -p secbird-core studio::reactive` and
`cargo test -p secbird-tray studio_reactive_source -- --test-threads=1`.
See [the validation record](validation/studio-reactive-replay.json) for results.

The final offline workspace run passes521 core,308 tray,4 CLI and1 doc test,
including all14 new Reactive checks. Six core and47 tray checks remain ignored.
An initial run failed32 existing file-persistence tests with Access is denied in
the sandbox default temp directory. The unchanged code passes with TEMP, TMP and
SYNAPSE_LITE_CONFIG_DIR set to isolated workspace folders. No optimized build or
whole-process memory measurement was performed for this diagnostic checkpoint.

## Remaining integration gates and next diagnostic

1. Recover the native frame, init and trigger methods without repeating the
   stalled constructor query. Next: repair/reuse an owned analysis session with
   a narrowly scoped analyzer/provider, resolve the Reactive vtable slots, then
   query only those small methods. Corroborate with a controlled non-input
   native effect harness and retain actual pulse frames, alpha, random state,
   duration quantization, same-key retrigger and cross-key overlap references.
2. Compare source keyboard flags and callbacks with passive **normal-mode**
   Windows Raw Input on the selected Ornata. Require the existing
   RawKeyboardBinding container/collection proof and validate it at observer
   start, first matching event and end. Account for initial arrivals, handle
   reuse, per-process keyboard registration ownership, backlog, device change,
   source error and teardown. No all-keyboard/PID-only fallback, interception,
   driver-mode entry or generated input. PrintScreen/Pause/Fn/media and `dkm`
   mapping remain unverified. The52 earlier vendor/MI01 records from driver mode
   do not establish ordinary normal-mode routing or universal compatibility.
3. Only after those gates, connect a separately explicit input-observation
   opt-in to local Studio preview. Connect saved-device playback through the
   existing explicit output session, ownership checks and authenticated layout.
   Neither authorization implies the other. Verify unplug/stop/sleep/restart,
   idle/display-off brightness and no automatic reconnect replay.
4. Compare matched source/capture states for Reactive and other retained UI.
   This checkpoint changes no painter, tabs or authoring controls and claims no
   new visual parity, physical lighting proof, all-device compatibility or
   three-run complete-build memory result.
