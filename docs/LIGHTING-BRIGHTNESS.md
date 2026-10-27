# Ornata brightness round trip

The installed Ornata V2 (025D/interface 2) passed five bounded brightness checks: ordinary brightness, source enable off/on with retained value 73, and temporary live suppression/restoration. Source region 0 and the existing LED 5 selector agreed for saved and live brightness. A 73% write read back as raw 186. Live writes changed profile 0 while profile 1 remained raw 186.

The meaningful declared effect header stayed unchanged across source brightness and live suppression checks. No mode or effect setter was sent. All four captured raw brightness registers were restored exactly; exercise and restoration error lists were empty. Stock user processes were temporarily closed and eleven were observed after restarting them and waiting three seconds. Services and drivers were unchanged.

See [the scoped result](validation/lighting-brightness-roundtrip.json). This validates brightness register behavior, not visible color output, arbitrary animation restoration, input recovery, or the runtime's complete display/idle lifecycle.
