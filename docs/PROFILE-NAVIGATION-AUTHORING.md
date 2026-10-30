# Profile Switch authoring

Customize can author Next Profile, Previous Profile, Cycle Up Profile, Cycle Down
Profile and Specific Profile for the supported Ornata/DeathAdder inputs, in
Standard and Hypershift layers. Save changes the local desired profile only;
input-triggered profile/lighting activation remains unavailable.

Specific Profile selects another profile on the same device by its retained
source GUID. Names are labels: rename preserves the assignment, deletion refuses
Save, duplicate GUIDs are ambiguous, and the current profile cannot be a target.
Profiles without source identities remain visible as unavailable targets. No
name fallback or identity migration is invented. Retained legacy ProfileId
references can be explicitly saved as GUID references when they resolve.
Legacy native profiles without a source envelope still need a verified migration
path before this editor can save them.

The optional lighting selector uses an opened, saved, unchanged Studio library.
Duplicate effect identities are unavailable. Save rereads the bounded archive
and resolves current device profiles, so missing/changed references cannot silently
retarget an assignment. Unchecking an unavailable link is an explicit edit.
Choosing modes, targets or links stays in the draft until Save; Undo, Cancel,
dirty-close handling and read-only guards use the shared Customize workflow.
Neither selecting nor saving activates a profile/effect.

The shared assignment boundary preserves opaque row/group/profile data, other
inputs and the opposite layer, retires conflicting inert drafts and removes the
old native projection for the edited layer. Atomic store persistence follows
stale-owner/profile checks and budget validation. The selector catalogue retains
names and GUIDs rather than every profile's full payload.

## Source and verification

Recovered 605 MapSwitchProfile.0bdfe495.chunk.js SHA256:
`669b20ce5f1245e3742e9e2b95ba01678d6104f4547f5bd0511e7ec3bc964b01`.
The 196 module is also inventoried. REA parent evidence
`ev_de84446d3c008a778e8c3dbffa9c0d95277e41472d5b8dc9bb246316024a2375`
covers both selected source chunks as inert artifacts; semantic coverage remains
partial/unknown, with 774 semantic unknowns. No vendor bootstrap or device code
was executed.

The source main bundle defines the five action IDs. Its isolated pure group
converter, with synthetic state and explicit conversion/spread helpers, supplies
ten independent reference cases in
`crates/secbird-core/tests/fixtures/profile-navigation-source.json`. This proves
tested output shapes, not runtime routing. Core tests cover source export/re-import,
opaque/opposite-layer retention and malformed/stale references. Model tests cover
all modes for keyboard and wired/wireless mouse contexts in both layers,
reopen/Undo, legacy/unknown references and catalogue memory retention. Native
checks use an owned window, simulated transport and private files; Save must not
issue device SET commands.

The initial negative-GUI test completed state assertions but waited at the owned
error-acknowledgement dialog during shutdown. Its process was explicitly identified
and stopped. Current coverage checks normal native-window cleanup, then exercises
expected missing-reference/read-only refusals directly at the same save boundary.
Automated acknowledgement of that modal remains a separate harness TODO. The
stopped run is not a passing test.

## Remaining acceptance

Implement ordered runtime navigation independently of BTreeMap name order, local
identity migration where needed, device capability expansion, input ownership,
terminal routing/release, linked-effect activation and recovery. Native radio
glyphs, the linked form and shared feedback controls still need complete matched
visual/DPI acceptance. Hardware delivery/readback, privacy/network verification
and three complete-build RAM runs remain open.
