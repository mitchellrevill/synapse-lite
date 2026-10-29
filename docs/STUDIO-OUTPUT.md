# Studio physical-frame preparation

`host_studio` connects the existing Studio compositor to the authenticated Ornata
logical-to-physical map and the existing seven-packet frame format. This is a
backend component, not an enabled Apply-to-Hardware feature. Normal startup and
the native canvas still do not start Studio device output.

`PreparedStudio` compiles one immutable composition snapshot and retains its
originating draft identity. Current admission is the compositor's existing single,
unrotated Ornata V2 placement, edition 0, supported source layout, default geometry
and Static/Spectrum/Fire layers. The production constructor requires the manifest
loader's opaque `AuthenticatedLayout`; declaring a hash in a composition cannot
substitute for source authentication.

The composition's product/edition/layout must match the authenticated selection.
Its logical and physical dimensions, ordered physical projection and mapping entry
count must agree too. Output retains the authenticated mapping instead of trusting
the composition's declared table. Holes map black. One current frame yields six
physical 6x22 row packets followed by the profile 0/region 0 CustomFrame8 commit.
Reading a frame repeatedly never consumes animation phase; advancement is explicit.
The nominal interval is 40 ms, not a measured USB cadence or stock-phase claim.

The preparation child checks Ornata 025D/interface 2/physical 6x22 before any request,
then performs a fresh GET 00:86 and loads the exact selected source manifest. It
binds the result to the selected device key/path and records conservative monotonic
observation timestamps, declared identity size, source hash and byte length. It
performs no mode, brightness, input, macro, effect or profile mutation. Target
key/path matching is not an ownership lease, connection-generation proof or fresh
hardware-state admission.

## Verification

Seven scoped tests pass in debug and optimized builds, including the opt-in actual
export/private-manifest test with a mock device. The regular serial tray suite has
245 passing tests and 31 opt-in exclusions. No physical device was used here.

- All 500 Fire/Spectrum physical frames match an independent scalar reconstruction
  and ordered source projection: RGB MD5 `b0c46374f3ff3a91d3bf64f197e30345`.
  The existing logical reference remains `15c72331781274566ba1d29ebfc5b0d1`.
- The frames traverse the existing streaming adapter into the protocol simulator:
  3,500 OUT packets, exact simulated RGB matrix contents, unchanged mode, effects,
  brightness, gaming bits and macro state. Accepted mock OUTs are not firmware ACKs
  or optical evidence.
- Repeated frame reads, snapshot/session separation, incorrect physical maps,
  dimensions, identities, missing/untrusted assets and extra placements are covered.
  Preparation performs a fresh identity GET on every attempt and never guesses an
  edition or layout.
- Cancellation, wrong mode and failures at row/commit boundaries produce no hidden
  retry or phase advancement. A future live controller must latch failures and own
  cancellation/completion; preparation itself is not that controller.

See `docs/validation/studio-physical-preparation.json` and the synthetic numeric
fixture `crates/secbird-tray/tests/fixtures/studio-physical-reference.json`.
The reference generator is private local analysis code and invokes neither Rust
nor vendor programs. Pinned Ornata source manifest SHA256:
`3595e11f1d8778180d1dd6edd2463c8b3b8e446542c5020dc1fa038c67937a93`.

## Source lifecycle and remaining integration

Cached lighting-engine main.a383ea9e.js SHA256
`c82c7074e5da8abcffbff95f23a6b962c1e14b04586715b9d4b7a6f11fe9e6d2`
uses separate saved Advanced and draft Preview engines. The per-user global
synapseChromaStudioUsers enabled/currentProfile selection is separate from device
Quick effects. Applying Studio must not overwrite saved device Quick settings or
silently apply an unsaved editor draft.

Source handoff disables eligible Quick engines before enabling Advanced, and
re-enables Quick after disabling Advanced. Unsupported devices can keep their
Quick path. Saved product/instance/device identities are resolved against live
device metadata before output. Canvas preview is not hardware output authority.
The source frame callback's Promise.all is not awaited inside its named lock, and
destroy/exit syntax does not prove callback drain or exclusive ownership. Shutdown
uses declared Quick/default policy, not restoration of an arbitrary prior RGB frame.
These source behaviors are documented in private work/studio-output/OWNER-CONTRACT.md.

Live output remains a TODO until explicit applied-revision/session generation,
fresh ownership and connection admission, Quick/Studio handoff, completion/drain,
partial-transfer fault handling and stop/recovery are connected to the worker.
Mode acquisition must coordinate with the input path; copying a normal-mode reset
on exit would not prove input preservation. Wider devices, remaining effects,
physical/optical verification and the complete-build 100 MB gate remain open.
This component does not narrow the full implementation goal.
