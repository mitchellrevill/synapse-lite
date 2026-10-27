# Authenticated local Ornata manifest loading

On Windows, `secbird_core::ornata_manifest::load_manifest(path, expected_identity)` reads one explicitly selected local file and returns an `AuthenticatedLayout`. It does not locate devices, query hardware, infer locale, select a fallback layout, execute vendor code, or integrate a lighting worker.

The loader authenticates the **entire actual file** using Windows BCrypt SHA256 before any JSON parsing or projection. The required digest is `3595e11f1d8778180d1dd6edd2463c8b3b8e446542c5020dc1fa038c67937a93`, the recovered `DeviceManifest_605_0.json`. Passing a declared digest or a modified projection cannot bypass this check through the public API. The resulting type has private fields and can only be constructed by the authenticated loader. It retains the actual digest, source byte length and validated spatial map; it retains no source JSON, SVG, file path or device observation.

The pinned artifact is 2,895,966 bytes. Metadata rejects files over 4 MiB before reading. The reader itself is capped at 4 MiB plus one lookahead byte, catching growth after metadata inspection. An explicit read loop uses a fixed buffer of that size; it never grows based on reader hints or EOF behavior. Only the same in-memory bytes that passed SHA256 are parsed, with no file reread. These are local loader bounds, not a measurement or guarantee for the entire application's RAM usage.

SHA256 uses the existing Windows cryptography dependency and the OS-owned SHA256 pseudo-handle in one synchronous BCryptHash call. No custom hash implementation, network dependency, global crypto configuration, provider installation or vendor DLL is involved. Crypto failures return an error rather than accepting an unauthenticated file.

After authentication, selective Serde visitors inspect the top-level identity and only the exact `/ledConfig/ledConfig_<edition>_<layout>` key supplied by the caller. Unknown fields and unselected layouts are skipped with `IgnoredAny`; a full `Value` tree, SVG strings and other layout projections are not retained. Standard Serde JSON recursion checks stay enabled. The sole production caller reaches this parser only after the pinned whole-file hash check. Synthetic parser tests exercise format handling through a private boundary without creating an authenticated result.

Top-level `vendorId` and `productId` must identify VID 1532/PID 025D. The selected record requires strictly typed unsigned 16-bit VID/PID/EID/Layout, dimensions, and exactly two coordinates per MatrixPos/DevicePos. Negative, fractional, string, null, overflowing and wrong-length coordinate values fail. Selected LedMatrix parsing stops at the 256-entry ceiling before accepting additional records. Duplicate selected/known fields and trailing JSON fail. Unknown fields are ignored only within this already authenticated artifact domain, and the original file is never edited or exported by the loader.

The projection is passed to [OrnataSpatialMap](ORNATA-SPATIAL.md), which validates the exact requested identity, known source layout, 7×23 logical and 6×22 physical dimensions, entry count and coordinate bounds. An absent exact key returns MissingLayout; the loader does not substitute layout 1 or reuse the historical UK observation. An unsupported or mismatched identity remains an error.

`AuthenticatedLayout::map()` borrows the validated map; `into_map()` transfers it. `authenticated_sha256()` and `source_byte_len()` expose the actual checked artifact metadata. These methods do not assert that the caller's identity is a fresh hardware observation. A later runtime must obtain and correlate that identity through its own reviewed boundary.

The local development asset is provisioned under the ignored `target/native-assets/DeviceManifest_605_0.json` path, alongside assets selected through `SYNAPSE_LITE_ASSETS`. The loader still takes an explicit path; it does not read that environment variable itself. The source file was hash-checked before copying, and the Rust loader then authenticated and parsed all 17 packaged layouts from that copied file. No full vendor manifest is committed to public source or embedded in the executable. This unit does not add the asset to a distributable package or change runtime/UI behavior.

Seven ordinary tests cover three standard SHA256 vectors, exact-limit/oversized reads and one-byte lookahead, digest failure before JSON parsing, strict selected records versus unselected data, malformed numeric/coordinate/dimension/identity data, entry budgets, duplicate/trailing JSON and file metadata rejection. One explicitly opt-in ignored test uses `SECBIRD_ORNATA_MANIFEST_TEST_PATH` to authenticate the private real artifact and validate every packaged layout, including UK 114 mapped pixels plus 18 holes and layout 12's 119 source records.

To run the private check, set that variable to an explicit local pinned manifest path, then run:

```powershell
cargo test -p secbird-core --lib ornata_manifest::tests::explicitly_selected_private_manifest_authenticates_all17_layouts -- --ignored
```

The actual private check passed for all 17 layouts. It exercised Windows SHA256, bounded file reads, selective parsing and pure coordinate validation only. It sent no HID commands and loaded no vendor runtime.
