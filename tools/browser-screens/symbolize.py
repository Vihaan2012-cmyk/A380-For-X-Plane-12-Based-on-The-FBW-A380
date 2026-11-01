#!/usr/bin/env python3
"""Names for the sampler's addresses: python symbolize.py <Log.txt> <win.xpl>"""
import bisect
import re
import struct
import subprocess
import sys
from pathlib import Path


def image_base(path):
    data = Path(path).read_bytes()[:4096]
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    return struct.unpack_from("<Q", data, pe + 24 + 24)[0]


def symbols(path):
    out = subprocess.run(["nm", "-n", "-C", "--defined-only", str(path)], capture_output=True, text=True, errors="replace").stdout
    rows = []
    for line in out.splitlines():
        parts = line.split(" ", 2)
        if len(parts) == 3 and parts[1] in "tT":
            rows.append((int(parts[0], 16), parts[2]))
    return rows


def main(log, xpl):
    base = image_base(xpl)
    rows = symbols(xpl)
    starts = [r[0] for r in rows]
    totals = {}
    windows = 0
    for line in Path(log).read_text(errors="replace").splitlines():
        if "main thread in our plugin:" not in line:
            continue
        windows += 1
        print("--- window")
        for rva, leaf, share in re.findall(r"rva=([0-9a-f]+) leaf=(.+?) ([\d.]+)(?:,|$)", line):
            i = bisect.bisect_right(starts, base + int(rva, 16)) - 1
            name = rows[i][1][:110] if i >= 0 else rva
            print(f"{float(share):5.1f}%  leaf {leaf:<22} {name}")
            totals[name] = totals.get(name, 0.0) + float(share)
    if windows:
        print("--- by function, mean share over the windows")
        for name, share in sorted(totals.items(), key=lambda kv: -kv[1])[:30]:
            print(f"{share / windows:5.1f}%  {name}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
