# Studio

The editor supports composition selection, local import/export, LED selection, layers/groups, effect properties, gradients, history and save/discard. File import and selection preserve unsaved work through cancel/error paths. Local preview and physical output are separate operations.

| Effect | Current renderer |
| --- | --- |
| Static | Implemented. |
| Spectrum Cycling | Implemented. |
| Fire | Implemented. |
| Breathing | Continuous deterministic variant. Random, finite and input-triggered variants remain incomplete. |
| Wave | Continuous cardinal variants. Other geometry and trigger modes remain incomplete. |
| Wheel | Implemented within the admitted composition/layout subset. |
| Starlight | Implemented, with source-derived frame references and native editor checks. Optical parity remains open. |
| Tidal | Authoring/preparation; renderer incomplete. |
| Reactive / Ripple | Authoring and contract work; ordinary input-driven playback incomplete. |
| Audio Meter | Capture/analysis backend exists; full effect rendering and integration incomplete. |
| Ambient Awareness | Synthetic diagnostic exists; ordinary screen sampling/projection incomplete. |
| Chroma Generate | Full generation integration incomplete. |

Unsupported effects must report a limit rather than silently substitute another animation. Imported unsupported data is retained.

## Physical playback

Supported saved compositions use the shared output worker and an authenticated logical-to-physical mapping. Current admission is limited to the implemented unrotated keyboard/layout path; general device output is not established by the asset catalogue.

Normal-mode playback changes only the owned volatile lighting effect. It does not write keyboard mode, input settings or macro registers. Play starts the admitted stream; Stop cancels packet admission and restores the prior owned effect. Device disconnection stops playback and requires explicit Play after reconnection.

Sustained Fire/Spectrum output and Stop were physically confirmed on the development setup. This is evidence for that shared path, not all effects or all devices. Starlight has independent source-derived frame references and a native simulated editor check; stock optical comparison remains outstanding.
