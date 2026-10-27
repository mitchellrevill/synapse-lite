# Ornata hardware identity GET

`Device::ornata_edition_readback()` reads the retained shipped Ornata V2
middleware's physical edition/layout tuple. It accepts only PID025D,
interface2, and never changes device mode, profiles, lighting, or assignments.
It does not read cached Synapse settings or infer a layout from Windows locale.

## Source provenance

The retained product605 middleware release is
`aef3665b36baca6993b26cdc296cda8bdab28bbd`. The source chunk
`rzDevice25.b98dfa3c9c4ec16dd781.js` has270524 bytes and SHA-256
`0a463302ecaf113705ebb648f71f377d90d37d7194d6d73578644af080c12a7f`.
Its zero-based UTF-8 byte offsets establish:

| Offset | Observation |
|---|---|
| 3119 | Decoder: data0 keyboardLayout, data1 edition, data2 firmwareId; helper calls sendCommand |
| 17438 | Export Se resolves the edition command descriptor |
| 19826 | Descriptor `[3,0,134]` and label Get Edition Information |
| 119880 |90-byte protocol body constructor and XOR checksum |
| 121555 | Normal response correlation/status decoder |
| 124318 | GET send/read transport path |
| 130913 | getEdition wrapper and bounded source retry behavior |

REA parsed the unchanged chunk as inert JavaScript, without executing its
bootstrap or target application. Parent Evidence:
`ev_5be7f3ba9bd58ae2b835c1a4a739dfb209cea9dbcfd175629a86868a4915ac31`;
summary Evidence:
`ev_129cda03ed0654435fc4e5046be06b2e1f6de3da3a1282b7f9fcf37e95dd0dcb`.
One file parsed with zero parse failures; the semantic graph was partial at
the100000-node ceiling. These conclusions follow the explicit local source
definitions and calls, not a claim of complete runtime data-flow recovery.

The Ornata claimInterface2 declaration occurs at UTF-8 offset130148 in
`main.1cb47376ca3911a35f14.js`, SHA-256
`9cdec67b89a1f8ef54a386fc2ce2a0e2f93f50830e9da385a7be91074ba90b18`.
The generic02:A4 getLayoutSwitch API means Mac/Windows operating mode and is
distinct from this physical-layout query. It is not used here.

## Exact contract

The GET is class00/ID86 with declared payload size3 and three zero input bytes.
There are no profile, region, edition, or layout input selectors. A Windows HID
feature packet is report ID0 followed by the90-byte protocol body:

| Body byte(s) | Request value |
|---|---|
| 0 | status0 |
| 1 | selected transaction ID |
| 2..4 | zero |
| 5 | declared size3 |
| 6..7 | class00 / ID86 |
| 8..87 | zero payload and padding |
| 88 | XOR of body bytes2..87;85hex for this request |
| 89 | zero |

The three successful response bytes are independent unsigned bytes:
`[layout_id, edition_id, firmware_id]`. No multibyte byte-order conversion is
involved. firmware_id is a raw ID, not a decoded firmware version.

The getter uses ordinary core GET transport validation: complete90/91-byte
carrier, checksum, transaction/class/command correlation, protocol framing,
success status, and at least three declared response bytes within the80-byte
capacity. Undeclared padding cannot supply missing identity bytes. Additional
declared bytes remain available as uninterpreted data. Query-only Device handles
permit this GET; there is no SET path or new outer retry transaction.

`OrnataEditionReadback` exposes raw IDs and declared data. A successful unknown
tuple remains a successful raw observation; unsupported commands, invalid data,
and no reply remain explicit errors. No US/UK/default/dock fallback is provided.

`layout_identity()` creates a raw `ornata_spatial::LayoutIdentity` tag with
vendor1532/product025D and the observed edition/layout. It does not certify
manifest membership or select geometry. A spatial caller must separately require
the exact source record and dimensions accepted by OrnataSpatialMap.

Cached application identity is weaker evidence: the shipped runtime can retain
its default layout after failed or unrecognized GET results. Historical logs of
edition0/layout6 cannot establish what the current device reports.

## Verification and limits

Five scoped simulator/spy tests cover raw baseline/unknown tuples and extensions;
short declared replies despite matching padding; malformed headers/checksum;
the exact91-byte zero-selector request on a query-only handle with90/91-byte
response carriers; wrong PID/interface with zero I/O; and unsupported versus
bounded invalid-response behavior with one GET and no fallback or resend.
The existing ordinary transport bounds invalid-response reads at30.

This checkpoint performed no physical hardware query, DLL execution, process
changes, or production runtime integration. It does not certify the installed
firmware's returned declared size, current layout, or optical/key geometry.
A future explicitly scoped GET-only probe should retain the raw correlated
response and declared size, and report manifest resolution separately.

## Opt-in diagnostic example

The separate `ornata_identity_readback` example accepts exactly
`<report.json> --read-only` and refuses any presence of `SECBIRD_SIM`. Building
or testing it does not execute hardware discovery; running the compiled example
with the explicit flag does. It selects exactly one025D/interface2 candidate and
uses the configured/bounded transaction-ID candidates for the serial GET opening
baseline, then performs one edition GET with no outer retry transaction.

A strict transport allowlist is installed **before** Device construction. It
accepts only canonical zero-payload serial00:82/size22 baseline packets and one
edition00:86/size3 packet. SETs, other GETs, modified arguments/headers, excess
baseline attempts, and a second edition request are denied before reaching the
underlying transport. The opened Device is additionally restricted to query-only.

The requested JSON file contains raw identity IDs, protocol/source provenance,
GET/OUT/read/denial counts, bounded request/response header traces, and the raw
correlated successful edition response. Serial argument bytes and device paths
are omitted. Up to64 response headers are retained, with omitted-read counts;
the existing transport still controls its bounded read attempts. A valid unknown
tuple remains raw data and does not select a spatial map. A baseline or edition
failure is reported without cache/default/dock fallback. Private HID error text
is omitted. Discovery/open failure stops before any identity query.

Four example tests use synthetic transports only: explicit argument/simulator
guards, canonical-command allowlisting and single-query enforcement, denied
commands reaching no inner transport with bounded private traces, and complete
baseline/query reporting with unknown IDs or unsupported status. No physical
execution of this example is included in its implementation checkpoint. It does
not modify the older lighting_readback example or its recorded binary evidence.
