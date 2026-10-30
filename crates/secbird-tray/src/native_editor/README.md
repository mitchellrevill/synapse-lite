# Native editor

The UI thread owns painting, hit testing and view/draft state. `device_worker.rs` owns serialized background jobs; `bitmap.rs` owns native image resources. Shared engine paths own device changes and output restoration.

See [architecture](../../../../docs/architecture.md), [development](../../../../docs/development.md), [assets](../../../../docs/assets.md) and [Studio](../../../../docs/studio.md). These describe the maintained boundaries and current limits.
