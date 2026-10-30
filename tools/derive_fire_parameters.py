"""Independent static-instruction Fire reference; never loads the vendor DLL.

Usage: python tools/derive_fire_parameters.py recovered-tables.json output.json
Tables use cooling_by_row / palette_index_by_row from retained native evidence.
This script uses the native descriptor interpolation path, independently of Rust.
"""
import hashlib
import json
import struct
import sys
from pathlib import Path


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def interpolate(first, last, count):
    value = f32(first)
    delta = f32(f32(last - first) / f32(count - 1))
    result = [first]
    for _ in range(count - 2):
        value = f32(value + delta)
        result.append(int(value))
    return result + [last]


def generate(tables, rate, seed, state):
    count = seed or 50
    interval = max(40, rate) // 40 + 1
    cooling = tables['cooling_by_row']
    keys = [[[0] * 23 for _ in range(7)] for _ in range(count)]
    for key in keys:
        for col in range(23):
            state = (state * 214013 + 2531011) & 0xffffffff
            value = ((state >> 16) & 32767) % 192 + 32
            key[6][col] = value - value * cooling[6][col] // 255
    for row in range(6, 0, -1):
        for index, key in enumerate(keys):
            for col in range(23):
                value = key[row][col]
                loss = value * cooling[row - 1][col] // 255
                keys[(index + 1) % count][row - 1][col] = max(1, value - loss)
    frames = []
    for index, key in enumerate(keys):
        channels = [interpolate(key[row][col], keys[(index + 1) % count][row][col], interval)
                    for row in range(7) for col in range(23)]
        frames.extend(bytes(channel[step] for channel in channels) for step in range(interval))
    return frames, state


def main():
    table_bytes = Path(sys.argv[1]).read_bytes()
    tables = json.loads(table_bytes)
    cases = []
    for rate, seed, state in [(0, 0, 1), (39, 1, 0), (40, 2, 1), (79, 7, 12345),
                              (80, 3, 0xffffffff), (119, 11, 77), (120, 5, 1),
                              (200, 9, 998), (10200, 1, 1), (40, 2048, 1)]:
        frames, final_state = generate(tables, rate, seed, state)
        cases.append(dict(rate=rate, seed=seed, initial_random_state=state,
                          frames=len(frames), final_random_state=final_state,
                          grayscale_md5=hashlib.md5(b''.join(frames)).hexdigest()))
    output = dict(provenance='Independent static-instruction reference; no vendor execution',
                  native_sha256='cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa',
                  tables_sha256=hashlib.sha256(table_bytes).hexdigest(), cases=cases)
    Path(sys.argv[2]).write_text(json.dumps(output, indent=2) + '\n', encoding='utf-8')
    print(f'{len(cases)} cases; {sum(case["frames"] for case in cases)} complete intrinsic frames')


if __name__ == '__main__':
    main()
