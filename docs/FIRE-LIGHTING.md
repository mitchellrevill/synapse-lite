# Recovered Studio Fire renderer

The Rust `FireCycle::studio` now implements the native Studio Fire preset's
generation and replay math: Mode0, Rate160, Seed100, infinite cycles,25fps.
Seed100 means100 keyframes, not a random seed. Five interpolated frames per
keyframe yield a500-frame cycle. No random draws occur at cycle boundaries.

This is a pure renderer, not connected Studio preview or hardware output yet.
The app's existing Fire controls still edit composition files only. Native
device/world-coordinate mapping, masks and ownership must be integrated before
the renderer can be used as equivalent application playback. Do not substitute
a device's advertised6×22 matrix for Studio's native world-coordinate region.

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
regenerating randomness. Finite cycles, nondefault Rate/Seed and rebuild-in-place
index preservation are outside this Studio-preset interface.

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

This is independent reconstruction evidence, not an executed vendor oracle or
optical observation. Full Studio frame mapping/compositing, live scheduler,
physical output, recovery, current-stock phase and final resource checks remain
unverified. The full application goal remains active.
