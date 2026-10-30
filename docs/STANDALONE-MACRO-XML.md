# Standalone macro XML files

Macro **Import** accepts standalone `.xml` files and definitions from Synapse4
profile exports. **Export** writes the selected saved macro to a new v4 XML file.
Save or Discard dirty drafts first. Picker cancellation changes nothing.
Existing destinations, read-only mode and stale source revisions refuse before
writing. Export never replaces profiles or another file. An I/O failure can
leave the new output incomplete and reports that failure. Device namespaces and
alternate streams are not export files.

Import changes the global macro library and retained source envelopes referencing
that GUID, without importing device profiles or changing active selection. GUID
conflicts, dependencies, hashes, revisions and whole-store limits retain their
existing guards. Files execute no input, commands, programs, URLs or movement.
This feature enables no live macro playback.

## Format and preservation

The source v4 `Macro` envelope contains Name, MacroEvents/MacroEvent,
DelaySetting, Guid, Version and MouseMoveType. Movement settings are none0,
absolute1, foreground2 and relative3. Positive settings replace the actionBar
recordProfile with mmtSetting as the source exporter does. Native caches,
binding playback mode and unrelated editor root state are not transported by
this format; full profile files cover broader data.

Supported rows: actionBar, fixed delay0, keyboard1, mouse2, nonempty sampled
motion3, text4, command5, loop6, nested macro7 and launch10. Import follows the
recovered constructor's typed conversion, three-decimal delay formatting and
editor identity ordering within the admitted domain. No HID namespace or native
prefix is inferred. Nested target definitions are not embedded; missing targets
are reported by the existing library. File support does not establish runtime
support for these events.

Original XML bytes are retained in versioned record provenance outside the body
hash. Unchanged definitions export those exact bytes. Known edits preserve the
original provenance and regenerate the v4 projection. New exports validate
against the admitted reader before opening a destination. This is not lossless
transport for arbitrary profile or root metadata.

The stock XML postprocessor decodes already XML-decoded entity-looking text a
second time. Controlled replay proves literal `&lt;`, `&amp;`, `&quot;` and
`&#39;` can change in stock. Lite retains the literal text. Stock re-import of
such strings remains a compatibility limit; reproducing that source bug would
corrupt local data.

A sole known appEngine.mouseMoveType hint contains no native event cache. That
exact shape now permits local event edits with the raw hint retained. Any other
AppEngine field keeps the cache-edit guard. Definition planning accepts the
none-only hint; non-none planning and motion execution remain unverified.
Cached events/hash checks are unchanged.

## Bounds

Uses the streaming event interface of
[quick-xml0.42.0](https://docs.rs/quick-xml/0.42.0/quick_xml/), default features
disabled. Input/output512KiB, depth32,16,384 element nodes,512 converted rows.
Accepts UTF-8, optional BOM and XML1.0/UTF-8 declaration. Attributes, namespaces,
DTDs, external entities, processing instructions, unknown fields/types, phases,
inline legacy delays and invalid/out-of-range numbers refuse the whole import.
Existing2MiB store and1MiB combined encoded-payload limits can refuse earlier.
Legacy standalone XML, other encodings and structured/random delay import are
explicit TODOs.

## Evidence and checks

Source main.3f4b9604.js SHA256
`fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d`:
import class UTF16 range[610856,615088), export handler[639430,640038), movement
map module62141/G, postprocessor[637176,637436). All four helper texts also match
cached main.55c19a92.js SHA256
`054dc209dded144571f7e2fde8c3fd5c6fd7d65c1aec7bcd675a12fe9e4d19eb`.
This does not prove which bundle is active or whole-app/current native identity.

REA summary ev_19ad5747bd3985c9a4ef09ed93900786dde36f243299a6ab0b4b8a61d9d0eb18;
parent ev_8f9b8b6db39cee0a2eca9468881eb47d17001b72acb4f121f56f40a139504b77.
Four literal extracts,5,307 bytes,1,781 AST nodes, zero parse failures, partial
semantic coverage. Vendor bootstrap and host actions were never executed.

Thirty-one controlled source cases cover the typed export envelope, v4 importer
and postprocessor, all supported row types, prefix states, ten mouse actions,
movement modes, Unicode and binary64 rounding near ties. Independent XML
serialization/ElementTree parsing connects the source helpers; this is not a
capture of the vendor serializer or live stock import. Six codec tests cover
the vectors, verbatim bytes, persistence/edits, conflicts, malformed data, bounds
and cache guards. Two file tests cover exclusive creation, path refusal and
Unicode names. The simulated native workflow checks Export's actual hit/owned
dialog cancellation, export/re-import and existing/dirty/read-only/stale refusal
with zero device commands. Exact results/hashes are in
[validation](validation/standalone-macro-xml.json).

The native capture was compared with stock-macro-editor-0.jpg. The existing
macro layout is interim: stock places file actions in its ellipsis menu, while
this checkpoint exposes Export on the toolbar. Exact menus, sidebar, rows,
child-edit capture and full pixel parity remain TODOs. Approved tabs are
unchanged. No stock import, physical input, normal profile migration, driver
load or complete-build memory acceptance was performed or claimed.
