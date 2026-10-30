# Customize Windows Shortcuts

This device-assignment category now offers the shipped standard Windows list and saves local desired assignments. It is separate from Global Shortcuts. No physical input interception, input injection, launch, clipboard access, device SET, driver or service change is added.

## Source contract

The inert 605 `MapWindows.9dc9e295.chunk.js` form (webpack 97569) reads the standard list (`29267.Zi`), conditionally appends advanced-multidial entries and optional product additions, restores the active Standard/Hypershift choice, and writes `win8ShortcutsGroup.windowsShortcutAssignment`. REA parent evidence: `ev_961d21ba17afb8374a9c9c5c377747d47a905de340ff2341bc3caa7624253cae`. Static call/closure coverage is partial and does not establish execution.

Both shipped 605 and 196 main bundles contain the same 26 standard IDs and English labels in the same order. Hashes, offsets and the independently recovered list are in `crates/secbird-core/src/profile_compat/windows_shortcuts_source.json`. The core test compares all IDs and labels against that evidence. No advanced dial or other product-specific option is inferred from shared constants. Screenshot reference: inventory `captures/stock-ornata-assignment-windows-shortcuts-0.jpg`.

The existing popup, rail, fonts, dropdown, scrollers, Standard/Hypershift selection and device input lists are reused. The dependency panel keeps the source 210-by-44 placement but explains local-only delivery. Approved tabs are unchanged. Native/source screenshots have different capture geometry and incompletely known stock DPI; they are review evidence, not proof of pixel identity.

## Persistence and restrictions

Save runs on the existing device worker. The endpoint checks connected identity, selected active profile, the full original profile snapshot and measurement read-only mode. The edit runs on a clone, validates the choice and input, and uses the existing bounded atomic profile-store save. This work never applies the profile to hardware.

Standard and Hypershift are independent. Duplicate records, malformed layer flags, incompatible input types, analog mappings and malformed Windows groups refuse without overwriting the original file. Unknown shortcut IDs remain visibly unknown until an explicit supported choice is selected. Unrelated mappings, old inactive groups, source GUIDs, import payloads and opaque metadata survive. The prior translated button projection for the edited input is removed so export cannot overwrite the new foreign Windows group with the old key/mouse action. Synapse export regenerates its payload/hash through the existing codec.

The native form follows the recovered per-input category list and disabled-input state. The backend conservatively supports known 605 keyboard usages and 196/197 Right Click, Scroll Click, buttons 4/5 and DPI input. Primary Left Click, mouse wheel directions, keyboard media/dial controls, unknown inputs and other products are refused. The category is an authoring feature; these restrictions are not a hardware capability claim.

Save/reopen restores the chosen ID. Cancel discards local edits. Dirty-close Save persists before following the pending transition; failed saving keeps the draft. No configuration file is created when the source mapping context is absent.

## Remaining acceptance

- Recover and implement a verified device-specific trigger/delivery route and its held-input/release/reconnect behavior before enabling execution. OS-wide hotkeys are not a substitute.
- Hardware readback, application behavior of each shortcut, firmware/transport restrictions and live stock re-import remain unverified.
- Additional catalogue product lists and advanced-multidial variants require their own recovered exports/capability evidence.
- Exact popup pointer/focus/tooltip states, status-panel raster detail and normalized lossless stock/native DPI comparisons remain visual TODOs.
- Final combined-build privacy, functional and three-run sub-100,000,000-byte memory acceptance remain open.
