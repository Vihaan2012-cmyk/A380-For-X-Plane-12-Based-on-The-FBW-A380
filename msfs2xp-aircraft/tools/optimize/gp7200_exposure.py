"""Occlusion check for the pylon seating: rays from cameras around the engine to points on the original pylon skin. A ray is exposed when, with the
moved pylon, the first surface it meets is neither the pylon nor the nacelle skin but another part (airframe, dressing, core) or nothing at all, i.e.
the move opened a view into the pylon or through it. Only the geometry of a380_exterior_031_gp.obj moves; the other objects are taken as they are in the
stage."""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gp7200_geom import frames  # noqa: E402
from gp7200_obj import Obj, engine_of_batch  # noqa: E402

FN = frames()
STOWED_AWAY = "a380_engcore_gp.obj"
OTHERS = ("a380_exterior_001_gp.obj", "a380_engdress_gp.obj", "a380_engcore_gp.obj", "a380_exterior_020_gp.obj", "a380_exterior_030_gp.obj")
SKIN = {1: "a380_exterior_022_gp.obj", 2: "a380_exterior_022_gp.obj", 3: "a380_exterior_023_gp.obj", 4: "a380_exterior_023_gp.obj"}


def window_triangles(obj, f, a_lim=1.3, s_lim=(-0.3, 6.0), b_lim=(1.2, 3.4), engine=None):
    P = obj.vt[:, :3]
    T = obj.idx.reshape(-1, 3)
    keep = np.full(len(T), engine is None)
    for b in obj.tris():
        if engine is not None and engine_of_batch(b) == engine - 1:
            keep[b["start"] // 3:(b["start"] + b["count"]) // 3] = True
        if obj.path.name == STOWED_AWAY and any("thrust_rev" in t for t in b["anim"]):
            keep[b["start"] // 3:(b["start"] + b["count"]) // 3] = False
    T = T[keep]
    loc = f.local(P)
    lo = np.minimum(np.minimum(loc[T[:, 0]], loc[T[:, 1]]), loc[T[:, 2]])
    hi = np.maximum(np.maximum(loc[T[:, 0]], loc[T[:, 1]]), loc[T[:, 2]])
    ok = (hi[:, 0] > -a_lim) & (lo[:, 0] < a_lim) & (hi[:, 2] > s_lim[0]) & (lo[:, 2] < s_lim[1]) & (hi[:, 1] > b_lim[0]) & (lo[:, 1] < b_lim[1])
    return T[ok]


def ray_hits(origins, dirs, tri):
    """Smallest positive t per ray against triangles tri (n, 3, 3); inf when none."""
    best = np.full(len(origins), np.inf)
    a, b, c = tri[:, 0], tri[:, 1], tri[:, 2]
    e1, e2 = b - a, c - a
    for s0 in range(0, len(origins), 16):
        o, d = origins[s0:s0 + 16], dirs[s0:s0 + 16]
        h = np.cross(d[:, None, :], e2[None, :, :])
        det = np.einsum("tk,rtk->rt", e1, h)
        ok = np.abs(det) > 1e-12
        inv = np.where(ok, 1.0 / np.where(ok, det, 1.0), 0.0)
        s = o[:, None, :] - a[None, :, :]
        u = np.einsum("rtk,rtk->rt", s, h) * inv
        q = np.cross(s, e1[None, :, :])
        v = np.einsum("rk,rtk->rt", d, q) * inv
        t = np.einsum("tk,rtk->rt", e2, q) * inv
        hit = ok & (u >= 0) & (v >= 0) & (u + v <= 1) & (t > 1e-4)
        best[s0:s0 + 16] = np.where(hit, t, np.inf).min(axis=1)
    return best


class Scene:
    def __init__(self, objects, engine, n_points=500, seed=3, pylon=None):
        objects = Path(objects)
        self.e, self.f = engine, FN[engine]
        self.pyl = Obj(pylon or objects / "a380_exterior_031_gp.obj")
        self.pyl_tris = window_triangles(self.pyl, self.f, engine=engine)
        self.skin = self._tris(Obj(objects / SKIN[engine]), engine)
        self.others = np.concatenate([self._tris(Obj(objects / n), None) for n in OTHERS if (objects / n).is_file()])
        rng = np.random.default_rng(seed)
        self.points = self._sample(n_points, rng)
        self.cams = self._cameras()

    def _tris(self, obj, engine):
        T = window_triangles(obj, self.f, engine=engine)
        return obj.vt[:, :3][T]

    def _sample(self, n, rng):
        P = self.pyl.vt[:, :3]
        tri = P[self.pyl_tris]
        loc = self.f.local(tri.reshape(-1, 3)).reshape(-1, 3, 3)
        ok = (loc[:, :, 2].max(axis=1) < 5.4) & (loc[:, :, 2].min(axis=1) > 0.1) & (np.abs(loc[:, :, 0]).max(axis=1) < 1.0) & (loc[:, :, 1].min(axis=1) > 0.8)
        tri = tri[ok]
        area = 0.5 * np.linalg.norm(np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0]), axis=1)
        pick = rng.choice(len(tri), n, p=area / area.sum())
        r = rng.random((n, 2))
        m = r.sum(1) > 1
        r[m] = 1 - r[m]
        t = tri[pick]
        return t[:, 0] + r[:, :1] * (t[:, 1] - t[:, 0]) + r[:, 1:] * (t[:, 2] - t[:, 0])

    def _cameras(self):
        side = -1.0 if self.e in (1, 2) else 1.0
        out = []
        for az in np.radians(np.arange(-80, 81, 20)):
            for el in np.radians((-10, 10, 30, 50)):
                for r in (7.0,):
                    a = side * r * np.cos(el) * np.cos(az)
                    s = 2.6 + r * np.cos(el) * np.sin(az)
                    b = 1.9 + r * np.sin(el)
                    out.append(self.f.point(np.array([a, b, s])))
        return np.array(out)

    def exposed(self, pyl_vt, label=""):
        P = self.pyl.vt[:, :3].copy() if pyl_vt is None else pyl_vt
        tri = P[self.pyl_tris]
        n_c, n_p = len(self.cams), len(self.points)
        o = np.repeat(self.cams, n_p, axis=0)
        tgt = np.tile(self.points, (n_c, 1))
        d = tgt - o
        dist = np.linalg.norm(d, axis=1)
        d = d / dist[:, None]
        t_pyl0 = ray_hits(o, d, self.pyl.vt[:, :3][self.pyl_tris])
        seen = np.abs(t_pyl0 - dist) < 2e-3
        t_skin0 = ray_hits(o, d, self.skin)
        t_oth0 = ray_hits(o, d, self.others)
        seen &= (t_skin0 > dist - 2e-3) & (t_oth0 > dist - 2e-3)
        idx = np.nonzero(seen)[0]
        o, d = o[idx], d[idx]
        t_pyl = ray_hits(o, d, tri)
        t_skin = ray_hits(o, d, self.skin)
        t_oth = ray_hits(o, d, self.others)
        first = np.minimum(t_pyl, t_skin)
        opened = t_oth < first
        see_through = ~np.isfinite(np.minimum(first, t_oth))
        sky = see_through | ~np.isfinite(first)
        enclosed = np.zeros(len(idx), bool)
        k = np.nonzero(sky)[0]
        if len(k):
            tgt_up = tgt[idx[k]] + self.f.up * 0.4
            o2 = o[k]
            d2 = tgt_up - o2
            dist2 = np.linalg.norm(d2, axis=1)
            d2 = d2 / dist2[:, None]
            h = np.minimum(np.minimum(ray_hits(o2, d2, tri), ray_hits(o2, d2, self.skin)), ray_hits(o2, d2, self.others))
            enclosed[k] = np.isfinite(h)
        bad = opened | enclosed
        return dict(rays=int(len(idx)), opened_to_other=int(opened.sum()), open_sky=int(sky.sum()), enclosed_gap=int(enclosed.sum()),
                    where=self.f.local(self.points[idx[bad] % n_p]) if bad.any() else np.zeros((0, 3)))
