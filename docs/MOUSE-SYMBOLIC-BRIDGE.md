# Mouse symbolic packet planning

The separate [mixed-output scheduler](MACRO-MIXED-OUTPUT.md) now adds logical
button ownership and serialized one-shot wheel completion. This planner remains
pure, and its runtime gaps still apply to actual input delivery. No live sender
or physical ownership is established by either module.

`secbird_core::mouse_symbolic_bridge::plan_mouse_packet` converts one supported `source_macro::Event` into one fixed 32-byte RZCONTROL packet using the existing `MouseInput` codec. It uses no heap allocation in packet planning, opens no endpoint, and has no sender, scheduling thread, input injection, completion claim, held-output ledger or automatic release operation.

Callers supply an explicit `keyboard_symbolic_bridge::RoutingIdentity` token/unit, a native modifier mask and optional native source UnitId. RoutingIdentity has no Default; its token is symbolic context and never enters the packet. Missing source UnitId means native zero and must match routing unit zero. A nonzero routing unit requires the same explicit source unit. Missing-unit zero and explicit-unit zero produce the same wire bytes but retain distinct mouse-specific `SourceUnitProvenance` values. Neither construction establishes a physical device identity or permission to route input.

Only the ten ordinary button transitions and four observed wheel axis/sign combinations are supported. Public Event fields are checked for consistency: button ID, button name and direction must agree; wheel ID, axis, raw u16 data and signed delta must agree. Unsupported/combined IDs, other event variants, wheel amounts outside ±120 and mismatches fail without a partial result. Wheel raw bits are copied unchanged. No virtual-key, UI-button or HID conversion is performed.

Fresh source constructor zeros remain explicit: mouse Flags, RawButtons, LastX and LastY are zero. ButtonFlags receives the source ID; ButtonData is zero for buttons or the source wheel u16. Metadata uses the existing six-bit transform exactly once. `modifiers()` retains the original mask for provenance; it does not expand or synthesize modifier presses. Separate optional metadata arguments do not broaden the source JSON parser's accepted fields.

Every result retains its source Event, identity and unit provenance, and always reports unresolved identity validation, endpoint routing, asynchronous completion, cancellation, held-output ownership and modifier expansion. A numeric button-up conversion exists in native code, but this module deliberately provides no automatic release policy: no per-owner cancellation/recovery contract is established. The macro source ready path itself can report acceptance without propagating the IOCTL submission Boolean.

Source evidence is the installed mapping_engine.dll SHA256 `6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a`, image base `0x180000000`:

- JSON parser RVA10BFED (REA `ev_c9c221c7de14bd2dbf7be75986efcb7ad0e00f94efe87ae6e22fe601febc8109`) reads native id/data/modifiers and optional unit.
- Constructor10D8A8 (`ev_12170ee703f13565361768f7f33e0286da5abd65006906d489e6c7f5d4ab7f1c`) zeros the payload and unit presence; setters10E18C/10E2F8 (`ev_f449d852bd27f73387a1d422e5c68342b3f6cbc3cc7b859e73755d08173a1132` / `ev_b0013d52f9e729a28347d8068150da809a8b997bbdd3fc2bb225e8b73de17ea4`) preserve this fresh-object contract.
- Writers112C60 (`ev_7a092f1f72a31988a0225057ad142c911ee6e71fd3f16fea8b4216dc48049161`) and134C80 (`ev_f0229b08ace76bed275685ce33b0db687a50bd5368f9d214e3207e16550196bd`) independently copy native object+38..4B to packet+8..27 and apply1219DC once at+28. Targeted assembly corroborates the byte offsets.
- Mouse ready dispatch130CF8 (`ev_e298b47973ed9bb7317761bf37489f5c33ba6c736c55b453968b9a078aa65e7d`) calls void1345AA; it does not propagate asynchronous completion or actual submit failure to the macro ledger.

The fixture `crates/secbird-core/tests/fixtures/native-mouse-packets-static-derived.json` contains 14 fixed byte vectors: all five buttons down/up and both wheel axes/signs. They are independently transcribed from these offsets, not produced by the Rust codec or captured from a vendor runtime. Additional tests cover mismatched units, source-unit presence, inconsistent public fields, a single-transform metadata discriminator, constructor zeros, immutable source provenance and always-present runtime gaps. No real input was generated.

The [raw codec](RZCONTROL-PACKETS.md), [source macro parser](SOURCE-MACROS.md), [keyboard bridge](KEYBOARD-SYMBOLIC-BRIDGE.md) and [driver ownership contract](DRIVER-CONTROL.md) remain separate boundaries. This is a pure encoding foundation, not live macro or remapping support.

Validation for this checkpoint: six focused bridge tests passed. Full core debug tests passed 299, failed 0, ignored 6. Tests ran offline with an isolated debug target and workspace TEMP/TMP; no release build or hardware operation was used.
