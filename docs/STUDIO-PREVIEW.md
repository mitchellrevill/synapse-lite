# Local Studio animation preview

Open a composition in Chroma Studio and choose **Local Preview**. Static, Spectrum,
Fire, deterministic continuous [Breathing](STUDIO-BREATHING.md), and imported
continuous cardinal [Wave](STUDIO-WAVE.md) layers animate
the native canvas. Edits and Undo/Redo rebuild the
preview from the current draft; file data is not changed merely by previewing.
Stop Preview drops its frame plan. Navigating away stops playback; reopening
Studio keeps the draft but does not silently restart animation.

This is **canvas preview only**. It issues no HID/driver requests and does not
apply the composition to the keyboard. Physical output, ownership/restoration
and optical acceptance remain separate unfinished work. The shape renderer,
property widgets and playback button remain interim UI, not stock pixel parity.

## Supported mapping and source behaviour

Current admission is one Ornata605/edition0, orientation0, explicit saved layout,
logical7×23 configuration, zero LED-size/offset fields and nonnegative whole
40-unit placement coordinates. The composition's config and explicit LED
inventory are required. This is offline file admission, not fresh live device
identity or authentication of arbitrary imported config. Rotations, multiple
lighting devices, custom cells/offsets, fractional placements and other visible
effects fail explicitly rather than producing a partially correct preview.

Native positioning multiplies saved y/40,x/40 by40, then subtracts the minimum
attached origin. A lone keyboard atx4520/y4840 therefore still has a7×23 engine
canvas. Source masks project only manifest-listed logical LEDs. Your109 selected
LEDs span half-open bounds(left1,top0,right23,bottom6): a6×22 Fire region, not
the full7×23 canvas. Fire's lower-left crop is placed inside that bounding box,
then the selection mask preserves holes. Preview readback contains161 logical
pixels; physical6×22 projection belongs to the later hardware writer.

The backend applies reversed root/child arrays and checks each layer's own
visibility. Region parameters override layer parameters. Each region compiles
its own generator and mask. The renderer uses inverse-percentage transparency
and native signed-difference truncation. Source AddEffect initially rebuilds
Fire for the whole canvas; SetMask rebuilds it again for the selected bounding
box and consumes another2,300 CRT draws. Both builds are accounted for. A new
local preview stream starts at1, then retains its completed rebuild state across
accepted recompilations. This is not a claim of matching stock's current stream,
phase, fade envelope, dispatch buffer or callback backlog.

Spectrum runtime is separate from the editor's projection. Positive raw durations
are honoured, while missing/zero means37740; the three ordinary authoring choices
remain75480/37740/18870. Missing endpoint anchors are added using first/last array
colours. A controlled source replay corrected a previous static interpretation:
the converter's sorted temporary is discarded, so original array order reaches
the native initializer. Duplicate/out-of-order stops trigger its default
red/green/blue/red palette. Editing/export still preserves those imported values.
Missing gradients use the same native fallback. None uses the percentage control
byte, distinct from black. Greater-than-ten native descriptor cases and unsafe
tiny fallback durations are refused; no import data is truncated or rewritten.

## Lifetime and resource boundaries

Compilation runs on the existing Studio background worker before the device
engine lock. Immutable session-bound `ReadSnapshot` tokens share raw JSON without
cloning layer rows. A reply is accepted only for the requested open view, epoch,
generation and current snapshot. There is at most one pending compilation, even
through rapid stop/start; stale work is dropped. Previous frame plans are freed
before recompilation, avoiding retained animation-history volumes.

The plan has a32MiB dynamic-storage admission budget, plus bounded compilation
temporaries. One current Fire/Spectrum composition retains under100KB of plan
data. That is not total process RAM or a peak-memory measurement. One40ms Windows
timer advances one frame per callback, with no elapsed-time catch-up policy.
Painting does not recreate native EDIT fields every tick. Stopping, navigation
and window destruction clean up the timer/frame plan. Full app scheduling,
source dispatch-frame latency, shutdown during long compilation, resource
measurements and the complete-build100MB gate remain unfinished.

## Verification

An independently written static-instruction reference checks500 complete combined
frames (161 RGB pixels each), including crop, holes, reversed layer order,
transparency and the two Fire rebuilds. The synthetic numeric fixture derives
its114 listed cells and109 source selection paths from the pinned manifest;
it contains no personal profile IDs. Other tests check origin cancellation,
hidden layers, snapshot ownership and refusal of unsupported geometry/effects.

The opt-in native actual-export test opens your private original composition with
simulated devices. It verifies two-effect animation changes, unchanged dirty
state, stop/drop, hide/drain, and recompilation after a real native Hot edit.
The original export and device-profile store stay byte-identical and no device
SET is submitted. It captures the application's own production paint while
running; this is not a screenshot-based reconstruction or stock pixel comparison.
See [actual-export evidence](validation/native-studio-actual-file.json).

## Static source evidence

Native target SHA256
`cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa`:

| Operation | REA Evidence |
|---|---|
|SetMask JSON action35/24610|`ev_1507eb1982ffccd57411ee533b70f6b998e6713b6a3f486504446bc324cf1f21`|
|SetMask projection4c270|`ev_aa4888b22a764a4eb63ed7549478f255a46f0ccefa09f1f0ec2a0311a3e79907`|
|Mask bounds/rebuild52a50|`ev_8006edde94751b9cfd1b5948a7f77020991346df7d1978fbd351786b68e7eaa7`|
|GetCurrentFrame JSON action52/24f80|`ev_e9fa37784e92430e2320dd4e4b2faa51f6f65e002948191988705e107bcd47da`|
|Logical device sampling4c620|`ev_21d5ef237a960e2a4719b480f899acf8cceb2feba384dee33b96d4e529e33e82`|

Existing geometry evidence4a910/4ad90/4b910/4beb0 supplies attachment, resize,
origin and position conversion. Successful REA sessions closed without executing
the vendor DLL. Fire and container/palette records are linked from
[Fire](FIRE-LIGHTING.md) and [pixels](LIGHTING-PIXELS.md).

Lighting-engine main SHA256
`c82c7074e5da8abcffbff95f23a6b962c1e14b04586715b9d4b7a6f11fe9e6d2`,
module2578 hash
`008bf7b8aac4e86f47dd0c4e498402949c384d640553ca08d9c2424ad3527bda`,
defines the parameter converter. The six controlled replay cases execute only
that selected JS body in an isolated Node VM with a fake AddEffect receiver and
refused native initialization; no app/account/network/device access occurs.
The checked-in `studio-native-codec-replay.json` records inputs/outputs. Native
Spectrum initializer61e90 independently establishes strict-stop fallback and
default duration. Cached/live exact revision equality remains unverified.

The full goal remains active: preview does not complete hardware functionality,
all effects/tools, profiles, input recovery, privacy or final resource acceptance.
