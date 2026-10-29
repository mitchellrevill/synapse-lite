# Local profiles linked to executable locations

Automatic profile switching can now distinguish applications with the same
filename installed in different directories. Existing name-based `apps` links
remain supported. Explicit `app_paths` links take precedence for a matching
foreground location; otherwise an intentionally configured name-based link can
still match. Without either match, the device returns to its active profile.
This changes the effective runtime profile, not the saved active selection.

The existing foreground watcher now queries the full Windows process-image path
using limited query rights and a bounded 32768-unit UTF-16 buffer. It does not
launch the program or inspect its arguments. Inaccessible/protected observations
return no identity. The log continues to show only the executable name, not its
installation directory. Local application suggestions can expose at most128
distinct visible-window executable locations through the authenticated API.

Configure a profile explicitly through the CLI:

```text
secbird -d DEVICE_KEY profile paths Gaming "C:\Games\Title\Game.exe"
secbird -d DEVICE_KEY profile paths Gaming
```

The second command clears only exact-path links. The separate `profile apps`
command continues to manage name-based rules. No profile is selected merely by
editing its links; the normal foreground transition/reconciliation applies the
effective profile. Per-device rules remain independent. Moving an exact path to
another profile removes that path from the other profiles of the same device.
Profile duplication starts with neither kind of link.

Authenticated `POST /api/profile` also accepts action `paths`, the existing profile
name, exact connected `id`, matching `expected_device`, and an explicit `paths`
array. Missing/null lists refuse; `[]` deliberately clears. Invalid identities,
paths, excessive lists or missing profiles leave the on-disk store unchanged.
`GET /api/application-paths` returns visible-window path suggestions; the existing
`/api/apps` response retains its filename-only shape. Both use the server's
existing token/Host/Origin checks. Enumeration runs outside the device-engine
lock; GET-only/body-free admission is verified while that lock is held.
Read-only measurement mode refuses changes.

## Identity and recovery

New path rules support absolute drive and UNC executable paths, including ordinary
extended drive/UNC prefixes. Slashes and ASCII case normalize. Relative/device
paths, dot segments, ambiguous empty/trailing components, reserved DOS names,
streams, wildcards and non-executable suffixes refuse. No environment expansion,
filesystem traversal, executable opening, game scan or network lookup occurs.
Authoring is limited to128 paths per profile and32767 UTF-16 units per path,
with the existing complete-store document budget still required before save.
Non-ASCII spelling is retained; Unicode case aliases, junctions, short names and
other file-identity aliases remain unverified rather than guessed.

Hand-edited duplicate exact bindings in different profiles fall back to the
active profile rather than selecting by alphabetical name. All list validation
finishes before any profile is mutated. Native backups retain path rules. Synapse
device-profile export currently refuses them, as it already refuses native
name-based links: cross-compatible global linked-game export is not implemented.

The foreground selector now caches observed per-device selections as well as the
executable identity. A changed store is reconciled even when the foreground
executable stays the same. This fixes stale selection after reassigning a link.
Repeated unchanged observations make no device calls; failed store reads retain
the previous observation and bindings, invalidate the stamp cache, and let a
later event retry even if the file modification time has not changed. Existing profile application,
button restoration and read-only guards remain in use. Transition/crash recovery
for the unfinished host input engine is a separate full-goal requirement.

## Evidence and verification

Installed cached Profiles main `main.776df2b1.js` SHA256
`5c765e57ea009fa8f66ce9f973a99cf8a1b2e7c994c99a776a0ad8feeba9a6c2`:
the game catalogue projection at decoded characters2059127–2059886 retains
`gameSignature.launch_exe` as `location` and `entry_guid` as game identity.
The linked-device reducer at2074681–2075336 binds game IDs to device product,
serial and profile identity/name. These are global relationships, not merely
strings inside the device's portable profile payload.

REA summary `ev_1c9a63d720b53819c84191dd79a84d3b624cdb34e16a2272a3136d57e2ba200a`,
parent `ev_626f666e42b7bb22f87ecbeb6168ca60d8849a2b0f1f28344bf8f84681bcaabb`:
two focused function projections,1420 bytes,457 AST nodes, zero parse failures,
112 semantic unknowns. This static evidence does not recover the vendor's exact
foreground comparison algorithm, run its scanner, or prove live Synapse parity.
The Rust matching rules above are explicit implementation behavior.

Four core checks cover path forms/refusals, same-name precedence, ambiguity,
atomic reassignment, independent devices, copy semantics and backup/export
behavior. A real read-only Windows query verifies this test process's executable
location without borrowing another window's identity. Simulated engine checks
switch DPI800→1600→3200→800 for two same-named applications, preserve saved
selection/profile bytes, avoid repeat writes, reconcile reassignment while the
same application stays in front, recover from a malformed store without
forgetting the observation, and refuse all writes in read-only mode.
The authenticated API check covers identity, missing lists, invalid members,
metadata-only persistence, read-only refusal and zero hardware SETs.

No normal user-store migration, new foreground hook, live physical switch or
input generation was performed. The existing hook supplies the new observations.
There is no new native Linked Games page yet, so this checkpoint changes no UI
paint and establishes no pixel-parity claim. Native cards/selection controls,
source GUID/game-library migration, game artwork/discovery, protected-game and
sleep/reconnect/real foreground acceptance remain TODOs. Full profile/device,
privacy, input and three complete-build memory gates stay open.

Workspace verification:462 core,295 tray,four CLI and one doc test passed; six
core and39 tray checks remain intentionally ignored. The public evidence keeps
private process locations and user profile records out of the repository.
