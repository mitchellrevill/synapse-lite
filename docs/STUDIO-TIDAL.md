# Studio Tidal authoring checkpoint

Layer Actions now includes Add Tidal. Select canvas LEDs to assign two colours,
None, Random Colour, speed and playback settings. Center Point picks a layer-wide
point on the canvas. Both direction tabs toggle Outward/Inward, including a click
on the active tab. Changes use the existing save, undo, redo, discard and read-only
workflow. No physical lighting stream or input interception is started.

This checkpoint implements authoring and bounded engine-descriptor preparation.
**Tidal animation is not implemented.** Local Preview and saved output retain an
explicit radial-renderer refusal. No Wave generator is substituted. This is not
full Tidal parity, pixel-fidelity acceptance, or complete-build RAM acceptance.

## Recovered settings

| Setting | Source behavior |
| --- | --- |
| Layer identity | `tidal`, Studio effect ID 19, default title `TIDAL` |
| New regional colours | Green `00FF00`, blue `0000FF`, Random false |
| Colour clear | Explicit source None sentinel `1677721600` (`64000000`) |
| Speed | Integer 0–50; default 10 |
| Hidden regional defaults | angle 160, split true |
| Start | `random`, `onPress`, `onSelectedKeys` |
| End | `never`, `after`, `toggle`; Toggle unavailable for Random start |
| Cycles | 1–100 for After; -1 for Never/Toggle |
| Default playback | Random/Never/-1 |
| Layer default center | canvas X2/Y2 5000/5000; engine X/Y 125/123 |
| Layer direction | `counterclockwise=false` means Outward; true means Inward |
| Center conversion | `round(X2 / 40)`, `round(Y2 / 40) - 2` |

Changing Start couples End/Cycles exactly as the shared widget does: Random selects
Never/-1; either key start selects After and the last edited cycle count, or 1 when
none was edited. Changing End selects that remembered count for After and -1
otherwise. Clicking an already selected Start/End is inert. This transient cycle
memory is not persisted in a new profile field. Saving key-trigger parameters does
not authorize or start input capture.

The bounded authoring subset accepts integer canvas points in 0–10000. Moving a
center saves all four coordinate fields together. Toggling direction writes only
`counterclockwise`, preserving even unusual imported coordinate spellings. Layout
fields are not copied into new regional overrides. The shared dirty-state check now
includes layer `params`, so layer-only changes cannot silently appear saved.

New layers remain empty until a property operation assigns selected LEDs. New
memberships receive source defaults; existing regional data does not silently gain
missing defaults. Region partitioning, unselected LEDs, metadata, large numbers and
foreign JSON fields retain the existing raw-value preservation behavior. Imported
custom gradients are retained; colour/Random edits explicitly refuse while those
gradients override the source pickers. Speed/playback edits preserve the gradient.

## Prepared descriptor and runtime boundary

`Draft::tidal_region_descriptors` prepares the recovered AddEffect19 shape only.
It merges regional values and layer-wide center/direction, adds source endpoint
anchors, maps missing gradient colours to None, and preserves the converter's
unsorted array order. An eight-case isolated replay of the actual shipped converter
checks the complete descriptor JSON, including omitted fields and Random/custom
precedence. It never loads the vendor DLL.

The fixed two-colour pulse positions are 0/10/25/40/60/75/90/100: black except for
the first colour at 25 and second at 75. Random uses positions
0/15/30/40/65/70/90/100 with native colour indices 1 and 2 at 30 and 70. An imported
`colorStops` array, even empty, takes precedence over Random. Split adds mode bit 2;
onPress adds 4096; onSelectedKeys adds 12288; Toggle adds 4. Direction is 1 outward,
0 inward. Width defaults to 100 and Pause to 0. Missing speed/angle/cycles/center
remain absent rather than receiving UI defaults.

Preparation admits at most ten imported stops (plus at most two generated anchors),
width 10–400, pause 0–60000ms, speed 0–50, angle 0–360, cycles -1 or 1–100 and centers
-2–250. Descriptor JSON totals at most 8 MiB. The existing 8 MiB document and
20-step/8 MiB history bounds still apply. These are bounded preparation contracts,
not evidence of the requested total-process RAM threshold.

The exact radial frame algorithm, its layout-distance math, phase, speed-zero
semantics, Random state ownership, finite completion and input-trigger lifetime
remain unverified. No runtime stream was added. The retained DLL RTTI identifies
`CTidalEffect` at descriptor RVA `1f5510`, vtable RVA `1acc48`, with candidate method
RVAs `68780`, `673b0`, `685a0`, `68650`, `68740`, `675c0` (these are not recovered
semantic signatures). Fresh REA opening with the Ghidra provider reported
`provider_unavailable`: `GHIDRA_INSTALL_DIR` is unset and Ghidra 12.1.x is unavailable.
Next work is function/assembly recovery at those candidates, followed by an
independent radial frame reference; transplanting Wave math is not a valid substitute.

## Evidence and visual comparison

Source hashes are in [the source manifest](validation/tidal/source-manifest.json).
The cached Studio UI is the July 2025 `main.88ac681e.js` build, not proof of the
currently installed UI. Module 2904 supplies defaults and construction, module 6257
constants, module 1981 the grid conversion, chunk 510/module 8154 the Tidal panel,
and module 5805 the shared playback widget. Engine `main.a383ea9e.js` module 2578
contains AddEffect19 conversion and module 5596 the mode constants. The native DLL
identity is SHA256 `cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa`.

`tools/replay_studio_tidal.cjs <cached-synapse-directory>` executes the source
settings and converter in a VM with inert unrelated dependencies and a captured
AddEffect call. Its fixture is committed under core tests. The UI replay tool
executes cached JSX/CSS with inert hooks/dispatch/host bridge and closed popups;
it does not observe a running vendor app. Its package path can be supplied via
`PLAYWRIGHT_MODULE`. All browser requests are aborted.

[Source panel replay](validation/tidal/source-tidal-panel.png),
[native default state](validation/tidal/tidal-native-default.png), and
[native authored state](validation/tidal/tidal-native-authored.png) are retained.
The native captures use PrintWindow on the isolated test window, including real
child edit controls; they do not capture another app. Both default screenshots
show green/blue, speed 10, outward and Random/Never. The authored screenshot shows
112233/None, speed 0, inward, a moved center and selected-key/After/7.

Visual review confirms the functional controls and the absence of overlap after
moving status text into the canvas footer. Significant differences remain:
source colour dropdowns/checkbox, speed slider/stepper, icon direction tabs,
playback dropdowns, native panel spacing/type and the existing chassis/key legends.
The native text fields/buttons are interim controls. Source icon/tooltip/popover
interaction, custom-gradient authoring and fractional center dragging need later
work. No pixel-perfect claim is made.

## Verification and integration

See [machine-readable validation](validation/studio-tidal.json) and
[the native interaction result](validation/tidal/interaction.json). The isolated
native test exercises add/selection, both colours, None, Random disables colours,
speed zero, active-tab direction toggling, canvas center conversion, playback
serialization, save/undo/redo, invalid input, read-only behavior and preview refusal.
It asserts zero simulated device requests and unchanged source composition/profile
fixture bytes. All temporary state and build output use this task's directories.

Implementation first ran on canonical integration base `7b99930`, then only the
Tidal changes were transplanted onto published `6149f7a9422ae93322365ed12df46ab681dc40a8`.
No unrelated unpublished integration commits are in the feature branch. It has no
new feature dependencies. Other teams' Settings, shortcuts, effect runtimes and
approved tabs were not changed.
