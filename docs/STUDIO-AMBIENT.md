# Ambient Awareness source contract

29 October 2026. Static inspection only. No vendor bootstrap, desktop capture,
screen selector, native effect, device request or profile write executed.

## Authenticated sources

Source base: task root work/synapse-renderer/cache/apps.razer.com/synapse/.

* Studio static/js/main.88ac681e.js: 436867 bytes, SHA256
  109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19.
* Studio static/js/958.0afb7ebc.chunk.js: 7399 bytes, SHA256
  e440842d98f73fe8a8f61cf7d2e7e746ac9a5ca6eddc39e1932e3b161fcad759.
* Lighting Engine static/js/main.a383ea9e.js: SHA256
  c82c7074e5da8abcffbff95f23a6b962c1e14b04586715b9d4b7a6f11fe9e6d2.

REA chunk-only parent ev_488d7babb96a60b67b99bd693d0261d439d09fd3dff75b51a32f1448e562b5ab,
summary ev_ceb7151af3300edf3e822a5b44c32dd3a3c71ed81e1ccfe518cc3c365c4a1d8d.
REA Studio main+chunk parent ev_792cf8d58ede97696189eff45c195daf026574cce69b686f6e05753f7bf573fa,
summary ev_93eea144c5aa9f6f879deccde7f1f8c2fc1443baeb2ca4b510a42b2f044954bd.
Application parsed completely; semantic graph partial/truncated at100000 node
ceiling,24962 unknowns. Do not equate this with complete behavioral recovery.

REA exact contiguous engine assignment extract char414792..418977 parent
ev_ad8d25efd03f536234cbb9a6b8535ab3bbfeb883054ab7b8a84ea05d0d9ac2a2,
summary ev_0f47c785aca298bc41c10cac89dfe265341c30abf2e14d9f853979fecb9f85f5.
Application complete for this selected excerpt; semantics partial/unknown,
178 unknowns. Scope is one selected assignment, not whole engine.

## Observed parameters

Studio main module2904 c9/rH, char60094 and61874: Ambient params are
{screen,screenRect,blur}; default-enabled region {screen:"full",blur:5}.
Default-disabled region omits screen (undefined) but keeps blur5. screenRect is
absent until custom authoring. No Ambient colour, colourStops, speed, duration
or playback settings are exposed by this constructor/normalizer.

Module6257 char183259: screen presets ["full","left","top","right","bottom"];
default full; screen coordinate scale65535; blur min1,max9,default5.
The chunk958 slider inherits integer step1 and labels the parameter Blend,
with Sharp and Blur endpoints. Legacy XML Gradient maps to this blur scalar,
default5; it is not a colour gradient.

screen is a region preset name, not a monitor index. Custom rect shape is
{Left,Top,Right,Bottom}, all capitalized. Native AddEffect resolves:

| preset | Left | Top | Right | Bottom |
|---|---:|---:|---:|---:|
|full|0|0|65535|65535|
|left|0|0|6553.5|65535|
|top|0|0|65535|6553.5|
|right|58981.5|0|65535|65535|
|bottom|0|58981.5|65535|65535|

Custom screenRect passes unchanged. Source permits fractions; do not coerce
the source-shaped raw fields to integers during persistence. Unknown/missing
screen falls through to full at the engine conversion boundary, but an editor
should not silently replace imported unknown fields.

chunk958 onRegionSelected parses comma-separated pixel endpoints and scales
x coordinates by65535/screenWidth and y coordinates by65535/screenHeight.
The browser custom-region handler in Studio main char423897 similarly scales
normalized endpoints by65535. Screen selection itself is native and must not
be invoked unattended.

## Observed UI

chunk958: Region section, preview, Edit Region button, Presets, Properties
section, Blend slider. Region/screenRect mixed selection marks the preview
mixed. Presets dispatch only screen (retaining last custom screenRect).
Custom preset becomes visible after Edit Region in this view session.

CSS958.eb094e3a.chunk.css: preview height128px, background#111, border1px#707070,
selected box#ccc; preset boxes white. Mixed hides the box and shows gray
question mark. English chunk362 helper: This effect may require your game or
application to run in border less window mode.

No ambient-specific stock screenshot was found among the named retained
captures inspected. Shared Studio screenshots do not prove Ambient pixel parity.

## Native boundary and unknowns

Effect ID11 resolves JS params to {ScreenRect,Blur} and invokes
lightingEngine.AddEffect(engineHandle,11,params). This is a handoff, not the
screen-to-device projection algorithm. The inspected JS does not establish
screen sampling kernel, gamma space, HDR behavior, LED projection, alpha/layer
blend equation, native frame cadence or resource ownership.

Existing retained lighting-runtime/all-effect-rtti.json identifies DLL SHA
cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa and
CAmbienceEffect descriptor RVA0x1f5170, vtable RVA0x1abfb8, leading candidate
methods RVA0x5d890,0x5cf80,0x5d0b0. These are historical candidate addresses,
not newly verified signatures. Existing native analyses record provider timeout
at another effect procedure; unchanged provider calls were not repeated here.

TODO: repair provider startup; authenticate DLL; trace CAmbienceEffect methods
and constructor xrefs; recover/inject a synthetic-only capture boundary before
any native Ambient runtime probe. Establish rectangle rounding, Blur1..9 kernel,
global device placement/LED projection, alpha/layer composition and cadence.
Until then synthetic preview must be labeled diagnostic reconstruction and
cannot establish vendor-equivalent or physical output.

## Implemented boundary

The native editor now adds Ambient Awareness layers, authors the five source
presets, accepts fractional normalized custom rectangles, and edits blur1..9.
Each selected regional edit uses the existing raw-preserving partitioner and
one history entry; save/discard/undo/redo retain profile identities, offline
memberships and opaque fields. Presets retain the previous custom rectangle.
There is no screen-index selector because the recovered screen field is a
region preset. Unknown colour/gradient/speed fields remain raw and inert.

SYNTHETIC TEST explicitly starts a ten-second local diagnostic at40ms ticks.
Only generated64x36 opaque moving colour bars enter it. No screen-capture API,
file recording, upload, vendor effect execution, or input interception exists
in this path. Stop, close and stale snapshot handling drop the retained plan;
pending results carry mode admission so restart cannot lose the deadline.
The pure sampler also accepts caller-owned RGBA8 buffers for deterministic tests.
They are not screenshots captured by this implementation.

Diagnostic geometry maps selected logical LED bounding-grid centers to the
chosen normalized rectangle. Sharp1 samples one nearest pixel; blurN uses a
clipped square box of radiusN-1, alpha-weighted RGB and percent transparency.
It composes through the existing retained transparency layer math and source
layer ordering. This is a documented reconstruction, not vendor sampling,
blur, alpha or projection equivalence. Eight MiB maximum input,262144 maximum
taps per frame, existing32MiB composition limit and10s UI lifetime bound work.
The existing exact single unrotated Ornata layout eligibility remains in place.

Physical Studio compilation uses the ordinary constructor and rejects visible
Ambient with an explicit unverified-native-sampling error before frame output.
No live capture or physical Ambient lifecycle is advertised as implemented.
Existing Static/Spectrum/Fire/Breathing/Wave output admission is unchanged.

## Visual comparison and remaining work

Compared native controls against retained chunk958 component/CSS and normalized
shared Studio captures; no Ambient-specific stock capture is available. Native
controls expose the same settings, source preset order and region-indicator
colours. The small region indicator replaces the source128px preview, text
fields replace the native custom-region overlay, preset buttons replace its
dropdown, and numeric blur replaces its slider. These are explicit visual gaps;
shared tabs were not changed. The HWND painter captures only its own window,
not the desktop; existing parent-painter evidence omits native child EDIT
contents, whose values are verified through saved JSON.

TODO after provider repair: prove vendor spatial projection and blur kernel
using injected synthetic capture; then implement an explicit bounded live-capture
opt-in with handle cleanup, monitor/DPI/HDR semantics and output cancellation.
Physical proof requires the user and remains TODO. No total complete-build
memory claim or all-device compatibility follows from this component.

## Validation (29 October 2026)

Offline workspace run:824 unit tests passed,54 intentional skips, one doc test
passed. Focused final Ambient core run:6 passed. Focused tray admission and
pending-result lifecycle tests:2 passed (HWND check intentionally excluded from
that ordinary run). Final opted-in isolated HWND run:passed in17.68s, including actual
native text control values saved to a separate composition, save/undo/redo,
invalid blur/nonfinite rectangle atomic rejection, read-only inertness,
synthetic preview, automatic10s cleanup, restart and explicit Stop. Source
export and isolated device-profile baseline remained byte-identical. Zero
simulated device requests; physical discovery disabled; no desktop capture.

The first broader test attempt hit32 Windows sandbox TEMP profile-rename
errors; rerunning with a workspace TMP/TEMP passed. A concurrent rebuild could
not replace a running test executable; the native check was rebuilt and run
sequentially. These environment issues did not require changing storage code.
Machine-readable evidence:validation/studio-ambient-authoring.json. Private
source/export/assets and owned-window PNGs are excluded from Git.

A later final-binary HWND retry exposed a busy-host startup delay:the owned
frame report appeared at10s after the generic4s startup wait had expired.
The isolated test now allows20s for startup and60s total window lifetime;
this does not extend the product synthetic preview's10s deadline.
The final run with the bounded startup budget passed in17.68s.
