"""usage: gp7200_pylon.py <objects dir> [--s1 S] [--s2 S] [--a1 A] [--a2 A] [--check-only]

Seats the nose of the lower fairing of every GP7200 pylon on the nacelle crown with one smooth vertical displacement field per engine, dz(a, s), a function
of the lateral position a and the station s in the engine frame only. The profile along s is a degree-5 polynomial fit of the crown clearance of the fairing's
centreline underside (so the centreline lands EPS above the crown from the nose to s1; the profile only ever lowers, so a nose that sits inside the nacelle stays hidden there) eased back to zero by s2; the lateral weight is one
across |a| < a1 and eases to zero by a2. Every vertex of a380_exterior_031_gp.obj and of the twelve a380_engdetach_<e>_a380_exterior_031_gp
[_burnt|_crushed].obj copies is moved by the same field wherever it lies, so shared edges stay shared. Only the VT position and normal fields
change; every other line is kept byte for byte. Run it on the stage after the chain, which restores 031 from the install.
The module refuses to write a field that folds triangles (solid triangles flipping more than 60 deg or losing more than 80 % of their area; slivers thinner than 1 cm,
whose normal is meaningless, are counted apart). A ray test (gp7200_exposure) checks that no view opens into the airframe, dressing or core behind the pylon; it also refuses a field that does. New triangle crossings are counted and reported with the largest relative shift that caused them."""

import json
import re
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gp7200_geom import frames, rb, rt  # noqa: E402
from gp7200_obj import Obj, engine_of_batch  # noqa: E402

FN = frames()
EPS = 0.01
S_FIT = (0.3, 3.4)
S_CLAMP = 0.3
DEFAULT = dict(s1=1.6, s2=3.0, a1=0.7, a2=1.2, deg=5)
WINDOW_A, WINDOW_S = 1.2, (0.0, 6.2)
MAX_FLIP_DEG, MAX_AREA_LOSS = 60.0, 0.8
SLIVER_ALTITUDE, THROUGH = 0.01, 0.02


def crown(a, s):
    s = np.asarray(s, float)
    top, bot = rt(s), rb(s)
    y0 = (top - bot) / 2
    r = top - y0
    return y0 + np.sqrt(np.maximum(r * r - np.asarray(a, float) ** 2, 0.0))


def smooth(x, a, b):
    t = np.clip((np.asarray(x, float) - a) / (b - a), 0, 1)
    return t * t * (3 - 2 * t)


def engine_vertices(obj, e):
    rows = []
    for b in obj.tris():
        if engine_of_batch(b) == e - 1:
            rows.append(np.unique(obj.idx[b["start"]:b["start"] + b["count"]]))
    return np.unique(np.concatenate(rows))


class Field:
    def __init__(self, e, ref_world, s1, s2, a1, a2, deg):
        self.e, self.f = e, FN[e]
        self.s1, self.s2, self.a1, self.a2 = s1, s2, a1, a2
        loc = self.f.local(ref_world)
        c = loc[:, 1] - crown(loc[:, 0], loc[:, 2])
        m = (np.abs(loc[:, 0]) < 0.12) & (loc[:, 1] > 1.0) & (loc[:, 2] >= S_FIT[0]) & (loc[:, 2] <= S_FIT[1])
        s, c = loc[m, 2], c[m]
        bins = np.round(s / 0.1).astype(int)
        xs, ys = [], []
        for b in np.unique(bins):
            k = bins == b
            xs.append(s[k].mean())
            ys.append(c[k].min())
        self.samples = np.array([xs, ys])
        self.poly = np.polynomial.Polynomial.fit(xs, ys, int(deg))

    def profile(self, s):
        s = np.maximum(np.asarray(s, float), S_CLAMP)
        want = (EPS - self.poly(np.minimum(s, S_FIT[1]))) * (1 - smooth(s, self.s1, self.s2))
        return 0.5 * (want - np.sqrt(want * want + 0.02 ** 2) + 0.02)

    def dz(self, world):
        loc = self.f.local(world)
        a, s = loc[:, 0], loc[:, 2]
        inside = (np.abs(a) < WINDOW_A) & (s > WINDOW_S[0]) & (s < WINDOW_S[1])
        w = 1 - smooth(np.abs(a), self.a1, self.a2)
        return np.where(inside, self.profile(s) * w, 0.0)

    def gradient(self, world, h=0.02):
        g = np.zeros_like(world)
        for axis in (self.f.side, self.f.up, self.f.d):
            d = (self.dz(world + axis * h) - self.dz(world - axis * h)) / (2 * h)
            g += d[:, None] * axis[None, :]
        return g

    def move(self, world, normals):
        d = self.dz(world)
        up = self.f.up
        g = self.gradient(world)
        n = normals - g * (normals @ up)[:, None] / (1 + g @ up)[:, None]
        n = n / np.linalg.norm(n, axis=1, keepdims=True)
        return world + d[:, None] * up[None, :], n


def tri_normals(p, t):
    a, b, c = p[t[:, 0]], p[t[:, 1]], p[t[:, 2]]
    n = np.cross(b - a, c - a)
    return n, np.linalg.norm(n, axis=1)


def seg_tri(p0, p1, a, b, c):
    """Proper segment/triangle crossings, vectorised over the leading axis."""
    d = p1 - p0
    e1, e2 = b - a, c - a
    h = np.cross(d, e2)
    det = np.einsum("ij,ij->i", e1, h)
    ok = np.abs(det) > 1e-12
    inv = np.where(ok, 1.0 / np.where(ok, det, 1.0), 0.0)
    s = p0 - a
    u = np.einsum("ij,ij->i", s, h) * inv
    q = np.cross(s, e1)
    v = np.einsum("ij,ij->i", d, q) * inv
    t = np.einsum("ij,ij->i", e2, q) * inv
    m = 1e-6
    return ok & (u > m) & (v > m) & (u + v < 1 - m) & (t > m) & (t < 1 - m)


def tri_tri(P, T, i, j):
    A, B = P[T[i]], P[T[j]]
    hit = np.zeros(len(i), bool)
    for k in range(3):
        hit |= seg_tri(A[:, k], A[:, (k + 1) % 3], B[:, 0], B[:, 1], B[:, 2])
        hit |= seg_tri(B[:, k], B[:, (k + 1) % 3], A[:, 0], A[:, 1], A[:, 2])
    return hit


def crossing_pairs(P0, P1, T, key, moved_tris):
    """Triangle pairs (at least one moved, none sharing a position) that cross in the original (P0) and in the moved (P1) mesh, with the
    relative shift of the two triangles of every pair that crosses only after the move: such a pair was clear of the other by no more than that shift
    (a joint that was touching or nearly touching), unlike one part passing through another."""
    tl = lambda P: (np.minimum(np.minimum(P[T[:, 0]], P[T[:, 1]]), P[T[:, 2]]), np.maximum(np.maximum(P[T[:, 0]], P[T[:, 1]]), P[T[:, 2]]))
    l0, h0 = tl(P0)
    l1, h1 = tl(P1)
    lo, hi = np.minimum(l0, l1), np.maximum(h0, h1)
    everyone = np.arange(len(T))
    mv = np.nonzero(moved_tris)[0]
    pairs = []
    for s0 in range(0, len(mv), 100):
        chunk = mv[s0:s0 + 100]
        ov = np.all((lo[chunk][:, None, :] <= hi[None, :, :]) & (hi[chunk][:, None, :] >= lo[None, :, :]), axis=2)
        ci, cj = np.nonzero(ov)
        i, j = chunk[ci], everyone[cj]
        keep = (i != j) & (~moved_tris[j] | (i < j))
        pairs.append(np.stack([i[keep], j[keep]], axis=1))
    pairs = np.concatenate(pairs) if pairs else np.zeros((0, 2), int)
    ki, kj = key[T[pairs[:, 0]]], key[T[pairs[:, 1]]]
    share = np.zeros(len(pairs), bool)
    for x in range(3):
        for y in range(3):
            share |= ki[:, x] == kj[:, y]
    pairs = pairs[~share]
    c0 = tri_tri(P0, T, pairs[:, 0], pairs[:, 1])
    c1 = tri_tri(P1, T, pairs[:, 0], pairs[:, 1])
    fresh = pairs[c1 & ~c0]
    D = (P1[T] - P0[T]).mean(axis=1)
    depth = np.linalg.norm(D[fresh[:, 0]] - D[fresh[:, 1]], axis=1) if len(fresh) else np.zeros(0)
    return int(c0.sum()), int(c1.sum()), fresh, depth, len(pairs)


def fold_report(P0, P1, T, key):
    n0, a0 = tri_normals(P0, T)
    n1, a1 = tri_normals(P1, T)
    longest = np.max(np.linalg.norm(P0[T] - np.roll(P0[T], 1, axis=1), axis=2), axis=1)
    altitude = 2 * a0 / np.maximum(longest, 1e-12)
    moved = np.any(P0[T] != P1[T], axis=(1, 2))
    solid = moved & (altitude >= SLIVER_ALTITUDE)
    sliver = moved & ~solid & (a0 > 1e-9)
    cosang = np.einsum("ij,ij->i", n0, n1) / np.maximum(a0 * a1, 1e-30)
    ang = np.degrees(np.arccos(np.clip(cosang, -1, 1)))
    flips = solid & (ang > MAX_FLIP_DEG)
    loss = solid & (a1 < (1 - MAX_AREA_LOSS) * a0)
    nb, na, fresh, depth, ncand = crossing_pairs(P0, P1, T, key, moved)
    return dict(moved=int(moved.sum()), solid=int(solid.sum()), flips=int(flips.sum()), area_loss=int(loss.sum()), slivers=int(sliver.sum()),
                sliver_flips=int((sliver & (ang > MAX_FLIP_DEG)).sum()), cross_before=nb, cross_after=na, new_cross=len(fresh),
                new_cross_through=int((depth > THROUGH).sum()), max_new_shift_mm=round(float(depth.max()) * 1000, 1) if len(depth) else 0.0,
                max_flip=round(float(ang[solid].max()), 1) if solid.any() else 0.0,
                min_area_ratio=round(float((a1[solid] / a0[solid]).min()), 3) if solid.any() else 1.0, candidates=ncand)


def position_keys(P, tol=1e-5):
    _, inv = np.unique(np.round(P / tol).astype(np.int64), axis=0, return_inverse=True)
    return inv.reshape(-1)


def build_fields(obj, params):
    fields = {}
    for e in (1, 2, 3, 4):
        verts = engine_vertices(obj, e)
        fields[e] = Field(e, obj.vt[verts, :3], params["s1"], params["s2"], params["a1"], params["a2"], params["deg"])
    return fields


def apply(fields, P, N, centre=None):
    """Applies every engine's field to all points; the fields live in disjoint windows."""
    off = np.zeros(3) if centre is None else np.asarray(centre)
    world = P + off
    newP, newN = P.copy(), N.copy()
    for fld in fields.values():
        mv = fld.dz(world) != 0
        if mv.any():
            pw, nn = fld.move(world[mv], N[mv])
            newP[mv] = pw - off
            newN[mv] = nn
    return newP, newN


def gap_table(fields, P, e):
    loc = fields[e].f.local(P)
    c = loc[:, 1] - crown(loc[:, 0], loc[:, 2])
    rows = []
    for s in (0.6, 1.2, 1.8, 2.5, 3.2, 3.9, 4.5):
        m = (np.abs(loc[:, 0]) < 0.12) & (np.abs(loc[:, 2] - s) < 0.1) & (loc[:, 1] > 1.0)
        rows.append(round(float(c[m].min()), 3) if m.any() else None)
    return rows


VT = re.compile(r"^VT\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)(\s.*)$")


def decimals(tok):
    return len(tok.split(".")[1]) if "." in tok else 0


def parse(lines):
    rows = [i for i, l in enumerate(lines) if l.startswith("VT")]
    toks = [VT.match(lines[i]) for i in rows]
    data = np.array([[float(t.group(k)) for k in range(1, 7)] for t in toks])
    return rows, toks, data


def write_rows(lines, rows, toks, new_pos, new_nrm, moved):
    for k in np.nonzero(moved)[0]:
        m = toks[k]
        pos = [f"{v:.{max(decimals(m.group(j + 1)), 4)}f}" for j, v in enumerate(new_pos[k])]
        nrm = [f"{v:.{max(decimals(m.group(j + 4)), 4)}f}" for j, v in enumerate(new_nrm[k])]
        lines[rows[k]] = "VT " + " ".join(pos + nrm) + m.group(7)


def check_file(path, fields, centre=None):
    obj = Obj(path)
    T = obj.idx.reshape(-1, 3)
    P0 = obj.vt[:, :3]
    P1, _ = apply(fields, P0, obj.vt[:, 3:6], centre)
    return fold_report(P0, P1, T, position_keys(P0)), P0, P1


def rewrite(path, fields, centre=None):
    lines = Path(path).read_text(encoding="utf-8").split("\n")
    rows, toks, data = parse(lines)
    P1, N1 = apply(fields, data[:, :3], data[:, 3:6], centre)
    moved = np.any(P1 != data[:, :3], axis=1)
    write_rows(lines, rows, toks, P1, N1, moved)
    Path(path).write_text("\n".join(lines), encoding="utf-8", newline="")
    return int(moved.sum()), float(np.abs(P1 - data[:, :3]).max())


def main(objects, params=None, check_only=False):
    params = {**DEFAULT, **(params or {})}
    objects = Path(objects)
    main_path = objects / "a380_exterior_031_gp.obj"
    obj = Obj(main_path)
    fields = build_fields(obj, params)
    manifest = json.loads((objects / "a380_engdetach.json").read_text(encoding="utf-8"))
    report = {"params": params, "files": {}}
    rep, P0, P1 = check_file(main_path, fields)
    report["files"][main_path.name] = rep
    report["gap_before"] = {e: gap_table(fields, P0, e) for e in fields}
    report["gap_after"] = {e: gap_table(fields, P1, e) for e in fields}
    report["fit_rms"] = {e: round(float(np.sqrt(np.mean((fields[e].poly(fields[e].samples[0]) - fields[e].samples[1]) ** 2))), 4) for e in fields}
    import gp7200_exposure as expo
    report["exposure"] = {}
    for e in fields:
        scene = expo.Scene(objects, e, n_points=200, pylon=main_path)
        r = scene.exposed(P1)
        report["exposure"][e] = {k: v for k, v in r.items() if k != "where"}
    detach = []
    for e in fields:
        centre = manifest["gp"]["engines"][str(e - 1)]["centre"]
        for suffix in ("", "_burnt", "_crushed"):
            p = objects / f"a380_engdetach_{e - 1}_a380_exterior_031_gp{suffix}.obj"
            if p.is_file():
                detach.append((p, centre))
    for p, centre in detach:
        report["files"][p.name] = check_file(p, fields, centre)[0]
    bad = {n: r for n, r in report["files"].items() if r["flips"] or r["area_loss"]}
    bad.update({f"exposure_{e}": r for e, r in report["exposure"].items() if r["opened_to_other"]})
    report["accepted"] = not bad
    if check_only or bad:
        return report
    rewrite(main_path, fields)
    for p, centre in detach:
        rewrite(p, fields, centre)
    return report


if __name__ == "__main__":
    a = sys.argv[1:]
    params = {k: float(a[a.index("--" + k) + 1]) for k in ("s1", "s2", "a1", "a2") if "--" + k in a}
    print(json.dumps(main(a[0], params, "--check-only" in a), indent=1, default=str))
