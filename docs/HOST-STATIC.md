# Ornata host Static

Source Quick Effect Static uses one uniform 132-pixel frame when the Ornata V2 is already in Driver Mode3 with profile0/region0 CustomFrame8. It uses the [bounded source frame writer](ORNATA-FRAMES.md); it does not acquire a mode or write a saved effect. Normal Mode0 retains the firmware path.

The source endpoint checks the exact device, active profile, configured lighting ownership, known competing processes and aggregate profile budget before device access. It checks mode and CustomFrame state before and after the batch. Responses distinguish accepted USB writes and readable control state from visible output: `optically_verified` remains false. Partial transfers, failed postchecks and failed saves report possible hardware changes without automatic retry.

Profile activation, reconnect and the tray's policy reconciliation use the same bridge. A successful steady Static is not streamed. Failed attempts remain latched until explicit reapply, changed intent or reconnect. Disabled brightness and display/idle/away suppression retain a deferred color; resume prepares that color before raising live brightness. A failed pending frame blocks brightness resume.

In Driver3, explicit brightness changes retain configured intent and reconcile live brightness after frame preparation. They report `saved_hardware_unchanged` and `live_policy_reconcile_pending`; the saved hardware register can retain its earlier byte. Initial profiles without brightness intent read saved/live registers and retain their exact observations as baseline provenance. They do not claim to recover Synapse's retained enable/value. Initialization avoids rounding and rewriting an unchanged raw live byte.

Simulator checks cover exact packets, first-profile baselines, frame-before-brightness ordering, suppression/resume, active-profile changes, ownership/budget refusals, partial failures, reconnect, unchanged ticks and standalone-core refusal before lighting writes. The separate [physical one-frame probe](validation/ornata-one-frame.json) accepted seven writes in38ms with readable registers unchanged and the stock owner restarted afterward. The complete Static endpoint/lifecycle, optical output and input recovery have not yet been physically verified.

Known process checks are not a global ownership lease. RGB buffer contents and previous animation phase cannot be read back through these checks. Other host effects remain unfinished; recovered [uniform animation math](UNIFORM-LIGHTING.md) alone is not a running animation engine.
