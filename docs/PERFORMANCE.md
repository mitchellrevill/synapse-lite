# Source-derived mouse Performance state

The installed DeathAdder UI retains every configured stage row, including its hidden values, independent-axis flag and unknown source properties. Hardware receives a separate projection: visible rows while cycling is enabled, or the selected original row alone while disabled. The active original row must map to its packed visible position. `performance::Performance` models this distinction; the legacy `dpi_stages` field remains a physical projection for older readers.

Import retains raw X/Y even when an unusual source row has independent controls off. Loading a profile does not normalize that data. Explicitly turning independence off or editing linked X couples Y to X, matching the shipped UI. Disabling the selected row selects the next visible row and wraps to the first; the UI prevents reducing visibility below two rows. Reordering retains row identity, values and opaque fields. Turning cycling off preserves the other rows for later restoration.

Synapse 4 export serializes the retained state and merges its enclosing profile metadata. It preserves hidden rows, their stored Y values, unknown fields and the original scalar DPI on unedited roundtrips. A source default can legitimately contain scalar DPI 800 while its selected stage is 1600. Activation applies stage intent and does not overwrite that result with the historical scalar afterward. Older Secretarybird builds can read the physical projection but drop the additional intent when saving.

`POST /api/performance` requires the expected active profile name. A stale request is rejected before a hardware write. The shared action normalizes the device-bound values and validates the complete persisted candidate against the interchange budget before applying it. Hidden rows remain unmodified. Explicit legacy DPI edits preserve existing intent; explicit legacy stage-list replacement clears that richer intent because its input contains no visibility/axis flags.

## Native view

The product 196 renderer uses source-derived 600 px widgets, 68 px linked rows or 114 px independent rows, 4 px gaps, hover controls, stage ordinal icons and 72 × 27 px polling buttons. Below 1280 CSS pixels the polling widget wraps under sensitivity. The native source view uses these rules and local recovered assets rather than an approximate count dropdown or a linear DPI slider.

The nonlinear slider anchors are 100/0%, 400/25%, 800/37.5%, 1600/50%, 10000/75%, 26000/100%. Slider conversion rounds to the nearest step within the selected segment; numeric commit rounds upward to 50 and clamps 100–26000. Drag updates remain local until release. Numeric blur selects the edited visible row; hidden-row edits retain the current selection. Spinner repeat is 300 ms. Source numeric input is controlled signed decimal text, including temporary empty/minus states.

Native edit windows use the measured Arial 14 input font and clipped regions so they do not cover their parent-painted spinner targets or escape the scrolled body. A queue bounded to 16 operations coalesces adjacent settings for the same target/profile while preserving intervening ordered actions. Closing drains accepted work rather than silently discarding it.

## Evidence and remaining gates

Source artifacts are product 196 `main.bbc961f9.js` and `main.116d273e.css`; focused retained REA parent `ev_a9d0d7d76b9f77b61854bcb159c64edb962a3363ffbb41355d8f6c6443aa5f23`. Source projections and inert DOM/CSS replay establish formulas, rectangles, media conditions and state transitions. They do not establish full runtime equivalence. Vendor JavaScript, CSS and raw artwork are not embedded in the public source.

The workspace passes 162 tests, with 5 existing opt-in tests ignored; the recovery-encoding example test also passes. Tests cover intent packing/activation, hidden-row restoration, independent editing, reorder identity, unknown-field export, source nonlinear geometry, candidate-budget refusal before hardware changes, and queue ordering/bounds. The physical mouse also passed seven checks, including disabled/hidden projections and activation priority, followed by exact raw restoration: [hardware evidence](validation/performance-hardware.json).

The native renderer remains incomplete. Exact browser pointer transfer, keyboard/focus/spinner edge cases, drag preview/animations, layer composition, asset alpha on hover, fractional text rasterization, source host clipping and footer differences still need verification. High-rate warning and in-game polling sections remain unfinished. Read-only presented-frame evidence does not prove end-to-end native input routing. Full Synapse application roundtrips, macros and a Rust driver replacement remain separate outstanding gates.
