# Playback capture and analysis checkpoint

The app now has an opt-in Rust WASAPI render-loopback backend, an owned capture
handle, and bounded PCM/RMS/peak/FFT analysis. It is not yet connected to the Audio
Meter screen or Studio renderer. Its analysis is explicitly defined here; it is
not claimed to reproduce the proprietary Razer FFT scaling or effect mapping.

Normal startup and endpoint discovery do not start capture. The explicit local
diagnostic captures1..30 seconds from the default Multimedia playback endpoint:

```
secbird-tray.exe --audio-capture-default SECONDS NEW_REPORT.json
```

It returns before HID Engine initialization, never overwrites a report, and saves
only format/counters/cleanup metadata. Raw PCM and spectra are not written to the
report. The internal handle also supports an exact endpoint ID and cancellation;
no capture-start HTTP route has been added. There are no microphone, volume/mute,
default-device, driver or lighting-setting writes. Stream Start/Stop apply only
to this capture client's own shared-mode loopback stream.

## Ownership and recovery

Endpoint identity is resolved once, checked through IMMEndpoint GetDataFlow, and
must be an active render endpoint before IAudioClient initialization. Exact IDs
reject empty/oversize/NUL values and never fall back to another endpoint. A default
selection is checked every500ms; a default change faults/stops this capture rather
than silently switching sources. Real unplug/default-change acceptance remains open.

One capture worker owns the COM apartment, client, buffer service and mix-format
allocation. Each nonempty acquired packet is validated and copied into a bounded
reusable local buffer, then released before FFT work or frame publication. Silent
packets do not dereference their data pointer. Discontinuities discard partial
analysis windows; timestamp errors are counted without using their timestamps.
Unknown packet flags, invalid sizes/encodings or nonfinite/excessive samples stop
the pipeline with an explicit error. The started stream receives one Stop attempt
on normal completion or failure; cleanup errors are preserved alongside processing
errors, and a failed packet release is not retried on Drop.

Stop/Drop immediately invalidate published frames and request cooperative cleanup.
An outstanding native call is not interrupted. Single-worker admission remains
held until the native function has returned and its owners have drained; a new
capture is refused meanwhile. An old handle cannot cancel a newer handle. Finished
results are consumed once, publication is latest-only, and frames older than250ms
are unavailable. The diagnostic wait has a deadline and reports unconfirmed OS
cleanup on timeout rather than claiming Stop succeeded. No automatic reconnect or
startup replay is implemented.

## Defined analysis and resource bounds

WAVEFORMATEX/EXTENSIBLE parsing accepts PCM8/16/24/32 and IEEE float32/64,1..32
channels and8000..384000Hz, with consistent byte rate/alignment and known subtype
GUIDs. PCM valid bits use the specified left-aligned container representation.
Float headroom up to absolute16 is accepted; unsupported/bad formats refuse.
Native and copied packet sizes are each bounded to16MiB. Analysis storage is
bounded independently of session duration: a2048-frame channel ring, Hann window,
one FFT scratch buffer and one1025-bin output. The largest admitted analyzer
retains under600KB; the observed8-channel96kHz analyzer retains180224 bytes.
These are component bounds, not a complete-process100MB result.

Generic analysis uses symmetric Hann windows, a2048-point radix2 FFT, coherent-gain
single-sided magnitude normalization, and50 percent overlap. Channel spectral
powers are averaged before taking the square root, so opposite-phase channels do
not cancel. RMS and peak use the unwindowed samples across channels. Meter mode
spaces analysis windows by max(1024, sample_rate/25), capping expensive FFT work
at25fps without changing the endpoint sample rate. The initial transport polls
every5ms without requesting a system timer-resolution change; event-driven/MMCSS
pacing and measured CPU/latency are follow-up work.

## Verification and remaining work

Independent NumPy2.3.2 rfft/hanning references check8200 bin magnitudes plus RMS
and peak across overlapping DC, opposite-phase stereo,8-channel tones and seeded
noise windows. Irregular packet splits produce the same windows. Tests cover
integer/float/extensible decoding, malformed geometry, silent pointers, partial
window reset, high-sample-rate cadence, native ABI slots, failed release handling,
late publication, cancellation/drain admission, old-handle isolation and one-shot
completion. SDK ABI fixture now includes audioclient.h and IMMEndpoint.

Live Windows checks passed default/exact setup, Stop and reopen. A two-second
standalone run reported float32,8 channels,96000Hz and a9600-frame native buffer,
with Stop succeeding. The selected playback endpoint was idle: **zero packets and
zero live analysis frames** were observed. Non-silent packet/FFT hardware evidence
therefore remains TODO; synthetic reference checks do not substitute for it. No
sound was played to force activity and no user audio was saved.

Next checks are non-silent playback when available, source Razer FFT/bin/scaling
and colour mapping, UI selection and persistence, explicit Audio Meter preview/
saved-output integration, device/default changes and protected/exclusive routes,
and measured complete-build CPU/memory/network/visual acceptance. Keep the full
implementation goal active and continue independent work when a check stalls.

API contract references: [WASAPI loopback](https://learn.microsoft.com/en-gb/windows/win32/coreaudio/loopback-recording),
[capture packet ownership](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getbuffer),
and [capture service](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nn-audioclient-iaudiocaptureclient).
