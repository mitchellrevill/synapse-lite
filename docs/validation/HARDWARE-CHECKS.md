# Installed hardware checkpoint

Five setting write/readback checks passed on the physical installed devices, using the fork's shared HID implementation:

| Device | Verified setting writes |
| --- | --- |
| DeathAdder V3 HyperSpeed wired, PID 00C4 | DPI axes, polling rate, sleep timeout, DPI stages |
| Ornata V2, PID 025D | Backlight brightness |

Every captured register matched its original bytes after restoration. The mouse snapshot includes saved/live DPI, polling, idle seconds and the complete stage reply; the keyboard snapshot includes saved/live brightness. A separate injected workflow error immediately after a verified DPI write also returned failure and restored every captured register. Synapse was restored afterward; services and drivers remained running throughout. Real profile files, input maps and macros were not touched.

These checks establish setting request/readback behavior. They do not establish physical polling timing, actual sleep/wake behavior, lighting effects, key mapping/macros, wireless transport, disconnect/reconnect, crash recovery, or visual parity.

## Reproduction

Build the opt-in example with `cargo build -p secbird-core --example installed_roundtrip --release --locked`. Close competing configuration clients before running it. `installed_roundtrip <00C4|025D> <report.json>` only snapshots registers. Add `--roundtrip` to exercise the setting writes and restoration. A mouse roundtrip with `--fail-after-dpi` deliberately returns exit 1 after its first verified change, then checks restoration. It must report the expected injected error and `restore_verified:true`; an arbitrary failure is not a passing recovery test.

The example accepts exactly one inspected device per requested PID and rejects simulator mode. It writes its recovery snapshot before mutation, refuses roundtrips with differing live/saved values, validates stage-slot encoding before mutation, restores stages before saved/live DPI, and compares all captured register bytes afterward. Failed restoration remains armed for a destructor retry. This is a test harness, not a guarantee against power loss, unplug, process kill or a stalled kernel driver.

The source-derived stage contract matters: command 04:06 uses selector 1, a one-based active stage, and zero-based SET slot IDs. GET returns one-based slot IDs. Payload length is 3+7×count; captured X/Y/Z bytes must be preserved. The shipped Synapse task runner explicitly verifies GET_ID=SET_ID+1. Its recovered middleware SHA-256 is `df8334aa3021d129893918cecb3c1d14b3727f476c42981bac3fb02f6736b633`, with the builder at UTF-8 offset 1681602 and comparison at 1694210. Replaying a GET packet unchanged as a SET packet would violate that contract.

The final example also observed that the captured stage register stayed unchanged immediately after a direct class 1 DPI write on this installed mouse. That is a measured device/firmware result, not a general protocol guarantee. Full values and raw snapshots remain private; [the public aggregate](installed-hardware.json) records checks, register sizes, executable digest and scope.

The newer [Power checkpoint](power-hardware.json) adds the raw critical-threshold register (07:81/01, one byte) to the pre-mutation snapshot and exercises one UI percentage change with exact raw readback. Recovery replays the saved raw byte directly; converting it through a displayed percentage can lose information. Eight mouse checks passed in this run, and every captured register restored exactly. The stock app was restored with ten user processes observed after three seconds. See [Power semantics and limits](../POWER.md).
