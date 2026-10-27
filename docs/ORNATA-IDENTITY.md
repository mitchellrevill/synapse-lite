# Ornata hardware identity GET

A later [physical checkpoint](validation/ornata-identity-physical.json) successfully read the current keyboard's declared `[6,0]`: layout 6, edition 0, firmware ID unavailable. One serial baseline GET and one edition GET produced two accepted OUT returns and two valid reads, with zero SETs or denied requests. Stock user processes were temporarily isolated and restored (11 before, two stable zero observations, 15 after the configured hidden restart); services and drivers were unchanged. This is a point-in-time identity observation, not an ownership lease or optical/Wave runtime acceptance.

The general simulator can explicitly supply `ornata_layout_edition: Some([layout, edition])` for runtime tests. Its default is unsupported and it infers no country. This mock declares exactly two bytes while the request remains three, so firmware ID stays unavailable. The regression covers known and unknown tuples through a query-only handle and verifies mode/brightness remain unchanged. These simulated values never certify a physical device.

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

The shipped source decodes three independent unsigned response bytes:
`[layout_id, edition_id, firmware_id]`. The installed firmware's observed reply
declares only two bytes. Layout and edition are required; `firmware_id()` returns
`None` with two declared bytes and `Some(raw_id)` with at least three. It is a
raw ID, not a firmware version. No multibyte byte-order conversion is involved.

The getter retains ordinary core transport checks: complete90/91-byte
carrier, checksum, transaction/class/command correlation, protocol framing,
success status, and declared data within the80-byte capacity. A narrow exception
for this025D/interface2 GET allows a two-byte minimum response to the unchanged
three-byte request. Ordinary `Device::get` still requires its request-sized
minimum. Undeclared padding cannot supply firmware ID or other identity bytes. Additional
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

Seven scoped simulator/spy tests cover raw baseline/unknown tuples and extensions;
short declared replies despite matching padding; malformed headers/checksum;
the exact91-byte zero-selector request on a query-only handle with90/91-byte
response carriers; wrong PID/interface with zero I/O; and unsupported versus
bounded invalid-response behavior with one GET and no fallback or resend;
two-byte firmware absence/three-byte presence; and unchanged ordinary GET
strictness on the same simulated device.
The existing ordinary transport bounds invalid-response reads at30.

The implementation tests performed no physical hardware query, DLL execution,
process changes, or production runtime integration. A separate root-operated
GET-only probe using the previous strict-three-byte binary returned one serial
baseline success and30 correlated successful edition headers declaring two
bytes after one00:86 OUT, with no SET. Their transaction1F, command00:86, and
checksum were valid; strict minimum3 correctly rejected the replies. The retained
report SHA-256 is
`4f2c86f4b2e0a30da26c4b9b876891007875e702fbf352bbee5500b9b1797567`.
It captured headers, not edition arguments: it does not establish layout0/6 or
any firmware ID. This observation grounds the narrow minimum2 response handling.
No new physical execution of the revised getter is included here. The next
explicit GET-only probe must retain the raw correlated response and declared
data, and report manifest resolution separately. Optical/key geometry remains
unverified.

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

The requested JSON file contains raw layout/edition IDs and optional firmware
ID (`null` when absent), protocol/source provenance,
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
