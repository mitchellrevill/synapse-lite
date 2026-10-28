# Rust kernel build checkpoint

An independent unsigned Rust KMDF forwarding candidate compiled and linked successfully. [Source and compile instructions](../experimental/kmdf-forwarding/candidate/README.md) are separate from the application workspace; ordinary app builds do not compile or load it.

The executed candidate marks itself as a filter, creates typed per-device context and a parallel default request queue, and forwards current requests to the lower target with SEND_AND_FORGET. A successful send transfers request ownership; failed sends complete with the WDF-reported status. Cleanup records closing state. It changes no keyboard/mouse CONNECT_DATA, implements no RZCONTROL routing or injection, and provides no lighting operation.

The original build used Rust1.91.0, LLVM20.1.8, existing MSVC14.44.35207 and official Microsoft WDK/SDK NuGet10.0.26100.6584 contents. KMDF1.33 is the build baseline, not a claim to reproduce the stock KMDF1.15 filters. The first successful two-job build took59.39seconds; a candidate-only rebuild took0.50seconds. Tools, SDK payloads and dependency caches stayed in the workspace. No global toolchain, registry, certificate, driver service or boot configuration changed.

[Static verification](../experimental/kmdf-forwarding/static-verification.json) records a13824-byte x64 Native PE, FxDriverEntry at RVA22F0, exported DriverEntry and imports only from ntoskrnl.exe/WDFLDR.SYS. SHA256 is `39692b54a89ccd2f707f495fd76494a425ce2e74068e59cd9089717c44ae47e3`. Five generated x64 WDF structure-size assertions compiled. Its certificate directory is empty and Authenticode reported NotSigned. File size does not establish runtime RAM.

The original staging check relocated relative dependency paths and excluded the
nested upstream workspace, resolving96 packages with offline locked metadata.
Its `staging-verification.json` is a historical snapshot, not the current helper
hash or proof of the later build.

The published relocated workspace has now actually built using `build.py
--offline`, Rust1.91.0 and the prepared tool/dependency set. The helper supports
an explicit existing `KERNEL_CARGO_HOME` and keeps compiler TEMP/TMP child-local
under `.build-state/tmp`. The latter fixes an observed archive rename denial in
the restricted system temp directory. The candidate-only retry succeeded
in1.19seconds; this is not a clean full-build timing.

[Relocated verification](../experimental/kmdf-forwarding/relocated-verification.json)
records exact source/lock/config/helper hashes and verifies all140 included
Microsoft source hashes at commit04523040779d80125f280c4f9421f54a6059f470. The new
13824-byte x64 Native PE has entrypoint22F0, exported DriverEntry, only ntoskrnl/
WDFLDR imports, an empty certificate directory and Authenticode NotSigned. SHA256
is `48b7d3cf2bf0eafbf4fafdc3c28e3e13b5d7b89c42de4f15fd0da818e62ea38c`.
It differs from the original artifact; byte reproducibility across workspaces
is not established. The binary and logs are separately retained at
`outputs/kernel-relocated-build-2026-10-28`, outside the source repository.

## Isolated runtime prerequisite

The prepared diskless Windows PE trial passed six preflight harness tests and
verified142 tool files, the published QEMU package hash and retained official
Microsoft media digest. Its fixed limits are two cores,4GiB guest RAM, five
minutes, no network, no writable guest disk and no physical-device passthrough.
It requests no installer navigation, license acceptance or driver installation.

The [actual launch preflight](validation/kernel-vm-preflight.json) found
6,825,955,328 available host RAM bytes, below its10GiB launch requirement.
**No guest process was started.** The preflight retained the configured resource
gate; no host boot/trust/service settings changed. This is a runtime-test
prerequisite still to resolve, not a successful boot or loaded-driver result.
The application/features and remaining full goal can continue independently.

The candidate has never been installed, loaded or attached to hardware. There is no INF, catalog, signing or installation flow. Ordinary-input preservation, callback rundown, PnP/power/removal, HVCI/HLK behavior, Razer feature parity and runtime memory remain unverified. The compiled candidate is a development seam, not a replacement driver. The included upstream build sources retain their original packaging helpers; the candidate's own build.py runs only cargo build and does not invoke them.
