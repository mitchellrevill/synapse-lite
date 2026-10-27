# Customize ancestor geometry and font baselines

The normal product toolbar is 38px by shipped CSS, rather than an assumed spacer. Recovered component ancestry and an inert populated DOM replay now establish the normal Customize origins at 125% display scale. The correction moves the keyboard's normal center 10px right, fixes its narrow drawer clamp by 5px and raises its control row 1px. It also uses the source 12px category text and aligns ten known text roles to their measured source baselines. This does not establish complete pixel parity.

## Ancestors and viewport

The recovered normal chain is:

```text
AppEngine WebContentsView (host offset42)
  MainContainer
    Toolbar38
    NavTabs48
    outer BodyWrapper (product y86, padding-top10)
      key-mapping-relative-wrapper
      BodyWidgets
        Customize
          drawer (absolute)
          inner BodyWrapper (absolute, top:auto)
            BodyWidgets
              product config wrapper/block
              populated config control row
              lower cards/widgets
```

The late CSS cascade gives the inner wrapper `height:calc(100% - 96px)`, `position:absolute`, `top:auto`, `left:0`, `min-width:0` and 100% width. Its static-position origin is product y96. Opening the drawer sets left 230 and width `calc(100% - 230px)`; its inner widgets are at least 1000px wide. The source enables both overflow axes. These are normal-mode rules; Macro/Armory branches have additional overrides.

For a 1280x678 product viewport, the source replay measured toolbar y0/h38, navigation y38/h48, outer body y86/h592 and inner body y96/h582. The mouse image starts at product y106 (native-client y148 after the 42px host); the keyboard image/SVG starts at product y112 (native y154). The mouse control row starts at native y508, and the keyboard row at y553. The keyboard image remains 730x340 while its SVG is 730x387.

The earlier keyboard replay supplied an empty control row. That synthetic empty flex item changed horizontal line layout, yielding an incorrect normal center `width/2 - 10`. Using the actual populated row yields `width/2`. The correct drawer center is `max((width+230)/2,750)`:755 at 1280px,750 at 1266px and750 at 1000px. Both product styles were replayed at 1280/1266/1000 widths with closed/open drawers, using the complete ordered styles and recovered fonts. No inline toolbar height was supplied.

The lower keyboard cards were omitted from the origin replay. Their existence is established by source, but the full vertical content scroll extent is not claimed. Native body scrolling and the remaining lower content still need fidelity work.

## Baseline correction

The translation atom normally returns a Fragment containing text. The replay gives the marker a text box equivalent to that anonymous flex item, rather than treating the marker as a separate flex child. A zero-size marker records its baseline. For parent-aligned inline text, the marker is measured relative to the actual drawing box rather than its glyph span.

| Role | Source font | Baseline from drawing-box top, CSS px | Previous GDI baseline at 125%, physical px | Corrected baseline, physical px |
| --- | --- | ---: | ---: | ---: |
| Popup header | Roboto14 |24 |28 |30 |
| Expanded category | Roboto12 |24 |31 (wrong14px font) |30 |
| Heading | RazerF5 16 |15 |22 |19 |
| Default value | Roboto14 |19 |24 |24 |
| Dropdown selection | Roboto14 |18 |23 |23 |
| Dropdown option | Roboto14 |17 |not measured |21 |
| Turbo label | Roboto14 |13 |17 |16 |
| Form button | Roboto12 |18 |22 |23 |
| Preview layer | Roboto14 |17 |21 |21 |
| Mouse label | Roboto14 |19 |25 |24 |

The native renderer places the baseline at the nearest physical pixel of `(CSS top + measured baseline) * scale`. It retains the source clipping box and native horizontal alignment. Clipping is restored independently of the open glyph path, so the actual drawing primitive can be inspected as vector outlines. This avoids treating GDI's font bounding-box height as the CSS line-box height: at 125%, the 20px RazerF5 font has 28px GDI metrics while the source 19px CSS line box occupies 23.75 physical pixels.

The simulated HWND test captures glyph paths from the actual rendering function, compares them with a baseline-zero reference and checks all ten expected origins. It also verifies that GDI selected Roboto/RazerF5 and that all three private font files loaded. No raster screenshots are involved.

Glyph advances still differ: for example, the measured RazerF5 heading is 161 physical pixels in GDI versus about 159.14 from Chromium's source-font metrics at 125%. Baseline equality therefore does not imply identical glyph placement or raster antialiasing. Native EDIT, multi-line text and other unmeasured roles remain outside this correction.

## Separate outer/client gap

On the simulated native window at 125%, outer bounds were 1600x900 physical pixels, client bounds 1584x884 and the client origin was inset 8px per side. The requested logical outer size was 1280x720. AppEngine's recovered bounds function instead uses `BrowserWindow.getSize()` with product bounds `{x:0,y:42,width:size[0],height:size[1]-42}` on the current Windows11 branch. Its version helper classifies build 26200 as Windows11 using the 22000 build threshold. A separate full-client/non-client resize correction is planned; no `WM_NCCALCSIZE` behavior changes in this unit.

## Evidence and validation

[The machine-readable evidence](validation/customize-ancestor-fonts.json) includes original source SHA256 values/offsets, twelve populated geometry cases, browser/source-font measurements, before/after GDI observations and the native outer/client measurement. REA parent`ev_20a1225ede84b09491c9d0f84bdd3c54ef3fe9c1d4742b06114ff60361c4954c`, summary`ev_3cb3655bda78c3f2f2b9384c3c564b10f9d0e98588e400578cc2b50dceae8f19` covers ten focused source projections,74081 bytes, no parse failures and 5054 semantic unknowns. Specific AST/CSS facts and controlled replay supplement that partial graph.

The focused model/layout tests and all six simulated native HWND regressions pass. Assignment interactions still add no simulated commands and leave profile bytes unchanged. The global frame change, remaining forms and full pixel parity are not included. No RAM measurement covers this correction.
