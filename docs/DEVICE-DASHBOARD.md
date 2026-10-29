# Portable model artwork in the native dashboard

The dashboard now resolves each connected device through its product/connection
catalogue identity and draws that model's artwork from a local, portable pack.
It no longer uses the DeathAdder photograph for every non-keyboard device or the
Ornata photograph for every keyboard. Existing two-model assets remain fallbacks
for those exact models only. Customize artwork is a separate role: an unavailable
model does not borrow another model's customization picture.

The current private pack contains 653 declared edition variants across 298 primary
product IDs, stored as 556 deduplicated PNGs, approximately 70 MB on disk. These
are source-defined category-default dashboard images, not all regional, sidepad,
wrist or customization assets. Source AVIFs remain in the private acquisition
snapshot; the [asset pipeline](ASSET-PIPELINE.md) also retains 556 deduplicated
originals beside the native PNGs. Provisioning preserves their decoded alpha without adding a matte.
Images are scaled down only when larger than 768 by 512 pixels and encoded
losslessly. No fonts, vendor code, installers, profiles or device APIs are bundled
by the pack tool.

## Provisioning

After the collection steps in DEVICE-CATALOGUE.md, run:

```
python tools/provision-device-dashboard.py CAPTURE CATALOG OUTPUT
```

Use a new private OUTPUT directory. Place its contents at `assets/device-pack`
next to the native executable, or under the selected SYNAPSE_LITE_ASSETS directory.
The pack includes index.json and catalog.json. Provisioning verifies source paths,
identity, byte size and SHA256 before decoding; differing captured catalogue bytes
are refused. PNG files are named by their digest so identical images share disk
storage. The pack is not committed to the public source repository.

## Runtime and recovery

The native editor reads a bounded index and catalogue, then loads images only for
visible device cards. It uses the OS PNG decoder with external GDI+ codecs disabled.
An LRU cache holds at most 12 entries and 8,388,608 decoded pixel bytes; cold images
are released before admitting another image. Failed loads are cached too. File
sizes, SHA256, PNG format/header dimensions and actual native dimensions must agree.
Only canonical digest filenames are admitted. Pack hashes provide local consistency,
not a verified vendor signature. Missing or damaged assets leave the rest of the
app usable and never trigger a network request during paint.

The current engine does not report physical editions. The dashboard uses stock's
edition-zero artwork fallback when no edition is known. An explicitly supplied
edition must match; it is not replaced with another edition. Shared receiver IDs
with multiple candidate products do not select the first photograph. Catalogue
metadata and artwork never grant hardware controls or bypass protocol validation.

The cache bound excludes the index, renderer overhead, frame buffers and all other
app components. It is not proof of the complete-build 100 MB memory requirement.
Very large visible device sets can cause cache churn; full dashboard layout/scroll
behavior, regional models and broad device control remain implementation TODOs.

## Verification

Two provisioner tests cover source alpha, catalogue/path admission and immutable
output. Native tests cover exact product/alias selection, shared-receiver refusal,
edition separation, PNG alpha/colors, corrupted files and repeated cache eviction.
All 653 provisioned variants loaded through GDI+ and passed native pixel sampling
with bounded residency. The isolated native dashboard test demanded only the exact
mouse image from the 653-entry index, with zero device calls from ten repaints and
unchanged simulated profiles. All 237 regular serial tray tests pass; 29 opt-in
tests remain excluded from the regular run. Relevant opt-in tests were run separately.

See `docs/validation/device-dashboard.json`. The Computer Use screenshot request
timed out awaiting app approval, so desktop screenshot comparison remains a TODO.
No pixel-perfect visual claim or catalogue-wide hardware support claim is made.
