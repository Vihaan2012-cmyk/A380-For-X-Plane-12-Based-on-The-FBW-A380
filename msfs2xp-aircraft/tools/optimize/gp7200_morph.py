import numpy as np

from gp7200_geom import Sheet
from gp7200_obj import Obj, engine_of_batch, has


def engine_triangles(obj, engine, skip_blockers=True):
    """All triangles (world position, normal, uv) the template draws for engine (1..4), as (n, 3, 8)."""
    out = []
    for b in obj.tris():
        if engine_of_batch(b) != engine - 1:
            continue
        if skip_blockers and any(t.startswith("ANIM_rotate_begin") and "thrust_rev" in t for t in b["anim"]):
            continue
        out.append(obj.batch_arrays(b))
    return np.concatenate(out) if out else np.zeros((0, 3, 8))


def describe(tris, frame):
    pts = frame.local(tris[..., :3])
    nrm = frame.local_vec(tris[..., 3:6])
    cen = pts.mean(axis=1)
    n = nrm.mean(axis=1)
    r = np.hypot(cen[:, 0], cen[:, 1])
    rad = cen[:, :2] / np.maximum(r, 1e-9)[:, None]
    nr = (n[:, :2] * rad).sum(axis=1) / np.maximum(np.linalg.norm(n, axis=1), 1e-9)
    na = n[:, 2] / np.maximum(np.linalg.norm(n, axis=1), 1e-9)
    area = 0.5 * np.linalg.norm(np.cross(pts[:, 1] - pts[:, 0], pts[:, 2] - pts[:, 0]), axis=1)
    return dict(pts=pts, cen=cen, r=r, nr=nr, na=na, area=area)


def island_labels(tris):
    """UV island of every triangle: triangles that share a vertex (same position and UV) belong together."""
    from scipy.sparse import coo_matrix
    from scipy.sparse.csgraph import connected_components

    key = np.round(tris[:, :, [0, 1, 2, 6, 7]].reshape(-1, 5) * np.array([1e4, 1e4, 1e4, 1e5, 1e5])).astype(np.int64)
    _, vid = np.unique(key, axis=0, return_inverse=True)
    vid = vid.reshape(-1, 3)
    n = int(vid.max()) + 1
    a = np.concatenate([vid[:, 0], vid[:, 1]])
    b = np.concatenate([vid[:, 1], vid[:, 2]])
    _, lab = connected_components(coo_matrix((np.ones(len(a)), (a, b)), shape=(n, n)), directed=False)
    return lab[vid[:, 0]]


def sheet_from(tris, frame, mask, rref, pick="min", min_island=40, min_area=0.0, keel_strips=False):
    """pick: which of overlapping surfaces wins a raster cell, the one nearest the axis ("min") or the outermost ("max")."""
    d = describe(tris, frame)
    lab = island_labels(tris)
    sel = np.nonzero(mask(d))[0]
    if len(sel):
        ids, cnt = np.unique(lab[sel], return_counts=True)
        big = np.isin(lab[sel], ids[cnt >= min_island])
        sel = sel[big] if big.any() else sel
    if not len(sel):
        return None
    pts = d["pts"][sel]
    th = np.arctan2(pts[:, :, 0], pts[:, :, 1])
    base = th[:, 0:1]
    th = base + (th - base + np.pi) % (2 * np.pi) - np.pi
    s = pts[:, :, 2]
    if min_area > 0:
        e1 = np.stack([s[:, 1] - s[:, 0], (th[:, 1] - th[:, 0]) * rref], 1)
        e2 = np.stack([s[:, 2] - s[:, 0], (th[:, 2] - th[:, 0]) * rref], 1)
        area = np.abs(e1[:, 0] * e2[:, 1] - e1[:, 1] * e2[:, 0]) / 2
        ids = lab[sel]
        tot = np.bincount(ids, weights=area, minlength=int(ids.max()) + 1)
        keep = tot[ids] >= min_area
        if keel_strips:
            n = int(ids.max()) + 1
            cnt = np.bincount(ids, minlength=n)
            cos_mean = np.bincount(ids, weights=np.cos(th.mean(axis=1)), minlength=n) / np.maximum(cnt, 1)
            smin = np.full(n, np.inf)
            smax = np.full(n, -np.inf)
            np.minimum.at(smin, ids, s.min(axis=1))
            np.maximum.at(smax, ids, s.max(axis=1))
            width = tot / np.maximum(smax - smin, 1e-6)
            keep &= ~((cos_mean[ids] < -0.97) & (width[ids] < 0.4))
        if keep.any():
            sel, pts, th, s = sel[keep], pts[keep], th[keep], s[keep]
    key = d["r"][sel] * (1 if pick == "min" else -1)
    return Sheet(s, th, tris[sel][:, :, 6:8], rref, key=key, island=lab[sel])
