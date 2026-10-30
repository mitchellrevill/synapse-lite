# Native Settings

The toolbar opens a Settings module using the existing host-tab lifecycle. Its
sections are **SYNAPSE** and **GENERAL**. Closing the editor retains the tray;
the tray's Exit command remains the process-exit action.

## Behavior

- Auto Launch reads the exact HKCU Run command for this executable. Missing,
  inaccessible, malformed and foreign-installation registrations are distinct.
  Errors appear in Settings; the tray marks unavailable registration as such.
  Writes propagate errors and require exact readback. Another installation's
  command is retained.
- New registrations use `--startup`. This reads `start_minimized` at launch.
  The child checkbox is disabled when Auto Launch is off or unknown. Changing
  it migrates this executable's legacy `--tray` registration. Explicit `--tray`
  remains an unconditional background launch. A normal launch opens the editor,
  including when a tray instance already exists. A background second launch
  leaves the existing editor alone.
- Registration does not prove Windows will execute it: Windows Startup settings
  can independently disable startup. This distinction is displayed on the page.
- Profile/DPI notifications use `popups_off` in the tray's event and foreground
  profile paths. Battery warnings separately use `battery_warning_off` and the
  validated 1–95% threshold. Errors remain visible. `auto_switch_off` is read
  before automatic foreground profile switching.
- Away lighting retains `away_lights`: keep, dim to 25% of saved brightness, or
  off. The tray reads it at lock/display events and restores saved lighting on
  return. Editing this choice while already locked takes effect at the next
  relevant event; the UI states that timing.
- Device lighting checkboxes call the retained per-device lighting-control
  action, with a device/profile identity check and the existing Studio guard.
  This stores `lighting_off` without issuing an immediate device SET;
  each connected device is listed in the scrolling card. This does not assert
  ownership of Windows
  Dynamic Lighting or another process.
- GENERAL exposes the actual build version, local English support, offline Help
  and the project's MIT license. It has no account, social, vendor-service,
  advertisement, recommendation or analytics controls.

Both native and browser editors use the same serialized read/modify/write path
for tray policies. `tray.json` preserves unknown top-level values, bounds input
and encoded output to 128 KiB, and uses a flushed sibling temporary file plus
atomic replacement. Corrupt, unsupported typed, oversized and unwritable files
are retained. Legacy tray saves return/log errors instead of discarding them.
Refreshing the page reads changes made by another editor. Read-only mutation
refusal exists in the native controls, native queue and shared server boundary.

## Source and visual evidence

The supplied shared-shell inventory and the normalized
`stock-synapse-settings-loaded` / `stock-synapse-settings-general` captures show
the section names, dark two-column cards, green card headings, dependent
startup child, notification control, Language and About. The native layout
retains those useful groupings and stacks its cards in a narrow window.
Recommendations, promotional tutorials and vendor-account/legal/social links
are intentionally absent.

REA static analysis of the supplied cached Settings application:
`ev_08f2eef085871eacaf238ddfc89b0ab4a7a062b2ed79977a8638c74da33b54e7`.
Source720 asset node:
`jag_node_57067f91b1b9dc83dbc10036c7d2380d4b4c57fe47be051b2b7a22d52df8a2c9`.
Its retained SHA-256 is
`09885484dcda19f25b9b216d575da5d2cbed2abe5678e6280c0011387be4ce64`.
The file parsed completely, but application graph coverage is partial: another
chunk exhausted its worker heap and semantic relationships exceeded the parent
result budget. Static facts and module facts were retained. No vendor JavaScript
was executed and static analysis does not establish runtime behavior. The
initial sandbox temporary-file transfer failed; a workspace-owned TEMP/TMP
resolved that specific failure without changing the target or host setup.

Pixel parity is not established. The native controls use the existing native
text/buttons, omit vendor artwork and marketing content, add retained local
tray policies, and differ in card height, glyph metrics and subtab rounding.
Private source assets, raw REA output and captures are excluded from Git.

## Validation and remaining dependencies

Unit tests inject the startup backend and use temporary configuration files.
They exercise readback, legacy command migration, foreign ownership, denied and
ignored writes, startup/window decisions, child dependency, unknown retention,
input/output budgets, corruption, write failure and read-only refusal. No test
writes real HKCU startup registration.

The opt-in `native_settings_navigation_persistence_and_read_only_are_consistent`
test uses one standalone HWND, a simulated mouse and a temporary config. It
checks Settings navigation/clicks, tray reader consistency, another editor's
refresh, local device lighting policy, read-only refusal and retained damaged
settings, plus scrolling in the narrow stacked layout. PNG captures use the owned client rendering, not an unattended
desktop screenshot.

Concrete follow-ups:

- TODO: implement and verify the source language catalog, shared preference
  broadcast and restart/loading protocol before enabling language selection.
- TODO: establish the OS/build capability gate, actual ownership readback,
  switching/cancellation and recovery before implementing Chroma versus Windows
  Dynamic Lighting handoff. Keep this separate from local `lighting_off`.
- TODO: package and browse complete offline dependency notices; the current
  dialog includes the project's MIT license and states the packaging gap.
- TODO: physically verify sign-in/window/tray behavior with the user's chosen
  installed executable. Current validation uses an injected registry and a
  simulated standalone editor; it changes no normal profile or autostart value.

This component does not establish all-device compatibility, complete-build RAM
acceptance, physical lighting ownership or input-driver recovery.

## Recorded checks

- `cargo test -p secbird-tray --offline -- --test-threads=1`: 310 passed,
  48 opt-in tests ignored.
- Final settings-focused tests: 7 passed, 3 opt-in tests ignored.
- Final isolated native Settings test: 1 passed. Its report records no hardware
  SETs and no autostart mutations, with all navigation, persistence, refresh,
  per-device lighting policy, read-only, damaged-file and stacked-scroll checks
  true. Both sections and failure/narrow states were rendered and inspected.
- `cargo build -p secbird-tray --offline`: passed for the final source.
- Extracted browser-editor JavaScript: `node --check` passed.
- `git diff --check`: passed.
- Scoped all-target tray Clippy completed with existing repository warnings;
  no diagnostics were reported on new files or modified lines.
- Workspace Clippy with `-D warnings` is blocked by existing findings in
  unchanged core files (including `source_macro.rs`, `profile_compat` tests and
  other core code). This change does not edit those files. This is a draft
  integration PR, not a claim that the repository's strict CI is green.

Next diagnostics for the source dependencies: trace source720's language
storage/broadcast callers and loading/restart events in an isolated replay;
trace its Device Lighting switch to the host capability/ownership API and prove
OS gating, readback and cancellation before attempting a physical handoff.
Neither diagnostic authorizes changing stock language or lighting ownership
unattended.
