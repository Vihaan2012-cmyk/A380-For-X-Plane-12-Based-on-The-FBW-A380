import json
import math
from pathlib import Path

import numpy as np
from scipy.spatial import cKDTree

SPEC = json.loads(Path(__file__).with_name("gp7200_spec.json").read_text(encoding="utf-8"))
ENGINES = (1, 2, 3, 4)


def unit(v):
    v = np.asarray(v, float)
    return v / np.linalg.norm(v, axis=-1, keepdims=True)


def smooth(x, a, b):
    t = np.clip((np.asarray(x, float) - a) / (b - a), 0, 1)
    return t * t * (3 - 2 * t)


class Frame:
    """Engine-local coordinates (a, b, s): a toward +x, b up, s aft along the axis; origin on the axis at the spinner tip."""

    def __init__(self, origin, d):
        self.o = np.asarray(origin, float)
        self.d = unit(d)
        up = np.array([0.0, 1.0, 0.0]) - self.d[1] * self.d
        self.up = unit(up)
        self.side = np.cross(self.up, self.d)

    def point(self, p):
        p = np.asarray(p, float)
        return self.o + p[..., 0:1] * self.side + p[..., 1:2] * self.up + p[..., 2:3] * self.d

    def vec(self, v):
        v = np.asarray(v, float)
        return v[..., 0:1] * self.side + v[..., 1:2] * self.up + v[..., 2:3] * self.d

    def local(self, w):
        q = np.asarray(w, float) - self.o
        return np.stack([q @ self.side, q @ self.up, q @ self.d], axis=-1)

    def local_vec(self, w):
        w = np.asarray(w, float)
        return np.stack([w @ self.side, w @ self.up, w @ self.d], axis=-1)


def frames(version="recommended"):
    out = {}
    for e in ENGINES:
        r = SPEC["engine_positions"]["per_engine"][str(e)][version]
        out[e] = Frame(r["axis_point_at_spinner_tip"], r["axis_direction_unit"])
    return out


OLD_AXIS = {1: (0.009164, 0.034294, -0.99937), 2: (0.0, 0.010104, -0.999949), 3: (0.0, 0.010104, -0.999949), 4: (-0.000783, 0.034294, -0.999411)}


def old_frames():
    out = {}
    pe = SPEC["engine_positions"]["per_engine"]
    for e in ENGINES:
        m = pe[str(e)]["current_gp_morph"]
        d = -unit(OLD_AXIS[e])
        c = np.array(m["fan_disc_centre"])
        out[e] = Frame(c - d * ((c[2] - m["spinner_tip_z"]) / d[2]), d)
    return out


def table(rows):
    a = np.array(rows, float)
    return a[:, 0], a[:, 1]


_TOP = table(SPEC["outer_nacelle"]["top"])
_BOT = table(SPEC["outer_nacelle"]["bottom"])
HL_TOP, HL_BOT = SPEC["inlet"]["highlight"]["top_s"], SPEC["inlet"]["highlight"]["bottom_s"]
HL_R = SPEC["inlet"]["highlight"]["mean_r"]
HL_Y0 = -SPEC["inlet"]["highlight"]["centre_below_axis"]
S_NOSE = SPEC["key_stations"]["nose_cowl_fan_cowl_split"]
S_TR = SPEC["key_stations"]["fan_cowl_tr_split"]
S_TE = SPEC["key_stations"]["fan_nozzle_exit"]
S_PLUG = SPEC["key_stations"]["plug_tip"]
S_LIP_END = -0.55
BAND = 0.20


def rt(s):
    return np.interp(s, *_TOP)


def rb(s):
    return np.interp(s, *_BOT)


def y0(s):
    s = np.asarray(s, float)
    body = (rt(np.maximum(s, -0.9)) - rb(np.maximum(s, -0.9))) / 2
    ext = body + (HL_Y0 - body) * smooth(-s, 0.9, 1.0725)
    return np.where(s >= -0.9, body, ext)


def rsec(s):
    s = np.asarray(s, float)
    body = (rt(np.maximum(s, -0.9)) + rb(np.maximum(s, -0.9))) / 2
    lip = rt(np.minimum(s, -0.9)) - y0(s)
    return np.where(s >= -0.9, body, lip)


def wscarf(phi):
    return (1 - np.cos(phi)) / 2


def s_highlight(phi):
    return HL_TOP + (HL_BOT - HL_TOP) * wscarf(phi)


def shear(sp, phi):
    sh = s_highlight(phi)
    return np.where(sp < S_LIP_END, sh + (sp - HL_TOP) * (S_LIP_END - sh) / (S_LIP_END - HL_TOP), sp)


def wind(P, T, N, flip=False):
    g = np.cross(P[T[:, 1]] - P[T[:, 0]], P[T[:, 2]] - P[T[:, 0]])
    n = N[T[:, 0]] + N[T[:, 1]] + N[T[:, 2]]
    bad = (np.einsum("ij,ij->i", g, n) > 0) != flip
    T = T.copy()
    T[bad] = T[bad][:, [0, 2, 1]]
    return T


class Mesh:
    def __init__(self):
        self.P = np.zeros((0, 3))
        self.N = np.zeros((0, 3))
        self.UV = np.zeros((0, 2))
        self.T = np.zeros((0, 3), np.int64)
        self.tag = []

    def add(self, P, N, UV, T, tag=""):
        base = len(self.P)
        self.P = np.vstack([self.P, P.reshape(-1, 3)])
        self.N = np.vstack([self.N, N.reshape(-1, 3)])
        self.UV = np.vstack([self.UV, UV.reshape(-1, 2)])
        T = np.asarray(T, np.int64) + base
        self.T = np.vstack([self.T, T])
        self.tag += [tag] * len(T)
        return len(T)

    def extend(self, other):
        self.add(other.P, other.N, other.UV, other.T, "")
        self.tag[-len(other.T):] = other.tag if len(other.tag) == len(other.T) else [""] * len(other.T)

    def tri_count(self):
        return len(self.T)

    def transform(self, fn_point, fn_vec):
        m = Mesh()
        m.P, m.N, m.UV, m.T, m.tag = fn_point(self.P), unit(fn_vec(self.N)), self.UV.copy(), self.T.copy(), list(self.tag)
        return m


def grid_tris(K, J, closed):
    jj = J if closed else J - 1
    T = []
    for k in range(K - 1):
        for j in range(jj):
            a, b = k * J + j, k * J + (j + 1) % J
            c, d = (k + 1) * J + j, (k + 1) * J + (j + 1) % J
            T += [(a, c, b), (b, c, d)]
    return np.array(T, np.int64)


def grid_tris_fast(K, J, closed):
    jj = J if closed else J - 1
    k, j = np.meshgrid(np.arange(K - 1), np.arange(jj), indexing="ij")
    a = k * J + j
    b = k * J + (j + 1) % J
    c = (k + 1) * J + j
    d = (k + 1) * J + (j + 1) % J
    return np.stack([np.stack([a, c, b], -1), np.stack([b, c, d], -1)], axis=2).reshape(-1, 3)


def grid_normals(P, closed):
    K, J, _ = P.shape
    dk = np.gradient(P, axis=0)
    if closed:
        dj = (np.roll(P, -1, axis=1) - np.roll(P, 1, axis=1)) / 2
    else:
        dj = np.gradient(P, axis=1)
    n = np.cross(dk, dj)
    length = np.linalg.norm(n, axis=-1, keepdims=True)
    bad = length[..., 0] < 1e-12
    n = n / np.maximum(length, 1e-12)
    if bad.any():
        for k, j in zip(*np.nonzero(bad)):
            k2 = k + 1 if k + 1 < K and not bad[k + 1, j] else k - 1
            n[k, j] = n[k2, j]
    return n


def orient_normals(P, N, kind, centre_b=None):
    if kind == "out":
        rad = P[..., :2].copy()
        if centre_b is not None:
            rad[..., 1] -= centre_b
        sign = np.sign(np.einsum("...i,...i->...", N[..., :2], rad))
        sign[sign == 0] = 1
        flipped = np.where(sign.mean() >= 0, 1.0, -1.0)
        return N * flipped
    if kind == "in":
        return -orient_normals(P, N, "out", centre_b)
    if kind == "aft":
        return N * (1.0 if N[..., 2].mean() >= 0 else -1.0)
    if kind == "fwd":
        return N * (1.0 if N[..., 2].mean() <= 0 else -1.0)
    return N


def surface(P, closed=True, kind="out", centre_b=None, tag="", uv=None):
    """Quad grid P[K, J, 3] in local coordinates -> Mesh with outward (or inward) normals and X-Plane winding."""
    K, J, _ = P.shape
    N = orient_normals(P, grid_normals(P, closed), kind, centre_b)
    Pf, Nf = P.reshape(-1, 3), N.reshape(-1, 3).copy()
    Nf[np.linalg.norm(Nf, axis=1) < 1e-9] = (0.0, 0.0, 1.0)
    T = grid_tris_fast(K, J, closed)
    area = np.linalg.norm(np.cross(Pf[T[:, 1]] - Pf[T[:, 0]], Pf[T[:, 2]] - Pf[T[:, 0]]), axis=1)
    T = wind(Pf, T[area > 1e-10], Nf)
    N = Nf.reshape(N.shape)
    m = Mesh()
    m.add(P, N, np.zeros((K * J, 2)) if uv is None else uv, T, tag)
    m.K, m.J, m.closed = K, J, closed
    return m


def cap_ring(ring, centre, normal, tag=""):
    J = len(ring)
    P = np.vstack([ring, centre[None]])
    N = np.tile(unit(normal), (J + 1, 1))
    T = np.array([(J, j, (j + 1) % J) for j in range(J)], np.int64)
    T = wind(P, T, N)
    m = Mesh()
    m.add(P, N, np.zeros((J + 1, 2)), T, tag)
    return m


def box(c, size, tag=""):
    hx, hy, hz = np.asarray(size, float) / 2
    P, N, T = [], [], []
    for axis, sgn in ((0, 1), (0, -1), (1, 1), (1, -1), (2, 1), (2, -1)):
        u, v = [i for i in range(3) if i != axis]
        n = np.zeros(3)
        n[axis] = sgn
        h = (hx, hy, hz)
        base = len(P)
        for du, dv in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
            p = np.zeros(3)
            p[axis] = sgn * h[axis]
            p[u] = du * h[u]
            p[v] = dv * h[v]
            P.append(np.asarray(c) + p)
            N.append(n)
        T += [(base, base + 1, base + 2), (base, base + 2, base + 3)]
    P, N, T = np.array(P), np.array(N), np.array(T)
    T = wind(P, T, N)
    m = Mesh()
    m.add(P, N, np.zeros((len(P), 2)), T, tag)
    return m


def tube(a, b, r, seg=8, tag="", caps=True):
    a, b = np.asarray(a, float), np.asarray(b, float)
    d = unit(b - a)
    ref = np.array([0.0, 1.0, 0.0]) if abs(d[1]) < 0.9 else np.array([1.0, 0.0, 0.0])
    u = unit(np.cross(d, ref))
    v = np.cross(d, u)
    t = np.linspace(0, 2 * np.pi, seg, endpoint=False)
    ring = (np.cos(t)[:, None] * u + np.sin(t)[:, None] * v)
    P = np.stack([a + ring * r, b + ring * r], axis=0)
    m = surface(P, True, "free", tag=tag)
    N = np.stack([ring, ring], 0).reshape(-1, 3)
    m.N = N
    m.T = wind(m.P, m.T, m.N)
    if caps:
        m.extend(cap_ring(P[0], a, -d, tag))
        m.extend(cap_ring(P[1], b, d, tag))
    return m


class Sheet:
    """A morph surface as triangles in the plane (s, theta*rref) carrying the morph's own UVs; queried through a raster of triangle ids.
    key orders the painting (larger keys first, so the smallest key wins a cell); island gives every triangle its UV island, which query returns."""

    def __init__(self, s, th, uv, rref, gs=1400, gt=900, key=None, island=None):
        s = np.asarray(s)
        th = np.asarray(th)
        self.rref = rref
        self.tri = np.stack([s, th * rref], axis=-1)
        self.uv = np.asarray(uv)
        self.island = np.zeros(len(s), np.int64) if island is None else np.asarray(island, np.int64)
        a, b, c = self.tri[:, 0], self.tri[:, 1], self.tri[:, 2]
        self.v0 = a
        self.e1, self.e2 = b - a, c - a
        self.det = self.e1[:, 0] * self.e2[:, 1] - self.e1[:, 1] * self.e2[:, 0]
        self.s0, self.s1 = float(s.min()) - 1e-6, float(s.max()) + 1e-6
        self.gs, self.gt = gs, gt
        self.period = 2 * np.pi * rref
        grid = np.full((gs, gt), -1, np.int32)
        ds = (self.s1 - self.s0) / gs
        dt = self.period / gt
        order = np.argsort(-np.abs(self.det)) if key is None else np.lexsort((-np.abs(self.det), -np.asarray(key)))
        for k in order:
            tri = self.tri[k]
            if abs(self.det[k]) < 1e-14:
                continue
            i0 = max(0, int(np.floor((tri[:, 0].min() - self.s0) / ds)))
            i1 = min(gs - 1, int(np.floor((tri[:, 0].max() - self.s0) / ds)))
            t_lo = tri[:, 1].min()
            j0 = int(np.floor(t_lo / dt))
            j1 = int(np.floor(tri[:, 1].max() / dt))
            ii, jj = np.meshgrid(np.arange(i0, i1 + 1), np.arange(j0, j1 + 1), indexing="ij")
            q = np.stack([self.s0 + (ii + 0.5) * ds, (jj + 0.5) * dt], axis=-1).reshape(-1, 2)
            w = _bary_pts(self.v0[k], self.e1[k], self.e2[k], self.det[k], q)
            inside = (w >= -1e-9).all(axis=1)
            grid[ii.ravel()[inside], jj.ravel()[inside] % gt] = k
        self.grid = grid
        ys, xs = np.nonzero(grid >= 0)
        self.fill = cKDTree(np.stack([self.s0 + (ys + 0.5) * ds, xs * dt + dt / 2], axis=1)) if len(ys) else None
        self.fill_id = grid[ys, xs]
        self.ds, self.dt = ds, dt
        self.row_fill = self._row_fill()
        nisl = int(self.island.max()) + 1
        self.isl_lo = np.full((nisl, 2), np.inf)
        self.isl_hi = np.full((nisl, 2), -np.inf)
        for c in range(3):
            np.minimum.at(self.isl_lo, self.island, self.uv[:, c])
            np.maximum.at(self.isl_hi, self.island, self.uv[:, c])

    def _row_fill(self):
        """For every raster cell the column of the nearest covered cell in the same row (circular), or -1 where the row is empty or the nearest is over 70 deg away."""
        out = np.full((self.gs, self.gt), -1, np.int32)
        far = int(self.gt * 70 / 360)
        cols = np.arange(self.gt)
        for i in range(self.gs):
            vj = np.nonzero(self.grid[i] >= 0)[0]
            if not len(vj):
                continue
            ext = np.concatenate([vj - self.gt, vj, vj + self.gt])
            pos = np.searchsorted(ext, cols + self.gt)
            lo = ext[np.clip(pos - 1, 0, len(ext) - 1)]
            hi = ext[np.clip(pos, 0, len(ext) - 1)]
            pick = np.where(np.abs(cols + self.gt - lo) <= np.abs(hi - cols - self.gt), lo, hi) - self.gt
            near = np.abs(pick - cols) <= far
            out[i] = np.where(near, pick % self.gt, -1)
        return out

    def find_seam(self, s, n=720):
        """The angle at which the UV of this sheet jumps most along the circle at station s."""
        th = np.arange(n) * 2 * np.pi / n
        uv, _, _ = self.query(np.full(n, float(s)), th)
        jump = np.linalg.norm(np.roll(uv, -1, axis=0) - uv, axis=1)
        return float(th[int(np.argmax(jump))] + np.pi / n)

    def query(self, s, th, k=1):
        q = np.stack([np.asarray(s, float), np.asarray(th, float) * self.rref % self.period], axis=-1).reshape(-1, 2)
        i = np.clip(np.floor((q[:, 0] - self.s0) / self.ds).astype(int), 0, self.gs - 1)
        j = np.floor(q[:, 1] / self.dt).astype(int) % self.gt
        ids = self.grid[i, j].copy()
        bad = ids < 0
        if bad.any() and self.fill is not None:
            jf = self.row_fill[i[bad], j[bad]]
            row = jf >= 0
            sub = np.nonzero(bad)[0]
            if row.any():
                r = sub[row]
                ids[r] = self.grid[i[r], jf[row]]
                q[r, 1] = (jf[row] + 0.5) * self.dt
            rest = sub[~row]
            if len(rest):
                _, near = self.fill.query(q[rest])
                ids[rest] = self.fill_id[near]
                cell = self.fill.data[near]
                q[rest] = cell
        v0 = self.v0[ids]
        qq = q.copy()
        qq[:, 1] = v0[:, 1] + ((q[:, 1] - v0[:, 1] + self.period / 2) % self.period - self.period / 2)
        w = _bary_pts_many(v0, self.e1[ids], self.e2[ids], self.det[ids], qq)
        w = np.clip(w, -0.25, 1.25)
        uv = np.einsum("nk,nkc->nc", w, self.uv[ids])
        isl = self.island[ids]
        uv = np.clip(uv, self.isl_lo[isl] - 0.0005, self.isl_hi[isl] + 0.0005)
        return uv, isl, ids


def _bary_pts(v0, e1, e2, det, q):
    p = q - v0
    l1 = (p[:, 0] * e2[1] - p[:, 1] * e2[0]) / det
    l2 = (e1[0] * p[:, 1] - e1[1] * p[:, 0]) / det
    return np.stack([1 - l1 - l2, l1, l2], axis=-1)


def _bary_pts_many(v0, e1, e2, det, q):
    det = np.where(np.abs(det) < 1e-14, 1e-14, det)
    p = q - v0
    l1 = (p[:, 0] * e2[:, 1] - p[:, 1] * e2[:, 0]) / det
    l2 = (e1[:, 0] * p[:, 1] - e1[:, 1] * p[:, 0]) / det
    return np.stack([1 - l1 - l2, l1, l2], axis=-1)


SEAM_INSET = 0.012


def _lookup(sheet, P, smap):
    th = np.arctan2(P[:, 0], P[:, 1])
    seam = getattr(sheet, "fold", None)
    if seam is not None:
        w = (th - seam) % (2 * np.pi)
        th = seam + np.minimum(w, 2 * np.pi - w)
    else:
        th = np.clip(th, -np.pi + SEAM_INSET, np.pi - SEAM_INSET)
    uv, isl, _ = sheet.query(smap(P[:, 2], th), th)
    return uv, isl


def _crossings(sheet, P, N, isl, smap, a, b, bias=0.006):
    """Where each edge a-b leaves the island of a: the point (found by bisection on the island raster) and the UV of the two sides of it."""
    lo, hi = np.zeros(len(a)), np.ones(len(a))
    pa, pb = P[a], P[b]
    for _ in range(18):
        mid = (lo + hi) / 2
        _, im = _lookup(sheet, pa + (pb - pa) * mid[:, None], smap)
        same = im == isl[a]
        lo = np.where(same, mid, lo)
        hi = np.where(same, hi, mid)
    t = (lo + hi) / 2
    X = pa + (pb - pa) * t[:, None]
    length = np.maximum(np.linalg.norm(pb - pa, axis=1), 1e-9)
    d = np.minimum(bias / length, 0.3)
    uv_a, _ = _lookup(sheet, pa + (pb - pa) * np.maximum(t - d, 0)[:, None], smap)
    uv_b, _ = _lookup(sheet, pa + (pb - pa) * np.minimum(t + d, 1)[:, None], smap)
    n = N[a] * (1 - t[:, None]) + N[b] * t[:, None]
    n /= np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-12)
    return X, n, uv_a, uv_b


def transfer(sheet, mesh, smap, spread=0.15, piece_spread=0.03):
    """Give every vertex of mesh the UV of the sheet at the same (mapped station, angle). A triangle with one corner on another UV island
    than the other two is cut exactly where its two edges cross the island boundary (found on the island raster); each piece takes the UV of
    the island it lies on. A triangle on three islands, or whose UVs still spread over more than `spread`, takes the UV of its centroid on
    vertices of its own. Returns (flat triangles, cut triangles)."""
    P, N, T = mesh.P, mesh.N, mesh.T
    tag = np.array(mesh.tag, dtype=object) if len(mesh.tag) == len(T) else np.array([""] * len(T), dtype=object)
    uv, isl = _lookup(sheet, P, smap)
    I = isl[T]
    eq = np.stack([I[:, 0] == I[:, 1], I[:, 1] == I[:, 2], I[:, 2] == I[:, 0]], 1)
    neq = eq.sum(axis=1)
    cut = np.nonzero(neq == 1)[0]
    keep = np.ones(len(T), bool)
    keep[cut] = False
    nP, nN, nuv, nT, ntag = [P], [N], [uv], [T[keep]], [tag[keep]]
    n_keep = int(keep.sum())
    base = len(P)
    if len(cut):
        pair = np.argmax(eq[cut], axis=1)
        lone = (pair + 2) % 3
        Tc = T[cut]
        r = np.arange(len(cut))[:, None]
        V = Tc[r, (lone[:, None] + np.arange(3)) % 3]
        e01 = np.stack([V[:, 0], V[:, 1]], 1)
        e20 = np.stack([V[:, 0], V[:, 2]], 1)
        pairs = np.concatenate([e01, e20])
        lo_hi = np.sort(pairs, axis=1)
        key = lo_hi[:, 0].astype(np.int64) * len(P) + lo_hi[:, 1]
        uniq, inv = np.unique(key, return_inverse=True)
        ea, eb = uniq // len(P), uniq % len(P)
        X, Xn, uv_lo_side, uv_hi_side = _crossings(sheet, P, N, isl, smap, ea, eb, 0.025)
        nE = len(uniq)
        nP += [X, X]
        nN += [Xn, Xn]
        nuv += [uv_lo_side, uv_hi_side]
        ia = inv[:len(cut)]
        ib = inv[len(cut):]

        def side_vertex(edge_idx, v_index, own):
            lo_vertex = ea[edge_idx]
            return base + edge_idx + np.where(own == lo_vertex, 0, nE)

        x01_b = side_vertex(ia, None, V[:, 0])
        x01_a = side_vertex(ia, None, V[:, 1])
        x20_b = side_vertex(ib, None, V[:, 0])
        x20_a = side_vertex(ib, None, V[:, 2])
        pieces = [np.stack([V[:, 0], x01_b, x20_b], 1), np.stack([x01_a, V[:, 1], V[:, 2]], 1), np.stack([x01_a, V[:, 2], x20_a], 1)]
        nT += pieces
        ntag += [tag[cut]] * 3
    P, N, uv = np.vstack(nP), np.vstack(nN), np.vstack(nuv)
    T, tag = np.vstack(nT), np.concatenate(ntag)
    c = uv[T]
    ext = np.maximum(np.ptp(c[:, :, 0], axis=1), np.ptp(c[:, :, 1], axis=1))
    bad = (ext > spread) | ((np.arange(len(T)) >= n_keep) & (ext > piece_spread))
    ids = np.nonzero(bad)[0]
    if len(ids):
        corner = T[ids]
        cuv, _ = _lookup(sheet, P[corner].mean(axis=1), smap)
        b0 = len(P)
        P = np.vstack([P, P[corner.ravel()]])
        N = np.vstack([N, N[corner.ravel()]])
        uv = np.vstack([uv, np.repeat(cuv, 3, axis=0)])
        T = T.copy()
        T[ids] = (b0 + np.arange(3 * len(ids))).reshape(-1, 3)
    area = np.linalg.norm(np.cross(P[T[:, 1]] - P[T[:, 0]], P[T[:, 2]] - P[T[:, 0]]), axis=1) / 2
    live = area > 1e-8
    mesh.P, mesh.N, mesh.UV, mesh.T, mesh.tag = P, N, uv, T[live], list(tag[live])
    return int(len(ids)), int(len(cut))
