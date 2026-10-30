# Profile switch assignment reference validation

Bounded unit: read-only per-device eligibility/resolution API, exported as
DeviceProfiles::navigation_status and profile::NavigationStatus.
It does not author/save assignments or trigger profile application.

Source: recovered 605 MapSwitchProfile.0bdfe495.chunk.js SHA256
669b20ce5f1245e3742e9e2b95ba01678d6104f4547f5bd0511e7ec3bc964b01.
assignValues resolves ProfileId for legacy UI state; getProfileNavigationGroup
emits profileNavigationAssignment with guid for Specific and optional lightpacId.
The recovered main.fcf07de2.js dataset lists NextProfile, PreviousProfile,
CycleUp, CycleDown, Specific. This API accepts only those five names. It does not
infer native ordering or delivery from those names. Device supported actions and
current Chroma IDs are explicit inputs. No firmware slot mapping is inferred.

Specific references resolve within one DeviceProfiles by retained source GUID.
Rename changes only the returned label. Deletion produces a missing state;
there is no name fallback, retargeting, or destructive cleanup. Duplicate GUIDs
are ambiguous. Current-profile targets and missing linked effects are refused.
Opaque source objects remain unchanged. Existing profiles without retained GUIDs
cannot be specific targets in this unit; no identity migration is invented.

Remaining: native Customize form and local assignment Apply/Cancel persistence,
stable identity provisioning for locally created profiles without source envelopes,
legacy ProfileId migration, device capability wiring, ordered runtime navigation,
input ownership/release and live delivery; source-matched screenshots and final
combined memory acceptance. No screenshot parity or complete feature claim.
The current form retains its existing honest disabled Save state.

Validation (30 October 2026): cargo test -j 1 -p secbird-core --
--test-threads=1: 576 passed, 6 ignored, 0 failed; one doc test passed.
Targeted navigation: 3 passed. cargo build --release -j 1 passed for workspace.
TEMP/TMP were set to the task-owned work/temp directory. The initial default-temp
run had 47 filesystem permission failures (529 passed, 6 ignored), and the initial
release archive rename failed with access denied; both resolved with owned temp.
Existing unused-code/import and incremental hard-link warnings remain.
No hardware/native UI test was run for this read-only API. No screenshot claim.
GitHub CLI auth status reports invalid tokens; no PR was published.
