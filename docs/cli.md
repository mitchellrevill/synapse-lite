# Command line

The executable remains `secbird` (`secbird.exe` on Windows). Crate names, command names and existing scripting interfaces are unchanged. `secbird --help` is the authoritative complete reference.

With multiple devices, use `-d DEVICE_ID` or `-d SERIAL`. Examples below change the selected device only where explicitly shown.

```text
secbird list
secbird -d DEVICE_ID info
secbird -d DEVICE_ID dpi 1600
secbird -d DEVICE_ID poll 1000
secbird -d DEVICE_ID stages 400 800 1600 --use 2
secbird -d DEVICE_ID color logo 55DD33
secbird -d DEVICE_ID effect logo spectrum
secbird -d DEVICE_ID lighting off
secbird -d DEVICE_ID profile new Gaming
secbird -d DEVICE_ID profile apps Gaming game.exe
secbird export backup.json
secbird import backup.json
secbird -d DEVICE_ID export-synapse4 profile.synapse4
```

`lighting off` releases app lighting control; it leaves the current RGB state untouched rather than switching the LEDs off. Device capabilities determine which controls are available.

## Local inspection

These commands inspect explicit files without activating profiles or applying lighting:

```text
secbird catalog inspect AvailableDevices.json
secbird profiles inspect-archive profiles.zip
secbird studio inspect lighting.ChromaEffects
secbird studio export lighting.ChromaEffects selection.ChromaEffects 1
secbird studio starlight lighting.ChromaEffects 1 LAYER_ID
secbird source-macro list backup.json
secbird source-macro plan backup.json binding.json plan.json
```

Additional linked-game migration and referenced-macro recovery commands are listed in `--help`. Prepared files are not automatically installed.

## Existing utilities

`secbird report` reads device information for diagnostics. `firmware` and `update` make explicit network requests; neither installs firmware. The upstream `debloat` utility remains available: its default command lists background components, `apply` changes them with an administrator session and backup, and `undo` restores the recorded changes. These commands are not run by repository cleanup.
