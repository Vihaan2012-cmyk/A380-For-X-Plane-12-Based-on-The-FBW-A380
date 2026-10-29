"""usage: wing_break_split.py <objects dir> [names...]

The wing sections X-Plane separates on its own wing failures: the skin and
slats are cut on the panel joints at 11.52 m and 19.5 m, and everything
outboard of the 6.5 m flex joint is hidden on rel_wing1, 2 or 3 by where it
lies, flap and spoiler panels whole. Runs inside wing_bend (unbent, sectioned,
bent again), so both tools stay rerunnable."""

import re
import sys
from pathlib import Path

import numpy as np

import wing_bend as wb

NATIVE = "sim/operation/failures/"
EDGES = [6.5, 11.52, 19.5, 31.5]
CUTS = [11.52, 19.5]
WING_MAX_Y = 4.0
FILES = re.compile(r"^a380_exterior_02[6-9](_hail)?\.obj$")
LEAVE = re.compile(r"RAT|gear|rel_engsep|ENG\d|engine|cowl|rel_wing|rel_hstb|rel_vstb")
SURFACE = "fbw/fail/sep/"


def cut(mesh, tri):
    for plane, c in enumerate(sorted({s * x for x in CUTS for s in (-1, 1)})):
        xs = mesh.v[tri, 0] - c
        zs = mesh.v[tri, 2].mean(axis=1)
        cross = (xs.min(axis=1) < -wb.EPS) & (xs.max(axis=1) > wb.EPS) & (zs <= wb.MAX_Z)
        if not cross.any():
            continue
        keep = [tri[~cross]]
        for t, d in zip(tri[cross], xs[cross]):
            keep.append(np.array(mesh.split(t, d, 100 + plane, c), np.int64).reshape(-1, 3))
        tri = np.concatenate(keep)
    return tri


def sections(points):
    ax = np.abs(points[:, 0])
    k = np.searchsorted(EDGES, ax, side="left")
    k[(k >= len(EDGES)) | (points[:, 1] >= WING_MAX_Y) | (points[:, 2] > wb.MAX_Z)] = 0
    return np.where(points[:, 0] < 0, -1, 1), k


def whole(mesh, tri):
    p = mesh.v[tri, :3]
    area = np.linalg.norm(np.cross(p[:, 1] - p[:, 0], p[:, 2] - p[:, 0]), axis=1)
    c = (p.mean(axis=1) * area[:, None]).sum(axis=0) / max(area.sum(), 1e-9)
    side, k = sections(c[None, :])
    return np.full(len(tri), side[0]), np.full(len(tri), k[0])


def prepare(items, mesh):
    out, stack = [], []
    counts = [0, 0, 0, 0]
    for it in items:
        if not isinstance(it, wb.Tris):
            t = it.strip()
            if t == "ANIM_begin":
                stack.append([])
            elif t == "ANIM_end":
                if stack:
                    stack.pop()
            elif t.startswith("ANIM_") and stack:
                stack[-1] += [w for w in t.split() if "/" in w]
            out.append(it)
            continue
        drefs = [d for frame in stack for d in frame]
        if not len(it.tri) or any(LEAVE.search(d) for d in drefs):
            out.append(it)
            continue
        if any(d.startswith(SURFACE) for d in drefs):
            tri = it.tri
            side, k = whole(mesh, tri)
        else:
            tri = cut(mesh, it.tri)
            side, k = sections(mesh.v[tri, :3].mean(axis=1))
        for key in sorted({(int(s), int(q)) for s, q in zip(side, k)}, key=lambda t: (t[1] != 0, t)):
            sel = tri[(side == key[0]) & (k == key[1])]
            if key[1] == 0:
                out.append(wb.Tris(it.indent, sel))
                continue
            counts[key[1]] += len(sel)
            out += ["ANIM_begin", f"ANIM_hide 5.5 6.5 {NATIVE}rel_wing{key[1]}{'L' if key[0] < 0 else 'R'}", wb.Tris(it.indent, sel), "ANIM_end"]
    print(f"  sections: wing 1 {counts[1]}, wing 2 {counts[2]}, wing 3 {counts[3]} triangles")
    return out


def main(objects_dir, *names):
    d = Path(objects_dir)
    files = [d / n for n in names] if names else [p for p in sorted(d.glob("a380_exterior_*.obj")) if FILES.match(p.name)]
    for f in files:
        print(f.name)
        r = wb.process(f, prepare)
        if r:
            print(" ", r)


if __name__ == "__main__":
    main(*sys.argv[1:])
