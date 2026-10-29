# Global Shortcuts

This adds an independent native Global Shortcuts page and an explicitly selected
Windows keyboard chord → multimedia application-command path. It does not install
input hooks, call the vendor input redirector, publish vendor mappings, or inject
keyboard/mouse transitions. Opening the page does not register shortcuts.

## Workflow and storage

Open **Global Shortcuts** on Home. Add creates a disabled local draft. Select a
card, choose a trigger from a native menu, choose modifiers/Hypershift context,
and choose an assignment. The page provides enabled state, duplicate, confirmed
delete, import/export, Save, Don't Save and Cancel. Navigation and window/tray
exit wait for the dirty draft decision. Trigger menus contain explicit choices;
they never record keystrokes. Duplicate copies the assignment and foreign fields,
renews the GUID, clears the trigger, and disables host execution.

`global-shortcuts.json` is app-level and independent of `profiles.json`. The
source envelope's `mappings`, `appEngine`, hashes and unknown fields survive
round trips. Mapping GUIDs and foreign group/record fields survive ordinary
edits. No device profile identity, active selection or recovery ledger is changed.
Preservation is of JSON values and opaque strings; whitespace and numeric spelling
may normalize on serialization. Out-of-range JSON numbers are refused, not coerced.
The existing 2 MiB JSON/complexity budget applies; there are at most 256 records,
eight modifier identities and 16,384 UTF-16 units per native assignment field.

Source fields retained include `guid`, `inputType` (`KeyInput`/`MouseInput`),
`inputID`, `inputModifiers`, `isHyperShift`, `outputType` and its group object.
Unknown input/output families remain exportable. Source `isEnabled` is retained;
it can represent derived device availability, so it is not used as host consent.
Only the local record extension below arms host dispatch:

```json
"_synapseLiteHost": {
  "version": 1,
  "scope": "windows-chord",
  "enabled": false
}
```

Import accepts the source `synapseGlobalShortcuts` JSON object (not a device
`.synapse4` profile). It is a draft until Save and always disables every host
trigger, including previously enabled Lite exports. An imported source trigger
also needs explicit **Use Windows Chord** consent. Unknown local versions are
preserved and refused. Editing an assignment/trigger disables it until enabled
again. Windows scope is explicitly any keyboard and either modifier side; it
does not assert equivalence to source per-device, side-specific input routing.

Save compares the full loaded base, including foreign fields, under an exclusive
writer lock. A stale editor keeps its draft and cannot replace external edits.
Export remains available to rescue that draft; Don't Save reloads the current
disk state. Accepted JSON is flushed to a new temp file before atomic rename.
Malformed files, excess budgets, stale saves and retained temp/lock files cause
errors without overwriting the current collection. After a crash, inspect/copy
`global-shortcuts.json`, `.json.tmp` and `.json.lock` before manually removing a
stale lock/temp file. No automatic corrupt-file reset or cleanup is performed.

Exports retain raw `appEngine`/hash fields as opaque source evidence. Lite does
**not** regenerate or publish native AppEngine mappings. Re-save edited mapping
JSON in a compatible source editor before using any retained AppEngine payload;
this is not a claim of direct stock native-runtime interchange.

## Categories and conflict rules

The recovered shortcut default restricts assignments to:

| Category | Source group | Lite support |
| --- | --- | --- |
| Switch device profile | `interDeviceGroup` / `profileNavigationGroup` | Retain; device eligibility/routing unproved |
| Switch device sensitivity | `interDeviceGroup` / `sensitivityGroup` | Retain; device eligibility/routing unproved |
| Switch lighting | `lightPacGroup` | Retain/edit reference ID; dispatch refused |
| Macro | `macroGroup` | Retain/edit GUID; dispatch refused |
| Text function | `textBlockGroup` | Retain/edit text; dispatch refused |
| Launch program | `launchGroup` | Retain/edit path or existing URL; dispatch refused |
| Multimedia | `multimediaGroup` | Six explicit one-shot commands |
| Windows shortcut | `win8ShortcutsGroup` | Retain/edit assignment ID; dispatch refused |

The page never offers the full device assignment list (keyboard remaps, mouse
remaps, Hypershift output, controller/joystick, AI launcher, etc.). Inter-device
forms and dependency pickers remain incomplete; importing those source groups is
the reliable retention route. Selecting those categories never arms execution.
Unsupported microphone volume/mute/all-mute media IDs and Turbo assignments are
retained and refused. Choosing a supported one-shot media menu entry deliberately
sets `useTurbo:false` and preserves other group fields.

Source conflict warnings compare nonempty input identity, the unordered modifier
set and Hypershift context, including disabled records. Windows additionally
collapses modifier sides. Saving two enabled records with the same Windows chord
is refused; the runner independently refuses both on a conflicting imported or
externally edited collection. Disabled records can keep the same trigger.
Windows registration failures are reported without stealing another registration.

## Host dispatch and lifecycle

Normal tray operation owns `RegisterHotKey` registrations on its own HWND/thread,
with `MOD_NOREPEAT`. Ctrl or Alt is required, F12 is refused, and only the explicit
alphanumeric/function/navigation key set is supported. Windows may reserve or
refuse a requested chord. The editor shows per-record registration results.
Standalone native-smoke/serve editors do not run this input runtime. Measurement
and active-lighting probe policies do not start shortcut registrations.

The supported assignments are source `VolumeUp`, `VolumeDown`, `MuteVolume`,
`Play`, `NextTrack` and `PrevTrack`. A matching notification sends one bounded
`WM_APPCOMMAND` to the captured foreground HWND, allowing the application/Windows
shell to handle it. Acceptance is not proof that a particular media app played
audio. UIPI, a hung window or an unacknowledged command is logged; there is no
retry or input-injection fallback.

Disable/reload/removal unregister existing IDs. IDs are not reused in the process;
queued stale notifications cannot execute a replacement assignment. Both ID and
packed chord must match. Lock, screen-off, suspend, session end and tray shutdown
clear registrations; resume/unlock reloads accepted configuration. This path has
no held synthetic keys/buttons to release. Mouse, device identity, Hypershift,
vendor hook/selector ownership and physical held-key recovery are unproved and
remain refused. There are no background network checks.

Microsoft API references: [RegisterHotKey](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey),
[WM_HOTKEY](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-hotkey),
[WM_APPCOMMAND](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-appcommand).

## Source evidence and remaining diagnostics

REA static analysis of the private dashboard chunks is retained outside Git in
this chat's work directory. Evidence
`ev_e4e43bc7391f94aa0d0c0589aec275444af1e2a4150b995016ece3dfea82cbbd`
covers route 4608. Expanded Evidence
`ev_3d94c12cdc9934c5ddacb9a984b25af779dff33e2a01282c9060cbb88fbcc8e2`
also covers chunk 55. These are static observations, not observed source input
recording or native execution. The first sandbox attempt failed on temp-file
access; a changed workspace-local TEMP resolved it without installing providers.

Route chunk `4608.7e65fc0e.chunk.js` SHA-256
`574bc1bc87e9cf0f16590dfb46f648b1998dcb0a2472f67fc681c5654b3e3f80`:
`loadShortcutsFromMappings`, `updateShortcut`, `hasShortcutDuplicates`,
`updateKeyInputMapping`, `updateMouseInputMapping`, `duplicateShortcut`, and
Save/Don't Save callbacks establish the retained schema and draft semantics.
Chunk 55 module 21368 `BUTTON.functionList` restricts the eight categories;
module 67124 `Ye`/`Re` supplies the multimedia IDs. Source `isEnabled` is also
derived from connected target availability in `assignInterDeviceGroup`.

Existing normalized capture `stock-synapse-global-shortcuts.txt` and its companion
2048×1104 image show the source empty list: centered 600px panel, SHORTCUTS title,
description and Add a Shortcut row. Lite uses that centered empty-panel structure
but describes its actual Windows scope. Populated cards and inline editing are
native functional equivalents, not pixel parity with the source drawer. Shared
tab appearance is unchanged. Remaining visual gaps include source card/drawer
metrics, scrollbar, icons, full assignment forms, popup typography and accessibility
parity. The stock capture contains no populated shortcut, so it cannot validate
populated visual parity.

Next diagnostics: recover exact inter-device selector/eligibility fields and
output-family serializers before implementing those forms; validate macro/text/
launch cancellation and ownership in isolated host tests before enabling them;
obtain an explicitly supervised device routing/held-key recovery proof before
enabling any mouse/device/Hypershift trigger. No hardware/driver action is justified
by ordinary Windows chord registration tests. Complete-build three-run memory
and all-device compatibility remain separate product acceptance gates.

## Validation

Focused tests cover source/foreign-field round trips, disabled import, duplicate
trigger clearing, conflict aliases, malformed/bounded inputs, stale saves and
retained recovery files. Opt-in tests exercise native Save/Don't Save/Cancel,
fields, menus, conflict refusal, import/export, read-only behavior and unchanged
profile bytes with a simulated device; a separate owned-HWND test covers actual
Win32 registration conflicts, simulated notification delivery, multimedia message
receipt, suspend/disable and unregister cleanup. No test records personal input.

On Windows, `cargo test --workspace --offline -- --test-threads=1` passed
825 tests with zero failures; 55 opt-in tests were skipped. The two new opt-in
tests above passed separately. The hotkey test needs a desktop context where
Windows permits `RegisterHotKey`; the sandbox refused it with error 5, and the
same scoped owned-HWND test passed outside that sandbox. The native editor test
uses only simulated transport and a private config directory. Default sandbox
TEMP access was refused, so all final tests used workspace-local TEMP/TMP.

New Rust modules pass `rustfmt --check`, and `git diff --check` passes. These are
component and regression results, not a complete-build memory or device-support
claim. Source reference assets were read from the existing private asset folder;
none were committed. The native capture was inspected for the dirty dialog,
cards and editor; explanatory text uses page-local wrapping so it does not change
the shared label painter or tab appearance.
