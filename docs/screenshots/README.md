# Screenshots

These PNGs are captured directly from the native editor at desktop resolution,
2560 × 1440 pixels. They are not enlarged versions of smaller captures.

- `macros.png`: isolated local macro editor with a synthetic library and the
  selected project logo. No fixture-device tab is shown.
- `studio.png`: isolated Studio session with a loaded composition, Starlight
  controls and provisioned local device geometry.

Both use the real window/message loop and production painter with simulated
transport. The screenshots contain no account data, serials or private paths.
They do not establish physical support or pixel-perfect stock parity.

Reproduce with the opt-in native tests
`native_macro_source_surface_selector_create_save_and_dirty_switch` and
`native_studio_starlight_properties_gradient_save_and_playback_preserve_source`.
Select the private asset pack with `SYNAPSE_LITE_ASSETS`; the Studio test also
requires `SYNAPSE_LITE_STOCK_STUDIO_FILE`. Run one at a time with `--ignored
--test-threads=1`. The tests print their private capture directories. Review
captures before copying them here. Do not publish their source profiles.
