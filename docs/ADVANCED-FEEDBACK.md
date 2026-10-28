# Advanced live feedback and interactions

28 October2026. Completes the first native implementation of the live graph,
Ctrl+wheel zoom, template dropdown and Rotation tool link. It does not establish
pixel identity or completion of the broader replacement.

## Behavior

The graph subscribes only for an enabled, supported Advanced page in a visible,
non-minimized editor. Device/profile changes, disabled state, hidden/closed
windows and restricted measurements stop it. A dedicated message-only window
and thread prevent GUI painting from delaying input ingestion. Registration is
passive mouse INPUTSINK|DEVNOTIFY, without NOLEGACY, keyboard capture, hooks,
synthetic movement, device SETs, vendor DLL initialization or network calls.

Core discovery requires exactly one active Raw Input mouse matching the selected
HID interface's nonzero Windows container, VID/PID and interface. Paths and
container IDs stay private and are not profile keys. Worker startup and its first
matching record fully revalidate identity; subsequent packets use that handle.
Only relative movement records from it contribute. Absolute, unknown, virtual
handle0, keyboard, button-only and other-mouse records are rejected.

Source196's formula sums absolute X and Y counts, computes the vector magnitude
over elapsed100ns ticks, publishes at a nominal60Hz with leading/trailing behavior,
resets accumulation on publication, and expires display feedback after150ms.
The native port uses monotonic Instant ingestion timestamps. The vendor's exact
native clock and batching are unproven; these are reported movement counts/ms,
not guaranteed pre-acceleration sensor telemetry or a physical velocity measure.
Metadata-startup backlog is discarded with bounded4096-message cleanup before
timing starts. The idle graph has no polling timer once its feedback expires.

The worker retains only its latest scalar and counters, with one pending UI
notification. Cancellation clears displayed activity immediately, requests
cleanup and waits at most100ms. A still-cleaning worker prevents another from
starting, bounding detached-thread accumulation. Other process-local mouse
registrations are refused/preserved. Notifications verify the target HWND still
belongs to this process. Cleanup errors remain explicit rather than claiming
unregistration.

Windows on this host returns0x100 from GetRegisteredRawInputDevices after a
successful requested0x2100 registration. Ownership accepts only that observed
form or0x2100 for the exact owning window; suppression/other-owner forms remain
rejected. Windows also sends initial arrivals for present mice. One arrival per
bounded captured inventory handle is accepted, with selected-device revalidation;
unknown, repeated or removal events invalidate and rebind. No grace-period timer
silently ignores real device changes.

Ctrl+wheel changes both custom chart axes without saving profiles, writing HID
commands or scrolling the page. The template menu supports pointer selection,
arrow/Enter selection, Escape and outside dismissal. None retains current points.
Dragging sets templateId3/None, matching the source's save semantics so stock
Synapse does not reseed an edited curve from its previous preset. The Rotation
tool is a fixed official link, opened only by an explicit click.

The live fill uses source colors at alpha0.3. Batched GDI+ curve/fill drawing
replaces per-segment pen creation. Pixel tests verify blending and chart clipping;
this is not a full screen comparison against stock.

## Verification and limits

- Core metadata selection/ABI tests and native speed, registration ownership,
  mailbox coalescing, cancellation, startup bounds and input decoder tests pass.
- Native HWND simulator test covers Ctrl+wheel without writes/scroll, dropdown
  selection and dismissal, None preservation, drag template reset, profile scope,
  Rotation/link action and no physical subscription for simulated devices.
- Wired00C4 metadata association was observed. A five-second passive physical
  check received623 actual movement records while the user moved the mouse,
  reported a peak about4.69counts/ms, and verified stopped speed0, registration
  restoration and worker termination. No settings were written and no movement
  was injected. See [physical report](validation/mouse-feedback-physical.json).
- Whole suites at this checkpoint:330 core,229 tray and1 doc test pass; ignored
  native/physical tests are run separately with their stated scopes.

Still unverified: the complete normal UI lifecycle with actual hardware across
unplug/sleep/profile transitions, receiver feedback/older filter combinations,
native clock/rate parity, full visual states and three complete-build RAM/CPU
runs. Optical tracking, stock profile re-import, Studio and the broader goal
remain separate acceptance items.

## Source provenance

Recovered source/assembly evidence and limitations are retained in workspace
`work/advanced-feedback/REPORT.md`. UI196 main.bbc961f9.js SHA256
`f99b1f88ca9496a77cab258d05814a312b876873cb5c586841c28a732db78c42`;
middleware6120 SHA256
`df8334aa3021d129893918cecb3c1d14b3727f476c42981bac3fb02f6736b633`.
REA middleware parent
`ev_bfbfa82f382fec19dd5f8e4373998fef6772903a27584a9acbac1dfd84de407c`.
Current4.0.896 mapping_engine.dll SHA256
`c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9`;
registration call RVA0xD3223, removal call0xD339C. Static trace coverage is partial.

Windows API contracts: [registration](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerrawinputdevices),
[flags](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawinputdevice),
[foreground packet cleanup](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-input).
