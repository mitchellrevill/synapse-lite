# Actual installed profile payloads

The two installed device configurations were read from a private offline copy of AppEngine local storage for https://apps.razer.com. Original Synapse storage was not modified. Only the synapse_196 and synapse_605 device records were read as values; account/authentication records were not queried. The isolated browser received one locally fulfilled inert document and executed no vendor JavaScript.

Each configuration contained one profile and no selected profile macros. A Synapse4 envelope was constructed according to the recovered stock export function wn: copy the selected profile, remove its top-level appEngine runtime member, Base64-encode UTF-8 JSON and calculate the observed stable-JSON MD5. This is an export reconstructed from actual stored profile data, not a file saved through the stock UI.

The current CLI imported both documents into a new isolated fork configuration, then exported each model. Both decoded profile payloads were unchanged at every field, profile names and product/category were preserved, and an independent JavaScript reader/hash implementation accepted both hashes. See [the scoped result](validation/actual-profile-roundtrip.json). CLI import used read-only discovery; no profile activation or setting command was requested.

Stock user processes were temporarily closed to copy consistent storage and restarted in finally. Eleven were present before copying and twelve after three seconds; services and drivers were unchanged. Raw database copies, profiles, names, device serials and private logs remain outside the public repository.

This strengthens the codec evidence with actual installed data. It does not establish import acceptance by the running Synapse application, macro execution, every edited setting, Synapse3 support or complete profile compatibility. Stock window access timed out, so the live-app round trip remains an explicit acceptance item.

Source provenance: product605 middleware2592.d84cf7ac79aa3da58b37.js SHA2569c4a687e1a4868cd9da4dec40024e5ce3403fe866907887b47995f493e23a3a8; function wn begins at UTF-8 byte1441528. The earlier focused REA profile analysis has parent ev_ff6dc972bf71433ce286388abede063d82564f9046937bc6c9813cc51e0efb96. Source static semantics and the offline browser/CLI observations remain distinct.
