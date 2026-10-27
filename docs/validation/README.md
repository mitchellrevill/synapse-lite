# Validation status

The source baseline is v0.5.0-beta1. The workspace tests and release build pass. Tests cover profile codecs and preservation, device framing/correlation, and existing simulated device behavior. Five opt-in live/network tests remain ignored; passing the suite does not establish hardware parity.

Three real runtime runs included the normal tray, native editor, device worker/listeners and observed descendants. Frame evidence proves that the requested visible dashboard, mouse Performance and keyboard Lighting view was presented with two actual devices discovered, three recovered source fonts and both artwork assets. The largest measured working-set high-water was 22,241,280 bytes. The largest sampled private committed memory was 7,614,464 bytes. No process-owned TCP connection was observed. These results cover the recorded views and intervals; they do not prove complete functionality, visual identity, long-term behavior or removal of all installed Razer components.

The source-built fork successfully queried the isolated physical Ornata V2 firmware (1.1) and capabilities. Lighting effects, mappings/macros, reconnect/sleep and recovery remain to be validated per feature. All stock application changes during these read-only runs were temporary; Synapse was restored, and services/drivers were not removed.

Synthetic CLI interoperability passed: Synapse 4 fixture import and re-export, native JSON export, and import/re-export through the original source-built Secretarybird beta with known settings retained. Older Secretarybird builds drop unknown extension metadata when saving, so their edits cannot be treated as lossless Synapse-source preservation. A real Synapse application import of the generated document remains unverified.
