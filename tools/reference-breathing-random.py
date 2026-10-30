"""Scalar reconstruction of retained Ghidra 0x180069600, never loads a DLL.

Uses the decompiler's wide multiply/carry arithmetic rather than the Rust
simplification. Reference output is BGR helper bytes plus post-call stream state.
"""
import json
from pathlib import Path


def color(state):
    while True:
        channels = []
        for _ in range(3):
            product = state * 0x343FD
            candidate = product + 0x269EC3
            state = product - (candidate & 0xFFFFFFFF00000000) + 0x269EC3
            mixed = (state >> 16) ^ state
            high = (mixed * 0x8080808080808081) >> 64
            value = (mixed + (high >> 7)) & 255
            channels.append(255 if value > 170 else 128 if value > 85 else 0)
        if len(set(channels)) != 1:
            return channels, state


cases = []
for seconds in [0, 1799999, 1800000, 1800000000, 1801800000]:
    state = (seconds // 1800000) * 1800000 | 31
    samples = []
    for _ in range(32):
        channels, state = color(state)
        samples.append({"bgr": channels, "state": state})
    cases.append({"unix_seconds": seconds, "samples": samples})
destination = Path(__file__).resolve().parents[1] / "crates/secbird-core/tests/fixtures/studio-breathing-random.json"
destination.write_text(json.dumps({"provenance": "Static reconstruction; no vendor execution", "cases": cases}, indent=2) + "\n")
