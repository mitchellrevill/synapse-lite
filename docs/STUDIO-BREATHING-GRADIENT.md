# Breathing custom gradient editing

Select a Breathing layer and LEDs, then choose **Custom Gradient**. The native
editor exposes stop position, RGB/None, Previous/Next, Add and Delete. Edits stay
local until Save and use the existing Undo/Redo and preview/saved-output paths.
Changing between the colour and gradient panels does not itself change the file.

The ordinary two-colour value projects to the recovered nine stops at
0/12/25/38/50/62/75/88/100. Opening the editor does not materialize those stops;
the first explicit stop edit does. Missing Colour1 falls back to Colour2, and
missing Colour2 supplies None, matching the existing converter. An explicit
imported gradient is used as-is. Inherited gradients are materialized only for
the selected memberships, retaining untouched raw stop fields and numeric tokens.
Duration, both ordinary colours, playback flags, unknown fields and unselected
memberships are preserved. None removes the Color field rather than writing null.

Authoring requires two to ten strictly ordered stops and RGB24 or None. The
existing renderer's endpoint expansion must also fit its ten-stop descriptor.
Duplicate positions, oversized gradients, invalid values, ambiguous inventories,
mixed selections and random-mode changes refuse before modifying history.
There are no borrowed Spectrum/Wave presets in the Breathing panel. The source
duration and playback defaults are not normalized by a gradient edit.

The implementation uses the already recovered converter and native palette
contract recorded in STUDIO-BREATHING.md. The installed lighting DLL still hashes
to cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa.
No new vendor DLL execution or recovered random algorithm is claimed.

## Verification

Four added core checks cover nine-stop conversion, first rendered RGB112233,
inherited raw metadata, selection partitioning, None, Add/Delete/ten-stop limits,
invalid and mixed/random refusal, history and None revealing the retained lower
layer. Existing 1,952 independent palette-reference pixels and 150-frame layered
reference checks still pass. The workspace passes 475 core,305 tray,4 CLI and
1 documentation checks, with six core and46 tray checks intentionally ignored.

The native Breathing check now edits a real child stop field, observes ABCDEF in
local preview, saves nine custom stops, exercises Undo/Redo, invalid input and
read-only refusal, and saves None as an omitted colour. It preserves the original
stock composition and device profiles and sends zero simulated device requests.
The panel capture was reviewed; painter captures omit child edit controls, whose
presence and values are verified separately. Stock-matched widgets, screenshots
and full visual acceptance remain TODOs.

The first optimized native check missed a colour present for only one40ms frame.
It now authors ABCDEF at both0 and12 percent and verifies those saved stops,
providing a stable segment for native polling. The core check still verifies the
exact first frame independently; renderer timing and pixel assertions are retained.

## Deferred work

Random animation, finite cycles, input-triggered/toggle playback, random native
fallback and optical verification remain unfinished. Closing the gradient panel
retains custom data; the guarded two-colour buttons still refuse to silently
replace a custom/inherited gradient. An explicit reset-to-two-colours workflow,
including safe inherited-gradient semantics, remains TODO.

A smaller Starlight constructor code-only request (procedure_pseudo_code at
0x1800601a0,2GiB analysis heap) also returned provider_timeout after240000ms,
CLI duration241267ms. Its private result is retained separately from the older
full-function attempt. No new Evidence ID or Starlight animation is claimed.
Do not repeat the same request without a provider/startup change; continue
independent effects/device work. Full APIs, input/recovery, profiles, privacy,
visual fidelity and three representative complete-build memory gates remain open.
