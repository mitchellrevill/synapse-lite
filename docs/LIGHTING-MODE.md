# Ornata device mode readback

The installed Ornata V2 (025D/interface2) answers GET00:84 with one declared argument. A scoped transport exception accepts that one-byte response only for this getter; other transactions retain their previous minimums and all framing, transaction, command, status and checksum checks.

The initial two-byte minimum rejected thirty identical valid success headers. After correction, a single response decoded mode3 (Driver Mode). The absent parameter is null/unknown: padding is retained for diagnostics but does not become a returned parameter. The getter also supports declared two-byte replies.

This was a mode-only query after closing stock user processes. No brightness, effect or mode SET was sent. Stock RazerAppEngine and razerwdl were restarted in finally; the later verification observed fourteen AppEngine processes and one razerwdl. Services and drivers were unchanged. See [the scoped result](validation/lighting-mode-readback.json).

Closing Synapse user processes did not establish Normal Mode. Firmware-effect application must refuse this state until a separately implemented ownership/recovery path exists; a saved firmware effect header cannot reconstruct the running host animation or prove exclusive ownership.
