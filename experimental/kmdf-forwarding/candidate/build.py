"""Compile only, using explicit tool locations and a process-local environment.

No packaging, certificate, service, INF registration or driver-loading command
is present. Requires Python3 and a prepared x64 MSVC/WDK/Rust/libclang tool set.
"""
from pathlib import Path
import json
import os
import subprocess
import sys

ROOT = Path(__file__).resolve().parent


def required_path(name):
    value = os.environ.get(name)
    if not value:
        raise SystemExit(f"Set {name}; see README.md")
    return Path(value).resolve(strict=True)


rust_bin = required_path("KERNEL_RUST_BIN")
kit_root = required_path("WDKContentRoot")
clang_bin = required_path("LIBCLANG_PATH")
vcvars = required_path("KERNEL_VCVARS64")
for file in [rust_bin / "rustc.exe", rust_bin / "cargo.exe", clang_bin / "libclang.dll"]:
    if not file.is_file():
        raise SystemExit(f"Missing prepared tool: {file}")
if not vcvars.is_file() or vcvars.name.lower() != "vcvars64.bat":
    raise SystemExit("KERNEL_VCVARS64 must identify the existing x64 MSVC setup batch file")

state = ROOT / ".build-state"
state.mkdir(exist_ok=True)
capture = state / "capture-msvc-env.cmd"
capture.write_text(f'@call "{vcvars}" >nul\n@if errorlevel 1 exit /b 1\n@set\n', encoding="utf-8")
# /u makes SET output unambiguous UTF16. Keep the complete environment only in
# memory; it may contain unrelated user secrets and is never logged to disk.
snapshot = subprocess.run(
    [os.environ.get("COMSPEC", "cmd.exe"), "/u", "/d", "/c", str(capture)],
    stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True,
)
env = os.environ.copy()
for line in snapshot.stdout.decode("utf-16-le").splitlines():
    if "=" in line and not line.startswith("="):
        key, value = line.split("=", 1)
        env[key] = value
env.update(
    WDKContentRoot=str(kit_root),
    Version_Number=os.environ.get("Version_Number", "10.0.26100.0"),
    LIBCLANG_PATH=str(clang_bin),
    RUSTC=str(rust_bin / "rustc.exe"),
    RUSTDOC=str(rust_bin / "rustdoc.exe"),
    CARGO_HOME=str(state / "cargo"),
    RUSTUP_HOME=str(state / "rustup"),
    # Upstream wdk-build finds Cargo.lock by walking OUT_DIR ancestors.
    CARGO_TARGET_DIR=str(ROOT / "target"),
    CARGO_BUILD_JOBS="2",
    CARGO_TERM_COLOR="never",
)
env["PATH"] = str(rust_bin) + os.pathsep + env["PATH"]
command = [str(rust_bin / "cargo.exe"), "build", "--locked", "--release",
           "--target", "x86_64-pc-windows-msvc", "--jobs", "2"]
if sys.argv[1:] == ["--offline"]:
    command.append("--offline")
elif sys.argv[1:]:
    raise SystemExit("Usage: python build.py [--offline]")
print("Compile only:", command, flush=True)
with (state / "compile.log").open("w", encoding="utf-8") as log:
    process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, text=True, encoding="utf-8",
                               errors="replace", creationflags=0x08000000)
    for line in process.stdout:
        log.write(line)
        log.flush()
        print(line, end="", flush=True)
    code = process.wait()
(state / "compile-result.json").write_text(json.dumps({"exit_code": code,
    "command": command, "scope": "compile/link only; no packaging/signing/loading"}, indent=2) + "\n")
raise SystemExit(code)
