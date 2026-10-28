# Replacing a running preview

Lite permits one tray instance. Starting a newer executable while an older
instance is running asks the older process to open its editor; it does not
replace the process or its assets. A newly packaged file is not proof that
the updated UI is on the desktop.

Exit the old tray instance, then run the new executable with its accompanying
assets directory. The new explicit `--quit` CLI option requests the same normal
window-destruction lifecycle used by Exit, including lighting-worker shutdown.
It works with the existing tray window class in older previews. It does not
initialize another HID engine, force-terminate a process, change startup
registration or delete profiles. It posts an asynchronous exit request; callers
must verify the old process has ended before launching another build.

`--quit` is idempotent when no tray exists and must be used alone. Do not use
Exit while an editor holds work you intend to keep. This restart was performed
after checking that the old preview had no targetable editor window.

28 October verification: the running `native-preview-91c8d72` process40952
ended after the explicit exit request, without forced termination. A second
no-instance invocation returned0. Combining `--quit` with measurement flags
returned2 before normal app initialization. Stock Synapse was not closed or
modified. This verifies replacement lifecycle, not hardware ownership read-back
or a complete-build functional/memory acceptance.
