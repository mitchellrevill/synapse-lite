# Explicit native host launches

The Rust core now provides `native_host_launch::Request`, single-use `Control`
and Windows `launch`. A caller constructs a request from a compiled
`HostAction::Launch` and dispatches it explicitly on an owned worker thread.
The API is not connected to profile activation, import or the macro Play UI.
The mixed input scheduler continues to reject host actions during admission.

The backend checks a nonempty `pathToCheck` through GetFileAttributesW, preserving
the source's existence check, including directories. This is a preflight read,
not an immutable executable identity or protection against a later file change.
It tries the complete mutable UTF16 command line with CreateProcessW, no handle
inheritance, the current process security context, inherited environment and
source show/hidden flags. The child directory uses GetTempPathW with the recovered
260-unit buffer; zero or a result of260 or more leaves the option empty. Shell
fallback keeps its directory argument null. No shell argv reconstruction changes that first
attempt. Failed creation falls back to ShellExecuteExW with verb `open`, flags
0x440 and the whole original command as the file. Only Win32 errors2/3 permit a
second shell attempt with the source's first-token split. That split recognizes
ASCII space/tab and an initial quoted token; it does not use shell escaping or
CommandLineToArgvW. Argument spelling and Unicode remain intact.

Successful launch returns an acceptance receipt and optional process ID. Shell
association activation can succeed without a new process identity. The backend
closes all returned process/thread handles immediately and does not wait for the
application to exit. No user app is terminated on Stop. COM initializes lazily
only when the invocation reaches shell fallback, and balances initialization on
the same thread. A fallback from an incompatible MTA context returns its HRESULT;
the direct CreateProcess route works in that context without COM setup.

Each Control admits at most one invocation, including concurrent calls. Stop
before admission prevents all backend work. During dispatch it requests stopping
before subsequent attempts; it cannot interrupt an OS call already entered or
undo a successful launch. A successful OS result is never relabelled Cancelled
after a Stop race. Error codes preserve CreateProcess and final shell failures;
COM failures retain their HRESULT separately. Existing source limits apply and
execution also counts the terminating NUL in CreateProcess's32767-unit limit.

## Current native evidence

Target: mapping_engine.dll from installed AppEngine4.0.896, SHA256
c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9.
Earlier4.0.827 addresses are historical and were not used for these new helpers.

| Procedure | REA Ghidra Evidence | Recovered contract |
| --- | --- | --- |
| 0x18011b736 | ev_60b86338bc2bdb7b174a3fa69578de7864313fb04101e669260d9fd6db7b11ff | ShellExecuteExW, open/0x440, hidden flag, close returned hProcess |
| 0x1801b1c00 | ev_f2d264a096b06817b9c9c6ee955e2744d7b1d2eb9c67c74d2f7876bfd5c44da3 | PathExists uses GetFileAttributesW != INVALID_FILE_ATTRIBUTES |
| 0x1801b35a0 | ev_7c54f4022cbd7094f15834b4df68c9ba386f336edde6a3e27dc56e7e61c66cf3 | Default launch options zero flags/token/environment overrides |
| 0x1800cf950 | ev_044a437f64c0df252a912b8c9c2e6145479409fb52918ce0d951c5886c838190 | Base process-wrapper cleanup |
| 0x1801b1ef0 | ev_7d4211efccf8841844da705ad110f4ac9bd0c1050e2eb226db8c0fcc39dbf015 | Launch directory comes from GetTempPathW(260), accepted lengths1–259 |

The [prior current-build dossiers](MACRO-HOST-PLANNING.md) establish the parser,
device-thread helper and base CreateProcess call. The helper calls the shell
fallback first with the whole line and retries a split only when its error masked
with0xfffffffe equals2. It reports acceptance through the device-thread completion
path. This is static branch evidence, not a measured live Synapse callback trace.
Broader macro dispatch, inter-event completion and clipboard/paste are still open.

Windows SDK contracts are checked against Microsoft's
[CreateProcessW documentation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw)
and [ShellExecuteExW documentation](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw).
COM initialization on shell fallback follows the latter's STA guidance; it is
Rust lifetime management, not a claim that the recovered helper initializes COM.

## Verification

Seven pure checks exercise route order, exact split/flags, source-only retries,
error propagation, path and UTF16/NUL limits, Stop races, single-use controls and
concurrent duplicate refusal. Three explicit Windows checks use only the built
`host-launch-fixture.exe`, which optionally checks its working directory using
read-only filesystem metadata, sleeps100ms and exits with a contract result.
It performs no file writes, device or network I/O and is excluded from normal app packages.

The fixture verifies direct CreateProcess, actual whole/split ShellExecuteEx
routes (with CreateProcess failure forced in the test adapter), child exit and
returned-handle cleanup, temporary-directory selection, direct creation from MTA
and incompatible shell apartment refusal. A test-only duplicate of the returned
process handle retains authoritative exit observation even if the child exits
before a later PID lookup; production closes its original handles immediately.
Production launch never waits for child exit; only the test observer does.
The first eager-COM attempt compared pre-initialization handle
counts with a warmed COM process and observed130→142. Moving COM initialization
to shell fallback avoids it on direct launches; the final direct check observed
132→132 across four launches. This is a scoped handle observation, not a full
memory/leak audit. See the validation JSON for final build/test counts and hashes.

Final verification:585 core, nine CLI and308 serial native-app tests pass;
nine core and48 opt-in native tests remain ignored in ordinary suites. The three
owned Windows checks are run explicitly within all10 passing optimized backend
checks. Both release binaries build. The final optimized pass observes132→132
parent handles across four direct launches. A distinct child exit marker42 proves
the temporary-directory check actually ran; a generic successful exit alone does
not substitute for that check. [Validation and artifact hashes](validation/native-host-launch.json)
retain the exact evidence and remaining scope.

```powershell
cargo build -p secbird-core --example host-launch-fixture -j 1
$env:SECBIRD_HOST_LAUNCH_FIXTURE = "$env:CARGO_TARGET_DIR/debug/examples/host-launch-fixture.exe"
cargo test -p secbird-core launch_owned -j 1 -- --ignored --test-threads=1 --nocapture
```

These checks do not run imported commands, websites or personal programs, change
stock apps/services, send input or exercise device drivers. Arbitrary shell/file
associations, visible-window behavior and UAC/security-context variations remain
unverified. Imported launch metadata never executes automatically. Shared
host/input sequencing, worker admission/drain, profile Play controls, clipboard
ownership/paste recovery, full driver/API/hardware coverage and three complete
replacement runs below100,000,000 bytes remain required by the active goal.
