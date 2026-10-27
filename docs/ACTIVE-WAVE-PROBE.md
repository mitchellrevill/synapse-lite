# Explicit active Wave runtime probe

`--runtime-wave-probe <seconds>` is a separate development measurement path,
limited to 1–600 seconds. It admits an existing, explicitly opted-in source
Quick Wave profile and runs the ordinary off-UI frame worker with the native
Lighting view. It actively submits frames. It is not the query-only
`--runtime-smoke` baseline or the uniform-only `--runtime-lighting-probe` path.
Mixed flags and extra arguments are rejected before application startup.

Only `SYNAPSE_LITE_SMOKE_VIEW=keyboard-lighting` is accepted for Wave. There is
no Wave color-editor workload. The existing report destinations remain
`SYNAPSE_LITE_SMOKE_REPORT` and `SYNAPSE_LITE_PROBE_REPORT`.

## Admission and lifetime boundary

Admission requires exactly one configured serial-bound Ornata025D profile,
the matching connected interface2 with physical matrix6×22, positive retained
brightness already matching the observed LIVE register, and consistent native,
source Quick4, and explicit Wave opt-in intent. Only Left/source1 and
Right/source2 at the recovered default speed are supported. Profiles containing
nonlighting settings, key frames, button mappings, macros, application links,
recovery state, extra profiles/devices, or the uniform probe's marker are refused.
Imports and normal startup do not create the opt-in.

Read-only preflight checks the current known competing-owner gate, Driver_Mode3,
profile0/region0 CustomFrame8 with flags0/rate0/count0, LIVE and saved brightness,
and a fresh GET00:86 layout/edition tuple. The authenticated manifest loader must
accept that actual tuple and the pinned asset. No initialization, Store save,
mode acquisition, frame submission or brightness SET occurs during preflight.
Unknown firmware remains unknown when the identity response declares two bytes.

The worker independently prepares identity and the asset after startup. This
deliberate second observation is not a renewal of an ownership lease, and the
preflight preparation is not injected into a new worker generation. The mock
lifecycle test verifies one identity GET at each stage. Existing frame admission,
control refresh, process checks, partial-output fault handling, and cooperative
deadlines remain unchanged. See [HOST-WAVE.md](HOST-WAVE.md) for the source math,
map authentication, and local scheduler limitations.

The active path skips generic profile/input/power/effect replay, foreground and
remap actions, automatic generic retries, and editable native actions for its
whole lifetime. Runtime brightness policy reconciliation remains active; after
a completed frame it may reapply the already configured LIVE brightness. It
does not write saved brightness or acquire device mode. A fresh bounded parse
and canonical content pin precedes candidate/brightness admission; changed or
invalid configuration latches refusal until process exit, including same-size
edits hidden by unchanged file timestamps. It never updates the pin to a newly
loaded profile.

## Report interpretation

Both native and worker reports retain the common uniform probe readiness and
clean-stop fields, and add `active_wave_probe`, `source_id:4`, `native_id:2`,
`authenticated_layout_ready_at_observation`, and `wave_preparation`.

Readiness requires a live worker, the bound active profile, at least three
accepted frames, no cached fault, a control checkpoint valid at the producer
observation, and authenticated Wave preparation. The latter checks the exact
manifest SHA256 and 2,895,966-byte size, known edition/layout, declared identity
size and optional firmware consistency, physical6×22/logical7×23 dimensions,
direction1/2, and a completed identity query whose elapsed observation age is
at least its query span. A source ID or an old Boolean alone is insufficient.

`wave_preparation` exposes only twelve numeric/nullable fields and the validated
64-character hexadecimal digest. No manifest path, raw JSON, serial or profile
payload is copied into it. `identity_age_at_producer_us` and
`conservative_identity_age_now_us` describe an observation's age, not fresh device
authentication at draw time. `current_identity_verified` and
`current_controls_verified` remain false. Snapshot freshness is limited to
2,500ms; controls were checked against the existing 500ms boundary when the
worker published the snapshot. Neither limit establishes an exclusive lease.

Shutdown retains the existing graceful batch-boundary request and bounded
two-second worker join. The bound concerns joining the worker, not total
process shutdown or interruption of a blocked HID syscall. Fresh post-join
faults remain authoritative; an older healthy observation cannot hide a partial
output or cleanup failure. Restarting stock software is an external recovery
step and does not prove optical state or animation-phase restoration.

## Validation at this checkpoint

Pure policy/readiness tests cover strict flag separation, both probe kinds'
marker/program refusals, source/native mismatches, nonlighting refusal,
authentication metadata, invalid timestamp relationships, stale publication,
sanitization and stop reporting. Mock Engine tests cover scope, owner, mode and
identity refusals without SET or persistence. Explicit ignored tests use the
pinned real manifest with a mock GET00:86 response `[6,0]`, and exercise GET-only
preflight plus the ordinary worker through authenticated readiness and a clean
join. Hardware discovery is disabled in those fixtures.

The separately selected private Left/Right seeds both passed strict Wave
admission and were rejected by uniform admission. They are synthetic derivatives
of an earlier lighting-only seed, not stock exports, and are not repository
fixtures. This checkpoint performs no physical execution or new RAM measurement.
Cadence, optical output and memory claims require a separately reviewed,
hash-pinned measurement run.
