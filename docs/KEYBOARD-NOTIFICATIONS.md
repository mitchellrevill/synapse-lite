# Selected Ornata notification transport

The Windows `keyboard_notifications` module explicitly opens a passive
notification subscription on the selected Ornata's existing signed filter.
It does not install selectors, enable input hooks, redirect input or submit keys.
The app does not start it automatically. This is not a read-only-runtime API:
the per-file notification subscription changes explicitly, then is disabled.

## Identity and lifetime

The configuration HID interface is025D/MI02. The present control endpoint is
025D/MI00. Both must have the same nonzero Windows ContainerId; a different unit
or ambiguous endpoint is refused. Both interfaces are revalidated before and
after opening the overlapped zero-access shared handle. Merely comparing the
interface numbers would reject this legitimate association.

The private operation enum expresses only ReceiveNotification `0x88883018`
(no input,304-byte output) and EnableInputNotify `0x88883038` (four-byte boolean).
There is no arbitrary control-code sender. Every request owns stable heap
OVERLAPPED storage, its event and buffers through actual terminal completion.
Cancellation targets that request; a cancellation request alone never frees or
reuses its storage. The dedicated worker drains completion before disabling its
own subscription and closing the handle. Even failed enable acknowledgement
leads to a disable attempt after draining.

The caller's stop waits at most300ms. A slow/broken driver can keep the worker
draining afterward, with its resources retained and new process-local admission
closed. Thus stop-requested, I/O-drained and worker-finished remain distinct.
The bounded128-event queue fails closed on a missing/slow consumer; this passive
observer does not suppress physical input. No raw key identities or text are
included in the example's aggregate report.

This lifetime design follows Windows' distinction between cancellation and
[actual completion](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelioex).
The native request/event layouts are separately checked by tests.

## Corrected callback evidence

The normal Ornata keyboard route is RzDev025D connect1EA8 role2, RzCommon
registration5104/5173, wrapper5680 and callback3BBC. It emits mask4/type1,
with the12-byte KEYBOARD_INPUT_DATA at notification offset20
(copy instructions3C76–3CA1). Another recovered helper25F4 emits type0xB
with the same layout. The decoder accepts precisely these two types under
mask4, preserving UnitId, native make code, raw flags, reserved and extra data.
Other notification masks/types remain uninterpreted; the exact304-byte length
is mandatory. The earlier ABI report's type0xB description did not cover the
normal role2 path and must not be used as its sole decoder.

EnableNotify36F0–3748 updates this file's mask4 subscription and aggregate54E.
The normal notification branch does not require EnableInputHook54D. Enabling
hooks merely to obtain passive events is therefore unjustified.
Global keyboard redirection54A is checked before ordinary notify: successful
mask1 delivery can bypass the mask4 branch. This is a source-established
condition, not proof that redirection is currently active. No safe dedicated
readback of that flag was established in the bounded investigation, and no
redirection state was changed.

Native source: installed RzCommon.sys SHA256
`64b72ef9ce409bb17d31096c119b68c4a6a02ab35b7535d763157a4e1a587e59`,
RzDev_025d.sys SHA256
`2e9d1f4adb0bee2cc5cf6ab58d3534e479f80579f23b03beed6cc4246e2700dc`.
The source/caller investigation and limitations remain in
[driver contract](DRIVER-CONTROL.md) and [Snap Tap](SNAP-TAP.md).

## Actual observations and remaining work

The opt-in `observe_ornata_notifications` example runs in a supervised child,
limits observation to1–10 seconds, caps its report, creates only a fresh output
file and records aggregate counts. An outer deadline does not prove cleanup if
the child has to be terminated; that is reported as failure.

Two physical endpoint sessions completed association, enable, pending receive,
targeted cancellation/drain, disable and worker exit. No selector, redirection,
input submission or HID setting command was issued. Both received zero total
notifications, including the five-second sample during which the user confirmed
pressing A/D. This proves the tested lifecycle, **not physical event delivery**.
See [physical sample](validation/ornata-notifications.json).

Read-only PnP inspection shows the MI01/Col01 keyboard has kbdhid plus
RzDev_025d as its upper filter; the separate MI00 keyboard has kbdhid without
Razer upper/lower filters. GET00:84 returned mode0,param0. Stock shared
middleware42057 requests driver3,param0 before registerDevice/inputevent setup,
and normal0,param0 on unload. Middleware SHA256
`9c4a687e1a4868cd9da4dec40024e5ce3403fe866907887b47995f493e23a3a8`.
These establish different paths and an initialization difference; they do not
prove mode3 redirects typing between those interfaces. No mode SET was attempted
in that initial checkpoint. The later separate [mode experiment](ORNATA-DRIVER-MODE.md)
verified0→3→0 and cleanup but still received zero notifications.

Further native analysis shows the MI00-named control child associates sibling
roles by ContainerID; its label is not evidence it targets the wrong MI01 role.
Common initialization also runs during hardware preparation. Its initialization
attempt latch is not callback-success readback; no guessed reset/initialize
operation is justified. The first common request can invoke the existing driver's
callback-registration machinery, so passive subscription is not a promise of
zero kernel initialization effects. The diagnostic sends no selector/interception
or input-generation operations.

Next establish the physical report path and source initialization dependency
through an owned, reversible mode/readback experiment or exact interface input
observation. Then connect narrowly owned selectors, assignments, completed output
tracking and held-key recovery. Interception, key generation, profile-switch,
unplug/sleep/crash recovery and live Snap Tap remain unfinished. This transport
does not satisfy complete input functionality, a replacement kernel driver or
the final resource gate.

Validation:342 core tests and the doc test pass. New tests cover the exact native
OVERLAPPED ABI, both keyboard notification layouts, short/unknown records and the
passive operation enum. Optimized probe, CLI and tray builds pass. These checks
do not turn the zero-event hardware observation into delivery acceptance.
