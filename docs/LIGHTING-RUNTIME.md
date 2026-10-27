# Host lighting policies

The tray implements retained Ornata V2 brightness intent with display-off and idle suppression. It writes live brightness only, keeps the configured enable flag and value in the profile, and restores the latest effective profile value when suppression ends. Global away dimming caps that value. The verified bridge is PID025D/interface2; standalone editor/serve modes report that the policy service is unavailable.

The existing three-second tray timer samples session input with GetLastInputInfo and reconciles changes. Display notifications reconcile immediately. Unchanged ticks do not repeat HID commands. Input timestamp wrap and ambiguous future values are handled explicitly. Failed operations retry with bounded backoff; a disconnected pending restoration survives reconnection. Normal shutdown attempts restoration and reports failures.

Known competing Synapse processes, incomplete process enumeration, disabled lighting ownership, missing required display/input state, and unreadable profiles prevent writes. Native process enumeration runs without helper processes. This gate does not establish exclusive ownership against WDL, OTFM, recorders or third-party animation clients. Forced termination cannot execute cleanup.

Pure state tests and a simulated device verify composition, profile switches, ownership failures, retries, disconnect/reconnect, shutdown and unchanged saved brightness/effect/sleep state. Physical brightness transport was checked separately in [the brightness round trip](LIGHTING-BRIGHTNESS.md). Actual Windows display-off, sleep/wake and idle workflows remain unverified. No driver replacement or complete effect engine is claimed.
