# Native overlay runtime checkpoint

The release executable at source checkpoint `b746eda` completed three passes of four read-only views: the Lighting color editor, empty profile Import, prepared profile Export and disabled Cloud state. Each view ran for12seconds with the actual keyboard's read-only snapshot, recovered fonts/assets and a presented native window. Reports require the requested overlay to be visible and ready; simply starting the process does not count.

Across all12 runs, peak working set was25,538,560 bytes, peak sampled private committed memory11,010,048 bytes and peak paged process memory11,268,096 bytes. One replacement process was observed per run. No TCP connection was observed by the sampler. The [complete sampled result](validation/overlay-native-memory.json) includes the executable SHA256, overlay metadata and metrics.

The stock user process count was15 before isolation,0 during the runs and15 five seconds after restart. Services and installed kernel drivers stayed running and are excluded from the replacement totals. This is not a whole-system memory-saving measurement.

The color editor uses a reported presentation-only Static override when the current effect has no color control; it does not submit that effect or alter the profile. Import does not browse or commit a file. Export constructs its real preview through the normal worker without saving a file. Separate simulated HWND regressions verify unchanged profile bytes and no new SET commands for these paths.

These results satisfy the100,000,000-byte threshold for these views at this checkpoint. They do not establish final-build acceptance, large imported profile/macro workloads, active animation resource use, visual pixel identity, or input/sleep/hotplug recovery. Source checkpoint and executable identity must remain attached to the numbers as implementation continues.
