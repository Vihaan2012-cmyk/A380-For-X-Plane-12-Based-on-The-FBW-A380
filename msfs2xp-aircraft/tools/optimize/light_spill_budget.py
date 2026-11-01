#!/usr/bin/env python3
"""lights.obj with fewer spill lights, and none in daylight.

    python light_spill_budget.py <lights.obj> [--dry-run]

Each `airplane_*_pm` is a deferred spill: one pass over every pixel its cone covers. The 33 converted from
systems.cfg double the GPU's frame from the flight deck with everything on. Pairs that sit together and switch
together become one light of twice the candela, a few only ever lit the wingtip, and what is left hides on
`fbw/anim/spills_hidden` (the plugin's `light_spills.rs`), which daylight outshines anyway. Billboards stay.
"""
import argparse
import re
import shutil
import sys
from pathlib import Path

HIDE = "ANIM_hide 0.5 1.5 fbw/anim/spills_hidden"


def parse(line):
    t = line.split()
    return {
        "kind": t[1],
        "pos": tuple(float(x) for x in t[2:5]),
        "index": t[8],
        "cd": float(t[9].rstrip("cd")),
        "tail": t[10:],
        "rgb": t[5:8],
    }


def emit(kind, pos, rgb, index, cd, tail):
    return "LIGHT_PARAM {} {:.6f} {:.6f} {:.6f} {} {} {}cd {}\n".format(kind, *pos, " ".join(rgb), index, int(round(cd)), " ".join(tail))


def near(a, b, tol=0.02):
    return all(abs(x - y) <= tol for x, y in zip(a, b))


# kind, the lights to merge (the first keeps its place in the file), position of the result (None: their middle).
MERGE = [
    ("airplane_landing_pm", [(0.255681, -0.807266, -30.60716), (-0.25, -0.807266, -30.60716)], None),
    ("airplane_landing_pm", [(-5.43, 0.17, -14.37), (-5.43, 0.36, -14.37)], None),
    ("airplane_landing_pm", [(5.43, 0.17, -14.37), (5.43, 0.36, -14.37)], None),
    ("airplane_generic_pm", [(-6.081, 4.259, 31.266), (-6.070, 4.279, 31.132)], None),
    ("airplane_generic_pm", [(6.081, 4.259, 31.266), (6.070, 4.279, 31.132)], None),
    ("airplane_generic_pm", [(-0.26, -0.54, -31.07), (-1.97, -1.01, -25.28)], (-0.26, -0.54, -31.07)),
    ("airplane_generic_pm", [(0.27, -0.54, -31.07), (1.97, -1.01, -25.28)], (0.27, -0.54, -31.07)),
    ("airplane_generic_pm", [(-3.35, 2.95, -24.72), (-3.13, 1.36, -27.27)], (-3.35, 2.95, -24.72)),
    ("airplane_generic_pm", [(3.35, 2.95, -24.72), (3.13, 1.36, -27.27)], (3.35, 2.95, -24.72)),
    ("airplane_strobe_pm", [(-0.016, 3.743, 35.501), (0.016, 3.743, 35.501)], None),
    ("airplane_beacon_pm", [(-0.293, 6.837, -14.577), (0.296, 6.837, -14.577)], None),
]
DROP = [("airplane_nav_pm", (-39.915, 2.185, 14.877)), ("airplane_nav_pm", (39.915, 2.185, 14.877))]
TOL = 0.06


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("obj", type=Path)
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()
    raw = a.obj.read_bytes().decode("latin-1")
    if "fbw/anim/spills_hidden" in raw:
        sys.exit("already done")
    lines = raw.splitlines(keepends=True)
    pm = {i: parse(l) for i, l in enumerate(lines) if l.startswith("LIGHT_PARAM") and l.split()[1].endswith("_pm")}
    print(len(pm), "spill lights")
    removed = set()
    replaced = {}
    for kind, spots, at in MERGE:
        found = []
        for spot in spots:
            tol = 0.01 if kind == "airplane_strobe_pm" else TOL
            hits = [i for i, p in pm.items() if p["kind"] == kind and near(p["pos"], spot, tol) and i not in removed and i not in found]
            if len(hits) != 1:
                sys.exit(f"{kind} {spot}: {len(hits)} matches")
            found.append(hits[0])
        first = pm[found[0]]
        members = [pm[i] for i in found]
        if len({m["index"] for m in members}) > 1 and kind != "airplane_landing_pm":
            sys.exit(f"{kind} {spots}: different switches")
        pos = first["pos"] if at is not None else tuple(sum(m["pos"][k] for m in members) / len(members) for k in range(3))
        replaced[found[0]] = emit(kind, pos, first["rgb"], first["index"], sum(m["cd"] for m in members), first["tail"])
        removed.update(found[1:])
    for kind, spot in DROP:
        hits = [i for i, p in pm.items() if p["kind"] == kind and near(p["pos"], spot, TOL)]
        if len(hits) != 1:
            sys.exit(f"{kind} {spot}: {len(hits)} matches")
        removed.add(hits[0])
    out = []
    kept = 0
    for i, line in enumerate(lines):
        if i in removed:
            continue
        if i in pm:
            kept += 1
            out.append("ANIM_begin\n")
            out.append(HIDE + "\n")
            out.append(replaced.get(i, line))
            out.append("ANIM_end\n")
        else:
            out.append(line)
    print(kept, "spill lights after")
    if a.dry_run:
        return
    a.obj.write_bytes("".join(out).encode("latin-1"))


if __name__ == "__main__":
    main()
