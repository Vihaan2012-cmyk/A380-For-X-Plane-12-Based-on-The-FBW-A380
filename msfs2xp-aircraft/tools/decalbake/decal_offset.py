"""usage: decal_offset.py <aircraft objects dir> <pre-bake objects dir> [clearance_m]

Lifts each lettering decal off the panel under it. FlyByWire's decals mostly sit 0.3-0.5 mm (some up
to 4 mm) inside the panel surface, which MSFS resolves in its G-buffer; X-Plane depth-tests them, so they
z-fought (strokes and half labels missing) at ATTR_poly_os 0 and vanished at ATTR_poly_os 1. Every
connected decal piece is measured against the cockpit surface below it (a ray back along its normal from
each triangle) and moved along its normals until its lowest point clears that surface by `clearance`
(default 0.5 mm). Positions always start from the pre-bake copy, so a rerun never lifts twice."""

import glob
import os
import sys
from pathlib import Path

import numpy as np
from scipy.spatial import cKDTree

DECALS = [f"a380_cockpit_{n:03d}.obj" for n in range(20, 26)]
PROBE = 0.012
REACH = 0.006
MAX_LIFT = 0.006


def load(path):
    lines = Path(path).read_text(encoding="latin-1").split("\n")
    v = np.array([[float(x) for x in l.split()[1:9]] for l in lines if l.startswith("VT")])
    idx = np.array([int(x) for l in lines if l.startswith(("IDX10", "IDX ")) for x in l.split()[1:]], dtype=np.int64)
    return lines, v, idx[: len(idx) // 3 * 3].reshape(-1, 3)


def panels(objects):
    tris = []
    for f in glob.glob(os.path.join(objects, "a380_cockpit_*.obj")):
        b = os.path.basename(f)
        if b in DECALS or "_clean" in b or "_lamps" in b:
            continue
        _, v, t = load(f)
        if len(v) and len(t):
            tris.append(v[t][:, :, :3])
    p = np.concatenate(tris)
    return p, cKDTree(p.mean(axis=1))


def depth(o, d, p, tree):
    k = tree.query_ball_point(o, 0.05)
    if not k:
        return None
    a, b, c = p[k, 0], p[k, 1], p[k, 2]
    e1, e2 = b - a, c - a
    h = np.cross(d, e2)
    det = np.einsum("ij,ij->i", e1, h)
    ok = np.abs(det) > 1e-14
    s = o - a
    u = np.einsum("ij,ij->i", s, h) / np.where(ok, det, 1)
    q = np.cross(s, e1)
    w = (q @ d) / np.where(ok, det, 1)
    t = np.einsum("ij,ij->i", e2, q) / np.where(ok, det, 1)
    hit = ok & (u >= 0) & (w >= 0) & (u + w <= 1) & (t > 0) & (t < PROBE + REACH)
    return t[hit].min() if hit.any() else None


def islands(tri, n):
    parent = np.arange(n)

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for a, b, c in tri:
        ra, rb, rc = find(a), find(b), find(c)
        parent[rb] = ra
        parent[find(rc)] = ra
    return np.array([find(i) for i in range(n)])


def lift(dst, src, p, tree, clearance):
    _, v, tri = load(src)
    lines = Path(dst).read_text(encoding="latin-1").split("\n")
    rows = [i for i, l in enumerate(lines) if l.startswith("VT")]
    if len(rows) != len(v):
        raise SystemExit(f"{dst}: {len(rows)} vertices, pre-bake has {len(v)}")
    nrm = v[:, 3:6] / np.linalg.norm(v[:, 3:6], axis=1, keepdims=True)
    need = np.full(len(v), clearance)
    for t in tri:
        pts = np.vstack([v[t, :3], v[t, :3].mean(axis=0)])
        for o, d in zip(pts, np.vstack([nrm[t], nrm[t].mean(axis=0)])):
            d = d / np.linalg.norm(d)
            t_hit = depth(o + d * PROBE, -d, p, tree)
            if t_hit is not None:
                below = t_hit - PROBE
                need[t] = np.maximum(need[t], min(MAX_LIFT, clearance + below))
    root = islands(tri, len(v))
    amount = np.zeros(len(v))
    for r in np.unique(root):
        m = root == r
        amount[m] = need[m].max()
    for i, row in enumerate(rows):
        x = v[i].copy()
        x[:3] += nrm[i] * amount[i]
        lines[row] = "VT " + " ".join(f"{c:.6f}" for c in x)
    Path(dst).write_text("\n".join(lines), encoding="latin-1", newline="\n")
    return len(v), np.percentile(amount, [50, 95]) * 1000, amount.max() * 1000


def main(objects, prebake, clearance=0.0005):
    p, tree = panels(objects)
    for n in DECALS:
        count, (mid, p95), top = lift(Path(objects) / n, Path(prebake) / n, p, tree, float(clearance))
        print(f"{n}: {count} vertices, lift median {mid:.2f} mm, p95 {p95:.2f} mm, max {top:.2f} mm")


if __name__ == "__main__":
    main(*sys.argv[1:4])
