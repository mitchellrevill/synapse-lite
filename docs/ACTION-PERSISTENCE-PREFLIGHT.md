# Action persistence preflight

Persisted core actions prepare a cloned native Store and validate its complete
serialized representation before issuing a protocol SET. The validation uses the
same bounded serializer as Store.save: 2 MiB, 32768 JSON tokens, and 64 nesting
levels. Unknown fields, the destination path, and device-key aliases survive the
clone. A deterministic budget refusal changes neither the caller's Store nor its
file and sends no SET. Read-only discovery or button-baseline GETs can precede it.

DPI and stage candidates contain the same rounded values that their setters send.
The stage projection is exposed as Device.fitted_dpi_stages and does no I/O.
Brightness retains its existing percentage clamp. Policy-only Lighting edits
still issue no HID request; explicit brightness edits still reapply unchanged
desired brightness to repair drift.

Ordinary actions validate the candidate, write the device, save, and then replace
the caller's Store. Serialization refusal is prevented before hardware mutation;
this is not a transaction across USB and the filesystem. A later device rejection
or disk permission/space failure can leave hardware changed without the final
saved intent. No hardware rollback is claimed.

## Button recovery phases

Onboard writes and restoration preserve the existing recovery contract. Every
deterministic recovery/final candidate is validated before the first SET. Original
assignments are saved durably before changing the slot; failure to save that
backup prevents SET. A hardware failure retains the recovery phase on disk and in
memory, including original assignments and unknown metadata. It must not be
erased by rolling the whole action back to its initial Store.

Changing slots validates the old-slot recovery, cleaned-up state, new-slot backup,
and final saved-remap state before restoring even the first old-slot assignment.
Restoring live remaps validates the resolved recovery and post-onboard state;
subsequent partial states only remove successfully restored button-map entries.
Failed live remaps remain configured. A partial onboard restore retains the full
originals record so repeating restoration remains possible.

## Multi-device sync

Sync preflights its complete successful candidate and conservative upper bounds
for every partial result before any SET. Byte and structural-token upper bounds
are computed separately: a long opaque string and a node-heavy array need not
have the same maximum. The bounds retain old/new object members and choose the
larger atomic value for each metric. They include original device aliases as
well as resolved keys. These synthetic bounds are validation-only and are never
saved or sent to hardware.

This conservative method can refuse a sync near the limit even when its actual
result would fit. It prevents a successful device from making a partial result
unsaveable when another device fails. Actual persistence contains only successful
zone changes; failed devices retain their settings, aliases, and unknown data.
An action with no successful zone writes does not save or migrate the Store.

## Verification

Synthetic simulator/spy tests exercise exact 2 MiB and 32768-token growth
refusals, alias preservation, fitted stage persistence, recovery-before-SET,
backup I/O failure, partial onboard/live restoration, new-slot backup growth
before old-slot writes, and multi-device partial-state bounds. No physical-device
execution is part of this change.
