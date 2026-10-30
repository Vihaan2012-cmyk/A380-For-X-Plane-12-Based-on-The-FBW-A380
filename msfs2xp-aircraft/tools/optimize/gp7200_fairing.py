"""usage: gp7200_fairing.py <objects dir> [--check-only]

Closes the aft opening between the pylon ramp and the nacelle tail on every GP7200 engine with a short sloping panel. For each engine the trailing edge
of the ramp roof is found by casting rays up from the nacelle crown; the panel starts 6 cm forward of that edge and 6 mm below the roof underside (tucked behind the ramp), runs aft and down
to rest 2 cm above the crown near the nacelle tail, and is textured with a plain grey metal panel of the pylon atlas (found by its albedo, stretched over the panel), so it needs no texture change. The triangles are appended to a380_exterior_031_gp.obj next to the engine's own pylon batch (same
animation block, so it hides with the engine) and to the twelve a380_engdetach_<e>_a380_exterior_031_gp[_burnt|_crushed].obj copies, the burnt and crushed
copies being rebuilt from the updated main copy with the same deformation as engine_damage.py. Run it after gp7200_pylon.py, before collision_data."""

import json
import re
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import engine_damage as dmg  # noqa: E402
import gp7200_exposure as X  # noqa: E402
from gp7200_geom import surface  # noqa: E402
from gp7200_obj import Obj, engine_of_batch  # noqa: E402
from gp7200_pylon import crown, seg_tri  # noqa: E402

FN = X.FN
A_COLS = np.linspace(-0.36, 0.36, 9)
N_ROWS = 6
TUCK, CLEAR, SEAM = 0.06, 0.02, 0.006
S_BOT = (5.0, 5.15)
UV_MARGIN = 0.004
DETAIL_UV = True


def roof_edge(tris, f):
    """For every column a: station where the ramp roof ends and the roof's underside height above the crown there (and TUCK forward of it)."""
    ss = np.arange(3.8, 5.6, 0.025)
    out = []
    for a in A_COLS:
        ac = float(np.clip(a, -0.30, 0.30))
        org = np.array([f.point(np.array([ac, crown(ac, s) + 0.03, s])) for s in ss])
        h = X.ray_hits(org, np.tile(f.up, (len(ss), 1)), tris)
        roof = np.isfinite(h) & (h > 0.4)
        s_end = float(ss[roof].max()) if roof.any() else 4.8
        k = int(np.argmin(np.abs(ss - (s_end - TUCK))))
        out.append((s_end, float(h[k] + 0.03) if np.isfinite(h[k]) else 1.0))
    return np.array(out)


def panel(f, edge):
    top, bot = [], []
    for a, (s_end, h) in zip(A_COLS, edge):
        s_t = s_end - TUCK
        s_b = float(np.clip(s_end + 0.30, *S_BOT))
        top.append(f.point(np.array([a, crown(a, s_t) + h, s_t])))
        bot.append(f.point(np.array([a, crown(a, s_b) + CLEAR, s_b])))
    top, bot = np.array(top), np.array(bot)
    v = np.linspace(0, 1, N_ROWS)[:, None, None]
    top = top - f.up * SEAM
    return top[None] + (bot - top)[None] * v


def grey_panel(pyl, f, tris_idx, albedo):
    """UV bounds of a plain mid-grey metal panel of the pylon atlas near the ramp: among the UV-connected triangle groups of the ramp area, the largest one
    whose mean albedo is a neutral grey between 90 and 125 and whose UV box is under 0.1 wide."""
    P, UV = pyl.vt[:, :3], pyl.vt[:, 6:8]
    loc = f.local(P)
    key = np.unique(np.round(P / 1e-4).astype(np.int64), axis=0, return_inverse=True)[1].reshape(-1)
    adj = {}
    for i, t in enumerate(tris_idx):
        for v in t:
            adj.setdefault(key[v], []).append(i)
    h, w, _ = albedo.shape
    seen_all, best = set(), None
    for i0 in range(len(tris_idx)):
        if i0 in seen_all:
            continue
        seen, stack = {i0}, [i0]
        while stack:
            i = stack.pop()
            for v in tris_idx[i]:
                for j in adj[key[v]]:
                    if j not in seen and any(key[u] == key[v] and np.linalg.norm(UV[u] - UV[v]) < 0.01 for u in tris_idx[j]):
                        seen.add(j)
                        stack.append(j)
        seen_all |= seen
        vs = np.unique(tris_idx[list(seen)].ravel())
        uv = UV[vs]
        span = uv.max(0) - uv.min(0)
        s_mid = loc[vs, 2].mean()
        if len(seen) < 30 or span.max() > 0.1 or not 3.4 < s_mid < 5.0:
            continue
        px = np.clip((uv * [w, h]).astype(int), 0, [w - 1, h - 1])
        col = albedo[h - 1 - px[:, 1], px[:, 0]].mean(0)
        if abs(col[0] - col[2]) > 15 or not 90 < col.mean() < 125:
            continue
        if best is None or len(seen) > best[0]:
            best = (len(seen), uv.min(0) + UV_MARGIN, uv.max(0) - UV_MARGIN, col)
    if best is None:
        raise RuntimeError("no grey panel found")
    return best


def wall_panel(pyl, f, tris_idx, albedo):
    """UV box of the riveted side-wall panel of the ramp: the UV-connected group of lateral-facing triangles of the ramp walls with the most albedo detail (rivet rows, panel lines)."""
    P, N, UV = pyl.vt[:, :3], pyl.vt[:, 3:6], pyl.vt[:, 6:8]
    loc = f.local(P)
    key = np.unique(np.round(P / 1e-4).astype(np.int64), axis=0, return_inverse=True)[1].reshape(-1)
    lat = np.abs(N[tris_idx].mean(axis=1) @ f.side) > 0.7
    idx = tris_idx[lat]
    adj = {}
    for i, t in enumerate(idx):
        for v in t:
            adj.setdefault(key[v], []).append(i)
    h, w, _ = albedo.shape
    seen_all, best = set(), None
    for i0 in range(len(idx)):
        if i0 in seen_all:
            continue
        seen, stack = {i0}, [i0]
        while stack:
            i = stack.pop()
            for v in idx[i]:
                for j in adj[key[v]]:
                    if j not in seen and any(key[u] == key[v] and np.linalg.norm(UV[u] - UV[v]) < 0.01 for u in idx[j]):
                        seen.add(j)
                        stack.append(j)
        seen_all |= seen
        vs = np.unique(idx[list(seen)].ravel())
        uv = UV[vs]
        span = uv.max(0) - uv.min(0)
        if len(seen) < 25 or span.min() < 0.035 or span.max() > 0.14 or not 3.3 < loc[vs, 2].mean() < 5.0:
            continue
        x0, x1 = (uv.min(0) * [w, h]).astype(int), (uv.max(0) * [w, h]).astype(int)
        patch = albedo[h - 1 - x1[1]:h - 1 - x0[1] + 1, x0[0]:x1[0] + 1].astype(float)
        if patch.size == 0 or not 70 < patch.mean() < 190:
            continue
        score = patch.std()
        if best is None or score > best[0]:
            best = (score, uv.min(0), uv.max(0), len(seen))
    if best is None:
        raise RuntimeError("no wall panel found")
    return best


def build(e, pyl, albedo):
    f = FN[e]
    tris_idx = X.window_triangles(pyl, f, a_lim=1.0, s_lim=(2.4, 5.4), b_lim=(0.5, 3.6), engine=e)
    all_tris = pyl.vt[:, :3][X.window_triangles(pyl, f, a_lim=3.0, s_lim=(1.0, 6.5), b_lim=(0.5, 4.0), engine=e)]
    edge = roof_edge(all_tris, f)
    P = panel(f, edge)
    m = surface(P, False, "free", tag="FAIR")
    if float(np.mean(m.N @ f.d)) < 0:
        m = surface(P[::-1], False, "free", tag="FAIR")
    if DETAIL_UV:
        score, lo, hi, n = wall_panel(pyl, f, tris_idx, albedo)
    else:
        n, lo, hi, score = grey_panel(pyl, f, tris_idx, albedo)
        score = 0.0
    loc = f.local(m.P)
    c = loc[:, 1] - crown(loc[:, 0], loc[:, 2])
    u = (loc[:, 0] + A_COLS.max()) / (2 * A_COLS.max())
    v = 1.0 - np.clip(c / max(float(c.max()), 1e-3), 0, 1)
    m.UV = np.stack([lo[0] + u * (hi[0] - lo[0]), lo[1] + (1 - v) * (hi[1] - lo[1])], axis=1)
    m.UV = np.clip(m.UV, lo, hi)
    return m, dict(roof_end=[round(float(x), 3) for x in edge[:, 0]], roof_h=[round(float(x), 3) for x in edge[:, 1]], wall_panel_tris=n, wall_panel_detail=round(float(score), 1), wall_panel_uv=[round(float(x), 3) for x in (*lo, *hi)])


def crossing_count(a, b):
    """Number of crossing triangle pairs between two triangle sets (n, 3, 3)."""
    lo_a, hi_a = a.min(axis=1), a.max(axis=1)
    lo_b, hi_b = b.min(axis=1), b.max(axis=1)
    hits = 0
    for i0 in range(0, len(a), 40):
        sl = slice(i0, i0 + 40)
        ov = np.all((lo_a[sl][:, None] <= hi_b[None]) & (hi_a[sl][:, None] >= lo_b[None]), axis=2)
        ia, ib = np.nonzero(ov)
        if not len(ia):
            continue
        A, B = a[sl][ia], b[ib]
        hit = np.zeros(len(ia), bool)
        for k in range(3):
            hit |= seg_tri(A[:, k], A[:, (k + 1) % 3], B[:, 0], B[:, 1], B[:, 2])
            hit |= seg_tri(B[:, k], B[:, (k + 1) % 3], A[:, 0], A[:, 1], A[:, 2])
        hits += int(hit.sum())
    return hits


def poke_check(objects, e, mesh):
    """Crossings of the fairing with the nacelle skin, the pylon, the airframe (wing, flaps), the dressing and the core of engine e."""
    objects = Path(objects)
    f = FN[e]
    tri = mesh.P[mesh.T]
    out = {}
    for label, name, engine in (("skin", X.SKIN[e], e), ("pylon", "a380_exterior_031_gp.obj", e), ("airframe", "a380_exterior_001_gp.obj", None),
                                ("dressing", "a380_engdress_gp.obj", None), ("core", "a380_engcore_gp.obj", None)):
        o = Obj(objects / name)
        T = X.window_triangles(o, f, a_lim=1.6, s_lim=(3.8, 6.0), b_lim=(0.5, 4.0), engine=engine)
        other = o.vt[:, :3][T]
        if label == "pylon":
            fair = {tuple(np.round(p, 4)) for p in tri.reshape(-1, 3)}
            other = np.array([t for t in other if not any(tuple(np.round(p, 4)) in fair for p in t)]).reshape(-1, 3, 3)
        out[label] = crossing_count(tri, other) if len(other) else 0
    loc = f.local(mesh.P)
    out["min_clearance_over_crown"] = round(float((loc[:, 1] - crown(loc[:, 0], loc[:, 2])).min()), 4)
    return out


def vt_lines(m, centre=None):
    P = m.P if centre is None else m.P - np.asarray(centre)
    return [f"VT {p[0]:.5f} {p[1]:.5f} {p[2]:.5f} {n[0]:.5f} {n[1]:.5f} {n[2]:.5f} {u[0]:.5f} {u[1]:.5f}" for p, n, u in zip(P, m.N, m.UV)]


def idx_lines(tokens):
    out = []
    k = 0
    while k + 10 <= len(tokens):
        out.append("IDX10 " + " ".join(map(str, tokens[k:k + 10])))
        k += 10
    out += [f"IDX {t}" for t in tokens[k:]]
    return out


def counts(lines):
    nv = sum(1 for l in lines if l.startswith("VT"))
    ni = sum(len(l.split()) - 1 for l in lines if l.startswith(("IDX10", "IDX ")))
    return nv, ni


def append_main(path, meshes):
    lines = Path(path).read_text(encoding="utf-8").split("\n")
    obj = Obj(path)
    last_vt = max(i for i, l in enumerate(lines) if l.startswith("VT"))
    last_idx = max(i for i, l in enumerate(lines) if l.startswith(("IDX10", "IDX ")))
    post0 = last_idx + 1
    nv, ni = counts(lines)
    ins, new_vt, new_idx = [], [], []
    spans = {}
    for e, m in sorted(meshes.items()):
        base = nv + len(new_vt)
        start = ni + len(new_idx)
        new_vt += vt_lines(m)
        new_idx += [int(base + i) for i in m.T.ravel()]
        spans[e] = (start, len(m.T) * 3)
        batches = [b for b in obj.tris() if engine_of_batch(b) == e - 1]
        ins.append((post0 + max(b["line"] for b in batches), f"TRIS {start} {len(m.T) * 3}"))
    for pos, text in sorted(ins, reverse=True):
        lines.insert(pos + 1, text)
    lines[last_idx + 1:last_idx + 1] = idx_lines(new_idx)
    lines[last_vt + 1:last_vt + 1] = new_vt
    pc = next(i for i, l in enumerate(lines) if l.startswith("POINT_COUNTS"))
    parts = lines[pc].split()
    parts[1], parts[4] = str(nv + len(new_vt)), str(ni + len(new_idx))
    lines[pc] = " ".join(parts)
    Path(path).write_text("\n".join(lines), encoding="utf-8", newline="")
    return spans


def append_detach(path, m, centre):
    lines = Path(path).read_text(encoding="utf-8").split("\n")
    last_vt = max(i for i, l in enumerate(lines) if l.startswith("VT"))
    last_idx = max(i for i, l in enumerate(lines) if l.startswith(("IDX10", "IDX ")))
    last_tris = max(i for i, l in enumerate(lines) if l.startswith("TRIS "))
    nv, ni = counts(lines)
    lines.insert(last_tris + 1, f"TRIS {ni} {len(m.T) * 3}")
    lines[last_idx + 1:last_idx + 1] = idx_lines([int(nv + i) for i in m.T.ravel()])
    lines[last_vt + 1:last_vt + 1] = vt_lines(m, centre)
    pc = next(i for i, l in enumerate(lines) if l.startswith("POINT_COUNTS"))
    parts = lines[pc].split()
    parts[1], parts[4] = str(nv + len(m.P)), str(ni + len(m.T) * 3)
    lines[pc] = " ".join(parts)
    Path(path).write_text("\n".join(lines), encoding="utf-8", newline="")


def rebuild_damage(objects, e, manifest):
    ent = manifest["gp"]["engines"][str(e - 1)]
    holes = dmg.dents(np.random.default_rng(1000 + (e - 1)), np.array(ent["half_extent"]))
    stem = f"a380_engdetach_{e - 1}_a380_exterior_031_gp"
    main = objects / f"{stem}.obj"
    src = main.read_text(encoding="utf-8", errors="replace").split("\n")[:60]
    burnt_old = (objects / f"{stem}_burnt.obj").read_text(encoding="utf-8", errors="replace").split("\n")[:60]
    textures = {a: b for a, b in zip(src, burnt_old) if a.startswith("TEXTURE ") and a != b}
    dmg.variant(main, objects / f"{stem}_crushed.obj", holes, ent["bottom"])
    dmg.variant(main, objects / f"{stem}_burnt.obj", holes, ent["bottom"], textures)


def main(objects, check_only=False, panel_detail=True):
    global DETAIL_UV
    DETAIL_UV = panel_detail
    objects = Path(objects)
    main_path = objects / "a380_exterior_031_gp.obj"
    manifest = json.loads((objects / "a380_engdetach.json").read_text(encoding="utf-8"))
    pyl = Obj(main_path)
    from PIL import Image
    albedo = np.asarray(Image.open(objects / pyl.texture()).convert("RGB"))
    meshes, report = {}, {}
    for e in (1, 2, 3, 4):
        meshes[e], info = build(e, pyl, albedo)
        report[e] = dict(triangles=len(meshes[e].T), vertices=len(meshes[e].P), **info)
    if check_only:
        return report
    for e, m in meshes.items():
        report[e]["poke"] = poke_check(objects, e, m)
    append_main(main_path, meshes)
    for e, m in meshes.items():
        centre = manifest["gp"]["engines"][str(e - 1)]["centre"]
        append_detach(objects / f"a380_engdetach_{e - 1}_a380_exterior_031_gp.obj", m, centre)
        rebuild_damage(objects, e, manifest)
    return report


if __name__ == "__main__":
    print(json.dumps(main(sys.argv[1], "--check-only" in sys.argv), indent=1))
