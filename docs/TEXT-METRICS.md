# Source text metrics and DirectWrite integration

Natural DirectWrite layout improves the measured source-font advances. On 28 October 2026 the bridge was linked into the native application for assignment headings and toolbar titles, then extended to host tab labels. The initial two-role evidence is retained below; the final section documents the tab extension. This does not establish raster or full UI parity; other text roles still use the existing GDI path.

The comparison uses the recovered Ornata605 and DeathAdder196 CSS, exact regular source fonts, inert component DOM, and network-blocked Edge156.0.4314.42 at100% and125%. It runs no vendor JavaScript, stock UI, screenshots or hardware operations. The twelve source labels include `RAZER ORNATA V2` and `RAZER DEATHADDER V3 HYPERSPEED`, with capitalization imposed by the shipped toolbar CSS. Ten additional diagnostic strings cover `AVATAR`, `To`, `fi`, `ffi` and their combined sequence in both fonts.

| Text / source role | Font / weight | DOM advance100% | Natural DWrite100% | GDI100% | DOM advance125% | Natural DWrite125% | GDI125% |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| MOUSE FUNCTION / assignment heading | RazerF5 16 /400 |127.3125 |127.3120 |128 |159.1406 |159.1400 |161 |
| MOUSE FUNCTION / category | Roboto12 /400 |101.2656 |101.2559 |102 |126.5820 |126.5698 |128 |
| Double Click / dropdown | Roboto14 /400 |77.3594 |77.3486 |75 |96.6992 |96.6858 |98 |
| CANCEL / button | Roboto12 /400 |45.2813 |45.2813 |46 |56.6016 |56.6016 |58 |
| RAZER ORNATA V2 / toolbar | Roboto14 /400 |119.9688 |119.9570 |121 |149.9609 |149.9463 |155 |
| RAZER DEATHADDER V3 HYPERSPEED / toolbar | Roboto14 /400 |243.6406 |243.6328 |243 |304.5508 |304.5410 |313 |

Advances above are physical pixels. DOM `Range` widths are quantized to1/64 CSS pixel; natural DirectWrite differs by less than that quantum for all22 strings at both scales, with maximum absolute difference0.01367188 CSS pixel. The ten earlier UI labels' natural advances agree with Chromium canvas measurements within0.00012 physical pixel. Canvas defaults are not a general authority: combined Roboto `AVATAR To fi ffi` measures96.5098 with canvas `fontKerning:auto`,96.2363 with canvas `normal`,96.25 in the DOM, and96.2363 with natural DirectWrite.

Explicit pair-kerning enablement and an explicit enabled `kern`/`liga`/`clig` feature list produce the same widths as unmodified natural layout for this set. Feature controls preserve the other enabled features while disabling one category: disabling kerning changes Roboto `AVATAR` from50.75 to53.2793; disabling standard/contextual ligatures changes Roboto `fi` from7.7520 to8.2578. RazerF5's `fi` difference comes from kerning, not the tested ligature controls. Keep default natural shaping; no feature override is justified by this evidence. GDI-compatible DirectWrite modes retain rounded advances, including the161px heading at125%, and are unsuitable for this correction.

The authoritative baseline remains the source component's line box. The assignment heading is15 CSS pixels from its drawing-box top; the toolbar title is23. Their physical offsets are15/18.75 and23/28.75 at100%/125%. DirectWrite's default baselines are17.312 CSS pixels for RazerF5-16 and13.302734 for Roboto14. Using those defaults directly would move the text. The pilot subtracts the natural layout baseline and applies the recovered source baseline. It also compensates rounded physical clip origins: source top189 at125% with clip top236 still yields absolute baseline255, rather than254.75.

The complete measured weights, computed CSS, DOM ranges, line boxes, font hashes, native modes, source stylesheet hashes and existing REA ancestor evidence are in [validation/text-natural-metrics.json](validation/text-natural-metrics.json). Every source label has weight400; no bold simulation was used. Both fonts cover every tested character without missing glyphs or simulated styles. These measurements do not cover arbitrary scripts, localized fallback fonts, multiline text, native EDIT, text overflow/ellipsis or all source roles.

## Pilot ownership and integration boundary

`native_editor/source_text.rs` and `source_text_bridge.cpp` are now built and called by the native application. The Rust owner is window-thread-only and exposes three fixed roles: `AssignmentHeading` (RazerF5-16/400, left, baseline15), `ToolbarTitle` (Roboto14/400, centered, baseline23) and `HostTab` (Roboto12/300, left, baseline11 from its title box). Do not apply the first role to unmeasured Lighting/widget headings merely because their font size matches. `SourceBox` uses absolute client CSS coordinates; `Clip` uses physical pixels. Pass the already-localized uppercase source label.

The bridge uses official SDK COM declarations. It reads exactly72,204 RazerF5 bytes,35,408 Roboto Regular bytes and35,468 Roboto Light bytes, verifies their recorded SHA-256 digests with Windows BCrypt, and passes those same buffers to `IDWriteInMemoryFontFileLoader::CreateInMemoryFontFileReference`. A null owner makes an immutable DirectWrite-owned copy under the SDK contract. The diagnostic deletes all three input paths before drawing, which still succeeds; same-size altered files fail authentication. There is no hash-then-reopen claim. The loader is registered with an isolated Factory5, and layouts/formats/faces/collection are released before unregistering it and releasing the factory.

Storage is bounded: three font copies totaling143,080 bytes, a single total32-entry role-aware LRU, at most256 UTF-16 units per label, and one reused bitmap limited to2048x256 physical pixels. Cache misses may temporarily hold one replacement layout. These are bounds on our resources and input, not a proof about DirectWrite's internal memory. All context/font/callback allocations use explicit Windows heap failures; the bridge uses no STL containers or C++ exceptions. It returns a status and HRESULT on missing/altered fonts, unavailable Factory5, unsupported glyphs/controls, invalid bounds, allocation, layout, bitmap, render or copy failures. The final destination is copied only after successful scratch rendering. Parent integration caches creation failure, retains an explicit diagnostic and uses the existing GDI fallback; no repeated paint-time initialization or repeated error dialogs.

Factory5 is gated by `NTDDI_WIN10_RS2` in the installed SDK. Runtime interface acquisition is checked; older Windows10 installations without it must fall back visibly. The pilot uses the same Windows10 BCrypt primitive already used by this application. Windows11 was tested; earlier systems were not. Rendering uses current Windows DirectWrite rendering parameters. Antialiasing, gamma, ClearType geometry, hinting, clipping edge pixels and monitor-specific behavior are not proven equivalent to Chromium/Skia.

The integration adds the already-locked `cc1.7.0` as a build dependency, compiles this one bridge only for Windows/MSVC with C++17, `/MD /EHs-c- /GR- /D_HAS_EXCEPTIONS=0`, and links `dwrite`, `gdi32`, `bcrypt`. It does not enable C++ exceptions or dynamically link the STL. The earlier standalone FFI harness imports no `MSVCP140.dll` or `VCRUNTIME140_1.dll`; production imports are checked separately. A non-MSVC Windows build is rejected explicitly rather than silently leaving unresolved bridge symbols.

`Fonts` owns a cached `Renderer`. Initialization and draw failures are retained, use the existing GDI fallback and display a local fallback notice; painting never repeatedly initializes a failed renderer. Font/DPI recreation creates a fresh context. The normal paint path calls the bridge for toolbar titles and assignment headings. The read-only frame report includes `source_text_fallback` and `source_text_cached_layouts` so an initialized factory alone cannot be mistaken for a successful draw.

Two Rust/linked-bridge tests pass: missing-font failure retention, and both measured roles at 100%/125% with natural advances and fractional source baselines. The simulated Customize interaction regression also passes with process-private fonts, preserving drafts and profile bytes and issuing no simulated SETs. The old GDI-vector heading assertion was replaced with an assertion of the actual DirectWrite call site's authenticated-font state and source-baseline metrics; other GDI roles keep their vector checks. The first sandbox run could not register private GDI fonts and reported Arial fallbacks, so it was not accepted; the same isolated simulated regression passed outside the sandbox. This is not physical-device or optical acceptance.

## Bounded diagnostics

From an x64 MSVC Developer Command Prompt, use the Windows SDK already installed. `tools/probe-source-text.cpp` measures the original two-font dataset; `tools/test-source-text-pilot.cpp` tests offscreen draw, fractional baselines, three source-byte copies, altered-font refusal, the total LRU bound, unchanged destination on invalid requests and cleanup. Build the smoke with C++17, `/EHs-c- /GR- /D_HAS_EXCEPTIONS=0`, and link `dwrite.lib gdi32.lib user32.lib bcrypt.lib`; run it with the source-font directory as its only argument. Define `SOURCE_TEXT_PILOT_TEST` to check five successful loader-unregister calls and zero outstanding font objects. It creates and deletes only its own temporary font copies. The diagnostic executable itself uses standard-library output; import assertions apply to the separately linked Rust/bridge harness, not that diagnostic executable.

The original offscreen smoke passed, including four successful loader-unregister calls after normal and early-failure cleanup, zero font objects at unregister, and restored GDI handle count. The Rust FFI harness also passed both roles and error propagation. OS allocation/COM failure injection is not exhaustive. Nonempty glyph pixels were checked without saving an image. No raster equality comparison, whole-app typography parity or final RAM acceptance is claimed.

The initial two-role release build passed a five-second read-only simulated dashboard smoke: the production frame is presented/matched, retains no typography fallback and reports one cached natural layout. See [validation/text-integration-runtime.json](validation/text-integration-runtime.json). In that sandbox the legacy GDI private-font count is zero; the successful DirectWrite draw uses its separate authenticated in-memory font collection. Production import inspection confirms DWrite.dll and no MSVCP140.dll or VCRUNTIME140_1.dll dependency. Resource acceptance and lossless stock/native raster comparison remain outstanding.

## Host tab labels: natural size and baseline

The current third role is `HostTab`: Roboto12/300, left aligned, baseline11
from the source14px title box. At default tab y8 the title starts at y18,
baseline29. DirectWrite's default baseline11.40234375 is corrected explicitly.
Tab text measurement uses the same authenticated face and32-entry layout cache
as drawing, without allocating a measurement bitmap. All three faces must
match their expected weight and have no simulated style.

Roboto Light's legacy TTF family is `Roboto Light`, but the SDK collection
groups both Roboto faces under `Roboto`; collection lookup of `Roboto Light`
fails. The runtime uses verified `Roboto`/weight300, not a guessed family or
simulated light face. The Light SHA256 is
`8cb65fe121080cb8721fb9b643f6b096d133d4b72b8575f6529ec458613dcc78`.

| Tab label | DOM advance CSS | Natural advance CSS | Fractional outer CSS width | Source JS clientWidth |
|---|---:|---:|---:|---:|
| SYNAPSE |51.734375 |51.732421875 |107.734375 |108 |
| DEATHADDER V3 HYPERSPEED |168.078125 |168.0703125 |224.078125 |224 |
| ORNATA V2 |62.28125 |62.267578125 |118.28125 |118 |
| AVATAR TO FI FFI |92.78125 |92.77734375 |148.78125 |149 |

The32 source-CSS fixtures cover four labels at DPR1/1.25/1.5/2 and offsets0/0.25.
Natural and DOM widths differ by at most0.013671875CSS, below the1/64CSS DOM
quantum. Stock JS rounds clientWidth for slot positions while the actual CSS
box remains fractional; native tabs now preserve that distinction for painting,
clipping and hits. The source45px fade is applied after the clipped text.

`tools/probe-host-tab-text.cjs` reproduces the inert replay with explicit local
evidence/output/Playwright/Edge paths. It blocks network, loads archived font
bytes, executes no vendor JavaScript and takes no screenshots. Edge156.0.4314.42
and a repeat with156.0.4314.50 produced identical geometry, advances and
baselines. See [all DOM fixtures](validation/host-tab-dom-metrics.json) and
[the consolidated report](validation/host-tab-text.json).

Three linked Rust tests pass, including all three draw roles at four scales,
natural measurement/slot widths, invalid bounds and the shared LRU limit.
The updated standalone smoke passes all three fonts after deleting their
source paths, rejects same-size corruption of each, verifies a fractional tab
baseline36.5625 at125%, and restores GDI resources. Native simulated mouse
navigation checks the actual Home width and no retained natural-text fallback;
frame resize/maximize checks still pass without profile/device writes.

The live stock font stylesheet permits system-local lookup before the shipped
WOFF files. Forced archived-TTF replay therefore does not prove the font chosen
by the live stock app. This remains metric/geometry evidence, not raster
equality, arbitrary-script/fallback coverage or complete-build RAM acceptance.
