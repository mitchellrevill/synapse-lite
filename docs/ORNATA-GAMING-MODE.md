# Ornata Gaming Mode: host shortcuts and separate indicator

28 October 2026. Corrects an earlier firmware interpretation; this checkpoint
does not implement live shortcut blocking or its native controls.

## Corrected behavior

The old GameMode API sent a shortcut bitmask to `00:08` and treated an echoed
value as proof that shortcuts were blocked. The recovered Ornata605 path does
not establish that contract. `getGameModeBit` packs a host profile fingerprint;
its reviewed uses compare profile versions. The generic SDK's `00:08` state
setter accepts a byte but does not establish those shortcut meanings.

For025D, `game_mode` and `set_game_mode` now refuse before any device request.
Profile activation reports missing host shortcut blocking for both legacy text
settings and retained source-only/conditional settings. No saved intent is
deleted or silently claimed active. Import notices distinguish preservation
from replay. The legacy text projection remains for compatible file editing;
it does not authorize sending firmware bits. Other products' legacy behavior
is unchanged, not independently verified by this checkpoint.

The three existing lighting activation regression tests now explicitly expect
the missing Gaming Mode result while continuing to verify frame/brightness
behavior. They do not suppress it or weaken the lighting assertions.

## Source behavior to implement

The current Customize card has a master switch, **Apply in-game only**, Windows
and Menu choices tied to that switch, and optional Alt+Tab/Alt+F4 choices. The
examined active component uses no three-mode dropdown. Additional options in
shared code require product capabilities not established for605.

The host mapping generator preserves existing assignments. When enabled it
blocks Menu (scan93 E0) and selected Windows input (scan91 E0). Optional Alt
shortcuts use cascades tracking left/right Alt; Tab and F4 must continue to work
normally outside the corresponding Alt condition. Ordinary and Hypershift
mapping/held-output behavior must coexist with Snap Tap. Conditional activation
depends on local running-game state. These are source observations, not verified
replacement runtime behavior.

Retained source locations (normalized character offsets):

- Shared MW `source-0.js`: `getGameModeBit`508612, fingerprint comparisons1480114/
  1481070; host generator507630, disabled/cascade mappings499250–504000;
  `taskRunnerSetGameMode`1663200–1666740.
- Shared UI `source-1.js`: current card7497040–7501750, Customize use7575239.
- SDK `source-2.js`: generic state command7009–7440/20015; LED methods24900–25920
  and LED command constants43863.
- Product605 main: explicit `hasGameModeLED` capability178774; original artifact
  SHA256 `9cdec67b89a1f8ef54a386fc2ce2a0e2f93f50830e9da385a7be91074ba90b18`.

Retained MW SHA256:
`f3defb31d150738ed0566d2014b1d742952c3a4588f5c4eb2a1dec54139d6345`.
UI: `b42d1f0485182b074880f3831c72d1c47778ae5347041821856d17f5b62ea4aa`.
SDK: `0d933c5f576fbc05df57b4079a69e63cd507906a7ae887cdd786e8390c97ec8a`.
The MW retained copy differs from the original in line endings as documented
in prior extraction evidence; character offsets are not original byte offsets.

REA parent `ev_233e7f36ce339471e3bc2b928cfe7c5658131e1c180acde6e2c837004116ca53`
and summary `ev_79b05df6354b95b2eeb6faaba83ebf1d8745517c4eba0b18e34fa3571e8cba8a`
cover the four exact retained source copies: four parsed files,534 modules,
zero parse failures. Semantic coverage is partial/resource-limited, particularly
the large MW/UI bundles; these records are not an exhaustive dataflow proof.
The focused source inspection above and private contract report retain the
literal search bounds, capability gates and unresolved dynamic paths.

## Indicator API and physical verification

The source uses LED class0/id8, `03:00`, three bytes `[0,8,on]`, and GET `03:80`.
`ornata_gaming_indicator` exposes only that separate indicator. It requires025D
interface2 and an exact class/id/boolean reply. SET first establishes a supported
GET, avoids an unchanged write, and verifies readback. Query-only devices still
refuse writes. The caller owns restoration after an uncertain write error.

The explicit `check_ornata_gaming_indicator` example supports read-only or
reversible modes with a fresh report. Its restoration guard retains the original
indicator before the change; it neither changes input mode nor submits keys.

The [physical report](validation/ornata-gaming-indicator.json) verifies indicator
off→on→off in the keyboard's observed driver mode3. Initial/final input mode was3.
No input mode, selector, profile or frame writes were requested. This proves
register readback/restoration at those observations, not optical inspection,
shortcut interception, concurrent-client ownership or a complete Gaming Mode.
The earlier normal-mode-only diagnostic refused before an indicator request;
the revised check accepts observed0/3 without switching modes.

367 core tests and the doc test pass. Three new indicator tests cover the codec,
wrong product/interface, query-only gate and indicator-only command routing.
Two new profile regressions cover source/native round trips, retained unknown
fields and source-only/conditional/legacy replay refusal without any I/O. The
old simulator assertion that echoed bits proved shortcut blocking was replaced
with an exact025D refusal/store-preservation regression.229 serial tray tests
pass; optimized CLI/tray builds pass. The native Gaming Mode controls, shortcut
runtime and disable/profile/close/crash/unplug/sleep acceptance remain open.
