# First physical Wave boundary

Two source Left Wave frames, indices 0 and 1, completed on the installed Ornata V2. Each returned seven accepted HID OUT calls. Fresh identity returned layout 6 / edition 0 with declared length 2; firmware ID remains unknown. The complete local manifest was authenticated before physical mapping. This establishes the prepared-frame submission boundary, not visible animation.

The separate ignored diagnostic wraps the handle before `Device::open` with a canonical GET allowlist and exact armed frame packet sequence. It permits at most fourteen frame OUT calls, forty-eight GET requests and thirty reads per GET. Only source Left indices 0 then 1 can be armed. A transport failure or cleanup read following partial output permanently stops subsequent frame output. There is no worker, Engine apply, Store access, mode acquisition, brightness setter, input or macro operation in this test.

The run recorded 14/14 frame OUT returns, 40/40 GET OUT returns and 40 reads, with zero denied requests or partial batches. Before, after each frame and at the final read, the declared controls agreed: mode 3; live profile 0 CustomFrame8 with zero flags/rate/colors; saved profile 1 effect 3; and raw live/saved brightness 255. Mode GET declared only one byte, so its optional parameter is unknown and is not verified by comparing two null values.

Each guarded submission took about 75.96 ms, including control checks; its seven OUT calls took about 27.97 ms. Starts were 200.979 ms apart because this diagnostic also performs complete register snapshots around output. Neither that interval nor the source's nominal 40 ms cadence is a measured product-worker or optical rate.

The exact test executable SHA256 is `0B7C8C4D5F98E4CCCC59D3EBBB811197E5E8F50D949D802824ACA627BB4F575E`. It was built from the isolated staged source tree `bf4e855739bc6ada409b8beedfcd794cca774e5d`, based on `b8b2ed6` plus this test-only module. All 195 regular tray tests and three pure probe guard tests pass; the physical test is ignored by ordinary validation. [Sanitized evidence](validation/physical-wave-two-frames.json) records exact source, executable and wrapper identities, control snapshots, counts and timing.

The reviewed external wrapper isolated stock user processes with observations 11, 0, 0, then restarted stock in `finally`. Eight were present at its five-second check; a later desktop-context check confirmed fourteen RazerAppEngine processes and one razerwdl. Services and drivers were unchanged. Restarting stock does not prove restoration of prior RGB pixels, animation phase or input state.

Full cycles, right direction, continuous worker/API/native UI operation, policy transitions, hotplug, sleep/wake, abrupt termination, optical placement and release resource use remain unverified by this diagnostic. See [host Wave](HOST-WAVE.md) for the separate simulated integration evidence and current runtime behavior.
