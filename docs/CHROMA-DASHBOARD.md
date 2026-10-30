# Native Chroma dashboard

The local Chroma dashboard is a separate module from the Studio composition editor.
Open **CHROMA** on Home, then use its effect strip or a device's **LIGHTING DETAILS**.
The existing shared host tabs and product Quick Lighting editor keep their appearance.
The dashboard adds no listener, network request, analytics, account/store/social panel,
Connect manager, Haptics module or background capture.

## Retained behavior and boundaries

- Cards come only from the engine's connected-device snapshot. Product artwork uses
  the existing authenticated, lazy device pack and its bounded residency cache.
  Catalogue rows never become connected devices. Missing artwork stays missing.
- The currently admitted Quick route is the known Ornata V2 025D route with enabled
  lighting control, exact device/profile identity and matching available brightness
  readback selectors. A fresh read-only mode snapshot offers firmware presets in
  Normal mode and Static/Spectrum/Breathing/Wave in Driver mode with the required
  worker; unsupported modes/read failures offer no route. Other products, unknown
  devices and disabled/unavailable
  targets retain an explanatory card. Receiver/discovery exclusions remain in the
  engine's unavailable-device notices. No extra capability is inferred from LEDs
  or a product photograph.
- Spectrum, Static, fixed Breathing, Wave and fixed Starlight use existing source
  effect IDs and endpoint projections. The endpoint retains its mode, transport,
  owner, durable-projection and worker admission checks. This UI does not make
  unsupported effects, normal-mode Wave, random flags, other hardware or input
  interception work. Use Studio for supported saved compositions and Quick Settings
  for the existing detailed product controls.
- A newly selected preset uses the current Quick editor's conservative default
  (green for color effects). Reselecting the current effect retains the complete
  source setting, including opaque fields. Profile settings, active selection,
  brightness/policy, unrelated profiles and foreign raw data remain the existing
  endpoint's responsibility. Saved effect names and mixed settings are read back
  from device profiles; choosing a strip button is a staged choice until Apply.
- Captured settings have a 2 MiB aggregate budget and selections at most 64 targets. Global Apply captures eligible membership and each exact active profile. The
  worker refreshes and preflights all captured identities and Studio ownership
  before the first write. No newly connected target is substituted. A failed
  transfer stops later targets, reports the affected target and keeps earlier
  successes visible. It performs no retry or compensating hardware writes and
  does not claim a multi-device hardware transaction.
- Each successful Quick selection persists through the existing profile store.
  There is no second global authority/file that can disagree with device profiles.
  Navigation/reopening reads state and does not automatically apply saved output.
- Studio navigation opens the existing composition editor. Apply Saved and Stop
  use its existing single shared worker and cancellation/control handle. Dashboard
  Stop is available during pending work; it does not take the HID mutex on the UI
  thread before requesting cancellation. Quick Apply refuses while Studio owns a
  selected device. Stop/drain releases that ownership so saved Quick can resume.
- Opening Chroma while Studio has an unsaved draft asks the user to save or discard
  there; it preserves the draft/window rather than using the exit continuation.
  Cached status reads do not query HID or renew the producer observation timestamp.
  Old/unavailable/stopped/conflicting output is labeled separately from saved intent.
  Device modal background hit targets are removed; Escape dismisses the modal.

## Source evidence

This is an implementation choice constrained by existing Rust routes and stock
structure, not a claim of complete stock Chroma behavior.

The supplied inventory's Chroma evidence is
`ev_3febcdae34c0c5f1ad2558897408162ced2a0d37d5ca95f35d2158eb09d89485`.
This task's fresh static REA dashboard directory analysis parent is
`ev_5f4b6e4acc40d19f9b502667f48cb8410c27ce9be51280d30a2c22f3a0f61e04`,
summary `ev_e42ca659e91a8591c1930f5ce9ac3a69127448226eb4324fbf367a2840bfe116`.
It parsed 81 JavaScript files without parse failures, 8,947,929 text bytes, and
reported **partial application coverage and truncated partial semantic coverage**.
No vendor code was executed. The root artifact SHA256 was
`dca93b5ece24934f524db06d5d28cbdfd0ef9031ec28350a34518aa8ce6010e9`.

Focused ordinary source inspection supplements that static inventory:

| Cached dashboard artifact | SHA256 | UTF-8 anchor | Observation |
| --- | --- | --- | --- |
| `4234.c5514adb.chunk.js` | `57bccf157ecc5d976125b36e30a94c8daf7d43c5f54be88890c02dfae852f87f` | 23675 `renderQuickEffects` | Quick summary is hidden for Chroma/other-app ownership; selected settings are distinct from global selection. |
| Same artifact | Same hash | 96566 `supportEffects` | Device/subdevice/port effect support participates in source selection. Lite does not pretend to have that all-device contract. |
| `6883.53d122e0.chunk.js` | `60ff51dfc51d4e1c90edf4c295eaedce0bea60c44c271d8d4ac5f75598167cfb` | 125056 `openChromaStudioTab` | Studio is separate navigation. |

The supplied `stock-chroma-dashboard-0.jpg` and lower dashboard capture establish
visible effect-strip/device-group structure, separate application navigation and
ownership notice. Their marketing/immersive/account sections are excluded here.
Cached source and captures are historical evidence; they do not prove current
service availability, hardware behavior or exact loaded bundle identity.

## Visual and product gaps

The native content uses source dark panels/green headings, real model artwork and
the approved shell. It is not pixel parity. The effect strip currently uses shared
text buttons rather than the source circular effect icons, and Chroma's tab ring
asset is not provisioned. Card sizing, source device modal/picker/tooltip styling,
collapsible/reorderable groups, responsive spacing and complete source typography
remain unfinished. Unsupported Fire/Wheel routes are not offered as working Quick
presets. The source's product/subdevice/port eligibility model remains broader than
this verified route. No owner acquisition, WDL integration, cloud profiles or
cross-device advanced composition mapping is added here.

Next diagnostics: recover a bounded authenticated Chroma icon/widget asset pack;
compare native modal and card captures against stock at the same viewport/DPI;
then add a device family only after its capability and ownership route is proved.
Physical optical output, unplug/wake recovery, selector ownership/driver routing,
all-device compatibility and three complete-build runs below 100,000,000 bytes for
both working set and private memory remain open. Simulated tests cannot close them.

Native tests substitute a clear external-process observer only for explicit mock handles in test builds; production keeps the original competing-process check. The two-device persistence fixture has a simulated worker-ready signal and source-aware mock transport; the separate Studio handoff test runs the actual shared worker. No normal host profile or running application is changed.

Validation: the serial workspace suite passed 4 CLI, 509 core, 309 tray and 1
documentation test (55 opt-in exclusions). Both Chroma native interaction tests
passed separately. The complete offline debug workspace build also passed.
No physical tests were selected. The final owned-window renders
were inspected, including two target cards, the modal, and Studio ownership refusal.
The frame reports 1600×900 pixels at scale 1.25 and no privately registered GDI
fonts; complete typography remains a stated gap. The source and native effect
areas were normalized only by content width (1120px), with stock DPI unknown; no
pixel-identity score is claimed. See `validation/chroma-dashboard.json`.
