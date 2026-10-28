# Ornata input-source attribution

The passive diagnostic now attributes selected-keyboard Raw Input separately to
MI00 and MI01 while receiving the vendor filter's notifications. This produced
real input delivery evidence in the combined driver-mode setup. **It does not
enable live Snap Tap, suppression or key generation.**

## Physical results

| Controlled condition | MI00 raw records | MI01 raw records | Vendor records during sample |
|---|---:|---:|---:|
| Normal mode0, eight-second gated sample |4 (2 make/2 break)|0|0 total session records|
| Driver mode3, eight-second gated sample |0|52 (26 make/26 break)|52 (74 over the whole subscription)|

Both samples used exact active endpoint/container association, validated first
selected records and identities at end, and completed Raw Input removal and
vendor subscription cancellation/drain/disable. There were zero unrelated
keyboard headers in either sample. The driver-mode parent verified mode0→3→0,
two mode SET attempts and quiet recovery. The source's parameter0 was explicit;
firmware's unreported parameter remains None. No selector, redirection, key
submission, brightness/frame or profile operation was performed.

See [normal-mode sample](validation/ornata-raw-source-normal.json) and
[driver-mode sample](validation/ornata-raw-source-driver.json). The user confirmed
readiness, then received typing cues after the observer was ready and its gate
was released. Key names/text are not in these aggregate reports. Equal52 counts
do not establish per-event identity/order or injection compatibility.

This establishes MI00 input in the tested normal condition and MI01 plus vendor
delivery in the tested combined driver condition. Earlier mode3 vendor-only
samples returned zero; those negative observations remain retained. The current
combined diagnostic also registers passive Raw Input and pumps a hidden message
window, and the physical typing samples differ. Do not describe mode3 **alone**
as a fully isolated explanation or Raw Input registration as a proven fix.

## Implemented boundaries

`keyboard_input_identity` is metadata-only. It reuses bounded mouse list/name/type
helpers without changing the existing mouse admission contract. Selected
configurationPID025D/MI02 must be active with a nonzero container; precisely one
type1 MI00 and one type1 MI01 keyboard must match it. Missing/duplicate endpoints,
null/repeated keyboard handles, stale collection names/type or changed containers
refuse observation. Paths, handles and container values are not serialized.

The standalone `observe_ornata_raw_sources` example owns a message-only window
and keyboard usage1:6 INPUTSINK|DEVNOTIFY registration. It refuses existing
process keyboard/page-wide registrations, checks owner/flags, handles only one
initial arrival per captured keyboard and invalidates on subsequent inventory
changes. Cleanup removes only its own registration. No NOLEGACY flag is used.

Only headers are read for unmatched devices. A full payload is read only after
the handle matches a selected endpoint; complete keyboard size/type/identity is
required before accessing the keyboard union. The diagnostic records make/break
counts rather than key identities or text. Windows
[RID_HEADER/RID_INPUT semantics](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getrawinputdata)
and [per-process registration ownership](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerrawinputdevices)
inform these boundaries. Registration lives in the diagnostic executable rather
than a library that silently takes another window's keyboard registration.

The vendor receiver's final snapshot counts the whole subscription; a separate
counter delta measures its gated sample. Pre-gate events are drained without
logging. The mode comparison's `--stock-driver-mode-raw-gated` option selects this
child and requires both raw observation validity and registration cleanup, in
addition to vendor cleanup and observed-mode restoration. Gate wait/report/message
backlog are bounded; no inventory snapshot is claimed atomic across hotplug.

## Validation and remaining work

352 core tests and the doc test pass, including four new keyboard identity tests
and all six existing mouse identity tests. Two example tests check complete
packet/source validation and keyboard/page-wide registration conflicts. The
physical samples prove the specified delivery/cleanup conditions, independently
of pure metadata tests. Optimized diagnostic, CLI and tray builds pass.

Next connect the verified receive path to narrow owned selectors, source
assignments, accepted/completed output tracking and release/rollback. Establish
selector collisions and preserve unrelated clients before enabling interception.
Mode/registration coordination must also fit lighting ownership and the app's
existing Raw Input usage. UI controls, real Snap Tap arbitration and input
recovery across disable/profile switch/close/crash/unplug/sleep remain unfinished.
The full compatibility, Studio, driver and final resource goal remains open.
