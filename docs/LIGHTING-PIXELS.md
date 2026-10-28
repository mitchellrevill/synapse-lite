# Native lighting pixels, palettes and layer composition

`lighting_pixels` implements the recovered four-byte palette and container blend
math without a clock, device access, frame generator or profile parser. The
fourth byte is **transparency percent**,0opaque to100transparent, rather than
ordinary0–255 RGBA alpha. Source None becomes packed0x64000000 (black/100).
Packed RGB input is converted once upstream; the Rust type retains RGB order.

The bounded pixel constructor distinguishes None from assigned black. Palette
interpolation includes all four channels, repeated f32 additions, byte truncation,
duplicated endpoints, quantized duplicate-stop movement and the native one-slot
final-segment quirk. Its input is already sorted/normalized0..100 with endpoints,
2–10 stops and at most16,384 entries. Those are current constructor limits;
they do not redefine import or interactive gradient authoring rules. The source
codec supplies missing endpoint anchors before the native call; native descriptor
capacity and normalization of every unusual import still need end-to-end checks.

For one layer the native factor is `(100-transparency)*opacity/100`, then
multiplied by an optional0..100 mask and divided by100. Native mode bit0x20
instead uses zero factor for black RGB and opacity for nonblack RGB, independent
of the control byte. Channel output is old plus truncation of the **signed
difference** times factor. Truncating the final interpolated colour is different:
255 blended halfway toward0 gives128 here, not127. Each intermediate multiply
and divide is scalar f32; no fused operation is substituted.

`Layer::blend` preserves existing destination colours. `compose` starts from
black and visits layers in caller-supplied order. Both validate lengths, finite
opacity0..1 and mask percentages before any output mutation; compose validates
all layers before clearing. Invalid native opacity/control/overflow behavior is
not imitated. Layer flattening, source-order reversal, masks, generator failures,
animation lifetime and resource admission remain the caller's responsibility.

## Evidence

Native target SHA256
`cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa`.
Container frame RVA52df0, REA
`ev_0e2487f1ef81f89b5a570abcb4e30d381eea341d4ab867b790f51281892b8cd4`,
retained private artifact SHA256
`31ddbf6c9f2f7395fce8fcf56ef0ac200bfd035ce3643d35ea00e5184fba9588`.
Instructions52f9a–53002 establish inverse coverage, optional nonblack mode and
percentage mask;52f10–52f83 establish signed CVTTSS2SI before byte addition.
Source clears destination and fills child scratch with0x64000000 before rendering.
This math does not by itself reproduce the container's child admission/lifetime.

Shared palette RVA692e0, REA
`ev_18490e4590fbff48a9601b33c5bb711772be71b56c0af9ed363a6f116f2d0094`;
instructions interpolate/saturate four bytes. These retained instructions were
also recovered in the successful Fire analysis. See [Fire](FIRE-LIGHTING.md).

The selected complete source modules2578 and5596 are byte-exact excerpts of
lighting-engine `main.a383ea9e.js`, SHA256
`c82c7074e5da8abcffbff95f23a6b962c1e14b04586715b9d4b7a6f11fe9e6d2`,
decoded character ranges405543..425119 and753528..757566. Their hashes are
`008bf7b8aac4e86f47dd0c4e498402949c384d640553ca08d9c2424ad3527bda` and
`f68ec8e42f28d4e060750b5d03554609fe3660464477cfdb06367756bca74b9e`.
REA parent`ev_942d8521b67e4562f40aa61df404a8b2af9afb4d1656fd8e9a426102d5afa060`,
summary`ev_1cdade3e0ad35f096a7a545c3964e7764bfb647317760c8197bb0cb50f830769`:
two files parsed, no parse failures. Extracted module bodies omit their Webpack
factory wrappers, so graph module count0 and dynamic dependencies remain unknown;
their syntax coverage is not proof of executing the complete app.

The codec replaces missing colours with sentinel0x64000000, adds missing0/100
anchors, sorts stops, sets Spectrum Mode0 and emits Studio Fire's fixed preset.
Native API conversion swaps RGB/BGR once, preserving the sentinel. The recovered
container formula treats its high byte as inverse percentage coverage. This
closes the earlier high-byte interpretation gap for the admitted pixel math.

## Verification

Four full four-channel palette vectors cover transparency, duplicates and the
final-segment quirk. Ordinary RGB matches all943 existing Spectrum reference
entries. Nine blend configurations compare all65,536 old/new byte pairs each
against an independent f32 reference:589,824 channel pairs including negative
deltas, None, opacity, masks and nonblack mode. Tests also cover caller order,
black initialization and atomic refusal. All pass in debug and optimized builds.

Reference vectors are derived from static instructions, not executed vendor
output. This module is connected to the pure Fire renderer, but no Studio preview,
hardware-output, timing, optical or complete-build memory claim follows from it.
