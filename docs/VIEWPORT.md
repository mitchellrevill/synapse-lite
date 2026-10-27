# Source viewport and error feedback

The recovered normal product root contains Toolbar, Navigation and BodyWrapper, with no bottom status child. Audited Ornata Customize/Lighting and DeathAdder Customize/Performance/Power pages now extend below native y128 to the client bottom. Dashboard, Help and unaudited generic views retain their existing footer pending source reconstruction.

Normal card-page scroll extents include the source widget10px bottom margin and BodyWrapper20px bottom padding: max(0,lastCardBottom+30-viewportBottom). Performance paint/hit/hover/EDIT clipping and Lighting scroll/EDIT bounds use the corrected viewport. Customize has separate nested/drawer rules; the card formula is not applied indiscriminately.

Failures remain visible through acknowledged native dialogs, an explicit port adaptation. No generic stock backend-error toast contract was established. The posted dialog callback releases editor state before opening a modal loop. A50ms retry timer retains errors encountered during another borrowed modal without a repost loop. Active/pending/acknowledged duplicates are suppressed until a new explicit operation; distinct failures survive successful replies and accepted queued work drains on close. Compatibility notices remain in Help.

Pure tests use recovered wide/narrow scroll extents. Both simulated native-window tests pass, including actual mouse/EDIT notifications, resize, capture, canceled gestures and acknowledgement of the error dialog. They do not prove stock pixel parity or physical input behavior. The [Lighting result](validation/native-lighting-interaction.json) records source_viewport=true and reserved_footer_css=0.

Source provenance: focused REA ev_11c22e6369bf3e1a9eafd079ce0cf6013e6f8d2f01f36a6bb4d96d05b79ce622 parsed five inert source projections without failures;20 controlled DOM cases establish viewport/card geometry. The installed AppEngine host bounds helper uses y42; product toolbar38 and navigation48 establish native content y128. No screenshots or vendor JavaScript execution formed the layout baseline.
