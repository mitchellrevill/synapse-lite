# Recovered Studio Fire renderer

The Rust `FireCycle::studio` implements the native Studio Fire preset's
generation and replay math: Mode0, Rate160, Seed100, infinite cycles,25fps.
Seed100 means100 keyframes, not a random seed. Five interpolated frames per
keyframe yield a500-frame cycle. No random draws occur at cycle boundaries.

This pure renderer now feeds [bounded local Studio preview](STUDIO-PREVIEW.md).
Hardware output and its ownership/restoration remain unfinished. Do not
substitute a device's advertised6×22 matrix for Studio's native world-coordinate
region; local preview uses separately recovered mask bounds and logical sampling.

## Generation and storage

The recovered fixed7×23 cooling and palette-index matrices control Fire. Bottom
ignition consumes23 CRT random draws per keyframe; six upward passes propagate
intensity with a cyclic temporal shift and integer attenuation. Two-colour
interpolation uses repeated f32 additions and native endpoint duplication.
This is not a generic heat-diffusion simulation.

A256-entry four-channel palette maps fixed spatial indices to Hot/Cold/black.
RGB is multiplied by intensity using integer division by255. The fourth byte
is copied without that intensity multiplication. None therefore needs the
native percentage-transparency semantics in [pixel composition](LIGHTING-PIXELS.md).
The upstream codec falls back to Cold when Hot is omitted; that rule is included.

`FireCycle` retains80,500 grayscale bytes plus1,024 palette bytes:81,524 dynamic
bytes, excluding the small fixed struct. It computes mapped pixels on demand
rather than retaining500 complete canvas frames. This preserves generated
values while reducing frame-volume storage. This number is not total process
RAM, peak construction memory or complete-build acceptance.

Region mapping preserves the source branch: regions smaller in both dimensions
crop the lower-left intrinsic tile; otherwise rows use scalar f32 nearest-neighbor
sampling and columns repeat modulo23. Output covers the complete native canvas;
outside-region pixels are zero/opaque black, as in the native allocated volume.
Compositor masks are separate. Half-open bounds reuse the established bounded
region representation (canvas up to64×64); this is an explicit current API
limit, not a recovered universal device/Studio canvas limit.

The caller supplies a u32 incoming CRT stream state and can read the final state.
The native recurrence is wrapping `state*214013+2531011`, returning
`(state>>16)&32767`. A newly initialized native CRT thread starts at1; the stock
process's current state depends on prior consumers. Each preset rebuild consumes
exactly2,300 draws. Reproducing a currently running stock animation's random
stream/phase is not established. Reset rewinds the retained cycle without
regenerating randomness. The lower-level parameterized interface below extends
this preset without changing Studio's source codec.

## Native parameters and bounded rebuilds (30 October 2026)

`FireCycle::new(region, hot, cold, random_state, FireParameters)` accepts recovered
native **Mode0**, unsigned **Rate**, unsigned **Seed** and **Cycles** (-1, 0 or
positive). Native initializer RVA624a0 clamps Rate below40 to40, substitutes50
keyframes for Seed0, and maps Cycles0 to infinite (-1). The number of interpolated
frames per keyframe is `Rate / 40 + 1`, using integer division; total retained
frames are that interval times normalized Seed. Rate changes interpolation count,
not the40ms callback interval. Exactly `23 * normalized Seed` CRT draws occur per
build, independent of Rate, colours and region. The implementation retains the
source integer attenuation, cyclic propagation, repeated f32 additions and
duplicated interpolation endpoints.

The native frame function RVA633b0 decrements finite Cycles **when emitting frame
zero**, including the first call. Consequently count1 emits only the first frame;
count2 emits one complete period plus its following frame zero. After completion,
`next_frame` succeeds without touching the caller's correctly sized buffer,
advancing phase or consuming randomness. `is_finished` and `remaining_cycles`
make this explicit. `render_frame` remains an independent stateless lookup;
`reset` is a local convenience that rewinds both phase and the original counter,
without drawing randomness. It is not a recovered vendor reset entry point.

`rebuild(region, hot, cold)` constructs from the previous final CRT state and
retains the replay index and remaining counter, including completed playback.
The native rebuild does not reset offsets188/18c. Parameters stay fixed for that
initialized generator; callers changing Rate/Seed/Cycles construct a new one.
Bad colours refuse before mutation. Rebuild uses a temporary replacement so the
previous cycle survives validation failure. Local Studio preview now uses this
same method for its initial whole-canvas AddEffect / bounded SetMask sequence;
the existing500-frame composition digest and final CRT state remain the contract.

Admission is explicitly bounded to4096 retained frames and256 frames per
interpolation segment (the source uses a1024-byte intermediate colour buffer).
These are local safe limits, not universal compatibility claims. Extreme inputs
are rejected before frame allocation or random draws. Maximum retained data is
660,480 bytes (4096×161 grayscale bytes plus1024 palette bytes). Rebuild can also
hold the previous cycle, replacement and temporary keyframes; retained bytes are
not peak allocation or total application RAM. Nonzero Mode and negative cycle
counts below-1 remain refused pending source-backed behavior and tests.

### Studio control boundary

Recovered lighting-engine module2578, `addEffect` case8, **hardcodes** Mode0,
Rate160, Seed100 and Cycles-1. It reads only Hot/Cold for Fire. Profile extension
fields named rate/seed/cycles/mode therefore do not customize Studio Fire and are
preserved without reinterpretation. A regression checks501 composed frames with
such fields against the ordinary preset. The existing native Hot/Cold/None
selection controls, file history and read-only guards remain the Studio controls;
no unsupported Rate/Seed/Cycles widgets or profile schema were invented here.
The independent UI evidence is Studio chunk941 module5591
(`941.38e182cf.chunk.js`, SHA256
`d5696fe5dcc4f75670c85a7cda0cd6c63834d79f7581748c4a925682253ccb89`):
the Fire component contains only Hot and Cold selectors, and main module2904
defaults its region to those two colours with empty layer parameters.
There are no UI layout changes in this checkpoint, so no changed-UI screenshot
comparison is claimed. Stock pixel fidelity remains an outstanding application
acceptance gate.

## Static source evidence

Installed `RzLightingEngineApi_v4.0.55.0.dll`, SHA256
`cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa`.
The first Ghidra helper session timed out after import. One retry with expanded
access for owned static helper communication succeeded; no vendor DLL execution,
installation, global configuration or device operation occurred. That difference
does not independently prove the original timeout's cause. The session closed.

| Native RVA / subject | Retained REA Evidence |
|---|---|
|625d0 generation/rebuild|`ev_b805bd5ffde38958c5666ad65fc7fa509dcad6dc18a01b1ac2bc8316d46217ab`|
|625d0 instructions|`ev_b7e2b907bc13aa016fc81360ebabb93602db39c19aefac38cb060aaf80f029e2`|
|624a0 initializer|`ev_21545cdc691cdb898e63590aad8e7e5da63aaeca0ae88874938673235a05f5c7`|
|633b0 frame replay|`ev_3655d4b9b0b9d4bf851059b80302b1cb336b89888b5911d4f0988c595c32786c`|
|1551d0 CRT recurrence|`ev_07f83582758565397e65274dd9e1a1b05b2f48d5c671ffc5457d77f580ea10a4`|
|CRT initial state1|`ev_0a6ceaa2f8d24d282f0440dc2bf31844a1a7e523bdbb648416a97ff54eea80d8`|
|Fixed tables at1f52c0/1f5370|`ev_c7944b035db121ade51d4b09a491e212dc5a4b103f9feeef23e747be73986671`|

The two decoded matrices' JSON carrier SHA256 is
`b934af3b78dc8c015ec1883484554bfe3ba1db8d263734f0b29ae088340b9281`.
`fire_tables.rs` contains only those functional constants, not vendor code/images.
Instruction anchors626e0–62907 establish ignition/attenuation;6296a–62a54
expansion;631ea–633ad interpolation;62b80 onward RGB/control handling;
63055–63189 region branch and scalar f32 mapping. Shared palette evidence is in
the pixel document. The source parameter codec is selected module2578, described
there with the original containing bundle hash and exact extraction range.

## Verification and remaining work

Nine complete500-frame cycles match an independently written, static-instruction
Python reference by byte digest, covering994,500 four-channel output pixels.
Cases include6×22 cropping, either dimension crossing its threshold, vertical
scaling, horizontal tiling, nonzero region offsets, several explicit RNG states,
None and Hot-to-Cold fallback. Tests check final RNG state, wrap/reset, unchanged
transparency across intensity changes and atomic refusal without phase advance.
These pass in debug and optimized builds. The reference never loads vendor code.

`native_fire_parameters.json` adds10 independent static-instruction reference
cases covering4588 complete intrinsic frames (738,668 intensity bytes), including
Rate0/39/40/79/80/119/120/200/10200, Seed0/1, the largest admitted segment and the
4096-frame cap. `tools/derive_fire_parameters.py` reproduces them from the retained
functional cooling/mask JSON carrier identified above. These vectors do not
execute the vendor DLL. Targeted tests also cover finite completion/no-write,
reset, atomic refusal, phase/counter preservation and following-stream rebuilds.
The original nine complete mapped colour-cycle digests remain unchanged.

Checkpoint validation:12 optimized Fire tests and16 debug Studio-preview tests
pass. The optimized isolated native-window regression also passes against the
explicitly selected stock export: Hot/Cold saved independently, None undo/redo,
read-only refusal, preview restart/drain, unchanged original/profile-store bytes
and zero device SETs. See [native interaction evidence](validation/fire-native-interaction.json).
Its screenshot is a local preview regression capture, not a stock pixel-parity
comparison. Existing source-dropdown/widget fidelity remains a UI TODO; this
checkpoint changes no layout and adds no fabricated Studio parameters.
The offline optimized workspace build passes; commands use a task-owned target
and temporary directory, shared Cargo cache and `-j1`.
See [checkpoint validation](validation/fire-parameters.json) for counts and scope.

This is independent reconstruction evidence, not an executed vendor oracle or
optical observation. Full Studio mapping beyond the admitted local preview,
source scheduler/phase, physical output, recovery and final resource checks remain
unverified. The full application goal remains active.
