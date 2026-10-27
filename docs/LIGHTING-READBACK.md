# Ornata lighting readback

The read-only API targets the inspected Ornata V2 (025D), interface 2, profile selectors 0/1 and region 0. `Device::ornata_effect_readback` sends GET 0F:82 with an 80-byte request capacity. The installed keyboard can return a six-byte effect header; the getter therefore has a scoped six-byte response minimum. Other commands keep their existing request-sized minimum. Transaction, command, length, protocol/reserved fields, checksum and status checks remain in place.

`EffectReadback` retains the full 80-byte argument buffer, its declared length, selectors, effect, flags, rate and bounded color count. Only declared colors are exposed as colors. Returned selectors are compared with the requested selectors rather than silently treated as aliases. `BrightnessReadback` requires exactly three bytes and keeps the raw brightness byte without a lossy percentage conversion. Neither type exposes a setter or restoration operation.

The initial physical query failed because the old transport rejected every response shorter than its request. Bounded header-only diagnostics recorded thirty identical checksummed success replies: request size 80, response size 6, command 0F:82, transaction 31, protocol/reserved fields zero. Three transport regressions cover the scoped exception, unchanged outgoing request, correlation/checksum/short-header rejection and invalid minimum refusal before I/O. Six decoder tests cover malformed counts/frames, selectors, raw retention and unsupported targets. Three example tests cover tracing privacy, classifications and bounds.

After the correction, a physical query of profile 1/region 0 returned effect 3, flags/rate 0, no colors and raw brightness 255, with matching selectors. Brightness queries for both selectors had also succeeded. The stock user processes were temporarily closed because discovery did not respond while they were active, then restarted; twelve stock processes were observed after three seconds. Services and drivers were unchanged. This is the state after those processes exited, not a capture of their running animation. See [the scoped result](validation/lighting-readback.json).

The shipped Ornata lighting manifest uses `rzDevice25ChromaEffect`, and its default exit effect is Spectrum 3. The observed effect is consistent with that code; this does not establish which path caused it. Firmware effect 8 represents CustomFrame and does not capture host effect identity, frames, phase, audio/ambient streams or ownership. Current device mode and active host ownership remain unverified. A successful register read alone does not authorize claiming complete restoration.

Reproduce the opt-in query with exactly one installed Ornata interface:

```powershell
cargo build -p secbird-core --example lighting_readback --release --locked
.\target\release\examples\lighting_readback.exe 1 report.json --read-only
```

The example refuses simulator mode, filters before opening a transport, omits serials/paths and observes only response headers while its effect query is active. At most 64 distinct headers are retained, with deduplicated counts. It issues no extra diagnostic query or setting command.

Source provenance: shipped lighting engine `main.a383ea9e.js` SHA-256 `c82c7074e5da8abcffbff95f23a6b962c1e14b04586715b9d4b7a6f11fe9e6d2`, module 5668 command headers and module 6411 getter/decoder; Ornata manifest SHA-256 `3595e11f1d8778180d1dd6edd2463c8b3b8e446542c5020dc1fa038c67937a93`. The local research ledger retains precise source offsets and the private raw hardware reports. Static evidence does not prove all inherited firmware commands are supported.
