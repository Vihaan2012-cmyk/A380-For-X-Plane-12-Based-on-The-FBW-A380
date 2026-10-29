"""usage: hail_dents.py <objects dir>

Hail damage for the nose and wing skins: each target object gets
<name>_hail.obj, an overlay of small dent patches projected onto the surfaces
hail hits (radome and nose cone, the wing leading-edge band), shown by
fbw/damage/hail_<region>. The patches reuse the skin's own textures and carry
the dent in their normals, so no extra texture is loaded."""

import sys
from pathlib import Path

import numpy as np
from PIL import Image

TARGETS = {
    "a380_exterior_009.obj": "nose",
    "a380_exterior_026.obj": "wing_l",
    "a380_exterior_027.obj": "wing_l",
    "a380_exterior_028.obj": "wing_r",
    "a380_exterior_029.obj": "wing_r",
}
WING_PAIRS = {"wing_l": ["a380_exterior_026.obj", "a380_exterior_027.obj"], "wing_r": ["a380_exterior_028.obj", "a380_exterior_029.obj"]}
NOSE_REGION_Z = -28.0
LE_BAND_M = 0.6
DENT_RADIUS_M = (0.012, 0.022)
COVERAGE = 0.10
MIN_RADIUS_PX = 1.6
SEED = 380
HAIL_MAP_SIZE = 2048
SLOPE = 0.4
GRID = 4
LIFT_M = 0.0015
DRAW_M = 150
FACING = 0.5


def gate(region):
    return [f"ANIM_hide -1000000 1000000 fbw/damage/hail_{region}", f"ANIM_show 0.5 1.5 fbw/damage/hail_{region}"]


def read_obj(path):
    raw = path.read_bytes().decode("utf-8", errors="replace")
    nl = "\r\n" if "\r\n" in raw else "\n"
    lines = raw.split(nl)
    vt = np.array([[float(x) for x in l.split()[1:9]] for l in lines if l.startswith("VT")])
    idx = np.array([int(x) for l in lines if l.startswith(("IDX10", "IDX ")) for x in l.split()[1:]], dtype=np.int64)
    return nl, lines, vt, idx


def unwrap_base(lines, region):
    marker = f"ANIM_hide 0.5 1.5 fbw/damage/hail_{region}"
    if marker not in lines:
        return lines, False
    i = lines.index(marker)
    assert lines[i - 1] == "ANIM_begin"
    out = lines[:i - 1] + lines[i + 1:]
    last = max(j for j, l in enumerate(out) if l == "ANIM_end")
    return out[:last] + out[last + 1:], True


def leading_edge_z(vertices):
    bins = np.floor(np.abs(vertices[:, 0])).astype(int)
    return {k: vertices[bins == k, 2].min() for k in np.unique(bins)}


def selected_vertices(name, vt, wing_le):
    region = TARGETS[name]
    if region == "nose":
        return vt[:, 2] < NOSE_REGION_Z
    bins = np.floor(np.abs(vt[:, 0])).astype(int)
    le = np.array([wing_le[region].get(k, np.inf) for k in bins])
    return (vt[:, 2] - le) < LE_BAND_M


def normal_map_name(lines):
    for l in lines:
        if l.startswith("TEXTURE_MAP normal ") or l.startswith("TEXTURE_NORMAL "):
            return l.split()[-1]
    return None


def texels_per_metre(vt, tris, w, h):
    a, b, c = vt[tris[:, 0]], vt[tris[:, 1]], vt[tris[:, 2]]
    world = 0.5 * np.linalg.norm(np.cross(b[:, :3] - a[:, :3], c[:, :3] - a[:, :3]), axis=1)
    uv = 0.5 * np.abs((b[:, 6] - a[:, 6]) * (c[:, 7] - a[:, 7]) - (c[:, 6] - a[:, 6]) * (b[:, 7] - a[:, 7])) * w * h
    ok = (world > 1e-4) & (uv > 1e-3)
    return float(np.median(np.sqrt(uv[ok] / world[ok]))) if ok.any() else 0.0


def clip(poly, axis, limit, keep_below):
    out = []
    for k in range(len(poly)):
        p, q = poly[k], poly[(k + 1) % len(poly)]
        pin = p[axis] <= limit if keep_below else p[axis] >= limit
        qin = q[axis] <= limit if keep_below else q[axis] >= limit
        if pin:
            out.append(p)
        if pin != qin:
            t = (limit - p[axis]) / (q[axis] - p[axis])
            out.append(p + t * (q - p))
    return out


def barycentric(p, a, b, c):
    v0, v1, v2 = b - a, c - a, p - a
    d = v0[0] * v1[1] - v1[0] * v0[1]
    s = (v2[0] * v1[1] - v1[0] * v2[1]) / d
    t = (v0[0] * v2[1] - v2[0] * v0[1]) / d
    return np.array([1 - s - t, s, t])


def dent_patch(centre, normal, radius, depth, tris, vt, winding):
    t = np.cross(normal, [0.0, 1.0, 0.0] if abs(normal[1]) < 0.9 else [1.0, 0.0, 0.0])
    t /= np.linalg.norm(t)
    b = np.cross(normal, t)
    rows = []
    for tri in tris:
        corners = vt[tri]
        p = corners[:, :3] - centre
        face = np.cross(p[1] - p[0], p[2] - p[0])
        size = np.linalg.norm(face)
        if size < 1e-12 or winding * (face @ normal) < FACING * size or abs(face @ p[0]) / size > radius:
            continue
        flat = np.stack([p @ t, p @ b], axis=1)
        cell = 2 * radius / GRID
        for gx in range(GRID):
            for gy in range(GRID):
                x0, y0 = -radius + gx * cell, -radius + gy * cell
                poly = [q for q in flat]
                for axis, limit, below in ((0, x0, False), (0, x0 + cell, True), (1, y0, False), (1, y0 + cell, True)):
                    poly = clip(poly, axis, limit, below)
                    if len(poly) < 3:
                        break
                if len(poly) < 3:
                    continue
                verts = []
                for q in poly:
                    w = barycentric(q, flat[0], flat[1], flat[2])
                    v = w @ corners
                    n = v[3:6] / np.linalg.norm(v[3:6])
                    rho = np.hypot(q[0], q[1]) / radius
                    if rho < 1.0:
                        slope = SLOPE * depth * 4 * rho * (1 - rho * rho)
                        radial = (q[0] * t + q[1] * b) / max(np.hypot(q[0], q[1]), 1e-9)
                        n = n - slope * (radial - (radial @ n) * n)
                        n /= np.linalg.norm(n)
                    pos = v[:3] + v[3:6] / np.linalg.norm(v[3:6]) * LIFT_M
                    verts.append(np.concatenate([pos, n, v[6:8]]))
                rows.append(verts)
    return rows


def winding_of(vt, idx):
    t = idx.reshape(-1, 3)
    face = np.cross(vt[t[:, 1], :3] - vt[t[:, 0], :3], vt[t[:, 2], :3] - vt[t[:, 0], :3])
    return 1.0 if np.einsum('ij,ij->i', face, vt[t, 3:6].sum(axis=1)).sum() >= 0 else -1.0


def overlay(lines, vt, idx, hit, ppm, rng, region):
    winding = winding_of(vt, idx)
    r_min = max(DENT_RADIUS_M[0], MIN_RADIUS_PX / ppm)
    r_max = max(DENT_RADIUS_M[1], MIN_RADIUS_PX * 1.5 / ppm)
    mean_r = 0.5 * (r_min + r_max)
    first_cmd = next(i for i, l in enumerate(lines) if l.startswith(("ANIM_", "ATTR_", "TRIS ")) and i > 0)
    new_vt, new_idx, commands, dents = [], [], [], 0
    for l in lines[first_cmd:]:
        if not l.startswith("TRIS "):
            commands.append(l)
            continue
        s, c = map(int, l.split()[1:3])
        tris = idx[s:s + c].reshape(-1, 3)
        tris = tris[hit[tris].all(axis=1)]
        if len(tris) == 0:
            continue
        a, b, cc = vt[tris[:, 0], :3], vt[tris[:, 1], :3], vt[tris[:, 2], :3]
        area = 0.5 * np.linalg.norm(np.cross(b - a, cc - a), axis=1)
        count = rng.poisson(area.sum() * COVERAGE / (np.pi * mean_r * mean_r))
        if count == 0:
            continue
        centroids = (a + b + cc) / 3
        reach = np.linalg.norm(np.stack([a, b, cc], axis=1) - centroids[:, None], axis=2).max(axis=1)
        start = len(new_idx)
        for k in rng.choice(len(tris), size=count, p=area / area.sum()):
            u, v = rng.random(2)
            if u + v > 1:
                u, v = 1 - u, 1 - v
            corner = vt[tris[k]]
            point = (1 - u - v) * corner[0] + u * corner[1] + v * corner[2]
            normal = point[3:6] / np.linalg.norm(point[3:6])
            radius = rng.uniform(r_min, r_max)
            depth = rng.uniform(0.6, 1.0)
            near = np.flatnonzero(np.linalg.norm(centroids - point[:3], axis=1) < radius * 1.5 + reach)
            shared = {}
            for poly in dent_patch(point[:3], normal, radius, depth, tris[near], vt, winding):
                ids = []
                for vertex in poly:
                    key = tuple(np.round(vertex, 6))
                    if key not in shared:
                        shared[key] = len(new_vt)
                        new_vt.append(vertex)
                    ids.append(shared[key])
                for j in range(1, len(ids) - 1):
                    if len({ids[0], ids[j], ids[j + 1]}) == 3:
                        new_idx.extend((ids[0], ids[j], ids[j + 1]))
            dents += 1
        if len(new_idx) > start:
            commands += ["ATTR_poly_os 2", "ATTR_no_shadow", f"TRIS {start} {len(new_idx) - start}"]
    while commands and not commands[-1].strip():
        commands.pop()
    header = [l for l in lines[:first_cmd] if not l.startswith(("VT", "IDX", "POINT_COUNTS")) and l.strip()]
    rows = [f"VT {' '.join(f'{x:.6f}' for x in v[:3])} {' '.join(f'{x:.5f}' for x in v[3:6])} {v[6]:.6f} {v[7]:.6f}" for v in new_vt]
    ids = [f"IDX10 {' '.join(map(str, new_idx[k:k + 10]))}" for k in range(0, len(new_idx) - len(new_idx) % 10, 10)]
    ids += [f"IDX {v}" for v in new_idx[len(new_idx) - len(new_idx) % 10:]]
    body = header + [f"POINT_COUNTS {len(new_vt)} 0 0 {len(new_idx)}", ""] + rows + ids + ["", f"ATTR_LOD 0 {DRAW_M}", "ANIM_begin"] + gate(region) + commands + ["ANIM_end", ""]
    return body, dents, len(new_idx) // 3, (r_min, r_max)


def main(obj_dir):
    obj_dir = Path(obj_dir)
    rng = np.random.default_rng(SEED)
    loaded = {}
    for name, region in TARGETS.items():
        nl, lines, vt, idx = read_obj(obj_dir / name)
        lines, changed = unwrap_base(lines, region)
        if changed:
            (obj_dir / name).write_bytes(nl.join(lines).encode("utf-8"))
        loaded[name] = (lines, vt, idx)
    wing_le = {region: leading_edge_z(np.concatenate([loaded[n][1] for n in names])) for region, names in WING_PAIRS.items()}
    for name, region in TARGETS.items():
        lines, vt, idx = loaded[name]
        hit = selected_vertices(name, vt, wing_le)
        normal_name = normal_map_name(lines)
        w, h = Image.open(obj_dir / normal_name).size
        scale = max(1, HAIL_MAP_SIZE // max(w, h))
        ppm = texels_per_metre(vt, idx.reshape(-1, 3)[hit[idx.reshape(-1, 3)].all(axis=1)], w * scale, h * scale)
        body, dents, tris, (r_min, r_max) = overlay(lines, vt, idx, hit, ppm, rng, region)
        out_name = name.replace(".obj", "_hail.obj")
        (obj_dir / out_name).write_text("\n".join(body), encoding="utf-8", newline="\n")
        print(f"{name}: {dents} dents, {tris} triangles, radius {r_min * 100:.1f}-{r_max * 100:.1f} cm -> {out_name}")


if __name__ == "__main__":
    main(sys.argv[1])
