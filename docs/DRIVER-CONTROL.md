# Read-only Razer control-driver interface

The fork exposes `driver-control list` and `driver-control probe <interface-index> [limit]` on Windows. They inspect the installed vendor driver; they do not replace it. Limits are 1–64 records per probe, 256 discovered endpoints, 64 KiB per interface descriptor and 1 MiB of child output. The CLI runs the operation in a separate process and requests termination after a five-second parent deadline. A stalled kernel driver's cancellation is not guaranteed by that request.

SetupAPI discovers only the present RZCONTROL interface class `{e3be005d-d130-4910-88ff-09ae02f680e9}`. The reader accepts only discovered paths and only permits queries for the inspected installed product IDs 025D/00C4/00C5. A synchronous zero-access handle retains every request buffer until completion. The only exposed IOCTL is EnumInputHook, `0x88883030`, with a four-byte index input and an exact 292-byte output. No hook registration, clearing, redirection, notification subscription or input injection operation is exposed.

Keyboard kind 1 contains the 12-byte keyboard input structure at offset 8; mouse kind 2 contains the 24-byte mouse input structure at offset 8. Unknown kinds and all remaining bytes are retained as opaque data. Mouse coordinates are signed; ButtonData remains raw. UnitId 0xABCD is not treated as a generic wildcard. The probe reports its exact stop reason and always leaves complete enumeration unproven: a generic error can have several causes, and concurrent clients can change the collection between requests.

## Evidence

The inspected AppEngine 4.0.827 `mapping_engine.dll` SHA-256 is `6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a`. Installed `RzCommon.sys` 1.0.0.76 SHA-256 is `64b72ef9ce409bb17d31096c119b68c4a6a02ab35b7535d763157a4e1a587e59`. Microsoft dumpbin disassembly supplied the native evidence because REA's native decompiler provider was unavailable. No invented REA Evidence ID is assigned to those results.

| Contract | Caller / kernel RVAs |
| --- | --- |
| EnumInputHook size and index validation | Caller111912–111942; kernel352E–35D0 and6698–6774 |
| Keyboard payload boundary | Caller1125D1–11261C; kernel4734 copies12-byte records |
| Mouse payload boundary | Caller112859–11288A and1132D5–113304; kernel3334–3348 and4CDE–4D04 copy24-byte records |
| Per-file cleanup | File callbacks 1800/1890 call 592C, then 61AC removes the notification client |
| Selector lifetime | Device hook manager at context +570; constructor694C–6B1E; allocation7630–7745 parents selector storage to the collection |

The inspected close path removes per-file notification subscriptions but does not clear the device-scoped selector collection or recompute its cached mode flags. Selectors can therefore outlive their installing client in the observed architecture. This does not prove all crash/fallback behavior; it means a writer must implement explicit, ownership-aware cleanup and key-release recovery rather than relying on handle destruction. Unrelated selectors must be preserved.

## Runtime status

The Rust reader successfully enumerated the installed keyboard 025D and wired mouse 00C4 endpoints. With stock Synapse running, it received four keyboard-endpoint selectors and one mouse-endpoint selector, all 292 bytes. The next mouse index produced generic Windows error 31. See [the redacted runtime summary](validation/driver-control.json).

Seven core tests cover endpoint scope, FFI layout, keyboard/mouse field boundaries, opaque kinds, short replies and IOCTL fields. CLI parsing rejects unsupported limits before spawning a child. These tests and read-only queries do not establish hook ownership, input interception, crash recovery, wireless behavior, or a Rust kernel-driver replacement. Those remain outstanding.
