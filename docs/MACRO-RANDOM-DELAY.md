# Editing retained random-delay ranges

Selecting an editable random-delay row in Macro shows minimum and maximum
milliseconds. Edit the two bounds, then Apply Delay or Save. Changes use the
existing local draft, Undo/Redo, explicit Save/Discard and revision checks.
The row label shows the actual source range in seconds. This does not enable
random-delay playback or recording settings.

The source range editor permits values up to5seconds. Lite edits ordered ranges
from0 to5000 whole milliseconds, including equal bounds. Values that cannot be
represented exactly in that domain remain preserved and non-editable. Opening
an imported row does not round it. Conversion back to seconds uses three decimal
places and retains the original GUID, profile references and opaque fields.
Unknown raw leaves inside the Number object and other rows/envelopes are retained;
only the explicit min/max fields change. Duplicate required fields, phased rows,
native caches, invalid bounds and stale revisions refuse instead of guessing.

The native representation remains Type0 with Number.min/Number.max in seconds.
This is distinct from the AppEngine randomDelay/minMs/maxMs projection. Six
controlled executions of the recovered pure converter verify the relationship,
including zero/equal ranges,5seconds and1.001seconds. No bundle bootstrap or
vendor runtime was executed. A separate pure input-handler replay establishes
the source's two-decimal typing truncation and automatic range adjustment.

## Source and implementation limits

The historical cached renderer chunk8190.61506f5b.js has SHA256
003b116007bb415462e5175c09fc84928d2a20c8d1a7c73113a432643d2cbe9f.
Module58190 was copied as an inert source slice with parent byte offsets and
hashes retained privately. REA parent Evidence
ev_7a5ab10206f5bdc262af92d96a72dc14193b301a7b893d7f17d43a96547aa1f9
parsed the slice without executing it. Its semantic graph is partial, reaching
the100000-node ceiling with8706 unknowns; it is not proof of runtime behavior.
The native converter comes from the previously recovered main.3f4b9604.js.
The committed fixture contains synthetic inputs/outputs, not vendor code.

The current Macro editor remains an interim native layout. Stock edits ranges
inline in seconds, with focus/blur behavior and automatic adjustment. This
checkpoint uses explicit validated millisecond fields in the existing sidebar.
Those presentation/interaction differences, the source typing precision behavior
and matched-DPI pixel comparison remain TODOs for final visual acceptance. The
painter capture does not composite child EDIT windows; native control text is
checked separately. Approved tabs were not changed.

When a random row is selected, Add1000ms Delay adds an ordinary fixed delay;
it does not claim a new recording policy or random-event creation workflow.
Native caches remain guarded until their invalidation/rebuild contract is
verified. Standalone XML structured-delay support remains separate.

Random sampling distribution, native randomDelay parsing/execution and timing,
mixed-input routing/ownership/recovery, recording integration and complete
hardware/API/privacy/visual acceptance remain open. All three final complete-
build working-set/private-memory runs below100000000bytes are still required.

See validation/macro-random-delay.json for the final checks and retained artifacts.
