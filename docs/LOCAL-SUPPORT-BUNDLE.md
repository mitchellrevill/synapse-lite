# Local support bundle

Tray menu: **Save local support bundle…**. Choose whether to include recent error categories, review the exact JSON, then select a new destination. Cancel at either prompt or the file picker writes nothing. Existing files are refused atomically with create_new, including destination races; failed writes attempt to remove only the newly created report. Nothing is uploaded.

Reports contain version/platform, explicit capability coverage limitations, uptime, up to 32 recent profile-apply durations, and optionally up to 16 session log categories. Only fixed categories survive; raw messages are never retained by this cache. Profiles, device identifiers/serials, user paths, raw audio and screenshots are excluded. The app's pre-existing ordinary disk log is unchanged and is never read into this report. No hardware requests are added.

Coverage is deliberately coarse. Catalogue support does not establish physical verification. Error categories detect logged error/failure/rejection words and are not a complete runtime trace; collection begins with this app process. Memory measurement is explicitly unavailable. Cancellation is supported before the short synchronous exclusive write; no background collection task needs draining.

Validation: focused core tests cover bounded categories, secret omission, optional category removal and exclusive output. Tray cargo check verifies Win32 integration. Stock comparison and native screenshot/interaction acceptance remain TODO: this is a new utility in the existing tray, with Windows prompts rather than a claimed stock screen reproduction. No approved product tabs were modified and no stock app or hardware was exercised.

Implementation checkpoint: one feature commit covers the bounded process cache, log/profile-apply observation hooks, explicit native preview/redaction/cancellation and exclusive writer. Baseline: 7b99930f9c63e659f1bcf3a51d9d9ca6a456904e. Isolated local clone; canonical working files were not copied, staged or edited. No REA runtime investigation was needed for this new utility; stock parity is not inferred from source inspection.

Validation result: 3 focused support_bundle core tests passed (579 filtered); Windows tray cargo check passed with existing unused/dead-code warnings. No native screenshot or hardware acceptance claim.
