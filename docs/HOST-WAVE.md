# Pure Ornata host Wave preparation

The tray `host_wave` module prepares source Quick Wave4 frames. It has no Engine,
server, worker, UI, clock, file loader, device GET, or HID writer integration.
Preparing a frame is not an ownership check, hardware admission, or optical
verification. A caller must establish fresh device identity, authenticate the
matching source layout, and enforce output ownership/runtime gates separately.

`Program::from_setting` accepts source direction1(left) or2(right). The retained
Quick choice must be4, with a matching native `Effect::Wave` at speed3 and no
native off/per-key override for `Program::from_profile` to return a program.
Split/up/down/rotational direction IDs, either value of counterclockwise, explicit
angle, and nondefault speed/width/pause/cycle/split parameters are rejected.
Unrelated opaque setting members are retained without mutation.

The existing interchange codec maps source direction1 to native reverse=false
and direction2 to reverse=true. Its legacy Effect Display labels call those
right and left, respectively. This unit preserves that codec contract and
checks its Boolean projection, while **rendering source1 left and source2
right**. It does not change legacy labels or approximate a firmware Wave rate.

An explicit action can call `Program::opt_in` only after its profile projection
matches the choice. The marker `_synapseLiteSynapse4.waveHostIntent` binds
version1/source4/direction and is outside the vendor `decoded` payload. Neither
import nor a bare/default profile gains opt-in here. `opted_in` rechecks the
current source/native choice, so a changed direction, native speed/override, or
stale marker does not authorize the old choice. Clearing/setting the marker
preserves the foreign export and opaque setting members. There is no runtime
consumer of this marker in this checkpoint.

`PreparedWave::new(program, &AuthenticatedLayout)` is the only production
constructor. A caller cannot substitute an OrnataSpatialMap made from a claimed
manifest digest. Authentication remains the bounded core manifest loader's
responsibility; the prepared object retains only its selected numeric map.
Its identity getter reports that selection, without claiming fresh hardware
identity or an exclusive lease.

The logical canvas is exactly23x7 with zero offset and half-open bounds
`WaveRegion::new(7,23,0,0,23,7)`. The existing source-derived WaveCycle uses
default speed25/width100/pause0/infinite unsplit linear behavior and maps the
logical frame through the selected source geometry to a physical6x22
PreparedOrnataFrame. Unmapped physical destinations remain black.
`render_frame(index)` is an immutable index query, wrapping modulo25 for this
geometry. It never advances hidden state; querying0 again returns the same
frame. Its40ms source sample interval is not an achieved-cadence or phase-parity
claim. Outer scheduling, suppression/resume, backlog, dropped frames, and
hardware transfer behavior are outside this module.

Source authority and exact math are recorded in [WAVE-LIGHTING.md](WAVE-LIGHTING.md),
[ORNATA-SPATIAL.md](ORNATA-SPATIAL.md), and [ORNATA-MANIFEST.md](ORNATA-MANIFEST.md).
The native renderer source SHA-256 is
`cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa`.
The authenticated source manifest SHA-256 is
`3595e11f1d8778180d1dd6edd2463c8b3b8e446542c5020dc1fa038c67937a93`.
This composition reuses the independently scalar-derived physical-cycle fixture
SHA-256 `f855946d0c43ad03a9ec5d29843d54db10cb17d4804adc6107622d727d68ee56`.
These numeric fixtures are static-derived; they are not vendor runtime captures.

Four isolated pure tests verify source direction/native projection consistency,
unsupported variants, explicit/stale opt-in with byte-identical foreign exports
and opaque-field preservation, and every physical RGB byte of both25-frame
left/right cycles. The latter checks all18 UK-layout holes, row/commit packing,
wrap/large indices, and non-advancing repeated index queries. A private test-only
constructor supplies numeric fixture maps; it is unavailable to production
callers. No manifest file or hardware was read during these tests. The unit is
registered as a Windows tray module, but has no production runtime caller in
this checkpoint.
