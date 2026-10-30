# Recovered native paste protocol

Saved text macros now include a clipboard-paste contract in the existing
read-only `secbird source-macro plan DOCUMENT BINDING NEW_OUTPUT` report.
The original native event JSON and source-row mapping remain unchanged. The
report adds the key sequence and remaining runtime requirements once per plan;
it does not duplicate the text or invent a current keyboard-state snapshot.

`secbird_core::native_paste_plan` reconstructs a bounded protocol from an
explicit source modifier snapshot. It retains separate suspend/chord/restore
roles and route policies. It creates no thread, clipboard object, input packet,
output lease or Windows call. Live text playback remains unavailable.
These steps belong inside one clipboard action. They must not become ordinary
keyboard macro events with the scheduler's separate keyboard-adjacency waits.

## Source proof and correction

The target is AppEngine4.0.896 `mapping_engine.dll`, SHA256
c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9.
The earlier complete REA dossiers provide the paste/modifier constructors and
callback behavior. After two full Ghidra queries timed out, bounded disassembly
used the already-installed Microsoft dumpbin14.12.25835.0. The DLL was never
loaded as executable code. Exact code intervals come from PE RUNTIME_FUNCTION entries, with
explicit bounded windows for leaf functions. These are code/unwind ranges, not
a claim of complete cross-function control flow. Indexes retain tool, target,
code and output hashes and the extraction script.

At0x1801cbc00, the incoming callback pointer is moved into its destination and
the previous reference is released. The incoming callback is not invoked there.
At0x1801cbb40, bound-state initialization stores the invoker at+8 and destroy
function at+0x10. Invokers0x1800ba050 and0x18011c810 pass the saved continuation
to the captured operation. The paste builder0x1800b94c0 captures its previous
callback before replacing it. Thus the wrapper chain executes in reverse of
construction order.

**Correction to the earlier private clipboard report:** its table mixed
construction and execution values. Direct stores at0x1800b9573/9581,
0x1800b963c/964a,0x1800b970b/9713 and0x1800b97d4/97dc establish:

| Construction order | Scan | Flags | Captured delay argument |
|---|---:|---:|---:|
| Ctrl up | 0x1D | 1 | 10 |
| V up | 0x2F | 1 | 0 |
| V down | 0x2F | 0 | 20 |
| Ctrl down | 0x1D | 0 | 30 |

The resulting execution order is Ctrl down30, V down20, V up0, Ctrl up10.
Completion helper0x1800b9b20 scales the captured argument by1000 only on callback
true; callback false advances with0. Duration-unit/wall-clock behavior remains
unverified. Advancing a continuation is not a terminal delivery receipt.

## Snapshot and route differences

The sampled virtual-key order is left Alt,Ctrl,Shift,Windows, then right
Alt,Ctrl,Shift,Windows. Snapshot bits are0x100/200/400/800 and0x10/20/40/80.
Only the high bit of each supplied SHORT contributes; toggle bits are ignored.
The compiler never calls GetKeyState. That API reports state associated with a
thread's keyboard-message queue, rather than an independently sampled hardware
state, so its results cannot grant physical-key ownership.
[Microsoft GetKeyState documentation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getkeystate).

Modifier callbacks are constructed left/right Alt,Ctrl,Shift,Windows, so each
suspend/restore group executes right Windows,left Windows,right Shift,left
Shift,right Ctrl,left Ctrl,right Alt,left Alt, omitting unset bits. Up/down
flags and E0 prefixes remain explicit. The protocol contains4–20 steps.

Modifier sender0x18011c140 first tries the endpoint; its unavailable-endpoint
path calls0x18011c9e0, the recovered OS SendInput helper. Paste-key sender
0x1800b9880 has no corresponding OS fallback and can advance without dispatch.
The route distinction is retained in each Rust step. Snapshot releases may
target physically held keys and must not become ordinary owned macro releases.
UnitId, device identity, output ownership and terminal delivery are not inferred.

## Clipboard publication findings and remaining work

The concrete ClipboardWin constructor0x18022ccaa stores vtable0x18033de10.
Its+0x100 method is0x18022ee10. The scoped writer destructor0x1801dcb2e calls
that method at0x1801dce85. That path acquires the clipboard lock and calls the
verified EmptyClipboard import at0x18022ee58 before dispatching representations.
This identifies a late publication path, not proof of every earlier write or
cross-thread ordering. The earlier service invokes its continuation before
destroying the scoped writer; a Rust translation still needs explicit commit
before paste and failure/Stop/restore handling.

The sequence-number wrapper0x18022cd40 compares the OS counter with its cached
counter and refreshes a token on change. Its presence does not establish that
the service protects content copied by another application during delayed
restoration. All supported formats, unknown formats, large data, ownership,
concurrent-copy policy and recovery still require implementation/verification.

Four core checks cover source-field fixtures, all256 snapshot masks, bit/order
handling, bounded roles and callback delay branches. The CLI integration check
uses the normal pre-discovery command path, retains its document, reports a null
snapshot and never reads/changes the clipboard or sends input. Fixtures are
synthetic data reconstructed from source fields; vendor code is retained only
in private analysis outputs. See validation/native-paste-protocol.json.

There are no UI/rendering changes in this checkpoint. Full input routing,
physical arbitration, delivery/cancellation, clipboard recovery, catalogue/API/
hardware/privacy/visual coverage and all three complete-build working-set and
private-memory runs below100000000bytes remain open.
