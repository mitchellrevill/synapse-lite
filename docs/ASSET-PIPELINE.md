# Reproducible private device asset catalogue

The private artwork provisioner now retains exact original AVIF bytes beside
the existing bounded native PNGs. All 653 edition variants remain present across
298 primary products. There are 556 deduplicated PNGs and 556 deduplicated original
AVIFs. Every regenerated PNG digest matches the preceding pack. The original
AVIFs total 14,720,921 bytes and retain their source resolution and alpha; native
thumbnails remain at most 768 by 512 pixels. No original is loaded by the native
renderer. Its lazy cache, admission limits and device identity resolution are
unchanged. The new private index is 469,784 bytes, below native's 1 MiB admission
limit. Index size can change with later provenance metadata.

`tools/provision-device-dashboard.py` now rejects conflicting category-default
definitions for one product/edition, non-static/non-AVIF inputs, path escapes and
oversized captures. Original files use `source-SHA256.avif`; index records retain
original digest, byte count, dimensions and exact captured edition/layout path.
The capture report digest is also included by new builds. Repeated identical
records deduplicate. No source or edition is guessed from a product photograph.

`tools/audit-device-assets.py` produces a deterministic per-product report from
an explicitly selected catalogue, product capture and two private packs. It:

- Verifies product capture sizes/digests and catalogue binding.
- Keeps MW/UI edition declarations, translation edition identities and regional
  layout lists separate, with their manifest hashes. It never treats their cross
  product as proof of real devices.
- Lists exact artwork and Studio identities and MW editions without either role.
  Missing MW metadata and unverified runtime DeviceInfo matching remain explicit.
- Refuses duplicate/ambiguous identities, mismatched filenames, unknown primary
  products, unsafe paths, corrupt bytes and PNG/AVIF dimension/alpha violations.
- Verifies retained originals when present and counts missing originals for older
  packs. A legacy thumbnail-only pack cannot acquire an original-retention claim.
- Optionally writes an exclusive, deterministic private ZIP containing only
  indexed files, under native's `device-pack/` and `studio-presets/` folders,
  plus the audit. Unindexed files and account databases never enter the archive.

ZIP entries use fixed timestamps, permissions, ordering and stored bytes,
independent of source mtimes and compression library versions. Files are hashed
again immediately before packaging; mutation aborts and removes the incomplete
archive. Existing archives and source packs are never overwritten. The auditor
limits individual reads, indexes and total unique payloads (256 MiB). Full
generated packs remain private because redistribution rights are unresolved.

## Run offline

Pillow with static AVIF decoding is required. Use fresh private destinations:

```text
python tools/provision-device-dashboard.py CAPTURE CATALOG NEW_ARTWORK
python tools/audit-device-assets.py CATALOG CAPTURE NEW_ARTWORK STUDIO_PACK NEW_REPORT --private-zip NEW_PRIVATE_ZIP
python -m unittest discover -s tools -p "test*.py"
```

The report and ZIP are portable private research outputs, not permission to
redistribute vendor artwork. Raw captured manifests and opaque source fields
remain intact in the capture directory; the report is a bounded projection of
their identity declarations, not a replacement for that source archive.

## Coverage boundary and next diagnostics

A fresh HTTPS GET of Razer's public AvailableDevices.json on 29 October 2026
returned the same 10,963-byte snapshot and SHA256
`f087e20349b803a11fa8299bfcb2e2c28c9e875c21aaa9242279655a22bc62e9`
as the 28 October capture. It declares 334 primary products. The asset audit
covers that exact set: 653 artwork identities/298 products and 1,396 Studio
identities/183 products. The source acquisition TODOs remain; no failed routes
were blindly retried and no resource budgets were raised.

The [official US keyboard catalogue](https://www.razer.com/shop/keyboards/gaming-keyboards)
was checked on 29 October. It lists Reclusa Mini 65%, Joro and Pro Type Ergo,
among other products and edition variants. Captured MW/UI name declarations also
contain those names. Name agreement alone cannot bind a retail SKU, region or
edition to a USB/DeviceInfo identity. The [official mouse catalogue](https://www.razer.com/store/gaming-mice)
likewise cannot establish exact asset or device API coverage. No all-market
completeness percentage is claimed from these storefronts.

Remaining work is explicit: recover each product's DeviceInfo edition/layout
resolver with REA; bind regional, sidepad, wrist and Customize artwork only from
source-declared roles; reconcile official retail SKUs to authoritative numeric
identities; extend CSS/path/affine-curve handling and chassis/legend rendering.
For a failed source route, the next diagnostic is a versioned official manifest
or exact shipped route, not an image borrowed from another chassis. For raster
differences, use the region-specific comparison workflow in STUDIO-TRANSFORMS.md.

Catalogue declarations, artwork availability, offline Studio editing, device API
implementation and physical verification are separate facts. This work makes no
new API, hardware, complete UI parity or three-run complete-build memory claim.
