# Static-derived native Spectrum fixtures

These fixtures were derived offline from Ghidra/dumpbin static instruction evidence for installed RzLightingEngineApi_v4.0.55.0.dll, SHA256 cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa. They are not vendor runtime observations or an optical validation.

The RGB file has943frames *3bytes in red,green,blue order (2829bytes), using the source ten-stop25fps Spectrum preset. SHA256:6258f96d86f84a7e73b6dcee488e3d844d36523e2728b5edca1d1393cb162017. The JSON preserves boundary samples and provenance. Full-byte equality is checked on all platforms; Windows tests also calculate SHA256 with the existing Windows crypto API.

Derivation: outputs/research/native-spectrum-breathing-math.md in the analysis workspace. Offline reference: work/lighting-runtime/palette_reference.py. Native palette builder RVA692E0, init61E90, frame62130. The implementation reproduces repeated f32 addition/truncation and duplicated segment endpoints, not HSV or a continuous-time approximation. Existing-animation phase and outer scheduling remain outside this fixture.
