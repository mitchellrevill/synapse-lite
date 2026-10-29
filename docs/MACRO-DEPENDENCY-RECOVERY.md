# Retained profile macro recovery

The local compatibility tool can attach verified macro definitions to existing
source profile references without reimporting profiles or changing identities,
settings or active selection:

```text
secbird profiles recover-macros NATIVE_BACKUP SOURCE_EXPORT EXACT_DEVICE_KEY NEW_OUTPUT
```

Inputs use the existing native backup/Synapse4 codecs. The native backup must
contain the exact target device key. Source hashes, conflicting identities,
malformed retained envelopes and the complete 2 MiB store budget are checked
before creating a new output. Existing output files are never overwritten.
Only directly referenced definitions are attached; unreferenced supplied macros
are ignored and unresolved references are reported. Repeating the same recovery
is idempotent. The command performs no device discovery, activation, input
dispatch, network access or installation into the normal profile store.

## Installed-data check, 29 October 2026

The earlier device-profile capture omitted IndexedDB. A stable shared-read copy
of the apps.razer.com IndexedDB files and blob sidecars recovered the one missing
referenced record. Source file sets/hashes were checked for stability. Stock
processes were not stopped and stock storage was never modified. A fresh offline
browser profile opened only the existing `macros` database and read the requested
GUID in a read-only transaction. All page requests were locally fulfilled or
aborted; no vendor JavaScript, account records or cookies were loaded. The first
unprivileged browser launch crashed; the subsequent scoped offline inspection
succeeded. Private database snapshots and identities are excluded from Git and
the portable package.

The recovered record contains one UI action-bar placeholder and **zero cached
native events**. It establishes a stored definition, not working macro playback.
The full raw record is retained privately. Portable export follows the recovered
stock projection: `guid`, `macroEvents`, and `macroPlaybackOption`, with the name
outside the encoded payload and the stable-JSON MD5 verified independently.

The prepared candidate adds one definition and leaves zero unresolved direct
references for this target. Independent JavaScript comparison verified that only
its retained macro array changed: every profile object, active selection and
other store field stayed equal. An isolated CLI re-export preserved the supplied
macro record exactly and passed an independent hash check.

**The candidate is not installed in the normal store.** A different workspace's
Lite process was running at preflight. It was left alone; no live store was
overwritten. Installation requires a fresh baseline/hash check with all Lite
writers closed. This is a mixed-time recovery: profiles captured on 27 October,
macro definition read on 29 October, not a fresh account-wide migration.

## Validation and provenance

Workspace checks: 451 core tests, 291 tray tests, four CLI tests and one doc test
passed; six core and 38 tray physical/explicit tests remain intentionally ignored.
Recovery tests cover idempotence, missing references, source GUI availability,
opaque-data preservation, exact export, conflicts, invalid hashes, malformed
envelopes, whole-store budget refusal and existing-file protection.

Recovered macro bundle `main.3f4b9604.js` SHA256:
`fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d`.
Stock profile export bundle `2592.d84cf7ac79aa3da58b37.js` SHA256:
`9c4a687e1a4868cd9da4dec40024e5ce3403fe866907887b47995f493e23a3a8`.
Focused two-file static REA summary:
`ev_5f20bf2bfca08676e1f8f69d1d7a95f6f3aa6de798e0ffb2e925aee56025248c`;
parent `ev_8a73dba5f353debc6a321559f43e267fab6ff9cfae0e9bd1f5c7c71b44e535cf`.
Two projections, 232759 bytes, 70406 AST nodes, zero parse failures. These static
projections establish the inspected storage/export contracts, not full runtime
or whole-application equivalence.

## TODOs; continue other features

- Install a freshly checked candidate when the shared store has no active writer.
- Implement macro authoring and the input scheduling/output/cancellation runtime;
  verify timing and held-key recovery on supported hardware.
- Resolve nested macro dependencies and global library editing separately.
- Verify stock import acceptance and complete profile activation. Synapse3 ZIP
  and macro-only source exports are not supported by this command.
- Continue remaining Studio effects, device families and shared workflows.
  Exact visual, hardware, privacy and three complete-build memory gates remain
  open. This change makes no new claim of pixel or complete-device parity.
