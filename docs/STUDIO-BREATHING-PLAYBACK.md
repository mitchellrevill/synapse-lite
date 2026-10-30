# Studio Breathing playback checkpoint

This branch starts from published main `6149f7a`, rather than canonical integration
`7b99930`, whose unrelated unpublished integrations and dirty core file were left
untouched. Work was isolated through a Git bundle after local clone transport
could not launch its helper on this host. Origin points to the user's GitHub repo.

## Implemented behavior

- Playback properties author fixed/random color, continuous/onPress/onSelectedKeys
  start, never/after/toggle end, and finite repeat counts or forever. Existing imported
  settings stay raw until explicitly changed. Repeat buttons increase/decrease a
  source count1–100; imported positive signed32 counts remain playable. Zero and
  -1 retain native continuous initialization. Changing start resets end/count to
  source defaults: random→never/-1, input→after/1. Never/toggle use -1; After uses1.
  Toggle is unavailable for random start.
  The playback view retains its last explicitly edited count when switching end
  or input start, matching the source component's local numeric-count cache.
- Explicit random Breathing uses the recovered five-stop native fallback shape,
  per-effect stream, quantized non-gray colors and time bucket. It rebuilds after
  emitting frame zero, matching the retained frame procedure. Fire's random stream
  is independent. `Preview::from_snapshot_at` supplies a reproducible Unix-seconds
  clock; ordinary preview uses current Unix seconds.
- Fixed and random finite playback contributes all requested cycles, then leaves
  scratch transparent so the lower layers remain visible. No final-frame hold is
  invented. Input state-machine diagnostics mirror qualification, press-edge,
  held-key, active-repeat and toggle guards through explicit synthetic calls.
- Production preview/output continues to refuse input-triggered/toggle settings.
  The synthetic state machine is not wired to native input and does not bypass
  ownership, device, file-save, read-only or output admission gates.
- Reset to Two Colours explicitly masks custom/inherited gradients with
  `colorStops:null`, clears `colorStopsCustom` and switches off random color. The
  shipped converter treats null as its ordinary non-array two-color branch.
  Layer gradient tokens remain untouched, and unselected memberships retain their
  original parameters. Undo restores removed regional gradient bytes exactly.
  Duration, repeat, start/end, colors and unknown leaves are preserved by reset.
- Enabling random preserves a retained gradient; switching random off restores
  its use. Ordinary color buttons still refuse an effective custom/random mode.
  Selected inherited colors are projected for display after reset. Picker None
  uses the recovered `0x64000000` sentinel to override inherited colors; explicit
  first-color None stays transparent rather than falling back to the second color.

## Source evidence and boundaries

No vendor DLL or hardware effect stream was executed. These are retained Ghidra
observations plus isolated JavaScript conversion and independent reconstruction,
not successful vendor runtime or optical acceptance.

Native DLL SHA256:
`cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa`.
Retained records live under the original task's `work/rea-native` directory:

| Record | Address | REA evidence |
|---|---|---|
| breathing-init.json | 0x1800651c0 | `ev_30008e015878effeb86e23de09ca15ea646559ae9ebe08ecd03069c822d7b388` |
| breathing-frame.json | 0x180065570 | `ev_b5eafe2c42e11cc3d33d41fcca61d46557fb6519ee5ab85c8a760b6db67d75b9` |
| breathing-trigger.json | 0x180065730 | `ev_93d359667518168e36976b6658eda5f07b721b7e6508392b8f5d3baa73e17853` |
| effect-random-color.json | 0x180069600 | `ev_0a8755b1a23589be46721edda44c2b9a14b4e4e0738725260f27477905a79631` |
| native-time-seed.json | 0x18014bdf0 | `ev_26c6711f9f49745355f9165ff15a0456839321a1e8abebd3691e499e1675baa4` |

The time helper converts FILETIME to Unix seconds. The initializer buckets this
value by 1,800,000 and ORs 31. The color helper advances the 214013/2531011 stream,
xors the high16 and low bits, reduces modulo255, quantizes at85/170, and rejects
gray triplets. Rust converts its native BGR bytes to the existing RGB pixel type.
`tools/reference-breathing-random.py` independently retains wide multiply/carry
arithmetic and emits 160 reference colors and post-call states across five clocks.
These reference values do not establish wall-clock scheduling accuracy.

Converter module2578 SHA256:
`008bf7b8aac4e86f47dd0c4e498402949c384d640553ca08d9c2424ad3527bda`,
from lighting-engine main.a383ea9e.js. Prior REA parent:
`ev_942d8521b67e4562f40aa61df404a8b2af9afb4d1656fd8e9a426102d5afa060`.
`tools/replay-breathing-playback.cjs` replays this exact module with AddEffect
stubbed; the checked-in codec evidence records random1, onPress4096,
onSelectedKeys12288, toggle4, finite counts and the null reset conversion.

The recovered Breathing UI chunk679.faa70d40.chunk.js has SHA256
`eb27c4ad24481e48cca90baba48f368328ef6c3bb4f592d982af77e3db786be0`.
Module2777 emits the picker None sentinel, while module5805 defines three end
modes, start/end coupling, toggle suppression for random start and the numeric
count control. Its min/max bindings resolve through main module6257 to1/100.
The main UI bundle SHA256 remains
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`.

## Remaining acceptance

Native input ownership/terminal completion remains a shared blocker. This feature
does not claim live input dispatch, device compatibility, physical/optical timing,
malformed-gradient random fallback, or the complete-build below100MB acceptance.
The current separate playback properties page is an interim native presentation;
source radio/dropdown/numeric-field placement and pixel fidelity remain open.
The inventory's retained Studio screenshots provide shell/style evidence, not a
matching stock Breathing playback screenshot. No exact visual match is claimed.

Validation results and captured native renders are recorded alongside this document
under `validation/studio-breathing-playback.json`.
