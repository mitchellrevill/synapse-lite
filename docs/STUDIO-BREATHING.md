# Studio Breathing

**30 October playback addition:** explicit random colors, finite repeats,
inherited/custom two-color reset and start/end authoring now exist. Synthetic input
state-machine checks are separate from the still-gated live input path. See
[STUDIO-BREATHING-PLAYBACK.md](STUDIO-BREATHING-PLAYBACK.md) for current scope,
source evidence, validation and remaining gaps. Historical checkpoint follows.

**29 October addition:** explicit custom gradients now have native stop authoring,
RGB/None, Add/Delete and history, with the existing renderer and file workflow.
See [STUDIO-BREATHING-GRADIENT.md](STUDIO-BREATHING-GRADIENT.md) for current checks
and remaining UI/mode/reset TODOs. The original checkpoint below describes the
two-colour implementation and its retained source evidence.

Layer Actions now includes **Add Breathing**. Select LEDs, assign Colour1/Colour2,
choose Slow/Normal/Fast, preview and save. Colour clearing retains membership;
erasing membership is separate. Colour and duration edits use the existing bounded
region partition/history mechanism, preserving the other colour, untouched regions
and opaque JSON data. Saved compositions use the existing explicit Studio output
controls under their ordinary device/mode/ownership admission.

This checkpoint implements deterministic continuous Studio effect5. Its defaults
are not the Quick Breathing presets. The source converter builds nine stops at
0/12/25/38/50/62/75/88/100, with black holds and two supplied colours. Missing Colour1
falls back to Colour2; a missing Colour2 uses source None. None contributes the
inverse-coverage byte100, which can reveal underlying layers, and is distinct from
opaque black. Each region has its own generator and mask. Layer/region parameters
merge through the existing compositor, while reversed layer order, logical holes
and physical routing remain unchanged. Fixed Breathing consumes no Fire random state.

The recovered Studio duration presets are **28000/14000/6000 ms**, default14000.
Positive imported durations are honoured; zero/missing uses14000. At nominal25fps,
table length is max(stop count, duration/40). Authoring offers only the recovered
presets and does not rewrite imported durations until explicitly edited. The palette
uses the existing native four-channel repeated-f32 interpolation and boundary quirks.
Each table is bounded to16384 entries, and the existing total32MiB preview-plan
admission still applies. These are storage bounds, not full-process memory evidence.

Explicit custom gradients can play when they fit the verified deterministic descriptor.
The converter adds missing endpoint anchors using the first/last array colour and
retains original order; its sorted temporary is discarded. Unlike Spectrum, malformed
Breathing stops invoke a native random/time-seeded fallback. Empty, duplicate or
unsorted stop cases therefore refuse playback rather than inventing a white loop.
The two-colour editor refuses imported custom gradients/random modes instead of
silently deleting those parameters or presenting an ignored colour edit as effective.

## Source and verification

Current installed RzLightingEngineApi_v4.0.55.0.dll SHA256 was read and matched the
existing complete native evidence:
`cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa`.
Relevant recovered functions are Breathing constructor0x1800650f0,
initializer0x1800651c0, frame0x180065570 and trigger0x180065730, together with the
existing common palette reconstruction. These are decompiler observations, not
original source or successful vendor execution.

The selected cached lighting converter modules were re-analysed with REA: two files,
zero parse failures, parent evidence
`ev_942d8521b67e4562f40aa61df404a8b2af9afb4d1656fd8e9a426102d5afa060`.
Their parent lighting-engine main.a383ea9e.js SHA256 is
`c82c7074e5da8abcffbff95f23a6b962c1e14b04586715b9d4b7a6f11fe9e6d2`.
Studio UI module6257 supplies duration/playback defaults from historical
main.88ac681e.js SHA256
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`.
Its build identifies July2025; it is not proof of today's exact active UI bytes.

Seven isolated converter replays stubbed AddEffect without loading any vendor DLL,
account, application or device. An independent scalar reconstruction of the retained
native instructions produces1952 reference palette pixels, including anchors, None,
zero/tiny durations and missing colours. The layered150-frame reference matches
logical RGB MD5 `8f489554e718c19f451c675c466ae1da` and physical RGB MD5
`4985598e08002f9bcd83ab1437fb3afe`. The latter traverses1050 mock OUT packets with
exact simulated matrix pixels and unchanged readable hardware controls.

The native HWND test creates Breathing, selects a source-path LED, edits both colours,
chooses fast6000, observes peak RGB17/34/51, saves and checks undo/redo. Original stock
export and device-profile bytes remain unchanged; local editing adds no simulated
device calls. An owned render was inspected; it omits child edit controls and is not
a desktop screenshot or optical proof. See `validation/studio-breathing.json` and
the synthetic reference fixture.

## Remaining work

- Random colour, finite-cycle completion, input-triggered/toggle playback and native
  random fallback need verified contracts and implementations. Playback currently
  admits missing/default continuous flags, start=random, end=never and cycles0/-1.
  Unsupported variants remain preserved in files and refuse execution.
- The current REA native provider lacks GHIDRA_INSTALL_DIR. Older complete procedure
  records are preserved locally; the newer analysis-view projector does not accept
  their legacy operation. No engine/config changes were made. Refresh focused native
  dossiers for remaining modes when available; this fixed-mode implementation uses
  matching hashed records, fresh JavaScript analysis and independent checks.
- Complete imported layer-level brush projection/clearing and a Breathing gradient
  editor. Two-colour authoring currently targets ordinary regional colour fields.
- Physical/optical output, timing, wider devices and remaining effects need work.
  Chassis/key legends, stock colour widgets, selection/speed states and general
  Studio layout remain visibly different; pixel-perfect acceptance is not claimed.
- Full device/API/input/profile/network coverage and three complete-build100MB runs
  remain open. This feature checkpoint does not redefine the implementation goal.
