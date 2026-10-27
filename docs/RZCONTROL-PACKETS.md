# RZCONTROL input packet codecs

`secbird_core::rzcontrol_packet` encodes/decodes the ordinary32byte input packet used by the installed Razer control driver. This is a pure codec: no handle opens, IOCTL calls, hooks, injection, clocks, callbacks or vendor runtime dependency. It does not implement host macro playback. [Driver control](DRIVER-CONTROL.md) describes the separate read-only driver probe and ownership limits.

The first u32 routing word must be0. Kind1 at offset4 carries12byte KEYBOARD_INPUT_DATA at offset8, followed by12zero bytes. Kind2 carries24byte MOUSE_INPUT_DATA at offset8. All numeric fields are little endian. Explicit UnitId, reserved, raw flag and metadata values are preserved without inventing their meaning; keyboard tail/unknown routing/unknown kinds/short or oversized packets are rejected. Mouse wheel data preserves its raw16bit signed pattern. This format is neither HID onboard macro bytecode nor Win32 user-mode INPUT.

Source identities and corroboration:

| Artifact | SHA256 | Location / evidence |
|---|---|---|
| mapping_engine.dll |6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a|RVA112C60 ordinary SendInput construction; REA ev_7a092f1f72a31988a0225057ad142c911ee6e71fd3f16fea8b4216dc48049161|
| RzCommon.sys |64b72ef9ce409bb17d31096c119b68c4a6a02ab35b7535d763157a4e1a587e59|keyboard dispatch RVA3334's neighboring kind1 branch and injection4734; mouse kind2 RVA3334..3348→4C3C; static dumpbin/PE evidence|
| mapping_engine.dll metadata mask |same hash|RVA1219DC; REA ev_ab1987fbefb25949807667cacdbcc5e8607b6d0ce54adb85850b1a988e97cc30|

The six-bit metadata transformation is numeric evidence only. It does not translate a macro's make/break flag, Windows virtual key, HID usage or special-key semantics. Event-to-raw conversion, endpoint selection, asynchronous completion lifetime and held-output recovery are separate requirements. No sender is exposed.

Macro timing/cancellation limits: native DispatchMacroEvent at RVA12BE6E inserts a10,000microsecond wait between consecutive keyboard-kind21 events (REA ev_bed07cbcc6184aaf6145e517884bf89970b7a48a8c5e1bb89b73117e82d41034). TimedWait RVA11AA78 proves microsecond units by multiplying by1000 before comparing QPC nanoseconds (ev_8fa354ac207d3f4b6da2173c7b9192ca12b9e833bafe3183238a418a04b755e1). A simple sum of explicit macro delays does not reproduce all source timing. StopRunningTurbosAndMacros ultimately signals work events; a successful acceptance callback does not prove all injected makes received releases. Do not infer input recovery from this codec or from closing a client handle.

Tests use fixed, fully specified keyboard/mouse byte arrays, negative movement and wheel bit patterns, unknown/length/tail rejection, and the recovered metadata bit mapping. These are synthetic static-ABI checks, not hardware execution. No vendor binary or decompiled implementation is shipped with this module.
