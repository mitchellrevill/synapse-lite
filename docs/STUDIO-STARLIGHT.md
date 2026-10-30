# Studio Starlight authoring

Layer Actions now includes **Add Starlight**. Select LEDs, choose Slow/Normal/Fast,
set density, and choose Random On/Off. Edit Gradient opens the shared preset and
individual percent/RGB/None stop editor. Back to Properties returns to the effect
settings. Edits use the existing bounded region partition, Undo/Redo, Save and
Discard paths. Random Colour retains the stored gradient and disables gradient
mutation until switched off.

**Starlight animation is not implemented.** Preview and saved-device playback
continue to refuse compositions containing active Starlight regions explicitly;
this checkpoint does not substitute another effect or an invented random pattern.
The controls edit compatible local composition data.

## Runtime descriptor preparation

`secbird studio starlight FILE NUMBER LAYER_ID` inspects the selected Starlight
layer in a local JSON-v1 ChromaEffects archive. NUMBER is the one-based composition
number reported by `studio inspect`. Output contains one descriptor per declared
region in order, after layer/region parameter inheritance, plus explicit
`animation_implemented:false`, `lighting_applied:false` and
`device_compatibility_checked:false` flags. This is a read-only diagnostic; it
loads no DLL, discovers no hardware and changes no profile or output ownership.
Hidden and offline regions are included as data, without playback admission.

`Draft::starlight_region_descriptors` provides the same preparation to callers.
The independent shipped converter replay covers all three duration presets,
all ten densities and both colour modes, plus missing scalars, absent/empty
stops, imported duration9999, duplicate/unsorted stops, None and stop metadata.
The 69 captured descriptors are test data from JavaScript execution, not a
second implementation of the Rust normalization. The replay script takes only
the exact hash-verified private module2578; vendor source is not committed.

Preparation preserves stop order, including stable duplicates. The converter
creates a sorted temporary but case7 discards it. Missing endpoint anchors copy
only the first/last colour; existing stop metadata and raw numeric spellings
remain intact. Missing Color becomes sentinel0x64000000. Random mode replaces
only outgoing stops with an empty array. Missing duration/stops remain absent;
missing density becomes `MaxStar:null` at the source JSON boundary. No1700ms UI
projection or default palette is invented for these missing runtime fields.
An imported explicit duration is retained in the descriptor.

The admitted preparation subset requires boolean randomColor, u32 duration,
integer density1..10 when present, at most ten saved stops, Stop0..100 and RGB24
or the None sentinel. It refuses malformed/coerced values. Each descriptor and
the aggregate descriptor JSON have an8MiB ceiling; inherited metadata therefore
cannot expand unchecked across regions. This bounds retained descriptor data,
not total process memory or complete-build acceptance.

## Retained source contract

- Studio effect name/id are `starlight`/7, distinct from quick-effect settings.
- Duration presets are **3000, 1700 and 500 ms**, default 1700. Density is **1–10**,
  default 1. The Studio default gradient is the seven-stop palette also used by
  Wave, with `colorStopsCustom:false` and `randomColor:false`.
- Missing/unlisted duration projects as 1700 without rewriting the file merely
  on load. Editing a different field preserves an imported duration's raw value.
- Stop edits write `colorStopsCustom:true`; presets write false. None omits Color.
  Stable duplicates, imported raw stop metadata, insertion/deletion limits and
  RGB/percentage validation use the shared gradient mutation. Starlight/Wave
  insertion stops at seven; Spectrum retains its separate ten-stop limit.
- Only selected memberships change. Unselected/offline memberships, layer data,
  unknown parameters and numeric spellings are preserved. Layer gradients are
  materialized for a selected regional stop edit when inherited.
- Invalid authored properties, ambiguous membership, conflicting effect IDs and
  random-enabled gradient mutations refuse atomically. Mixed gradients require
  choosing a preset. Newly assigned memberships receive source defaults plus
  retained layer parameters; exact stock projection ordering across every mixed
  selection/tool remains unverified.

The source converter maps density7 to `MaxStar:70`, with Regen10, Cycles1 and
Pause0. Random mode sends Mode1 and empty runtime stops while retaining the saved
gradient. These are native descriptor fields, not proof of a count of 70 LEDs or
of a recovered population/timing algorithm.

## Evidence

Cached Studio main `main.88ac681e.js` SHA256
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`:
module6257 provides defaults, palettes, durations and density bounds. The fixture
`studio-starlight-settings.json` was extracted from literal AST expressions,
independently of the Rust implementation. Chunk `813.18e17573.chunk.js` contains
`custom-starlight`, its duration/density controls, Random Colour checkbox and
gradient disable rule. Module2578 supplies the native descriptor conversion.

Fresh REA summary `ev_419270560085515dd0e8ae60e55e9158fda7a319cfa0a40a2252800be5f30e8b`,
parent `ev_2d8646dbabaf5ad9884b20cf31a4b389afee8821ab8a3bfda2bbac3ec114aab5`:
four source files/projections, 57123 bytes, 16766 AST nodes, zero parse failures,
3329 semantic unknowns. Static evidence does not establish live frontend execution.
Two controlled converter replays intercept the native boundary; no vendor DLL is
loaded and no device calls occur.

The connected native provider initially lacked its Ghidra setting. The existing
workspace helper used the installed Ghidra and exact lighting DLL/snapshot, but
`analyze_function` for constructor0x1800601a0 returned `provider_timeout` after
240000ms. Analysis-process inspection confirmed it was live before terminal
failure. This was a provider timeout, not an approval rejection. The later code-only constructor pseudocode request at the same address also
returned provider_timeout after240000ms (the dispatch records a2GiB heap).
Neither failed decompilation request was repeated in this follow-up. The new
assembly-only route and concrete next diagnostic are recorded below.

## Assembly-only follow-up and remaining runtime gate

On2026-10-29 a new private REA/Ghidra session opened the installed lighting DLL
with the matching SHA256 below and the existing immutable analysis snapshot.
It requested **procedure_assembly** at0x1800601a0, rather than either previously
failed decompilation operation. That also returned `provider_timeout` with a
240000ms deadline (252211ms recorded tool duration). No instructions or new
native Evidence ID were returned. The private Ghidra log shows completed default
auto-analysis and execution of ReaGhidraBridge before the timeout. This does not
prove whether bridge startup/communication or later request handling stalled.
The helper session was closed; no vendor DLL execution or device calls occurred.
The request was not repeated.

Exact native target: `RzLightingEngineApi_v4.0.55.0.dll`, SHA256
`cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa`.
The reused selected source module2578 hash is
`008bf7b8aac4e86f47dd0c4e498402949c384d640553ca08d9c2424ad3527bda`;
its original lighting-engine bundle hash and extraction ranges are recorded in
[LIGHTING-PIXELS.md](LIGHTING-PIXELS.md). Existing REA source parent
`ev_942d8521b67e4562f40aa61df404a8b2af9afb4d1656fd8e9a426102d5afa060`
and Starlight parent above are reused, without repeating that analysis.

| Runtime question | Evidence status |
|---|---|
| Parameter/gradient conversion to source descriptor | Implemented;69 intercepted source computations |
| Palette sample count, interpolation and invalid-stop fallback for Starlight | Unresolved; generic shared palette evidence alone is insufficient |
| Placement, population selection and randomization | Unresolved; MaxStar70 is a descriptor percentage, not proof of70 stars |
| Lifetime/fade, regeneration and speed mapping | Duration/Regen inputs recovered; animation law unresolved |
| Deterministic seed, random colour and timing semantics | Unresolved; no invented CRT/clock semantics admitted |
| Layer/None/brightness animation, preview and opted-in output recovery | Still blocked on the renderer; existing refusal remains |

Concrete next diagnostic: inspect the owned Ghidra bridge endpoint/startup logs
and request handshake after auto-analysis completes, retaining their timestamps.
Do not repeat constructor decompilation with unchanged settings. Once the bridge
can return focused instructions, follow the constructor's vtable through
initialization, frame and renewal methods, then obtain source-derived reference
frames before adding playback. Physical/optical checks need the present user.

## Verification and TODOs

The descriptor-preparation follow-up passes512 core tests (six ignored), five
CLI tests and one doc test. The69 source converter cases match in the core
suite. The CLI debug build succeeds. Executable inspection checks two inherited regions,
None and random mode, exact opaque floating/integer spellings, three rejected
requests and an unchanged source SHA256. Detailed results are recorded in
[studio-starlight-preparation.json](validation/studio-starlight-preparation.json).
The earlier authoring checkpoint evidence below remains historical.


Four core regressions verify independently extracted defaults, regional partition,
inheritance, opaque raw data, independent scalar/gradient edits, random guarding,
history, invalid values and identity/inventory refusal. The native interaction
check loads the actual exported composition with simulated hardware, creates
Starlight, saves duration500/density7/randomfalse and a transparent custom stop,
checks disabled gradient history while random, Undo/Redo, invalid density,
read-only controls and explicit preview refusal. It leaves source/profile files
unchanged and makes zero simulated device requests.

Workspace checks pass: 458 core,291 tray,four CLI and one doc test, with six
core and39 tray checks explicitly ignored. Seven optimized native Studio checks
pass; the targeted Starlight check also passes after the status layout correction.
The optimized workspace build succeeds.

Native production-painter captures were inspected. The status paragraph originally
overlapped Apply Density; its layout now uses the gradient-effect status area.
Painter captures omit native child edit controls, whose existence and values are
checked separately. This is an interim panel: stock sliders, gradient popup,
checkbox, colour picker, handles, icons and exact panel layout remain TODOs.
No matched-state stock Starlight screenshot or pixel equality is claimed.

Population selection, random colours/seeding, star fade/renewal timing, preview,
saved output and optical verification remain open. Continue other effects and
device/input features instead of blocking on those details. Full profile/device,
privacy, visual and three complete-build memory acceptance remain open.
