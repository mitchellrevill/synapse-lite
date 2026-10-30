"""Read one Windows stock census through psutil; never launch/modify processes.

Developer-only optional backend for protected services whose CIM path or .NET
counter properties are unavailable. The PowerShell sampler validates this frame.
"""
import datetime
import json
import math
import ntpath
import os
import re
import sys


def inside(path, roots):
    path = ntpath.normcase(ntpath.normpath(path))
    return any(path.startswith(ntpath.normcase(ntpath.normpath(root)).rstrip("\\") + "\\") for root in roots)


def utc(seconds):
    return datetime.datetime.fromtimestamp(seconds, datetime.timezone.utc).isoformat()


def capture(roots, processes, fresh_process, transient_errors):
    rows = []
    inaccessible_unrelated = 0
    seen = set()
    for index, process in enumerate(processes):
        if index >= 4096:
            raise RuntimeError("Process census exceeds its4096-process bound")
        name = process.name()
        named = re.match(r"^(Razer|Rz|GameManager|Synapse)", name, re.IGNORECASE)
        try:
            image = process.exe()
        except transient_errors:
            if named:
                raise RuntimeError(f"Stock candidate PID{process.pid} has no readable image; no baseline")
            inaccessible_unrelated += 1
            continue
        if not image:
            if named:
                raise RuntimeError(f"Stock candidate PID{process.pid} has an empty image; no baseline")
            inaccessible_unrelated += 1
            continue
        if not inside(image, roots):
            if named:
                raise RuntimeError(f"Stock candidate PID{process.pid} is outside the declared roots")
            continue
        try:
            born = process.create_time()
            with process.oneshot():
                memory = process.memory_info()
                cpu = process.cpu_times()
            # Fresh Process object: do not accept a cached creation identity.
            observed = fresh_process(process.pid).create_time()
            if born != observed or process.pid in seen:
                raise RuntimeError("Stock process identity changed or duplicated; no baseline")
            seen.add(process.pid)
            values = (memory.rss, memory.private, cpu.user, cpu.system, born)
            if any(not isinstance(n, (int, float)) or not math.isfinite(n) or n < 0 for n in values):
                raise RuntimeError("Invalid stock process counters; no baseline")
            rows.append(dict(Name=name, ProcessId=process.pid, ExecutablePath=image,
                             CreationDate=utc(born), StartUtc=utc(observed), Id=process.pid,
                             WorkingSet=memory.rss, PrivateBytes=memory.private,
                             CpuSeconds=cpu.user + cpu.system))
        except transient_errors as error:
            raise RuntimeError(f"Stock process PID{process.pid} became unreadable; no baseline") from error
    if not rows:
        raise RuntimeError("No stock processes visible; empty is not zero memory")
    return dict(processes=rows, inaccessible_unrelated_images=inaccessible_unrelated)


def main():
    if os.name != "nt":
        raise RuntimeError("This counter backend requires Windows")
    import psutil  # Explicit developer prerequisite; never downloaded at runtime.
    raw = sys.stdin.read(32 * 1024 + 1)
    if len(raw) > 32 * 1024:
        raise RuntimeError("Snapshot request exceeds32KiB")
    request = json.loads(raw)
    roots = request["roots"]
    if not isinstance(roots, list) or not roots or len(roots) > 32 or any(not isinstance(r, str) or not ntpath.isabs(r) for r in roots):
        raise RuntimeError("Expected a bounded explicit root list")
    result = capture(roots, psutil.process_iter(), psutil.Process, (psutil.AccessDenied, psutil.NoSuchProcess))
    result["backend"] = "psutil " + psutil.__version__
    result["identity_check"] = "Fresh process birth identity before/after the native counter read"
    print(json.dumps(result, ensure_ascii=True, allow_nan=False))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"Stock snapshot refused: {error}", file=sys.stderr)
        sys.exit(1)
