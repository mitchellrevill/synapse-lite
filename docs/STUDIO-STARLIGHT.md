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
failure. This was a provider timeout, not an approval rejection. No repeated
analysis or substitute animation was attempted. Next native check: a bounded
constructor/vtable pseudocode request, then initialization/frame/renewal evidence.

## Verification and TODOs

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
