# Catalogue and Studio asset residency

Scope: isolated branch `feat/lazy-asset-cache`, based on `7b99930`.

Existing source already loads dashboard PNGs only when drawn (8 MiB decoded pixel
budget, 12 entries), and loads only Studio layouts referenced by the selected
composition (32 placements, 4 MiB source, 8 MiB geometry capacity). This change
extends those paths rather than acquiring or embedding catalogue assets.

Changes:
- Dashboard entries with the same declared SHA-256 share one native allocation.
- Least recently used eviction still releases cold images before admission.
- File length/modified-time changes invalidate successes and failures. Missing
  files can be repaired without reopening the window. Index/catalogue changes
  release all cached images before revalidating declarations and hashes.
- Native images detach from the source file after decode, allowing replacement.
- Existing smoke diagnostics expose hits, misses, evictions, invalidations,
  failures, cumulative decode microseconds and peak retained pixel bytes.
- Studio diagnostics count distinct Arc allocations and their capacity, rather
  than counting the same geometry for every placement. Canvas replacement drops
  unreferenced geometry; there is no catalogue-wide persistent geometry cache.

Quality: source resolution and alpha remain intact; no thumbnail reduction,
matte, regeneration, or DPI-specific duplicate is introduced. Existing GDI+
bicubic rendering is unchanged. Pixel-alpha and fractional-DPI renderer tests
provide automated evidence; no screenshot/pixel-perfect comparison is claimed.

Limitations: metadata invalidation does not detect a deliberate same-length edit
with an identical restored modification timestamp. Changed content is still
SHA-256 checked on admission. Pixel budgets measure retained pixels, not GDI+
allocator overhead or transient decoding/cloning. The detach operation briefly
holds source and destination pixels; a maximum-size dashboard image is 1.5 MiB.
Total application working set/private memory below 100,000,000 bytes still needs
three complete-build runs. No physical device tests were performed.

Validation:
- Isolated Windows Rust test build, jobs=1, target-lazy-assets; first run exposed
  the file-lock bug, and the independent pixel copy fixed it.
- Dashboard suite: 4 passed, 1 ignored (private pack required).
- Final explicit-pixel copy dashboard rerun: 4 passed, 1 ignored.
- Studio canvas: 4 passed, 2 ignored (private source assets required).
- Image renderer: 2 passed, including fractional scaling and alpha.
- Existing dead-code warnings and incremental-cache hard-link fallback remain;
  compilation and all selected tests completed successfully.
- git diff --check: clean.
- Draft PR unavailable: gh auth status reports invalid saved GitHub credentials.

