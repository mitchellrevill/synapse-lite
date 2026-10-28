# Experimental unsigned KMDF forwarding candidate

This independent Cargo workspace compiled and linked as an x64 native KMDF1.33 PE using Rust1.91.0, LLVM20.1.8 and the Microsoft WDK/SDK26100.6584 NuGet contents. The published relocated source now also builds offline with the locked dependencies and included Microsoft source subset. That nested upstream workspace is excluded from the candidate's workspace membership. See the separate historical and relocated build evidence below.

The driver marks itself as a filter, allocates typed per-device context, records atomic cleanup/forwarding state, forwards current request types to the lower target with SEND_AND_FORGET, and completes failed sends with their reported status. Keyboard/mouse CONNECT_DATA is unchanged. It implements no RZCONTROL remapping, selectors, injection, virtual consumer device or lighting operation.

**It has never been loaded or attached to hardware.** Compilation does not validate ordinary-input preservation, callback rundown, PnP/power behavior or Razer parity. There is no INF, catalog, certificate creation, signing, installation or loading flow in this workspace. Do not treat it as a complete Rust replacement driver.

## Prepared tool prerequisites

- Windows x64; Python3; existing MSVC x64 build tools.
- Portable Rust1.91.0 compiler/Cargo plus x86_64-pc-windows-msvc standard library.
- A prepared WDK content root merging the `c/` trees of Microsoft.Windows.WDK.x64, Microsoft.Windows.SDK.cpp.x64 and Microsoft.Windows.SDK.cpp, all10.0.26100.6584. The content directory version is10.0.26100.0.
- A compatible prepared libclang DLL directory. The executed experiment used the official LLVM20.1.8 x64 MSVC archive, selectively extracting its DLLs/resources/licenses.

Toolchains, SDK payloads, caches and generated bindings are intentionally excluded. `vendor/windows-drivers-rs` contains140 unmodified upstream build-source files and licenses at commit04523040779d80125f280c4f9421f54a6059f470; see upstream-provenance.json. Cargo.lock retains the exact dependency graph/checksums. The dependencies' included licenses apply to that source.

## Compile only

From this directory, set locations for your prepared tools in the current PowerShell session. Replace the example directory variables with your own absolute paths; none is installed by this script.

```powershell
$env:KERNEL_RUST_BIN = Join-Path $ToolRoot 'rust\bin'
$env:LIBCLANG_PATH = Join-Path $ToolRoot 'llvm\bin'
$env:WDKContentRoot = $PreparedWdkRoot
$env:Version_Number = '10.0.26100.0'
$env:KERNEL_VCVARS64 = Join-Path $VisualStudioRoot 'VC\Auxiliary\Build\vcvars64.bat'
python build.py
```

The script captures the MSVC environment in memory and sets Rust/WDK/Clang paths only for its build child. Cargo/Rustup homes and logs are private `.build-state` folders; target output remains under this workspace because upstream wdk-build requires Cargo.lock to be an ancestor of OUT_DIR. It does not change the default Rust toolchain, registry or persistent PATH. Cargo may acquire the locked registry dependencies into that private cache. After they exist, use `python build.py --offline` for an offline build.

For an already prepared external Cargo cache, set `KERNEL_CARGO_HOME` to that
existing directory. It is used only by the build child, avoiding a second download
or copying cache credentials. With `--offline`, missing dependencies cause a
build failure rather than network acquisition. The default remains the private
`.build-state/cargo` directory. Target output stays inside this candidate.
Compiler temporary files also stay in `.build-state/tmp` through child-local
TEMP/TMP settings, avoiding archive moves through a restricted system temp folder.

Output: `target/x86_64-pc-windows-msvc/release/rz_passthrough_candidate.dll`. Despite the Cargo extension, inspect the PE Native subsystem and WDF entrypoint; do not execute it as a user-mode DLL. A byte-identical `.sys` copy was made only for the saved experiment artifact. No installable package is generated.

Do not invoke the upstream default `cargo make` packaging task for this compile-only experiment: that task creates a test certificate/store and signs artifacts. No such command is present in build.py.

## Evidence boundary

The original private-path build took59.39seconds; its final candidate-only rebuild
took0.50seconds. [Historical static verification](../static-verification.json)
and `staging-verification.json` retain that earlier scope and source snapshot.

The later published-path rebuild actually ran `build.py --offline`, using an
existing prepared cache. Its first compile reached linking but failed moving a
compiler archive through the restricted system temp folder. The child-local
TEMP/TMP fix above allowed the candidate-only retry to finish in1.19seconds.
[Relocated verification](../relocated-verification.json) records the new13824-byte
unsigned binary, exact current build inputs and140 verified upstream source
files. Its digest differs from the historical binary; byte reproducibility
between those workspaces is not claimed. Both binaries remain private artifacts.
See [the build and VM evidence](../../../docs/KERNEL-BUILD.md). SDK/tool payloads,
caches and private logs are not committed. Neither build loads the driver.
