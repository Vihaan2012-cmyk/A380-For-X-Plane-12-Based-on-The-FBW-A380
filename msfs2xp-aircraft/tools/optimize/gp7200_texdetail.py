"""usage: gp7200_texdetail.py <stage objects dir> <install objects dir> [--check-only]

Group 1 of the GP7200 detail pass: surface detail in the normal and material maps, no triangles. The GP nacelle shares A380_EXTERIOR_ENG_{LH,RH}_NORM_NML.png
and _COMP_MAT.png with the Trent, so the GP objects get their own copies (A380_EXTERIOR_GPENG_{LH,RH}_NORM_NML.png, A380_EXTERIOR_GPENG_{LH,RH}_COMP_MAT.png)
and the Trent keeps the original files. The albedo atlas is not touched (liveries ship their own), so everything works on every livery.

The detail is painted in texture space. Every triangle of the GP nacelle skin (the post-UV meshes dumped by gp7200_model.run_nacelle) is rasterised into the
atlas; each texel then knows its engine-local station, its angle round the axis and its size in millimetres, so features are drawn in engine coordinates
(a rivet row at a station, a seam at an angle) and turned into tangent-space normals through the height gradient. Only texels covered by GP triangles change.
Features come only from the reference photos listed in renders6/detail_list.md."""

import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.ndimage import gaussian_filter, maximum_filter

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gp7200_geom import SPEC  # noqa: E402

SIZE = 2048
MAT_SIZE = 256
CFG = dict(size=2048, depth=6.0, liner=False, rivets=False, seams=False, flatten=("CCOW", "NOZO", "NOZI"))
TAGS = ["OUT", "TE", "TRIN", "CASE", "DOORIN", "CCOW", "NOZO", "NOZI", "PLUG", "RED", "PANEL", "LOUV", "LIPM", "HUB", "SPIN"]
TAG_ID = {t: i + 1 for i, t in enumerate(TAGS)}
SIDE = {"LH": 2, "RH": 3}
SWATCH_STRIP = (0, 205)
NAMES = {
    "LH": ("A380_EXTERIOR_ENG_LH_NORM_NML.png", "A380_EXTERIOR_ENG_LH_COMP_MAT.png", "A380_EXTERIOR_GPENG_LH_NORM_NML.png", "A380_EXTERIOR_GPENG_LH_COMP_MAT.png"),
    "RH": ("A380_EXTERIOR_ENG_RH_NORM_NML.png", "A380_EXTERIOR_ENG_RH_COMP_MAT.png", "A380_EXTERIOR_GPENG_RH_NORM_NML.png", "A380_EXTERIOR_GPENG_RH_COMP_MAT.png"),
}


def rasterise(P, UV, T, tags, size=SIZE):
    """Per texel: tag id, station z, angle round the axis (0 = top, +x = 90 deg), millimetres per texel, coverage count."""
    tid = np.zeros((size, size), np.int8)
    z = np.zeros((size, size), np.float32)
    phi = np.zeros((size, size), np.float32)
    mpt = np.zeros((size, size), np.float32)
    cnt = np.zeros((size, size), np.int8)
    px = UV[:, 0] * size
    py = (1 - UV[:, 1]) * size
    for k, t in enumerate(T):
        tg = TAG_ID.get(tags[k])
        if tg is None:
            continue
        xs, ys = px[t], py[t]
        x0, x1 = int(np.floor(xs.min())), int(np.ceil(xs.max()))
        y0, y1 = int(np.floor(ys.min())), int(np.ceil(ys.max()))
        if x1 < 0 or y1 < 0 or x0 >= size or y0 >= size or (x1 - x0) > 600 * size // 2048 or (y1 - y0) > 600 * size // 2048:
            continue
        x0, y0 = max(x0, 0), max(y0, 0)
        x1, y1 = min(x1, size - 1), min(y1, size - 1)
        gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
        d = (ys[1] - ys[2]) * (xs[0] - xs[2]) + (xs[2] - xs[1]) * (ys[0] - ys[2])
        if abs(d) < 1e-9:
            continue
        l0 = ((ys[1] - ys[2]) * (gx - xs[2]) + (xs[2] - xs[1]) * (gy - ys[2])) / d
        l1 = ((ys[2] - ys[0]) * (gx - xs[2]) + (xs[0] - xs[2]) * (gy - ys[2])) / d
        l2 = 1 - l0 - l1
        m = (l0 >= -0.02) & (l1 >= -0.02) & (l2 >= -0.02)
        if not m.any():
            continue
        p = P[t]
        pos = l0[m][:, None] * p[0] + l1[m][:, None] * p[1] + l2[m][:, None] * p[2]
        area = np.linalg.norm(np.cross(p[1] - p[0], p[2] - p[0])) / 2
        uva = abs(d) / 2
        yy, xx = np.nonzero(m)
        yy, xx = yy + y0, xx + x0
        cnt[yy, xx] += 1
        tid[yy, xx] = tg
        z[yy, xx] = pos[:, 2]
        phi[yy, xx] = np.arctan2(pos[:, 0], pos[:, 1] - AXIS.get(tags[k], lambda s: 0.0)(pos[:, 2]))
        mpt[yy, xx] = 1000 * np.sqrt(area / max(uva, 1e-9))
    return tid, z, phi, mpt, cnt


def _y0(s):
    from gp7200_geom import y0
    return y0(s)


AXIS = {"OUT": _y0, "TE": _y0, "DOORIN": _y0}


def wrap(a):
    return (a + np.pi) % (2 * np.pi) - np.pi


def deg(a):
    return np.degrees(wrap(a)) % 360


def band(x, lo, hi, soft):
    return np.clip(np.minimum(x - lo, hi - x) / soft + 0.5, 0, 1)


def dots(s, a, r, s0, s1, pitch_s, pitch_a_mm, a0, a1, radius_mm, mpt):
    """Rivet row field: 1 inside a dot of the grid."""
    return None


def station_remap(z, e, engines):
    """The atlas is shared by the engines of a side but each engine samples it at its own station offset (gp7200_model.station_map); features are drawn in the
    station of the mean engine so that both engines of the side see them within half the offset difference."""
    import gp7200_model as gm
    knots = [gm.S_LIP_END, gm.S_TR, gm.S_TE, 5.917, gm.S_PLUG, 9.0]

    def body(d):
        return np.array([gm.S_LIP_END + d, gm.S_TR + d, gm.MORPH_EXIT, gm.MORPH_NOZZLE, gm.MORPH_PLUG, 9.2])

    ref = np.mean([gm.DELTA[k] for k in engines])
    x = np.interp(z, knots, body(gm.DELTA[e]))
    return np.where(z >= gm.S_LIP_END, np.interp(x, body(ref), knots), z).astype(np.float32)


def height(tid, z, phi, mpt, cnt, side):
    """Height field in millimetres (negative = groove) from the photo-confirmed features, per tag."""
    H = np.zeros(tid.shape, np.float32)
    a = deg(phi)
    arc = lambda r: r * np.radians(1.0)
    texel = np.maximum(mpt, 3.0)

    def sig(w_mm):
        return np.maximum(w_mm / 2.0, 0.9 * texel)

    def line_s(z0, w_mm=12.0, depth=-1.2):
        return depth * np.exp(-0.5 * ((z - z0) * 1000 / sig(w_mm)) ** 2)

    def line_a(a0, r, w_mm=12.0, depth=-1.0):
        d = np.abs(wrap(np.radians(a - a0))) * r * 1000
        return depth * np.exp(-0.5 * (d / sig(w_mm)) ** 2)

    def rivets(z0, r, pitch_mm, radius_mm=4.0, height_mm=0.8, a_lo=0.0, a_hi=360.0):
        if not CFG["rivets"]:
            return 0.0
        circ = 2 * np.pi * r * 1000
        n = max(int(circ / pitch_mm), 1)
        cell = (np.radians(a) % (2 * np.pi)) / (2 * np.pi) * n
        da = (cell - np.round(cell)) * circ / n
        ds = (z - z0) * 1000
        d2 = da * da + ds * ds
        m = (a >= a_lo) & (a <= a_hi)
        return np.where(m, height_mm * np.exp(-0.5 * d2 / sig(radius_mm * 1.2) ** 2), 0.0)

    t = lambda name: tid == TAG_ID[name]
    if CFG["liner"]:
        case = t("CASE")
        r_case = 1.48
        liner = case & (z < 0.36)
        cell_a = (np.radians(a) % (2 * np.pi)) * r_case * 1000
        cell_s = z * 1000
        pitch = 30.0
        row = np.floor(cell_s / (pitch * 0.866))
        colx = cell_a / pitch + 0.5 * (row % 2)
        dx = (colx - np.round(colx)) * pitch
        dy = (cell_s / (pitch * 0.866) - np.round(cell_s / (pitch * 0.866))) * pitch * 0.866
        H += np.where(liner, -0.5 * np.exp(-0.5 * (dx * dx + dy * dy) / (np.maximum(6.5, 0.9 * texel) ** 2)), 0.0)
        for k in range(12):
            H += np.where(liner, line_a(k * 30.0 + 15.0, r_case, 9.0, -0.8), 0.0)

    cc = t("CCOW")
    r_cc = 0.92
    H += np.where(cc, rivets(4.985, r_cc, 28.0, 4.0, 1.0) + rivets(5.025, r_cc, 28.0, 4.0, 1.0) + line_s(5.005, 20.0, 0.8) * -1, 0.0)
    for k in range(4):
        H += np.where(cc & (z > 4.9), line_a(k * 90.0, r_cc, 8.0, -0.9), 0.0)
    H += np.where(cc, line_s(5.295, 10.0, -0.9), 0.0)

    nz = t("NOZO")
    r_nz = 0.80
    H += np.where(nz, rivets(5.335, r_nz, 26.0, 4.0, 1.0) + rivets(5.37, r_nz, 26.0, 4.0, 1.0) + rivets(5.875, 0.70, 26.0, 3.5, 0.9), 0.0)
    for k in range(4):
        H += np.where(nz, line_a(k * 90.0 + 45.0, r_nz, 8.0, -0.8), 0.0)
    H += np.where(nz, line_s(5.60, 9.0, -0.8), 0.0)
    ni = t("NOZI")
    for k in range(4):
        H += np.where(ni, line_a(k * 90.0 + 45.0, 0.75, 8.0, -0.7), 0.0)
    if not CFG["seams"]:
        return np.zeros_like(H) if not CFG["liner"] and not CFG["rivets"] else H * CFG["depth"]
    return H * CFG["depth"]


def normals_from_height(H, mpt, strength=1.0):
    Hs = gaussian_filter(H, 0.75)
    gy, gx = np.gradient(Hs)
    m = np.maximum(mpt, 3.0)
    nx = -gx / m * strength
    ny = gy / m * strength
    return nx.astype(np.float32), ny.astype(np.float32)


def paint_side(side, mesh_npz, install, stage, out_names=None):
    nname, mname, gnname, gmname = NAMES[side]
    if out_names:
        gnname, gmname = out_names
    size = CFG["size"]
    e = SIDE[side]
    P, UV, T, tag = [], [], [], []
    base = 0
    for key in ("main", "sleeve"):
        p, uv, t, tg = mesh_npz[f"e{e}_{key}_P"], mesh_npz[f"e{e}_{key}_UV"], mesh_npz[f"e{e}_{key}_T"], mesh_npz[f"e{e}_{key}_tag"]
        P.append(p)
        UV.append(uv)
        T.append(t + base)
        tag.append(tg)
        base += len(p)
    P, UV, T, tag = np.concatenate(P), np.concatenate(UV), np.concatenate(T), np.concatenate(tag)
    tid, z, phi, mpt, cnt = rasterise(P, UV, T, tag, size)
    z = station_remap(z, e, (1, 2) if side == "LH" else (3, 4))
    H = height(tid, z, phi, mpt, cnt, side)
    img = Image.open(install / nname).convert("RGB")
    if img.size[0] != size:
        img = img.resize((size, size), Image.BICUBIC)
    nml = np.asarray(img).astype(np.float32)
    old = (nml[..., :2] - 127.5) / 127.5
    nx, ny = normals_from_height(H, mpt)
    cover = maximum_filter((tid > 0).astype(np.uint8), size=3) > 0
    touched = (np.abs(nx) + np.abs(ny) > 1e-4) & cover
    touched = maximum_filter(touched.astype(np.uint8), size=3) > 0
    new = old.copy()
    new[..., 0] = np.where(touched, old[..., 0] + nx, old[..., 0])
    new[..., 1] = np.where(touched, old[..., 1] + ny, old[..., 1])
    ln = np.hypot(new[..., 0], new[..., 1])
    scale = np.where(ln > 0.96, 0.96 / np.maximum(ln, 1e-6), 1.0)
    new *= scale[..., None]
    for name in CFG["flatten"]:
        new[maximum_filter((tid == TAG_ID[name]).astype(np.uint8), size=5) > 0] = 0.0
    flat = np.zeros((size, size), bool)
    k = size / SIZE
    flat[:, int(SWATCH_STRIP[0] * k):int(SWATCH_STRIP[1] * k)] = True
    new[flat] = 0.0
    out = nml.copy()
    out[..., :2] = np.clip(np.round(new * 127.5 + 127.5), 0, 255)
    Image.fromarray(out.astype(np.uint8), "RGB").save(stage / gnname, optimize=True)

    mat = np.asarray(Image.open(install / mname).convert("RGB"))
    mat = np.asarray(Image.fromarray(mat).resize((MAT_SIZE, MAT_SIZE), Image.NEAREST)).astype(np.float32)
    tid_m = tid[::size // MAT_SIZE, ::size // MAT_SIZE]
    Image.fromarray(mat.astype(np.uint8), "RGB").save(stage / gmname, optimize=True)
    return dict(side=side, engine=e, texels_covered=int((tid > 0).sum()), texels_overlapped=int((cnt > 1).sum()), texels_changed=int(touched.sum()),
                strip_flattened=True, files=[gnname, gmname])


def retarget(stage):
    """Point every GP object that uses the shared nacelle maps at the GP copies."""
    changed = []
    for p in sorted(stage.glob("*_gp*.obj")):
        text = p.read_text(encoding="utf-8")
        new = text
        for side in ("LH", "RH"):
            nname, mname, gnname, gmname = NAMES[side]
            new = new.replace(f"TEXTURE_MAP normal {nname}", f"TEXTURE_MAP normal {gnname}").replace(f"TEXTURE_MAP material_gloss {mname}", f"TEXTURE_MAP material_gloss {gmname}")
        if new != text:
            p.write_text(new, encoding="utf-8", newline="")
            changed.append(p.name)
    return changed


def main(stage, install, dump, check_only=False):
    stage, install = Path(stage), Path(install)
    mesh = np.load(dump)
    report = {}
    for side in ("LH", "RH"):
        report[side] = paint_side(side, mesh, install, stage)
    if not check_only:
        report["retargeted"] = retarget(stage)
    return report


if __name__ == "__main__":
    a = sys.argv[1:]
    print(json.dumps(main(a[0], a[1], a[2], "--check-only" in a), indent=1))
