"""usage: clear_fbw_hud_labels.py <aircraft objects dir> [--dry-run]

FlyByWire's own HUD control strip lettering (NORM / X WIND / BRT / HUD / DECLUTTER and the scales) sits
on the glareshield where make_hud_hardware.py puts the HUD control panels, about a millimetre above it, so
it showed through the new panels. Its triangles inside each panel's footprint are collapsed to a point
in the decal objects, which keeps every TRIS range and animation as it is."""

import sys
from pathlib import Path

import numpy as np

from make_hud_hardware import PANEL_HALF, PANEL_X, PANEL_Y, STRIP_NORMAL, STRIP_POINT

DECALS = [f"a380_cockpit_{n:03d}.obj" for n in range(20, 26)]
MARGIN = 0.006
DEPTH = 0.012


def footprints():
    n = STRIP_NORMAL / np.linalg.norm(STRIP_NORMAL)
    v = np.cross(n, [1.0, 0.0, 0.0])
    v /= np.linalg.norm(v)
    out = []
    for x in (PANEL_X, -PANEL_X):
        z = STRIP_POINT[2] - n[1] * (PANEL_Y - STRIP_POINT[1]) / n[2]
        out.append((np.array([x, PANEL_Y, z]), v, n))
    return out


def inside(c):
    hit = np.zeros(len(c), bool)
    for centre, v, n in footprints():
        d = c - centre
        hit |= (np.abs(d[:, 0]) < PANEL_HALF[0] + MARGIN) & (np.abs(d @ v) < PANEL_HALF[1] + MARGIN) & (np.abs(d @ n) < DEPTH)
    return hit


def process(path, dry):
    lines = path.read_text(encoding="latin-1").split("\n")
    vt = np.array([[float(x) for x in l.split()[1:4]] for l in lines if l.startswith("VT")])
    rows = [i for i, l in enumerate(lines) if l.startswith(("IDX10", "IDX "))]
    idx = np.array([int(x) for i in rows for x in lines[i].split()[1:]])
    tri = idx[: len(idx) // 3 * 3].reshape(-1, 3)
    hit = inside(vt[tri].mean(axis=1)) & (tri[:, 0] != tri[:, 1])
    if hit.any() and not dry:
        tri[hit] = tri[hit][:, :1]
        flat = tri.ravel()
        k = 0
        for i in rows:
            n = len(lines[i].split()) - 1
            lines[i] = ("IDX10 " if lines[i].startswith("IDX10") else "IDX ") + " ".join(map(str, flat[k:k + n]))
            k += n
        path.write_text("\n".join(lines), encoding="latin-1", newline="\n")
    return int(hit.sum())


def main(objects, dry=False):
    for n in DECALS:
        print(n, process(Path(objects) / n, dry), "triangles under the HUD panels" + (" (dry run)" if dry else " collapsed"))


if __name__ == "__main__":
    main(sys.argv[1], "--dry-run" in sys.argv)
