# Legacy archive inspection checkpoint

Command: `secbird profiles inspect-archive FILE`. Runs before HID discovery and only reads the supplied file. Outputs JSON member names, byte counts and `profile_conversion_supported: false`. Never extracts, rewrites, activates or installs anything. Existing ZIP import rejection remains unchanged. No GUI changed, so there are no changed-screen comparisons in this checkpoint.

## Evidence and boundaries

Read-only shipped renderer recovery: `work/synapse-renderer/ornata-main.js` in the 2026-10-27 source investigation; SHA-256 `186a0185c6a94fbc56f12063f434ccf363fbe839361688cfb9d7aa10f8d7f162`.
Character offsets in that recovered file:

- 2035213: `handleDataFile` / `getData` use JSZip, dispatch multi-member archives to `getSynapse3Data`, and accept a single XML device envelope.
- 2036658: `getFileDataFromZip` reads a blob and uses xml2js compact/nativeType conversion, ignoring XML declarations, instructions, attributes, comments, CDATA and doctype.
- 2041312: `convertSynapse3Device` reads Name, Product_ID and Type. `convertSynapse3Profile` reads Name/ProfileId and handles GamingMode, LedBrightness, LedPowerSettings, LightingEffects and Mappings.
- The `getSynapse3Data` code identifies DeviceInfo and Profiles prefixes. These observations establish a ZIP/XML family, not safe lossless round-trip behavior.

These are static source observations, not runtime or real export acceptance. No `.synapse3` fixture was found under the recovered renderer folder. Synthetic ZIP tests are structural adversarial tests, not vendor profile samples.

## Accepted ZIP subset and limits

At most 2 MiB input, 256 entries, 8 MiB declared aggregate expanded size and 100:1 declared per-member ratio. Only stored members (method 0) are accepted, so actual accepted expansion is 1:1. CRC and local/central sizes, methods, flags, names and offsets must agree. Contiguous local members must precede the central directory. No ZIP64, multi-disk, encrypted members, descriptors, compression, EOCD comments, prefixes or trailing bytes. Unsupported forms fail explicitly. Names are limited to ASCII, 512 bytes, with case-insensitive duplicates, traversal, absolute/drive/UNC names, Windows reserved names, trailing dots/spaces and controls rejected. Symlinks and special Unix files are rejected. This intentionally strict subset is not a general ZIP extractor.

## Remaining work

Obtain a user-owned real Synapse 3 archive and establish exact XML encoding, member naming and macro-reference shapes against all source converter helpers. Add bounded deflate decoding with actual output limits before broadening ZIP support. Add a lossless XML representation retaining unknown nodes, attributes, records and references; the vendor converter itself discards fields and must not be copied as a lossless codec. Then add explicit selection/preview and transactional import with foreign records retained, without device activation. Unicode names, archive comments and other ZIP variants remain unsupported pending evidence and tests. Legacy profile import and pixel parity are not complete. No complete-build RAM acceptance is claimed.

## Checkpoint provenance

Initial isolated clone base: `7b99930` (integration merge). Feature reapplied on published main `6149f7a9422ae93322365ed12df46ab681dc40a8` to exclude unrelated unpublished commits. One feature commit contains inspection API, read-only CLI integration, adversarial tests and this documentation. Builds use a task-private target directory and one job.

Validation: six archive tests passed on the published base after final filename checks; CLI file-preservation regression passed (1 test). Optimized CLI build passed after moving TEMP/TMP to task-private storage (initial restricted temporary archive rename failed with Windows access denied). Command smoke check on a Python ZIP_STORED archive containing an opaque binary member succeeded and reported conversion unsupported. Input SHA-256 remained E8496675EB5C1BB7E2D701FBB8E4AF56199CAFD09C6C48A38AB55F667B9D7E78. No hardware was opened. Existing unused source-macro budget warning remains unrelated.
