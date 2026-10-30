"""Summarize measured data; never launch an app or change its configuration."""
import argparse
import collections
import hashlib
import json
import statistics
from pathlib import Path


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--source-commit", required=True)
    args = parser.parse_args()
    root = args.directory.resolve()
    observed = read_json(root / "process-before.json")
    integrity = read_json(root / "normal-profile-integrity.json")
    assert integrity["unchanged"] and integrity["before_sha256"] == integrity["after_sha256"]
    errors = [row for row in observed["rows"] if "error" in row]
    assert not any(row["group"] == "lite" for row in errors), "Normal Lite counters must be complete"
    error_counts = collections.Counter((row["group"], row["name"], row["error"]) for row in errors)
    lite = [row for row in observed["rows"] if row["group"] == "lite" and "error" not in row]
    assert lite and len({row["pid"] for row in lite}) == 1, "Expected one normal Lite instance"
    normal = {
        "samples": len(lite),
        "working_set_median_bytes": statistics.median(row["working_set_bytes"] for row in lite),
        "working_set_peak_bytes": max(row["working_set_bytes"] for row in lite),
        "private_median_bytes": statistics.median(row["private_bytes"] for row in lite),
        "private_peak_bytes": max(row["private_bytes"] for row in lite),
        "cpu_percent_machine_mean": statistics.mean(row["cpu_percent_machine"] for row in lite if row["cpu_percent_machine"] is not None),
        "handles_range": [min(row["handles"] for row in lite), max(row["handles"] for row in lite)],
        "threads_range": [min(row["threads"] for row in lite), max(row["threads"] for row in lite)],
    }
    grouped = {}
    raw_files = [root / "process-before.json", root / "catalog-baseline.json", root / "normal-profile-integrity.json"]
    for path in sorted(root.glob("measured-*/runtime.json")):
        report = read_json(path)
        assert report["IntrinsicReadOnlyReportedInEveryRun"]
        raw_files.append(path)
        for run in report["Runs"]:
            frame = run["PresentedFrame"]
            assert frame["matched"] and frame["presented"] and frame["visible"] and not frame["failed"]
            assert run["StartupToPresentedFrameMilliseconds"] > 0
            samples = run["Samples"]
            warm = [sample for sample in samples if sample["ElapsedSeconds"] >= 2]
            assert len(warm) >= 2
            cpu_one_core = 100 * (warm[-1]["CpuSeconds"] - warm[0]["CpuSeconds"]) / (warm[-1]["ElapsedSeconds"] - warm[0]["ElapsedSeconds"])
            grouped.setdefault(run["Scenario"], []).append({
                "binary_sha256": report["BinarySha256"].lower(),
                "startup_ms": run["StartupToPresentedFrameMilliseconds"],
                "peak_working_set_bytes": run["PeakWorkingSetBytes"],
                "peak_sampled_private_bytes": run["SampledPeakPrivateBytes"],
                "peak_paged_bytes": run["PeakPagedMemoryBytes"],
                "warm_cpu_percent_one_core": cpu_one_core,
                "warm_cpu_percent_machine": cpu_one_core / observed["logical_processors"],
                "processes": run["MaxObservedProcesses"],
                "samples": len(samples),
            })
    assert set(grouped) == {"dashboard", "keyboard-lighting", "chroma"}
    assert all(len(runs) == 3 for runs in grouped.values()), "Require three repetitions per presented scenario"
    hashes = {run["binary_sha256"] for runs in grouped.values() for run in runs}
    assert len(hashes) == 1, "All baseline runs must use the same executable"
    catalogue = read_json(root / "catalog-baseline.json")
    availability = read_json(root / "counter-availability-check.json")
    assert not any("error" in row for row in availability["rows"])
    stock = read_json(root / "stock-services.json")
    raw_files.extend([root / "counter-availability-check.json", root / "stock-services.json"])
    summary = {
        "application_source_commit": args.source_commit,
        "binary_sha256": next(iter(hashes)),
        "normal_observation": normal,
        "logical_processors": observed["logical_processors"],
        "observation_errors": [{"group": group, "name": name, "error": error, "count": count}
            for (group, name, error), count in sorted(error_counts.items())],
        "normal_profile_integrity": integrity,
        "collector_availability_check": "pass; memory retained when service CPU/start counters unavailable",
        "stock_service_subset": {
            "summary": stock["Summary"],
            "max_processes": max(sample["ProcessCount"] for sample in stock["Samples"]),
            "max_cpu_measured_processes": max(sample["CpuMeasuredProcesses"] for sample in stock["Samples"]),
            "scope": "Existing service-matched subset only; not full stock Synapse or complete CPU coverage",
        },
        "runs": grouped,
        "catalogue": catalogue,
        "full_build_100mb_acceptance": False,
        "raw_sha256": {str(path.relative_to(root)): sha256(path) for path in raw_files},
    }
    (root / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    lines = [
        "# Performance baseline — 30 October 2026", "",
        "No application refactor is included. The experimental catalogue index was reverted before the final measurements. Only measurement tooling changes.", "",
        f"Normal build: `{summary['application_source_commit']}`. Binary SHA-256: `{summary['binary_sha256']}`.", "",
        "## Normal desktop observation", "",
        f"31 observations over approximately 30 seconds: median working set {normal['working_set_median_bytes']/1e6:.2f} MB; median private memory {normal['private_median_bytes']/1e6:.2f} MB; mean CPU {normal['cpu_percent_machine_mean']:.3f}% of machine capacity ({observed['logical_processors']} logical processors). This was the existing desktop session, with no scripted interactions. It is not a controlled active workload.", "",
        "## Repeated page presentation", "",
        "Three fresh processes per scenario, eight-second intervals, same packaged release binary/assets, isolated fresh configuration, real connected Ornata, intrinsic query-only policy. Existing normal Lite was gracefully closed and restored. Descendants were enumerated; one app process was observed per run. Filesystem caches were not cleared: these are warm-cache process starts, not cold boot measurements.", "",
        "| Scenario | Startup median (range), ms | Peak working set, MB | Peak sampled private, MB | Mean warm CPU, machine % |",
        "| --- | ---: | ---: | ---: | ---: |",
    ]
    for scenario, runs in sorted(grouped.items()):
        startup = [run["startup_ms"] for run in runs]
        lines.append(f"| {scenario} | {statistics.median(startup):.0f} ({min(startup):.0f}–{max(startup):.0f}) | {max(run['peak_working_set_bytes'] for run in runs)/1e6:.2f} | {max(run['peak_sampled_private_bytes'] for run in runs)/1e6:.2f} | {statistics.mean(run['warm_cpu_percent_machine'] for run in runs):.3f} |")
    lines.extend([
        "", "Startup is process creation to the file recording the first confirmed, visible requested frame. It includes discovery/initialization; it is not click-to-response latency. Memory uses decimal MB; working set is the OS process high-water mark, private memory is sampled at approximately 200 ms, and shared pages may be counted in each descendant. CPU uses deltas after two seconds of warmup.", "",
        "## Catalogue microbenchmark", "",
        f"{catalogue['products']} products; {catalogue['connections']} connection IDs; {catalogue['queries_per_pass']} queries per pass including misses. Nine samples, 500 passes each. Median lookup: {statistics.median(catalogue['lookup_ns_per_query']):.0f} ns/query. Median parse and drop: {statistics.median(catalogue['parse_ns_per_catalogue'])/1e3:.1f} microseconds/catalogue. Release profile, metadata only, no device discovery or protocol calls. This is not a UI latency or full device-support benchmark.", "",
        "## Evaluation and limits", "",
        "The measured read-only pages fit below 100,000,000 bytes for both memory metrics. The normal observation has roughly 16 MB of working-set headroom, but this does not establish the complete-build memory goal. These short samples cannot establish leak freedom or long-session stability.", "",
        "The catalogue operation is approximately one microsecond. That does not establish it as a meaningful app bottleneck, so optimizing it is not justified by this baseline alone. Preserve this baseline while restructuring for maintainability, and compare the same binary profile, assets, configurations and workloads after each coherent refactor.", "",
        "Not covered: active Studio composition preview and device output, macro recording/playback, Linked Games under a real catalogue, input remapping load, long sessions, cold-cache startup, UI interaction latency and the absent mouse Performance page. Query-only runs intentionally disable generic profile/policy application and mutation workers. No full feature-load or RAM acceptance is claimed.", "",
        "The first stock observation was incomplete: protected Razer service start/CPU counters returned null, and that collector rejected their rows. Only one service had complete counters. This is not evidence that the other services were absent. Normal Lite had no counter errors. Do not use that partial service total as a stock comparison; the full stock application/workload comparison remains unmeasured. Any separate stock-services sample covers its recorded existing service state only. The exploratory indexed benchmark and stale build-cache run are retained under excluded/ and are not baseline evidence.", "",
        "Raw measurements and hashes are in summary.json and the individual runtime.json files. The current normal-profile integrity check is recorded separately. No new application features or production refactor are part of this checkpoint.", "",
    ])
    (root / "BASELINE.md").write_text("\n".join(lines), encoding="utf-8")
    print(root / "BASELINE.md")


if __name__ == "__main__":
    main()
