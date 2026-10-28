# Native Customize assignment popup

The installed Ornata V2 (605/025D) and mouse196 (00C4, with197/00C5 family alias) Customize pages now open a native source-derived assignment popup. Keyboard clicks use the recovered SVG paths. Mouse clicks use the shipped clickable label rows, not approximate regions over the product image. This is an interactive draft editor; assignment Save remains disabled for these devices because Ornata mapping readback and the mouse layer alias are not verified.

The popup uses the existing owner-bound snapshot of the persisted desired profile. Its complete opaque profile and matching source mapping records remain unchanged. Missing source metadata is shown as unknown; absent desired mappings are not hardware readback. A changed device, active profile or retained profile payload received in an ordinary snapshot invalidates an open draft. Opening, editing, cancelling and discarding submit no device job and save no profile.

## Recovered geometry and behavior

The source popup is 292px wide, with a 36px header and 40px category rail that expands over the form to 250px on hover. Body geometry is `(41,1,250,height-2)` relative to the popup. Popup height clamps product viewport height minus 140 to 310..570. Normal inline top is 36px inside the source wrapper at product y90; the native host contributes a separate 42px. Right/left anchoring uses the actual key/label bounds,292px width, strict 20px clearance and the 230px drawer clamp. Category order follows each input's recovered function list and the current preview layer.

Default, Disable and Mouse Function forms have distinct source footer positions. Mouse Function provides all ten source mouse choices and its input-specific Turbo exclusions. Turbo defaults to 7 presses/sec, with controlled integer input 1..20,300ms step repeat and a pointer slider. The dropdown has25px rows and a 180px viewport. The 230px drawer has a 67px header, explicit All Buttons/Customized choices, source row ordering, wheel scrolling and draggable scrollbar. Standard/Hypershift changes the local preview layer. Source categories whose forms are not implemented show an explicit unsupported state; they do not translate unknown records or enable Save.

Cancel directly discards the draft. Closing a dirty Mouse Function draft shows the recovered 400x213 Save Remapped Button alert. Dismissing it keeps the draft; Don't Save follows the pending transition. Save remains visibly disabled and the form explains the protocol limitation. Source primary Left Click stays disabled. Opaque source records survive all these paths.

## Evidence and local assets

Artifact URLs, SHA256 hashes, UTF8 offsets and REA evidence IDs are recorded in [validation/customize-assignment-native.json](validation/customize-assignment-native.json). Source605 popup1270 and source196 popup257 share the shell. Source196 `main.bbc961f9.js` canvas metrics, label component and canvas component start at UTF8 offsets4717771,5297820 and5349364. Source recovery used retained REA analysis plus bounded AST/source excerpts. Semantic coverage is partial. Inert reconstructed DOM/CSS/source-font replay measured normal/narrow layouts; vendor JavaScript was not executed and screenshots were not used.

Run the existing font/product provisioner first. Additional assignment assets come from your recovered local cache:

```text
node tools/provision-assignment-assets.cjs
  --media <605/ui/static/media>
  --mouse-media <196/ui/static/media>
  --stylesheet <605/ui/static/css/main.f65da71b.css>
  --output <ignored-runtime-assets>
  --playwright <local-playwright-module>
  --browser <local-Edge-or-Chromium-executable>
  --scale <physical-pixels-per-CSS-pixel>
```

The command is shown across lines for readability; pass the arguments in one shell command. Scale defaults to1; current validation used 1.25. It provisions 58 private assets, including category state variants, background-correct close icons and the source cycle-button artwork. Network requests are aborted. Chromium decodes SVG/CSS or AVIF and flattens straight-alpha output to BMP. Saved BMP RGB must exactly match its browser Canvas reference. Generated proprietary assets and source bundles remain outside the public repository. `SYNAPSE_LITE_ASSETS` selects the local asset directory.

## Verification and limits

All 168 ordinary tray tests passed (eight opt-in tests ignored). All six simulated native HWND regressions passed, including the pre-existing Lighting, Power, profile and overlay flows. Two assignment regressions exercise actual window messages for source key/label hits, category/dropdown selection, Turbo numeric rejection/repeat/drag, disabled Save, dirty alert, discard/cancel, drawer filter/anchor/wheel/thumb and Escape. Both assert zero added simulated commands and byte-identical profile storage. These checks never discover physical devices.

This is not full pixel parity. Asset conversion has pixel equality against Chromium; GDI text/path antialiasing, animations, header search, hover tooltips, all remapped binding descriptions and all remaining category forms are incomplete or unverified. Normal-mode toolbar and Customize ancestor origins are now source-bound; see [the geometry/font correction](CUSTOMIZE-GEOMETRY-FONTS.md). The native outer/client inset is addressed by [the full-client frame correction](FULL-CLIENT-FRAME.md). Native EDIT's visible text region is clipped before the source spinner overlay. Scrollbar geometry has a 20px safety floor that does not affect the validated normal layouts. The explanatory unsupported-Save text and permanently disabled assignment Save are deliberate functional adaptations. No new RAM measurement covers this UI change.

## Display scaling correction

The keyboard selection outline now converts the logical key viewport to physical pixels before tracing its GDI path. Previously the image and click targets scaled with display DPI while the outline used unscaled coordinates, shifting it up and left and reducing its size at 125% and higher. The source image, key paths, hit testing and popup anchoring are unchanged.

A synthetic-key GDI regression checks actual emitted path coordinates against image and hit positions at 100%, 125%, 150% and 200%, including the drawer and scrolling. The isolated alignment-only build passes the native assignment interaction test, which also checks unchanged profile storage and no added simulated device commands.
