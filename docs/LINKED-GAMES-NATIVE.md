# Native Linked Games

The dashboard now opens a Linked Games module. Add an executable with the plus
button, open its card, and choose a saved profile for each connected device.
The Devices view offers the inverse list. Not linked removes that executable's
explicit path assignment for the chosen device; Remove game removes its local
catalogue record and explicit assignments across devices. Legacy filename rules
remain intact and are reported separately in the chooser. No program is launched.
Search, linked/all filtering, alphabetical order and bounded scrolling are local.

This is manual management for the exact-path switching runtime described in
APPLICATION-PATH-LINKS.md. Editing a link saves metadata; it does not activate a
profile immediately or issue device SET commands. The existing foreground engine
reconciles the saved path links on its next observation. Profiles are selected by
exact device key and saved profile name; stale identities are refused.

## Storage and recovery

Catalogue version1 lives in Store.extra._synapseLiteLinkedGames in profiles.json.
Catalogue and profile links are validated in a cloned candidate and saved by the
existing atomic native store writer. Unknown catalogue/game fields survive supported
edits. Unsupported/malformed catalogues are refused and retained. Limits are512
games,128-byte IDs,256 UTF-16 name units and the existing path/profile/store budgets.
Duplicate normalized paths are refused. The native worker releases the device-engine
lock before file work and independently refuses mutations in measurement read-only
mode before opening a store. Failed work retains the displayed data and reports an
error through the existing close/save acknowledgement flow.

The module reuses the approved tab surface and lifecycle without changing its
styling. Opening from dirty Studio uses the existing Save/Discard gate; after that
close completes, opening Linked Games may require a second click. Continuing the
requested navigation automatically is a TODO. No silent Studio discard is intended.

## Source and visual evidence

The installed cached Profiles chunks9449.8f17b519 and1255.b556ae48 were inspected as
unexecuted shipped JavaScript through REA. Full hashes and analysis identifiers are
in validation/linked-games-native.json. This is source evidence, not execution of
the vendor's scanner or a proof of its foreground matching algorithm.

The recovered CSS defines290x220 tiles with150px body,70px footer,5px radius and
20px inter-card gap; the native page uses those logical dimensions. The stock60px
default game image and module favicon are recovered from hash-verified local SVGs
using tools/provision-host-assets.cjs. Recovered assets remain private build data.
Device cards reuse the existing bounded device artwork cache.

The native Games, Devices and profile-dialog captures were reviewed against retained
stock captures and source geometry. Toolbar, popup layout, focus/hover/pressed,
filter/order controls and game metadata remain visibly interim. Native1600x900 and
stock2048x1104 captures do not establish matched-DPI pixel equality. Full visual
acceptance remains open; no tab styling was revisited for this checkpoint.

## Verification and explicit TODOs

A simulated native-window check adds a synthetic executable, links Gaming while
retaining saved Default selection, unlinks it, removes the game, and verifies the
entire native profile file returns byte-for-byte to its initial state. Device DPI
is unchanged and the command log contains zero SETs. Read-only UI changes are inert;
the worker separately refuses before creating a store. Games/Devices and shared
tab close/reopen routes pass. The regression injects the executable path instead
of automating the native file picker; real file-picker acceptance remains a TODO.

Core checks cover reversible metadata edits/backups, stale identities, duplicate
paths, invalid input, unknown-field preservation and unsupported catalogue refusal.
No live personal store was modified and no physical foreground switching was
performed. Existing Studio save-failure/exit recovery is checked separately.

TODO: source global game/GUID migration; installed-game scanning; cover-art/name
import and rename; launch-location lifecycle; disconnected-device management;
protected-game/real foreground switching; sleep/reconnect; matched-DPI visual
states; remaining path aliases. These are separate follow-ups, not blockers for
manual local linking. Full device/input/profile/privacy and three complete-build
memory gates remain open.

Workspace validation:465 core,295 tray,four CLI and one doc test pass; six core
and40 tray checks remain intentionally ignored. The native Linked Games flow and
Studio save-failure/exit recovery checks pass separately with simulated hardware.
The optimized workspace build and optimized native Linked Games interaction also pass.
