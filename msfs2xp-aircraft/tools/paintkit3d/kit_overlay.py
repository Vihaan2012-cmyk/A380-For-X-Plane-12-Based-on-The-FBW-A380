"""Panel line and UV island overlays, rasterised from mesh UVs for display in the kit."""
import numpy as np

CREASE_DEGREES = 35.0
CHUNK_POINTS = 6_000_000


def _edge_groups(ids, tri):
    t = ids[tri.reshape(-1, 3)]
    e = np.stack([t[:, [0, 1]], t[:, [1, 2]], t[:, [2, 0]]], axis=1).reshape(-1, 2)
    lo, hi = e.min(axis=1), e.max(axis=1)
    key = lo.astype(np.int64) * (int(ids.max()) + 1) + hi
    order = np.argsort(key, kind='stable')
    sk = key[order]
    start = np.flatnonzero(np.r_[True, sk[1:] != sk[:-1]])
    count = np.diff(np.r_[start, len(sk)])
    return order, start, count


def _uv_segments(uv, tri, slots):
    f, k = slots // 3, slots % 3
    t = tri.reshape(-1, 3)
    a = uv[t[f, k]]
    b = uv[t[f, (k + 1) % 3]]
    return np.concatenate([a, b], axis=1)


def panel_segments(pos, uv, tri, part):
    """UV segments of creases, open borders and part borders."""
    pos = np.asarray(pos, np.float64)
    t = tri.reshape(-1, 3)
    if len(t) == 0:
        return np.zeros((0, 4), np.float32)
    _, weld = np.unique(np.round(pos * 1000).astype(np.int64), axis=0, return_inverse=True)
    weld = weld.reshape(-1)
    order, start, count = _edge_groups(weld, tri)
    p = pos[t]
    n = np.cross(p[:, 1] - p[:, 0], p[:, 2] - p[:, 0])
    ln = np.linalg.norm(n, axis=1)
    n = n / np.maximum(ln, 1e-12)[:, None]
    face = order // 3
    keep = count == 1
    pair = np.flatnonzero(count == 2)
    f1, f2 = face[start[pair]], face[start[pair] + 1]
    sharp = np.einsum('ij,ij->i', n[f1], n[f2]) < np.cos(np.radians(CREASE_DEGREES))
    border = part[f1] != part[f2]
    keep[pair[sharp | border]] = True
    slots = order[np.repeat(keep, count)]
    return _uv_segments(uv, tri, slots)


def island_segments(uv, tri):
    """UV segments of island borders: edges whose two sides do not share the same UV edge."""
    if len(tri) == 0:
        return np.zeros((0, 4), np.float32)
    _, ids = np.unique(np.round(np.asarray(uv, np.float64) * 8192).astype(np.int64), axis=0, return_inverse=True)
    order, start, count = _edge_groups(ids.reshape(-1), tri)
    return _uv_segments(uv, tri, order[start[count == 1]])


def raster(segments, width, height, thick):
    """Boolean mask (rows bottom to top, like Blender pixels) with the segments drawn on it."""
    mask = np.zeros((height, width), bool)
    if len(segments):
        seg = np.asarray(segments, np.float64) * np.array([width, height, width, height])
        n = np.maximum(np.abs(seg[:, 2] - seg[:, 0]), np.abs(seg[:, 3] - seg[:, 1])).astype(np.int64) + 1
        s = 0
        while s < len(seg):
            e = s
            total = 0
            while e < len(seg) and total + n[e] <= CHUNK_POINTS:
                total += n[e]
                e += 1
            e = max(e, s + 1)
            ns = n[s:e]
            idx = np.repeat(np.arange(e - s), ns)
            first = np.cumsum(ns) - ns
            t = (np.arange(ns.sum()) - first[idx]) / np.maximum(ns[idx] - 1, 1)
            sg = seg[s:e][idx]
            x = np.floor(sg[:, 0] + (sg[:, 2] - sg[:, 0]) * t).astype(np.int64) % width
            y = np.floor(sg[:, 1] + (sg[:, 3] - sg[:, 1]) * t).astype(np.int64) % height
            mask[y, x] = True
            s = e
    if thick and mask.any():
        mask |= np.roll(mask, 1, axis=0) | np.roll(mask, 1, axis=1) | np.roll(np.roll(mask, 1, axis=0), 1, axis=1)
    return mask


def to_rgba(mask, color):
    out = np.zeros(mask.shape + (4,), np.float32)
    out[mask] = (*color, 1.0)
    return out
