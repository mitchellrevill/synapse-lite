# Visual reference and fidelity gate

The reference is the installed RazerAppEngine 4.0.827 and its cached Synapse product interfaces, recovered using REA and deterministic artifact parsing. No screenshot is the implementation baseline.

| Source | SHA-256 |
| --- | --- |
| AppEngine electron/index.css | 30e429ff6289bd694e8c519a73ef42f7cb181e11c8687d44fe59893ecf95792c |
| Dashboard shared 55.4e8559cb.chunk.css | aba3cdc0ae2b3f36197ee327cc9a681a5680cf9117b6ee1bf59a867ebb57daf8 |
| Dashboard App.5fe618fc.chunk.css | a5a6809d62e4baab4c6caae6d33393201caa060808fef351d13ddecb188169b4 |
| Product 605 UI main.f65da71b.css | d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4 |
| Product 196 UI main.116d273e.css | cb6c673798b7e9bc844829d41cea64b5a0ab2d389f8863604df3bdcccc68138c |

Focused REA dashboard/window-tab analysis is retained as Evidence `ev_ddc10b59b092ad7c21fab5fbf2efc155949f995190000a8c7ceae58e23901120`. Static parsing and source recovery provide declarations and component relationships; they do not prove active rendered state or native pixel identity.

Recovered facts used by the native implementation include a 42 px host tab strip, 38 px product toolbar, at least 48 px product navigation, #222222 canvas, #cccccc foreground, #44d62c accent, Roboto control text and RazerF5 section headings. Product navigation is device-specific: the mouse has Customize, Performance, Power, Calibration, Advanced and Help; the keyboard has Customize, Lighting and Help.

The exact recovered font outlines and product artwork are private generated runtime assets, provisioned from local evidence. Their original program bundles are neither embedded nor executed by the fork. The helper in `crates/secbird-tray/src/native_editor/` records asset hashes. Source font tables were reconstructed into sfnt files and their global checksums verified before Windows private loading.

The current native editor is a partial port. It does not yet reproduce the complete source DOM/state model, keyboard hit regions, all profile actions, macro editing, advanced/calibration controls, accessibility behavior or Chromium text rasterization. Those are required iteration work, not waived acceptance criteria. Source-derived dimensions alone must not be advertised as pixel-for-pixel parity.

## Keyboard Customize and profile interactions

Focused product605 source recovery identifies the UK layout6/edition0 Customize viewBox as730x387 with109 paths and three dial actions without separate shapes. Its730x340 bitmap and overlay share the top-left origin, at scale1; the config-block follows two10px body paddings and a6px top margin. Inert full-CSS DOM replay corroborates this relationship without running vendor JavaScript or using screenshots. Local runtime geometry and icons are supplied through the ignored asset directory, with source hashes/provenance. No raw vendor path table or artwork is committed.

The source profile bar has a230px dropdown at x46,25px option rows and a26px ellipsis hit target at x286. Add creates defaults, Duplicate copies and renews identity, and both activate the result. Rename replaces the dropdown with a32-character inline input. The native implementation follows these flows but does not yet reproduce every hover, drawer, modal and overflow state. Profile import/export use native file dialogs as a functional adaptation. Retained REA parent: ev_95834af4e4e6514a194a837857248e20e745bf363d55cad95da8d9d1a1454942.
