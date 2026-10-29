"""usage: pylon_stub.py <objects dir> <base a380_sep_scars.obj> <out a380_sep_scars.obj>

The torn root of each pylon left hanging under the wing once its engine has
separated: side and end walls from just inside the lower skin down to a torn
edge, closed underneath, along the pylon's footprint on the skin. Appended to
the scar object and shown only while sim/operation/failures/rel_engsep<n> is
failed."""

import sys
from pathlib import Path

import numpy as np

import engine_detach as ed
from separation_scars import CAP, COVE, uv

SKIP = ("_gp", "_hail", "engdetach", "partdetach", "sep_scars", "cowldetach")
BIN = 0.25
WALL = 0.3
INTO = 0.03
MARGIN = 0.03


def load(objects_dir):
    gated, rest = {}, []
    for path in sorted(Path(objects_dir).glob("a380_exterior_*.obj")):
        if any(k in path.stem for k in SKIP):
            continue
        lines, vt, idx = ed.read(path)
        if not vt:
            continue
        v = np.array([[float(x) for x in row[:3]] for row in vt])
        i = np.array(idx)
        mask = np.zeros(len(i), bool)
        for e, ranges in ed.engine_tris(lines).items():
            for s, c, _ in ranges:
                mask[s:s + c] = True
                gated.setdefault(e, []).append(v[i[s:s + c]])
        rest.append(v[i[~mask]].reshape(-1, 3, 3))
    return {e: np.concatenate(p) for e, p in gated.items()}, np.concatenate(rest)


def hits(tris, x, z, lo, hi):
    a, b, c = tris[:, 0], tris[:, 1], tris[:, 2]

    def side(p, q):
        return (q[:, 0] - p[:, 0]) * (z - p[:, 2]) - (q[:, 2] - p[:, 2]) * (x - p[:, 0])
    s1, s2, s3 = side(a, b), side(b, c), side(c, a)
    inside = ((s1 >= 0) & (s2 >= 0) & (s3 >= 0)) | ((s1 <= 0) & (s2 <= 0) & (s3 <= 0))
    ys = []
    for t in tris[inside]:
        n = np.cross(t[1] - t[0], t[2] - t[0])
        if abs(n[1]) < 1e-9:
            continue
        y = t[0][1] - (n[0] * (x - t[0][0]) + n[2] * (z - t[0][2])) / n[1]
        if lo < y < hi:
            ys.append(round(float(y), 3))
    return sorted(set(ys))


def rows(pts, tris):
    near = tris[(tris[:, :, 0].max(1) > pts[:, 0].min() - 1) & (tris[:, :, 0].min(1) < pts[:, 0].max() + 1)]
    out = []
    for z in np.arange(pts[:, 2].min(), pts[:, 2].max() + BIN, BIN):
        band = pts[np.abs(pts[:, 2] - z) <= BIN / 2]
        if len(band) < 3:
            continue
        top = band[:, 1].max()
        lo, hi = top - 0.6, top + 2.5
        centre = hits(near, float(np.median(band[:, 0])), z, lo, hi)
        if not centre or top < centre[0] - 0.35:
            continue
        sel = band[band[:, 1] > centre[0] - 0.2]
        if len(sel) < 2:
            continue
        x0, x1 = sel[:, 0].min() - MARGIN, sel[:, 0].max() + MARGIN
        if x1 - x0 < 0.25:
            continue
        e0, e1 = hits(near, x0 + 0.02, z, lo, hi), hits(near, x1 - 0.02, z, lo, hi)
        y0 = e0[0] if e0 else centre[0]
        y1 = e1[0] if e1 else centre[0]
        out.append((z, x0, x1, y0, y1))
    return out


def quad(q, n):
    n = np.array(n, float)
    return [(q[0], q[1], q[2], n), (q[0], q[2], q[3], n), (q[0], q[2], q[1], -n), (q[0], q[3], q[2], -n)]


def stub(pts, tris):
    rs = rows(pts, tris)
    if len(rs) < 2:
        return []
    out = []
    span = max(1e-6, rs[-1][0] - rs[0][0])
    top = lambda x, y, z: np.array((x, y + INTO, z))
    bot = lambda x, y, z: np.array((x, y - WALL, z))
    for (z1, a1, b1, ya1, yb1), (z2, a2, b2, ya2, yb2) in zip(rs, rs[1:]):
        u1, u2 = (z1 - rs[0][0]) / span * 4, (z2 - rs[0][0]) / span * 4
        for xa, ya, xb, yb, sgn in ((a1, ya1, a2, ya2, -1.0), (b1, yb1, b2, yb2, 1.0)):
            out += quad([(top(xa, ya, z1), uv(COVE, u1, 0.0)), (top(xb, yb, z2), uv(COVE, u2, 0.0)), (bot(xb, yb, z2), uv(COVE, u2, 1.0)), (bot(xa, ya, z1), uv(COVE, u1, 1.0))], (sgn, 0.0, 0.0))
        w = max(b1 - a1, b2 - a2)
        out += quad([(bot(a1, ya1, z1), uv(CAP, 0, u1 / 4)), (bot(b1, yb1, z1), uv(CAP, (b1 - a1) / w, u1 / 4)), (bot(b2, yb2, z2), uv(CAP, (b2 - a2) / w, u2 / 4)), (bot(a2, ya2, z2), uv(CAP, 0, u2 / 4))], (0.0, -1.0, 0.0))
    for (z, a, b, ya, yb), sgn in ((rs[0], -1.0), (rs[-1], 1.0)):
        out += quad([(top(a, ya, z), uv(COVE, 0, 0.0)), (top(b, yb, z), uv(COVE, 1, 0.0)), (bot(b, yb, z), uv(COVE, 1, 1.0)), (bot(a, ya, z), uv(COVE, 0, 1.0))], (0.0, 0.0, sgn))
    return out


def read_scars(path):
    lines = Path(path).read_text(encoding="utf-8").split("\n")
    vts = [tuple(float(v) for v in l.split()[1:9]) for l in lines if l.startswith("VT ")]
    idx = [int(v) for l in lines if l.startswith(("IDX ", "IDX10")) for v in l.split()[1:]]
    head = [l for l in lines if l.startswith(("I", "800", "OBJ", "TEXTURE"))][:4]
    at = lines.index("ATTR_no_blend 0.5")
    cmds = [l for l in lines[at + 1:] if l.strip()]
    return head, vts, idx, cmds


def main(objects_dir, base, out):
    head, vts, idx, cmds = read_scars(base)
    gated, rest = load(objects_dir)
    for e, pts in sorted(gated.items()):
        tris = stub(pts, rest)
        start = len(idx)
        for t in tris:
            (p0, uv0), (p1, uv1), (p2, uv2), n = t
            n = np.array(n, float)
            n = n / (np.linalg.norm(n) or 1.0)
            for p, (u, v) in ((p0, uv0), (p1, uv1), (p2, uv2)):
                idx.append(len(vts))
                vts.append((p[0], p[1], p[2], n[0], n[1], n[2], u, v))
        x = float(np.mean([v[0] for v in vts[idx[start]:]]))
        section = int(np.searchsorted([6.5, 11.52, 19.5, 31.5], abs(x)))
        wing = [f"ANIM_hide 5.5 6.5 sim/operation/failures/rel_wing{section}{'L' if x < 0 else 'R'}"] if 1 <= section <= 3 else []
        cmds += ["ANIM_begin", f"ANIM_hide -1 5.5 sim/operation/failures/rel_engsep{e}", *wing, f"TRIS {start} {len(idx) - start}", "ANIM_end"]
        print(f"engine {e + 1}: {len(tris)} stub triangles")
    lines = head + ["", f"POINT_COUNTS {len(vts)} 0 0 {len(idx)}", ""]
    lines += [f"VT {x:.4f} {y:.4f} {z:.4f} {a:.4f} {b:.4f} {c:.4f} {u:.5f} {v:.5f}" for x, y, z, a, b, c, u, v in vts]
    lines += [f"IDX {i}" for i in idx]
    lines += ["", "ATTR_no_blend 0.5"] + cmds
    Path(out).write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main(*sys.argv[1:4])
