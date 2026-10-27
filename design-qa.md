# Design QA

final result: blocked

The source reference is recovered from actual Synapse application artifacts through REA. Geometry was not estimated from screenshots. The native build has presented the dashboard, mouse Performance keyboard Lighting and keyboard Customize views using the recovered fonts/artwork; the read-only frame evidence is in docs/validation/runtime-memory.json.

Pixel-for-pixel fidelity is unproven. The current port intentionally remains marked incomplete while full source-state recovery and a complete render comparison are outstanding. Known P1 gaps include complete keyboard drawer/hover/state fidelity, macro workflows, end-to-end profile import/export validation, source Lighting/Power and advanced/calibration controls, drag/layer/animation fidelity and dynamic view updates. GDI and Chromium text rasterization have not been shown to match.

A passing compilation, source token extraction, or low memory measurement does not satisfy this gate. The project must not be handed off as a complete visual clone until these gaps are implemented and verified against the source reference across the required states and display scales.

The current native editor uses locally recovered UK key geometry and source icon assets. Profile Add/Duplicate/Rename are source-derived; Import/Export currently use native Windows file dialogs, so their modal rendering differs from Synapse. Inert DOM/CSS replay established the keyboard image/overlay shared origin and the nested body padding. Global host clipping and fractional CSS placement are still unresolved.

The source Performance renderer and retained stage-state model are integrated, with nonlinear controls, source spacing/media wrapping, axis linking, hover icons and polling buttons. Seven presented-frame workloads pass under 100 MB at 125% DPI. The hidden/independent render variants are presentation-only, and all native pointer/focus paths still require validation. Fractional text rasterization, browser range transfer, composition/alpha and exact main-host clipping remain open; this is still not a verified pixel-identical clone.
