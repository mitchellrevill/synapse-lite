# Actual installed profile payloads

## Normal-store migration, 28 October 2026

The two captured `MITCH-Default` profiles are now transferred into Lite's normal
profile store, bound to the connected receiver and keyboard identities. The old
store was backed up, Lite exited normally, the current-store hash was compared
with the prepared baseline, and the complete candidate was installed atomically.
Existing profile data and active selection remain unchanged. Both imported
profiles remain inactive; the keyboard receives an empty Default profile so
startup cannot apply imported settings implicitly. No stock database was edited.

Independent JavaScript validation decoded current CLI exports and verified their
stable hashes. Both imported payloads equal the captured originals at every
field, including GUIDs and macro references. This caught and fixed an Advanced
export regression: unchanged optional fields were being filled with native
defaults. Unchanged Rotation and Dynamic Sensitivity objects now retain their
exact source shape; edited settings still overlay the retained opaque data.
Import notices no longer incorrectly label valid translated Advanced fields as
untranslated. Invalid/unsupported data still produces notices and is preserved.
All331 core tests and the doc test passed; the optimized CLI/tray build passed.
See [migration validation](validation/normal-store-migration.json).

This migrates the captured27October device profiles, not a newly captured live
account. A mouse Hypershift mapping references a macro absent from the captured
export. Its reference is retained, but recovery/execution remains unfinished.
Ornata Snap Tap and function-key preference also remain preserved without
functional replay. Real stock import acceptance, macro bodies, linked programs,
Studio compositions and complete profile activation/hardware acceptance remain
open. Availability in the normal store does not establish full compatibility.

## Earlier capture and codec evidence

The two installed device configurations were read from a private offline copy of AppEngine local storage for https://apps.razer.com. Original Synapse storage was not modified. Only the synapse_196 and synapse_605 device records were read as values; account/authentication records were not queried. The isolated browser received one locally fulfilled inert document and executed no vendor JavaScript.

Each configuration contained one profile and no selected profile macros. A Synapse4 envelope was constructed according to the recovered stock export function wn: copy the selected profile, remove its top-level appEngine runtime member, Base64-encode UTF-8 JSON and calculate the observed stable-JSON MD5. This is an export reconstructed from actual stored profile data, not a file saved through the stock UI.

The current CLI imported both documents into a new isolated fork configuration, then exported each model. Both decoded profile payloads were unchanged at every field, profile names and product/category were preserved, and an independent JavaScript reader/hash implementation accepted both hashes. See [the scoped result](validation/actual-profile-roundtrip.json). CLI import used read-only discovery; no profile activation or setting command was requested.

Stock user processes were temporarily closed to copy consistent storage and restarted in finally. Eleven were present before copying and twelve after three seconds; services and drivers were unchanged. Raw database copies, profiles, names, device serials and private logs remain outside the public repository.

This strengthens the codec evidence with actual installed data. It does not establish import acceptance by the running Synapse application, macro execution, every edited setting, Synapse3 support or complete profile compatibility. Stock window access timed out, so the live-app round trip remains an explicit acceptance item.

Source provenance: product605 middleware2592.d84cf7ac79aa3da58b37.js SHA2569c4a687e1a4868cd9da4dec40024e5ce3403fe866907887b47995f493e23a3a8; function wn begins at UTF-8 byte1441528. The earlier focused REA profile analysis has parent ev_ff6dc972bf71433ce286388abede063d82564f9046937bc6c9813cc51e0efb96. Source static semantics and the offline browser/CLI observations remain distinct.
