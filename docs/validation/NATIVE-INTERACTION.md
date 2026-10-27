# Native interaction checks

The opt-in Power test creates the production native window and message loop with one simulated mouse. It sends Win32 button/move/release messages at source-derived coordinates. Requests pass through the normal hit tests, gesture state, serialized worker and shared API dispatcher. It does not inject an OS pointer, take screenshots or open physical HID devices.

The fixture disables HID discovery before creating its context. Its rescan preserves the simulated device, its event-opening path returns no physical handle, and configuration goes into a unique temporary folder. The test runs alone because it temporarily changes process-local configuration/smoke environment and the process-global HID discovery setting. It has a twelve-second window timeout. Production builds do not contain the simulated engine constructor or test.

Verified checks:

- Neither slider sends a setting command before release.
- Sleep changes exact simulated seconds without adding a profile sleep override.
- Threshold changes persist at device scope and match raw readback.
- Cancelling a gesture prevents a later mouseup from committing it.
- Closing while the worker's actual engine mutex is deliberately held retains the accepted tail; after the lock is released the final 55% threshold is saved and reads raw 141.

Cancellation is handled explicitly before calling ReleaseCapture. Relying on DefWindowProc's synchronous WM_CAPTURECHANGED alone can re-enter the state borrow guard while the outer message holds that state.

Run on Windows with an interactive desktop, as a separate invocation:

```powershell
cargo test -p secbird-tray native_power_pointer_routes --locked -- --ignored --test-threads=1 --nocapture
```

`SYNAPSE_LITE_ASSETS` may point to locally provisioned source fonts/assets. The fixture prints the location of its temporary frame, synthetic profiles and final result. The recorded [result](native-power-interaction.json) binds the test executable and test source by SHA-256.

This proves the listed native-message and simulated-transport paths. It does not prove OS pointer transfer, keyboard/accessibility behavior, all resize/DPI/modal transitions, native rasterization parity, physical writes, sleep/wake or wireless behavior.
