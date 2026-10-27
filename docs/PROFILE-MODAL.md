# Profile Import/Export modal

The native modal for Ornata V2 (605) and DeathAdder (196/197) follows the recovered renderer components and CSS. No screenshots were used as the design baseline. This is a working selection/preview/commit flow; whole-application pixel identity remains unproven.

## Source layout and behavior

The modal is 602 Ã— 481 CSS pixels, at x=viewportWidth/2-300 and product-local y104. The separate native host contributes 42 px, giving native y146. The source intentionally differs by 1 px from centering a 602 px box. Border #515151, radius 5 and product backdrop black alpha 179/255 follow the source.

Export has a 40 px selection header,317 px list and86 px footer. Import has a 49 px Local/Cloud switch,47 px browse row,40 px selection header,247 px list and60 px footer. Profile checkboxes are 20 px; expanded macro rows are 31 px. The source custom scrollbar uses an 8 px track,6 px minimum thumb and 1 px transparent border; thumb capture/drag, wheel and track paging are connected. Browser thumb rounding/page behavior uses [Chromium implementation evidence](https://chromium.googlesource.com/chromium/src/+/5246ef8d724d69e63169a4ac8733a056738091bd/third_party/blink/renderer/core/scroll/scrollbar_theme.cc) alongside shipped CSS.

Cloud is disabled as in the source. Its measured English Roboto 16 message box is 352.953125 Ã— 80, at modal-relative (124.515625, 236.5); text is 260.953125 Ã— 19. The recovered source can reset visible macro checks through Select all while retaining stale action-selection flags. The native modal preserves the visible transition and synchronizes available macro action flags, so checked macros are actually included. This fixes a source behavior defect without changing the layout.

## Bounded profile operations

Browse parses on a worker and never imports immediately. Browse/cancel do not save profiles or activate hardware. Preview replies carry a dialog token and device/profile owner; commit rechecks the fresh owner. A changed profile or disconnected device fails visibly.

GUI Synapse preview preserves the file's profile-array order. Each corrupt profile/macro payload or hash is excluded independently with a warning; valid records remain available. An all-invalid file has an empty list and disabled Import. The original 2 MiB document, JSON complexity and decoded-payload bounds apply before filtering. Native backups and CLI codecs retain strict validation.

Selected Synapse imports receive fresh profile GUIDs and case-sensitive unused name suffixes, remain inactive, remap colliding macro identities and remove deselected/unavailable macro bindings. Export initially selects the active profile and honors profile/macro choices. Native previews recover retained source profile-array order by GUID, preserving position across renames and omissions after deletion; profiles without retained source order follow afterward in native name order. Selected Synapse exports follow the displayed row order. This does not infer creation chronology for native-only profiles or alter the original full backup. The codec regression covers a nonlexical source list, rename, deletion, a new native profile, selected output order and opaque-field preservation; all11 GUI codec tests pass. Macro execution, linked-game application and Synapse 3 ZIP migration are not implemented.

Full native backup is an explicitly separate Save format and readonly all-selected import preview. It retains every saved device/profile and opaque extension; it does not silently filter foreign devices. The existing CLI/native backup merge semantics remain unchanged. Source-envelope composition happens only in temporary GUI export candidates; conflicts refuse Synapse translation while full backup remains available. Imports do not apply hardware settings.

## Local source assets

Use the locally recovered media directory and an installed Playwright module/Chromium executable. Set the three PowerShell path variables below to those local locations; generated vendor assets must stay private and ignored.

```powershell
node tools/provision-profile-modal-assets.cjs `
  --media $RecoveredMedia `
  --output target/native-assets `
  --playwright $PlaywrightModule `
  --browser $ChromiumExecutable `
  --scale 1.25
```

Scale defaults to 1. At 125% DPI the logical20/40 px icons are provisioned as 25/50 px bitmaps, matching the native painter's explicit logical bounds. Chromium decodes the SVG CSS/classes and flattens the image directly over the source background. The saved BMP RGB is compared byte for byte against browser Canvas pixels. Separate straight-RGBA checks detect alpha interpretation errors; no premultiplied samples are double blended. The former PyMuPDF/Pillow converter was removed because it ignored source SVG classes and mishandled alpha.

Local provenance records source/output/RGBA/RGB hashes, browser engine/version, scale and color/alpha counts. No network, vendor JavaScript or screenshots execute during conversion. Source 605 renderer SHA256: 307e2b199037ed53a8fde90d76fc67a9a3766a9361fb152e7820d8bf05f19c65; CSS SHA256: d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4. Raw bundles/assets are not bundled in the public repository.

## Validation and limits

The [sanitized native result](validation/profile-modal-native.json) records actual HWND mouse messages, worker parsing/persistence and file-output assertions on a synthetic Ornata fixture. Physical discovery is disabled. The three native Lighting/Power/profile regressions passed sequentially after the asset correction. These checks prove the exercised simulator flow, not physical hardware behavior or acceptance by stock Synapse. Existing CLI tests and actual private payload round trips remain separate evidence.

GDI font rasterization/antialiasing, source transition animation, Cloud box shadow, held-track scrollbar autorepeat/snap-back and contextual-tooltip raster fidelity have not been certified pixel-identical. OS Open/Save dialogs and the full native backup controls are explicit native adaptations. Three passes of the [read-only modal/color-editor views](OVERLAY-RUNTIME.md) passed the memory threshold at a declared checkpoint; large imported documents, active workflows and final-build acceptance remain pending. Asset pixel equality is limited to the converted icons and declared backgrounds.
