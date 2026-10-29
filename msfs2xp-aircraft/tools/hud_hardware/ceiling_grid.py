"""usage: ceiling_grid.py <aircraft objects dir> <out.json> [centre_u centre_v half_u half_v step]

Samples the cockpit ceiling lining under the captain's HUD projector panel: a grid over the panel's
footprint on the sloped ceiling (hud_blender.py's CEIL_FWD/CEIL_AFT plane), each point the first ceiling
surface met along the plane's normal. hud_blender.py drapes the panel on these points so it sits flush
on the real, curved lining instead of a flat plane that sinks into it or floats off it."""

import glob
import json
import os
import sys

import numpy as np

X0 = -0.524
CEIL_FWD = (3.461, -32.27)
CEIL_AFT = (3.632, -32.07)
CENTRE_UV = (0.03, -0.06)
HALF_UV = (0.12, 0.14)
STEP = 0.003


def ceiling_tris(objects):
    out = []
    for f in glob.glob(os.path.join(objects, "a380_cockpit_*.obj")):
        b = os.path.basename(f)
        if "_clean" in b or "click" in b or "_lamps" in b:
            continue
        lines = open(f, encoding="latin-1", errors="replace").read().split("\n")
        v = np.array([[float(x) for x in l.split()[1:4]] for l in lines if l.startswith("VT")])
        idx = np.array([int(x) for l in lines if l.startswith(("IDX10", "IDX ")) for x in l.split()[1:]], dtype=np.int64)
        if not len(v) or not len(idx):
            continue
        t = v[idx[: len(idx) // 3 * 3].reshape(-1, 3)]
        c = t.mean(axis=1)
        keep = (np.abs(c[:, 0] - X0) < 0.35) & (c[:, 1] > 3.2) & (c[:, 2] > -32.6) & (c[:, 2] < -31.7)
        out.append(t[keep])
    return np.concatenate(out)


def hit(P, o, d):
    a, b, c = P[:, 0], P[:, 1], P[:, 2]
    e1, e2 = b - a, c - a
    h = np.cross(d, e2)
    det = np.einsum("ij,ij->i", e1, h)
    ok = np.abs(det) > 1e-12
    det = np.where(ok, det, 1)
    s = o - a
    u = np.einsum("ij,ij->i", s, h) / det
    q = np.cross(s, e1)
    w = (q @ d) / det
    t = np.einsum("ij,ij->i", e2, q) / det
    good = ok & (u >= 0) & (w >= 0) & (u + w <= 1) & (t > 0)
    return t[good].min() if good.any() else None


def main(objects, out, cu=CENTRE_UV[0], cv=CENTRE_UV[1], hu=HALF_UV[0], hv=HALF_UV[1], step=STEP):
    cu, cv, hu, hv, step = map(float, (cu, cv, hu, hv, step))
    nu, nv = int(round(2 * hu / step)) + 1, int(round(2 * hv / step)) + 1
    P = ceiling_tris(objects)
    (yf, zf), (ya, za) = CEIL_FWD, CEIL_AFT
    along = np.array([0.0, yf - ya, zf - za])
    along /= np.linalg.norm(along)
    down = np.array([0.0, -along[2], along[1]])
    if down[1] > 0:
        down = -down
    mid = np.array([X0, (yf + ya) / 2, (zf + za) / 2])
    xs = np.array([1.0, 0.0, 0.0])
    grid = []
    for j in range(nv):
        row = []
        for i in range(nu):
            u = cu - hu + 2 * hu * i / (nu - 1)
            v = cv - hv + 2 * hv * j / (nv - 1)
            o = mid + xs * u + along * v - down * 0.20
            t = hit(P, o, down)
            row.append(None if t is None else (o + down * t).tolist())
        grid.append(row)
    missing = sum(p is None for r in grid for p in r)
    json.dump({"grid": grid, "along": along.tolist(), "down": down.tolist(), "mid": mid.tolist(),
               "half": [hu, hv], "centre": [cu, cv]}, open(out, "w"))
    print(f"{nu}x{nv} ceiling points, {missing} missing")


if __name__ == "__main__":
    main(*sys.argv[1:8])
