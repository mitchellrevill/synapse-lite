# Ornata source startup-mode experiment

**Subsequent source-attribution experiment:** the combined Raw Input/vendor child
observed52 MI01 raw and52 vendor sample records in mode3, then restored mode0 and
cleaned up both channels. See [source attribution](ORNATA-INPUT-SOURCE.md). Earlier
vendor-only zero-event samples remain historical observations; differences in
window/registration setup and typing are not independently isolated.

An explicit mode lease and separate opt-in experiment now reproduce the retained
source startup's Normal0 → Driver3 → Normal0 transition. They do not install input
hooks, redirect input, generate keys, send lighting frames or modify profiles.
The normal app does not call this experiment automatically.

## Source and readback limits

Shared605 middleware module42057 exports iS/J. Startup requests driver mode before
registerDevice and the inputevent listener; unload requests normal mode. The
Protocol2.5 source GET is00:84,size2, and SET is00:04,size2 with mode,param.
Stock writes parameter0 in both directions. Shared middleware SHA256 is
`9c4a687e1a4868cd9da4dec40024e5ce3403fe866907887b47995f493e23a3a8`;
product605 middleware SHA256 is
`9cdec67b89a1f8ef54a386fc2ce2a0e2f93f50830e9da385a7be91074ba90b18`.

The physical GET declares only one byte. The initial strict experiment therefore
refused before any SET. Padding is not a parameter readback. `Observation` keeps
the parameter as None; the default lease requires a reported zero parameter.
The separately named `open_source_startup` policy explicitly permits the stock
zero command argument when firmware omits the parameter. This verifies mode
restoration; exact restoration of an unreported parameter is not established.

Unknown modes, nonzero reported parameters, wrong devices/interfaces and
query-only handles are refused. Opening only reads. Entry checks the current
mode against the captured baseline, records intent before SET, and verifies
readback. Recovery restores only an owned Driver3 state; an external change is
retained and reported. Repeated entry/recovery is idempotent. Drop makes a final
best-effort recovery attempt. Comparisons are not atomic interprocess ownership,
and this register lease is not a complete physical input recovery contract.

## Controlled observation

`check_ornata_driver_mode <fresh-report.json> --stock-driver-mode <1..10>` owns the
mode lease in its parent process, starts the adjacent passive notification
example in a hidden child, bounds the child/report, restores the observed mode
even if the child fails, and checks final readback. The `--stock-driver-mode-gated`
variant waits for a fresh readiness marker from a successfully subscribed child,
then a separate fresh gate marker before its timed sample. Pre-gate events are
discarded without logging keys. Gate wait is bounded30seconds. This avoids
asking for typing while setup is still waiting for released keys.

The parent checks250ms of released Win32 virtual-key state before entry and
before recovery. This is a controlled-experiment precaution, not proof of every
physical/Fn key's state or a replacement for accepted-output tracking. Parent
termination, driver stalls and real intercepted-held-key recovery still need the
full runtime design. Stock user input processes were checked absent before these
experiments; the existing Lite preview remained open on an empty Default profile.

The short setup experiment verified mode0 →3 →0 with two mode SET attempts,
successful readbacks, quiet recovery and notification subscription/drain/disable.
The separately gated eight-second typing sample also verified the transition,
cleanup and restoration, but received **zero total notifications**. Mode3 alone
has not resolved the observed input-delivery problem. See
[physical report](validation/ornata-driver-mode.json). The prior normal-mode
sample had human-confirmed A/D presses; for this mode3 sample the user confirmed
readiness and received a typing cue after subscription readiness and gate release.
It does not independently prove every intended physical press occurred.

One earlier typing attempt was refused before SET because the typing cue was
given during the quiet-key setup. Its failure was corrected with the gated
protocol; it is not evidence that mode changes or cleanup failed.

## Remaining work

Retain the [notification routing findings](KEYBOARD-NOTIFICATIONS.md): filtered
keyboardMI01 and unfilteredMI00 are different physical paths, and a normal
callback uses type1. Firmware mode3 causing report routing between them remains
unproven. A metadata-only Raw Input probe identifies both MI00 and MI01 as
keyboard-type endpoints sharing the selected configuration's container. This is
candidate identity evidence, not attribution of actual typing; see
[metadata summary](validation/ornata-raw-keyboard-metadata.json).

Native follow-up shows the control child's role association uses exact ContainerID
equality, so its MI00 label does not imply a wrong logical instance. Common
initialization also runs during hardware preparation; there is no evidence for a
mode-order/replug repair. Initialization byte548 is an attempt latch rather than
callback success. The examined stack has no supported0x8888310C reset contract;
0x88883110 reports only that latch. Trace actual per-interface typing and live
role delivery before selectors/output; do not manufacture an initialization
fix through guessed commands.

Snap Tap interception, source assignments, completed-output ownership, release,
profile switch, unplug/sleep/crash recovery, UI controls and physical functional
acceptance remain unfinished. Neither this experiment nor its tests establishes
the complete build's resource or driver-replacement goals.

Validation: six mode-lease tests cover strict/source parameter policy, unknown
states, no-op baseline, compare-before-write, foreign-state preservation,
acknowledgement ambiguity, explicit/Drop rollback, idempotence and read-only/wrong
device guards. All348 core tests and the doc test pass. Hardware observations
remain separate from the simulated lease tests. Optimized probes, CLI and tray
builds pass.
