# Studio Wave playback checkpoint

Saved Studio Wave2 regions now compile into the local compositor and the existing
explicit Apply Saved device-output path. This checkpoint adds playback of imported
compositions. A subsequent [authoring checkpoint](STUDIO-WAVE-AUTHORING.md) adds
native direction, preset and motion controls. Quick Wave remains unchanged.

Supported: continuous linear cardinal angles0/90/180/270, explicit cycles-1,
speed0..50, width100..400 percent and pause0..60000ms. Layer parameters merge with
regional overrides. Selected LED bounds determine each regional generator;
membership masks preserve holes and transparency exposes underlying layers.
Speed0 freezes phase instead of advancing a very slow wave. Pauses contribute
transparent frames, not opaque black. No Fire random state is consumed.

Required saved parameters are angle, speed, width, pause and cycles. Missing
descriptor defaults are not guessed. Start may be absent or `random` (the source
continuous start mode); end may be absent or `never`. Split, either boolean value
of counterclockwise, finite cycles and key-triggered modes explicitly refuse
playback. Unsupported compositions remain readable/exportable without mutation.

Custom RGB24/None gradients use the common native four-channel palette. Missing
endpoint anchors are added using the original first/last colour; the original
array is not sorted. Malformed gradients currently refuse playback, even though
native Wave has a deterministic fallback. Absent stops use native red/green/blue/
red at0/33/66/100. This is different from Quick's blue stop67 and from the Studio
UI seven-stop default. Historical Studio defaults are speed15, width100, pause0,
angle90, splitfalse, cycles-1. Its preset colours at0/26/42/55/68/81/100 are
FF0000/FFF300/36FF48/62F1FF/000CFF/8000FF/FF0000; these should be seeded when
Wave authoring is implemented, not silently substituted for an absent descriptor.

## Evidence and checks

Retained complete REA native records cover initializer0x180065a80,
geometry0x180065c90, linear renderer0x180065f50 and frame0x1800666b0 in
RzLightingEngineApi_v4.0.55.0.dll SHA256
`cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa`.
Converter module2578 of the previously analysed lighting-engine main.a383ea9e.js
sets Width/Speed/Pause/Angle/Cycles and playback mode bits. Source parent SHA256
`c82c7074e5da8abcffbff95f23a6b962c1e14b04586715b9d4b7a6f11fe9e6d2`.
Historical Studio module6257 in main.88ac681e.js supplies the UI constants;
its July2025 build is not proof of today's exact active UI bytes. Parent SHA256
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`.
These are reconstructed observations, not execution of the vendor renderer.

Geometry preserves f32 operation order: projected distance / speed-dependent step,
truncation to a period, then double width scaling. Palette length is at least the
stop count. Pause frames are floor(milliseconds/40). Cardinal phases use region
dimensions and the unscaled period. At widths100 or greater the recovered index
numerator stays nonnegative, avoiding the unresolved underflow branch below.
Palette storage is bounded to16384 entries; the existing32MiB preview-plan cap
still applies. These bounds do not close the full-process100MB acceptance gate.

An independent Python scalar reference checks20 regional cases across all four
directions, speeds0/1/15/25/50, widths100/125/400, pauses0/79/120/160 and None
gradient colours, with200 frames each. A masked Wave-over-Static composition
matches200 independent logical and physical frames. The host test also submits
1400 simulated OUT packets, checks exact simulator matrix bytes, and preserves
brightness, firmware effects, macros and gaming mode. Draft/snapshot immutability,
regional override precedence, wrong-size non-advancement and unsupported-mode
refusal are covered. Machine-readable evidence: validation/studio-wave.json.

## Deferred without blocking other effects

- Width10..99: decompiler unsigned index arithmetic can wrap for negative phases;
  next check is the corresponding machine instructions, not a guessed modulo.
- Arbitrary angles, split/rotational and finite/input-triggered variants.
- Custom gradient-stop authoring; native Add Wave, direction, five presets and
  speed/width/pause authoring are now implemented in the linked checkpoint.
- Physical/optical verification, real stock import of authored Wave, screenshot
  comparison of its future controls, and complete-build memory/network gates.

The native REA provider currently lacks GHIDRA_INSTALL_DIR. Retained evidence
supports this bounded checkpoint; fresh assembly recovery is a TODO rather than
a reason to halt independent implementation. No vendor DLL, input interception,
driver installation, user profile write or physical device output was invoked by
these automated checks. No pixel-perfect or universal-device claim is made.
