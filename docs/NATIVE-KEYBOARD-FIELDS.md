# Native keyboard event fields

Native AppEngine event JSON can now carry optional `modifiers` (u32) and
`unitId` (u16) through `SourceJson`, `Event`, the keyboard packet bridge and
the mixed scheduler. Both fields retain absence separately from an explicit
zero. Null, booleans, strings, negative/fractional/overflow values, duplicates
and unknown fields refuse the complete plan. Original source text is retained.
`delayInMs` and `releaseAfterMs` remain unsupported; they are never ignored.

Explicit source values override the bridge caller's fallback values. Omitted
values preserve the existing explicit-caller behavior; a caller wanting the
native fresh-object defaults supplies zero modifiers and no source UnitId.
The resolved UnitId must match the separately supplied routing identity.
No endpoint token, device label or filter sentinel supplies a missing UnitId.
The scheduler still validates its supplied fallback contexts at admission.

Each source modifier mask is translated exactly once by the existing recovered
metadata transform. It does not imply modifier-key presses or prove a native
ownership protocol. The scheduler admits varying metadata on different keys
and successive presses. A matching down/up must produce the same native packet
template after clearing the break bit: the current ledger releases the exact
original down metadata. A different requested up template refuses atomically
before any command is handed out, even if earlier source actions were valid.
Raw masks that translate to the same packet metadata remain admissible.
Stop and late/uncertain completion preserve the original release template.

## Source and version boundary

Retained shipped-artifact evidence is AppEngine4.0.827 mapping_engine.dll SHA256
`6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a`.
Parser10BC0D reads optional modifiers with zero initial value and optional
u16 unitId, then selects omitted/explicit-unit setters10D3D2/10D3F8.
The retained parser transcription has SHA256
`241ad96a4eb688eb3002e547b2207e659a9fddc7b4565f57046732998530fbb7`.
Existing packet, transform and constructor evidence is linked in
[the keyboard bridge](KEYBOARD-SYMBOLIC-BRIDGE.md).

The installed AppEngine is now4.0.896. Its mapping_engine.dll SHA256 is
`c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9`
(3,948,744 bytes). Historical addresses must not be applied to that binary.
Current-version static revalidation is tracked separately in validation. The
scoped CLI follow-up with the existing Ghidra12.1.4 and1GiB heap timed out at
240,000ms; its complete result is retained privately. No current-version
procedure identity or compatibility is inferred, and no identical retry is
required without a changed provider/startup condition.
Historical evidence does not establish current runtime delivery.

## Limits and acceptance

The original byte/node/depth/event/repetition budgets remain. Optional fields
consume the existing node budget. This is backend work with no changed UI and
no new screenshots. No store migration, driver load or input submission occurs.
Saved GUI definitions still require complete cache equality; this change does
not allow optional cache fields to bypass that check.

Tests cover source widths/absence/zero, strict failures, duplicates, exact
packet values, UnitId conflicts, varying/equivalent metadata, preflight
refusal and late uncertain-down cleanup. See
[validation](validation/native-keyboard-fields.json) for results.
Live routing/selector ownership, native completion/cancellation, modifier
expansion, concurrent/physical holds, conditional E1 scheduler support, remaining
events/modes and final three-run complete-build memory acceptance remain open.
