"""usage: gp7200_model.py <objects dir> [--only NAME ...]

Writes the GP7200 twins (a380_exterior_001/020-023/030/031/034-041_gp.obj and a380_engine_covers_gp.obj) from the measured spec
(gp7200_spec.json) instead of morphing the Trent meshes. Each twin keeps the installed twin's ANIM lines, gates, texture lines and
per-engine batches; the engine batches are replaced by new geometry in the pre-cowl-split layout (one batch per engine, the translating
sleeve and the reverser hardware kept as their own batches), so engine_detach, cowl_separation, cowl_clip and cowl_hinges can run on
the result exactly as they did on gp7200_variant's output. Skin UVs come from the installed morph by (station, angle) correspondence."""

import re
import sys
from pathlib import Path

import numpy as np

import gp7200_parts as core_ext
import gp7200_fan as fan
import gp7200_nacelle as nac
from gp7200_geom import (
    HL_BOT, HL_TOP, OLD_AXIS, S_LIP_END, S_NOSE, S_PLUG, S_TE, S_TR, SPEC, Mesh, frames, old_frames, smooth, transfer, unit, wind,
)
from gp7200_morph import engine_triangles, sheet_from
from gp7200_obj import COWL, ENGSEP, Obj, engine_of_batch, write_obj

FN = frames()
FO = old_frames()
DELTA = {e: float((FN[e].o - FO[e].o) @ FO[e].d) for e in FN}
ENG_OBJ = {1: "022", 2: "022", 3: "023", 4: "023"}
CORE_OBJ = {1: "020", 2: "020", 3: "021", 4: "021"}
BLUR = [f"a380_exterior_0{k}" for k in range(34, 42)]
COVER_RE = re.compile(r"fbw/walkaround/eng_inlet_cover_(\d)")
OLD_TIP_R = SPEC["current_morph_engine2"]["measured"]["blade_tip_radius"]
FAN_SCALE = fan.TIP_R / OLD_TIP_R
MORPH_EXIT, MORPH_NOZZLE, MORPH_PLUG = 5.063, 6.0, 7.197
PANEL_FIX = True
PARTS = True
SIDE_SWATCH = {"SLAT": {1: (0.58423, 0.56323), 2: (0.58423, 0.56323), 3: (0.21997, 0.42603), 4: (0.21997, 0.42603)},
               "CASEBAND": {1: (0.73413, 0.78149), 2: (0.73413, 0.78149), 3: (0.90356, 0.07935), 4: (0.90356, 0.07935)},
               "CASEAFT": {1: (0.58423, 0.56323), 2: (0.58423, 0.56323), 3: (0.21997, 0.42603), 4: (0.21997, 0.42603)}}
PANEL_UV = {1: (0.41187, 0.84497), 2: (0.41187, 0.84497), 3: (0.84058, 0.41772), 4: (0.84058, 0.41772)}
SWATCH = {"PANEL": (0.04, 0.45), "LOUV": (0.36, 0.50), "FIT": (0.12, 0.62), "DOOR": (0.30, 0.62), "RED": (0.93, 0.35), "OGV": (0.08, 0.52), "HUB": (0.08, 0.52)}
GREY_PANEL = (0.04, 0.45)
CORE_GREY = {1: (0.0361, 0.5894), 2: (0.0361, 0.5894), 3: (0.0996, 0.5557), 4: (0.0996, 0.5557)}
CORE_DARK = {1: (0.8066, 0.8867), 2: (0.8066, 0.8867), 3: (0.7964, 0.7339), 4: (0.7964, 0.7339)}

MASKS = {
    "OUT": lambda d: (d["r"] > 1.55) & (d["nr"] > 0.3) & (d["cen"][:, 2] > -1.0),
    "DOORIN": lambda d: (d["r"] > 1.55) & (d["nr"] < -0.3) & (d["cen"][:, 2] > -0.3) & (d["cen"][:, 2] < 2.7),
    "TRIN": lambda d: (d["cen"][:, 2] > 2.6) & (d["r"] > 1.3) & (d["r"] < 1.62) & (d["nr"] < -0.3),
    "CASE": lambda d: (d["r"] < 1.65) & (d["r"] > 1.3) & (d["nr"] < -0.3) & (d["cen"][:, 2] < 0.6),
    "CCOW": lambda d: (d["r"] < 1.3) & (d["r"] > 0.7) & (d["cen"][:, 2] > 1.0) & (d["cen"][:, 2] < 5.2) & (d["nr"] > 0.3),
    "NOZO": lambda d: (d["cen"][:, 2] > 4.2) & (d["r"] < 0.95) & (d["r"] > 0.55) & (d["nr"] > 0.2),
    "NOZI": lambda d: (d["cen"][:, 2] > 4.2) & (d["r"] < 0.95) & (d["r"] > 0.45) & (d["nr"] < -0.2),
    "PLUG": lambda d: (d["cen"][:, 2] > 5.0) & (d["r"] < 0.55) & (d["nr"] > 0.0),
}
SEAM_BIAS = 0.02
PLUG_S_LO, PLUG_S_HI = 6.15, 7.15
INNER_OR_SKIN = {"OUT", "DOORIN", "TRIN", "CASE", "NOZI"}
LIP_MASK = lambda d: (d["cen"][:, 2] < -0.6) & (d["r"] > 1.3)
RREF = {"OUT": 1.8, "DOORIN": 1.7, "TRIN": 1.5, "CASE": 1.5, "CCOW": 1.0, "NOZO": 0.8, "NOZI": 0.8, "PLUG": 0.4}
TAG_SHEET = {"OUT": "OUT", "TE": "OUT", "FIT": "OUT", "DOOR": "OUT", "RED": "OUT", "DOORIN": "DOORIN", "TRIN": "TRIN", "CASE": "CASE", "CCOW": "CCOW", "NOZO": "NOZO", "NOZI": "NOZI", "PLUG": "PLUG"}


def station_map(e):
    d = DELTA[e]

    def smap(s, th):
        w = (1 - np.cos(th)) / 2
        shm = SPEC["current_morph_engine2"]["measured"]["highlight_top_s"] + (SPEC["current_morph_engine2"]["measured"]["highlight_bottom_s"] - SPEC["current_morph_engine2"]["measured"]["highlight_top_s"]) * w
        shn = HL_TOP + (HL_BOT - HL_TOP) * w
        lip = shm + (s - shn) * ((S_LIP_END + d) - shm) / (S_LIP_END - shn)
        body = np.interp(s, [S_LIP_END, S_TR, S_TE, 5.917, S_PLUG, 9.0], [S_LIP_END + d, S_TR + d, MORPH_EXIT, MORPH_NOZZLE, MORPH_PLUG, 9.2])
        sm = np.where(s < S_LIP_END, lip, body)
        for seam in (S_NOSE, S_TR):
            edge = seam + d
            sm = np.where(s <= seam + 0.002, np.minimum(sm, np.where(sm > edge - SEAM_BIAS, edge - SEAM_BIAS, sm)), sm)
            sm = np.where(s >= seam + 0.004, np.maximum(sm, np.where(sm < edge + SEAM_BIAS, edge + SEAM_BIAS, sm)), sm)
        return sm

    return smap


def sheets_for(e, obj_eng, obj_core):
    tris = engine_triangles(obj_eng, e)
    out = {}
    for k, m in MASKS.items():
        out[k] = sheet_from(tris, FO[e], m, RREF[k], "min" if k in INNER_OR_SKIN else "max", 150 if k == "OUT" else 40, 0.15 if k == "OUT" else 0.0, k == "OUT")
    core = engine_triangles(obj_core, e)
    out["LIPM"] = sheet_from(core, FO[e], LIP_MASK, 1.7)
    return out


def apply_uv(mesh, e, sheets):
    smap = station_map(e)
    by_tag = {}
    tags = np.array(mesh.tag)
    out = Mesh()
    for tag in sorted(set(mesh.tag)):
        sel = tags == tag
        sub = Mesh()
        T = mesh.T[sel]
        used, inv = np.unique(T.ravel(), return_inverse=True)
        sub.P, sub.N, sub.UV, sub.T = mesh.P[used], mesh.N[used], mesh.UV[used], inv.reshape(-1, 3)
        sub.tag = [tag] * len(sub.T)
        key = TAG_SHEET.get(tag, tag if tag == "LIPM" else None)
        if key and sheets.get(key) is not None:
            sm = smap
            if tag == "PLUG":
                sm = lambda s, th, base=smap: np.clip(base(s, th), PLUG_S_LO, PLUG_S_HI)
            if tag == "CASE":
                limit = float(np.max(sheets["CASE"].tri[:, :, 0]))
                sm = lambda s, th, base=smap, lim=limit: np.minimum(base(s, th), lim)
            sheets[key].fold = sheets[key].find_seam(6.5) if tag == "PLUG" else None
            transfer(sheets[key], sub, sm)
        elif tag in SIDE_SWATCH:
            sub.UV = np.tile(SIDE_SWATCH[tag][e], (len(sub.P), 1))
        elif tag in SWATCH:
            sub.UV = np.tile(PANEL_UV[e] if tag == "PANEL" and PANEL_FIX else SWATCH[tag], (len(sub.P), 1))
        out.add(sub.P, sub.N, sub.UV, sub.T, tag)
        out.tag[-len(sub.T):] = sub.tag
    return out


def world(mesh, e):
    return FN[e].point(mesh.P), unit(FN[e].vec(mesh.N)), mesh.UV, mesh.T


class Out:
    def __init__(self):
        self.P, self.N, self.UV, self.idx = [], [], [], []
        self.nv = 0

    def add(self, P, N, UV, T):
        start = sum(len(t) for t in self.idx)
        self.P.append(P)
        self.N.append(N)
        self.UV.append(UV)
        self.idx.append((np.asarray(T) + self.nv).ravel())
        self.nv += len(P)
        return start, len(self.idx[-1])

    def arrays(self):
        vt = np.hstack([np.vstack(self.P), np.vstack(self.N), np.vstack(self.UV)]) if self.P else np.zeros((0, 8))
        idx = np.concatenate(self.idx) if self.idx else np.zeros(0, np.int64)
        return vt, idx


def flatten(post):
    out = []
    i = 0
    while i < len(post):
        if post[i].strip() == "ANIM_begin" and i + 1 < len(post) and COWL.match(post[i + 1].strip()):
            depth = 0
            while i < len(post):
                t = post[i].strip()
                if t == "ANIM_begin":
                    depth += 1
                elif t == "ANIM_end":
                    depth -= 1
                    if depth == 0:
                        i += 1
                        break
                i += 1
            continue
        out.append(post[i])
        i += 1
    return out


def batch_kind(b):
    anim = b["anim"]
    if any(t.startswith("ANIM_rotate_begin") and "thrust_rev" in t for t in anim):
        return "blocker"
    if any(t.startswith("ANIM_trans_begin") and "thrust_rev" in t for t in anim):
        return "sleeve"
    return "main"


def retarget(post, line, start, count):
    pad = post[line][:len(post[line]) - len(post[line].lstrip())]
    post[line] = f"{pad}TRIS {start} {count}"


def affine(e, k, shift, rigid=False):
    """Old-world -> new-world: the old engine's local (a, b, s) with the radius scaled by k about the axis and s moved by shift."""
    def point(p):
        q = FO[e].local(np.asarray(p, float))
        if not rigid:
            q = np.stack([q[..., 0] * k, q[..., 1] * k, q[..., 2] + shift], axis=-1)
        else:
            q = q + np.array([0.0, 0.0, shift])
        return FN[e].point(q)

    def vec(v):
        q = FO[e].local_vec(np.asarray(v, float))
        q = np.stack([q[..., 0] / k, q[..., 1] / k, q[..., 2]], axis=-1) if not rigid else q
        return unit(FN[e].vec(q))

    return point, vec


def engine_from_x(x):
    a = abs(x)
    return (1 if x < 0 else 4) if a >= 20.25 else (2 if x < 0 else 3)


def fill_object(name, installed, builders, keep, stage):
    """name: 'a380_exterior_022'; builders[(e, kind)] -> (P, N, UV, T) world arrays; keep[e] -> kind of transform for batches no builder covers."""
    src = Obj(installed / f"{name}_gp.obj")
    post = flatten(src.post)
    src.post = post
    out = Out()
    batches = src.tris()
    for b in batches:
        e = engine_of_batch(b)
        if e is None:
            m = [COVER_RE.search(t) for t in b["anim"]]
            m = next((x for x in m if x), None)
            e = int(m.group(1)) - 1 if m else None
        arr = src.batch_arrays(b)
        if e is None:
            e = engine_from_x(float(arr[:, :, 0].mean())) - 1
        e += 1
        kind = batch_kind(b)
        if (e, kind) in builders:
            res = builders[(e, kind)]
        else:
            fn = keep(e, kind, b)
            P = arr[:, :, :3].reshape(-1, 3)
            N = arr[:, :, 3:6].reshape(-1, 3)
            UV = arr[:, :, 6:8].reshape(-1, 2)
            point, vec = fn
            res = (point(P), vec(N), UV, np.arange(len(P)).reshape(-1, 3))
        s, c = out.add(*res)
        retarget(post, b["line"], s, c)
    vt, idx = out.arrays()
    return src, vt, idx


def move_pivots(post, fn_for):
    """Move the static ANIM_trans pivot pair around every rotation on an N1 or reverser-door dataref."""
    pat = re.compile(r"ANIM_rotate_begin \S+ \S+ \S+ fbw/anim/(N1_\d_anim|thrust_rev_\d)$")
    out = list(post)
    for i in range(len(out) - 1):
        a = out[i].strip().split()
        if a[:1] != ["ANIM_trans"] or len(a) != 7 or a[1:4] != a[4:7]:
            continue
        m = pat.match(out[i + 1].strip())
        if not m:
            continue
        j = i + 1
        while out[j].strip().startswith("ANIM_rotate_"):
            j += 1
        b = out[j].strip().split()
        p = np.array([float(v) for v in a[1:4]])
        if b[:1] != ["ANIM_trans"] or [float(v) for v in b[1:4]] != [-v for v in p]:
            continue
        q = fn_for(m.group(1), p)
        pad = out[i][:len(out[i]) - len(out[i].lstrip())]
        padj = out[j][:len(out[j]) - len(out[j].lstrip())]
        out[i] = pad + "ANIM_trans " + " ".join(f"{v:.6f}" for v in [*q, *q])
        out[j] = padj + "ANIM_trans " + " ".join(f"{-v:.6f}" for v in [*q, *q])
    return out


def set_n1_axes(post):
    out = list(post)
    for i, l in enumerate(out):
        m = re.match(r"(\s*)ANIM_rotate_begin (\S+) (\S+) (\S+) fbw/anim/N1_(\d)_anim$", l)
        if m:
            e = int(m.group(5))
            d = -FN[e].d
            out[i] = f"{m.group(1)}ANIM_rotate_begin {d[0]:.6f} {d[1]:.6f} {d[2]:.6f} fbw/anim/N1_{e}_anim"
    return out


def pivot_fn(kind_scale):
    def fn(key, p):
        if key.startswith("N1_"):
            e = int(key[3])
            return FN[e].point(np.array([0.0, 0.0, FN[e].local(p)[2]]))
        e = int(key.split("_")[2]) + 1
        point, _ = affine(e, kind_scale, -DELTA[e])
        return point(p)
    return fn


def write(path, src, vt, idx, post, marks=("# gp7200_model", "# gp7200_exhaust")):
    pre = [l for l in src.pre if l not in marks] + list(marks)
    write_obj(path, pre, vt, idx, post)


def build_nacelle_meshes(e, sheets):
    fan_s, tr, door_st = nac.outer_skin()
    parts_m = [fan_s, nac.door_inner(door_st), nac.barrel_inner(), nac.core_cowl(), *nac.nozzle(), nac.plug(), nac.upper_fairing(), nac.oil_door(-1), nac.red_line()]
    if not PANEL_FIX:
        parts_m.extend(nac.lower_panel(-1 if e in (1, 2) else 1))
    if PARTS:
        parts_m += [nac.door_details(), nac.tr_vent(), nac.latch_dots(-1 if e in (1, 2) else 1), nac.lip_joint()]
    parts_t = [tr, nac.tr_inner(), nac.te_ring(), nac.step_band()]
    if e not in (2, 3):
        parts_m += parts_t
        parts_t = []
    return parts_m, parts_t


def merge(parts):
    m = Mesh()
    for p in parts:
        if not len(p.tag) == len(p.T):
            p.tag = [""] * len(p.T)
        m.extend(p)
    return m


def run_nacelle(installed, stage):
    objs = {n: Obj(installed / f"a380_exterior_{n}_gp.obj") for n in ("022", "023", "020", "021")}
    nac_b, core_b, stats = {}, {}, {}
    tag_dump = {}
    for e in (1, 2, 3, 4):
        sheets = sheets_for(e, objs[ENG_OBJ[e]], objs[CORE_OBJ[e]])
        pm, pt = build_nacelle_meshes(e, sheets)
        for key, parts in (("main", pm), ("sleeve", pt)):
            if not parts:
                continue
            m = apply_uv(merge(parts), e, sheets)
            nac_b[(e, key)] = world(m, e)
            stats[(e, key)] = m.tri_count()
            tag_dump[f"e{e}_{key}_P"] = m.P
            tag_dump[f"e{e}_{key}_N"] = m.N
            tag_dump[f"e{e}_{key}_UV"] = m.UV
            tag_dump[f"e{e}_{key}_T"] = m.T
            tag_dump[f"e{e}_{key}_tag"] = np.array(m.tag)
        lip_skin, lip_aft = nac.lip_parts()
        lip_aft.tag = ["LIPM"] * len(lip_aft.T)
        lipm = apply_uv(lip_aft, e, sheets)
        lip_skin.UV = np.tile(CORE_GREY[e], (len(lip_skin.P), 1))
        ogv = core_ext.ogv_ring(e)
        flat = [lipm, lip_skin, ogv]
        if PANEL_FIX:
            panel, louv = nac.lower_panel(-1 if e in (1, 2) else 1)
            panel.UV = np.tile(CORE_GREY[e], (len(panel.P), 1))
            louv.UV = np.tile(CORE_DARK[e], (len(louv.P), 1))
            flat.extend([panel, louv])
        core_b[(e, "main")] = world(merge(flat), e)
        stats[(e, "core")] = sum(m.tri_count() for m in flat)
    dump = Path(stage).parent / "nacelle_dump"
    dump.mkdir(parents=True, exist_ok=True)
    np.savez_compressed(dump / "nacelle_tags.npz", **tag_dump)
    for n, mapping in (("022", nac_b), ("023", nac_b), ("020", core_b), ("021", core_b)):
        engines = {"022": (1, 2), "023": (3, 4), "020": (1, 2), "021": (3, 4)}[n]
        builders = {k: v for k, v in mapping.items() if k[0] in engines}

        def keep(e, kind, b, n=n):
            if kind == "blocker":
                return affine(e, 1.06, -DELTA[e])
            return affine(e, 1.0, -DELTA[e], rigid=True)

        src, vt, idx = fill_object(f"a380_exterior_{n}", installed, builders, keep, stage)
        post = move_pivots(src.post, pivot_fn(1.06))
        write(stage / f"a380_exterior_{n}_gp.obj", src, vt, idx, post)
        print(n, len(vt), "vertices", len(idx) // 3, "triangles")
    return stats


def run_fan(installed, stage):
    objs = Obj(installed / "a380_exterior_030_gp.obj")
    builders = {}
    for e in (1, 2, 3, 4):
        m = fan.fan_assembly()
        m = core_ext.fan_uv(m, e, objs)
        builders[(e, "main")] = world(m, e)
    src, vt, idx = fill_object("a380_exterior_030", installed, builders, None, stage)
    post = set_n1_axes(move_pivots(src.post, pivot_fn(1.0)))
    write(stage / "a380_exterior_030_gp.obj", src, vt, idx, post, marks=("# gp7200_model",))
    print("030", len(vt), "vertices", len(idx) // 3, "triangles")
    for n in BLUR:
        def keep(e, kind, b):
            return affine(e, FAN_SCALE, 0.0)

        src, vt, idx = fill_object(n, installed, {}, keep, stage)
        post = set_n1_axes(move_pivots(src.post, pivot_fn(1.0)))
        write(stage / f"{n}_gp.obj", src, vt, idx, post, marks=("# gp7200_model",))
        print(n, len(vt), "vertices", len(idx) // 3, "triangles")


def run_hardware(installed, stage):
    for name, k, shift in (("a380_exterior_001", 0.93, True), ("a380_engine_covers", 0.93, True)):
        def keep(e, kind, b, k=k):
            return affine(e, k, -DELTA[e])

        src, vt, idx = fill_object(name, installed, {}, keep, stage)
        write(stage / f"{name}_gp.obj", src, vt, idx, move_pivots(src.post, pivot_fn(k)), marks=("# gp7200_model",))
        print(name, len(vt), "vertices", len(idx) // 3, "triangles")


def main(installed, stage, pylon=True, panel_fix=True, parts=True, fan_detail=True):
    global PANEL_FIX, PARTS
    PANEL_FIX, PARTS = panel_fix, parts
    fan.DETAIL = fan_detail
    installed, stage = Path(installed), Path(stage)
    run_nacelle(installed, stage)
    run_fan(installed, stage)
    run_hardware(installed, stage)
    if pylon:
        core_ext.run_pylon(installed, stage)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
