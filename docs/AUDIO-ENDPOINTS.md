# Windows playback-device discovery

The app now has its own Rust Core Audio endpoint discovery, available through the
authenticated GET `/api/audio/endpoints` endpoint and the explicit diagnostic:

```
secbird-tray.exe --audio-endpoints NEW_REPORT.json
```

It lists active playback endpoints with their opaque Windows IDs, friendly names,
state bits, default Console/Multimedia/Communications roles, volume scalar and mute
readback. Names and IDs stay local. Diagnostic files are created new and never
overwrite an existing report. This mode returns before Engine creation/HID scans;
ordinary startup does not trigger discovery. The HTTP route checks Host, Origin
and session token before calling the provider and does not hold the HID engine
mutex. POST and request bodies are refused. Provider errors return503.

This supplies a backend prerequisite for Audio Meter. It is not a working Audio
Meter, audio-device selector or vendor-wrapper ABI emulation. A subsequent
[capture checkpoint](AUDIO-CAPTURE.md) adds opt-in loopback and defined analysis;
source FFT/effect parity and UI integration remain unfinished.
The scalar is the OS volume setting, not measured sound intensity or FFT input.
There are no audio streams, microphone reads, callbacks, volume/mute setters,
default-device changes or vendor SDK calls in this discovery module. Source
effect math, UI selection/persistence, removal/default-change lifecycle and final
lighting integration remain TODOs. Endpoint listing also does not implement
headset EQ, THX, mixers or proprietary microphone controls. The retained App Engine API migration and
complete-build visual/profile/input/device/memory gates remain open.

## Lifetime and bounds

Each explicit request uses a512KiB-stack worker with its own MTA apartment. COM
references, task-allocated strings and property variants have scoped owners. The
apartment outlives its references; no COM pointer crosses threads. The result
contains ordinary owned Rust data only. S_FALSE initialization is balanced with
one CoUninitialize. UTF16 strings are limited to4095 units and malformed text is
reported; at most64 endpoints are admitted with no silent truncation. Critical
identity/count errors fail discovery. Optional name/volume/mute failures appear
as explicit per-endpoint issues rather than guessed values.

Caller wait is capped at2500ms. An outstanding OS call retains the single-worker
admission after timeout; retries report busy until it actually returns. No native
call interruption is claimed, no replacement workers accumulate, and late results
are dropped. Default roles match exact IDs; a missing default never falls back to
the first device. Enumeration is a point-in-time read, not an atomic OS snapshot
or future capture authorization. A disappeared/unlisted default produces a warning;
capture must revalidate its chosen endpoint when implemented.

## Evidence

Bindings follow the installed Windows SDK10.0.26100.0 declarations. The fixture
`tests/fixtures/audio-endpoint-sdk-abi.json` independently extracts vtable method
order and retains hashes of mmdeviceapi.h, propsys.h and endpointvolume.h. It
checks every used slot against the Rust ABI structures. Other tests cover counted
reference release, allocated UTF16 failures, invalid volume values, exact default
identity, duplicates/count limits, authenticated read-only routing without an
engine lock, and timed-out single-worker admission through actual completion.

Live validation completed20 successive Windows reads on owned apartments. A
separate standalone diagnostic found9 active playback endpoints: all9 had names,
volume and mute readbacks, all3 default roles resolved, and no issues/warnings
were reported. The full local report retains IDs/names privately; public validation
contains counts only. These reads do not verify capture, FFT, hardware Audio Meter
output, device unplugging or a complete-process RAM threshold.

Primary API references:

- [IMMDeviceEnumerator](https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nn-mmdeviceapi-immdeviceenumerator)
- [Default endpoint by role](https://learn.microsoft.com/en-us/windows/desktop/api/Mmdeviceapi/nf-mmdeviceapi-immdeviceenumerator-getdefaultaudioendpoint)
- [Endpoint volume readback](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudioendpointvolume)
