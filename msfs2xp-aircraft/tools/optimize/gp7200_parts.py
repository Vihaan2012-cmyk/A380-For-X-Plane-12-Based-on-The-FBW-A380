import numpy as np

import gp7200_fan as fan
from gp7200_geom import SPEC, Mesh, Sheet, cap_ring, frames, old_frames, smooth, surface, unit, wind
from gp7200_morph import engine_triangles, describe, sheet_from
from gp7200_obj import Obj, engine_of_batch
from gp7200_variant import PYLON_BAND, PYLON_FULL, PYLON_NONE, PYLON_REACH, WING_CLEAR, WINGS, engine_of, smoothstep, vertices, wing_floor

OGV = SPEC["fan"]["ogv"]
N_OGV = OGV["count_estimate"]
S0, S1 = OGV["station"]
R_HUB, R_TIP = OGV["hub_r"], OGV["tip_r"]
SPIN_UV, SWIRL_UV = (0.9492, 0.5786), (0.7295, 0.4238)


def ogv_vane(theta, thick_scale=1.0, nr=4, nc=9):
    f = (1 - np.cos(np.linspace(0, np.pi, nc))) / 2
    ts = np.linspace(0, 1, nr)
    chord = S1 - S0
    faces = []
    for sign in (1, -1):
        P = np.zeros((nr, nc, 3))
        for i, t in enumerate(ts):
            r = R_HUB + (R_TIP - R_HUB) * t
            half = 5 * 0.09 * thick_scale * chord * (0.2969 * np.sqrt(f) - 0.126 * f - 0.3516 * f ** 2 + 0.2843 * f ** 3 - 0.1015 * f ** 4)
            lean = np.radians(8) * (1 - t)
            s = S0 + f * chord
            tau = (f - 0.5) * chord * np.tan(np.radians(14)) * (1 - 0.5 * t) + sign * half
            ang = theta + tau / r
            P[i, :, 0] = r * np.sin(ang)
            P[i, :, 1] = r * np.cos(ang)
            P[i, :, 2] = s
        faces.append(P)
    out = Mesh()
    for k, P in enumerate(faces):
        out.extend(surface(P, False, "free", tag="OGV"))
    for P in (np.stack([faces[0][:, 0], faces[1][:, 0]], 1), np.stack([faces[0][:, -1], faces[1][:, -1]], 1)):
        out.extend(surface(P, False, "free", tag="OGV"))
    return out


def ogv_ring(e):
    out = Mesh()
    ph = np.arange(96) * 2 * np.pi / 96
    s = np.array([S0 - 0.03, S1 + 0.03])
    P = np.stack([np.outer(np.full(2, R_HUB), np.sin(ph)), np.outer(np.full(2, R_HUB), np.cos(ph)), np.repeat(s[:, None], 96, axis=1)], axis=-1)
    out.extend(surface(P, True, "out", tag="HUB"))
    for k in range(N_OGV):
        th = 2 * np.pi * k / N_OGV
        strut = min(abs(np.degrees(th) % 180), abs(180 - np.degrees(th) % 180)) < 1.0
        out.extend(ogv_vane(th, 3.0 if strut else 1.0))
    return out


def fan_uv(mesh, e, obj030):
    tris = engine_triangles(obj030, e)
    fo = old_frames()[e]
    d = describe(tris, fo)
    th = np.arctan2(d["cen"][:, 0], d["cen"][:, 1])
    out = Mesh()
    tags = np.array(mesh.tag)
    UV = np.zeros((len(mesh.P), 2))
    pts_all = mesh.P
    r = np.hypot(pts_all[:, 0], pts_all[:, 1])
    th_v = np.arctan2(pts_all[:, 0], pts_all[:, 1])
    vert_tag = np.empty(len(mesh.P), dtype=object)
    for t_idx, tag in zip(mesh.T, mesh.tag):
        vert_tag[t_idx] = tag
    for tag, uv in (("SPIN", SPIN_UV), ("HUB", SPIN_UV), ("CAP", SPIN_UV), ("SCREW", SWIRL_UV), ("SWIRL", SWIRL_UV)):
        UV[np.array([t == tag for t in vert_tag])] = uv
    bl = (d["r"] > 0.6)
    wedge = bl & (np.degrees(th) >= -180) & (np.degrees(th) < -165)
    rb = d["pts"][wedge]
    nth = (d["nr"] * 0)
    nrm = describe_normals(tris, fo)[wedge]
    th_w = th[wedge]
    nt = nrm[:, 0] * np.cos(th_w) - nrm[:, 1] * np.sin(th_w)
    rr = d["r"][wedge]
    ss = d["cen"][wedge, 2]
    bins = np.linspace(rr.min(), rr.max(), 24)
    le = np.array([ss[(rr >= a) & (rr < b)].min() if ((rr >= a) & (rr < b)).any() else np.nan for a, b in zip(bins[:-1], bins[1:])])
    te = np.array([ss[(rr >= a) & (rr < b)].max() if ((rr >= a) & (rr < b)).any() else np.nan for a, b in zip(bins[:-1], bins[1:])])
    mid = (bins[:-1] + bins[1:]) / 2
    ok = ~np.isnan(le)
    uv_w = tris[wedge][:, :, 6:8]
    sheets = {}
    for face, m in ((0, nt > 0), (1, nt <= 0)):
        if m.sum() < 3:
            continue
        t_ = (rr[m] - rr.min()) / (rr.max() - rr.min())
        f_ = (ss[m] - np.interp(rr[m], mid[ok], le[ok])) / np.maximum(np.interp(rr[m], mid[ok], te[ok]) - np.interp(rr[m], mid[ok], le[ok]), 1e-3)
        pts = d["pts"][wedge][m]
        rv = np.hypot(pts[:, :, 0], pts[:, :, 1])
        tv = (rv - rr.min()) / (rr.max() - rr.min())
        lev = np.interp(rv, mid[ok], le[ok])
        tev = np.interp(rv, mid[ok], te[ok])
        fv = (pts[:, :, 2] - lev) / np.maximum(tev - lev, 1e-3)
        sheets[face] = Sheet(fv, tv, uv_w[m], 1.0)
    for face in (0, 1):
        v = np.array([t == "BLADE%d" % face or (face == 0 and t == "BLADE") for t in vert_tag])
        if not v.any() or face not in sheets:
            continue
        p = pts_all[v]
        rv = np.hypot(p[:, 0], p[:, 1])
        t = np.clip((rv - fan.HUB_R) / (fan.TIP_R - fan.HUB_R), 0, 1)
        s_le = np.array([fan.blade_station(x)[1] for x in t])
        ch = np.array([fan.blade_station(x)[2] * np.cos(fan.blade_station(x)[3]) for x in t])
        f = np.clip((p[:, 2] - s_le) / ch, 0, 1)
        uv, _, _ = sheets[face].query(f, t * 1.0)
        UV[v] = uv
    mesh.UV = UV
    return mesh


def describe_normals(tris, fo):
    return fo.local_vec(tris[..., 3:6]).mean(axis=1)


def run_pylon(installed, stage):
    from gp7200_model import FN, FO, write
    from gp7200_obj import write_obj
    spec = SPEC["engine_positions"]["per_engine"]
    wing = np.concatenate([vertices(Path_read(installed / n))[1] for n in WINGS])
    geo = {}
    for e in (1, 2, 3, 4):
        m = spec[str(e)]["current_gp_morph"]
        c = m["fan_disc_centre"]
        geo[e] = dict(cx=c[0], cy=c[1], tip=m["spinner_tip_z"], plug=m["plug_tip_z"], top=SPEC["current_morph_engine2"]["measured"]["max_r_top"])
    src = Obj(installed / "a380_exterior_031_gp.obj")
    vt = src.vt.copy()
    eng = engine_of(vt[:, 0])
    for e, g in geo.items():
        crown_new = FN[e].point(np.array([0.0, 1.82, 1.4]))[1]
        crown_old = FO[e].point(np.array([0.0, g["top"], 1.4]))[1]
        delta = crown_new - crown_old
        p = vt[eng == e]
        near = (np.abs(p[:, 0] - g["cx"]) < PYLON_REACH) & (p[:, 2] > g["tip"] - 0.5) & (p[:, 2] < g["plug"] + 0.5)
        hi = np.minimum(g["top"] + PYLON_NONE, wing_floor(wing, g, p[:, 2]) - WING_CLEAR)
        lo = np.minimum(g["top"] + PYLON_FULL, hi - PYLON_BAND)
        w = near * (1 - smoothstep(p[:, 1] - g["cy"], lo, hi))
        s_o = p[:, 2] - g["tip"]
        dz = np.interp(s_o, [2.0, MORPH_EXIT, MORPH_PLUG], [0.0, 4.967 - MORPH_EXIT, 7.07 - MORPH_PLUG])
        p[:, 1] += w * delta
        p[:, 2] += w * dz
        vt[eng == e] = p
    write(stage / "a380_exterior_031_gp.obj", src, vt, src.idx, src.post)
    print("031", len(vt), "vertices", len(src.idx) // 3, "triangles")


MORPH_EXIT, MORPH_PLUG = 5.063, 7.197


def Path_read(p):
    return p.read_text(encoding="utf-8", errors="replace").split("\n")
