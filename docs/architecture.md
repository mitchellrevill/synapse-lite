# Architecture

The workspace has three crates:

| Crate | Responsibility |
| --- | --- |
| `secbird-core` | Device catalogue and HID transport, protocol framing, profiles, interchange, lighting math and reusable state models. |
| `secbird` | Existing command-line entry points and explicit local-file inspection tools. |
| `secbird-tray` | Windows tray, native editor, engine workers, output ownership and optional legacy browser editor. |

The native editor uses Win32 GDI/GDI+ and a bounded DirectWrite bridge. Restored windows request small DWM corners and draw a one-pixel muted outline. The default system border remains suppressed in both focus states; maximized windows keep square edges without the outline. The UI thread owns view state, drafts, hit testing and painting. `native_editor/device_worker.rs` owns the background request/reply protocol and serialized device jobs. Accepted setting updates are bounded and only adjacent compatible updates coalesce. File jobs release the engine lock before doing composition I/O.

`engine.rs` owns connected devices and configuration. Host lighting and macro sessions retain explicit ownership and connection identity. A disconnect invalidates a live session; reconnecting does not silently resume a prior output stream.

## Studio

`secbird-core/src/studio` preserves composition data and supplies selection, edits, history and preview compilation. Effect generators produce pixels without device I/O. Host output maps the compiled logical frame through an authenticated physical layout, then sends a bounded frame batch through the existing transport.

Normal-mode keyboard playback owns only the volatile lighting effect. It does not switch keyboard mode or alter input settings. Stop, failure and shutdown restore the prior owned effect where the connection and ownership checks permit it.

## Native images

`native_editor/bitmap.rs` owns DIB pixels and their borrowing GDI+ view. Dispose the view before freeing the pixels. Provisioned alpha images use an `SLBGRA1` header, dimensions and premultiplied BGRA bytes. Decode and validate bounds before native allocation. The project's Synapse mark is embedded and independent of private asset packs.

## Files and compatibility

Keep native configuration separate from installed application databases. Interchange retains opaque source fields where supported and reports unsupported translations. Draft persistence, active profile writes and device output are separate operations; importing or previewing a file must not implicitly start output.
