# Design QA

final result: blocked

The source reference is recovered from actual Synapse application artifacts through REA. Geometry was not estimated from screenshots. The native build has presented the dashboard, mouse Performance and keyboard Lighting views using the recovered fonts/artwork; the read-only frame evidence is in docs/validation/runtime-memory.json.

Pixel-for-pixel fidelity is unproven. The current port intentionally remains marked incomplete while full source-state recovery and a complete render comparison are outstanding. Known P1 gaps include keyboard key-shape/hit-region fidelity, macro/profile import/export workflows in the native editor, advanced/calibration controls and dynamic view updates. GDI and Chromium text rasterization have not been shown to match.

A passing compilation, source token extraction, or low memory measurement does not satisfy this gate. The project must not be handed off as a complete visual clone until these gaps are implemented and verified against the source reference across the required states and display scales.
