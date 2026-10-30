"""usage: gp7200_core.py <installed objects dir> <stage objects dir>

Builds a380_engcore_gp.obj, the GP7200's internal core (splitter, 5-stage booster, intermediate case, 9-stage HPC, diffuser, annular
combustor with fuel nozzles, 2-stage HPT, turbine centre frame, 6-stage LPT, turbine exhaust case, shafts, casings with flanges, the
core-mounted gearbox and the externals traced from the cowl-off photos) from gp7200_spec.json, with its own texture set
A380_GPCORE_ALB/NML/MAT.png baked in Blender (gp7200_corebake.py, tools/engine_interior/bake_gp_core.py). LOD 0 (0-60 m) carries the whole core, LOD 1 only the two faces seen through the intake and the exhaust.
The part seen through the intake and the exhaust (splitter, first booster stages, last LPT stages, exhaust case) is always drawn; the
rest is drawn while fbw/anim/engine_bay_<n> is up. The LP rotors turn with fbw/anim/N1_<n>_anim. Every engine also gets a falling copy
a380_engdetach_<e>_a380_engcore_gp.obj listed in a380_engdetach.json."""

import json
import shutil
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gp7200_geom import SPEC, Mesh, box, cap_ring, frames, surface, tube, unit, wind  # noqa: E402

CORE = SPEC["core"]
FN = frames()
TEX = "A380_GPCORE"
OVERLAY = {
    "BAND": (857, 29, (168, 162, 122), (0, 90)),
    "LINE": (857, 51, (210, 205, 170), (0, 90)),
    "CONE": (857, 73, (58, 55, 46), (0, 70)),
    "DASH": (857, 95, (235, 235, 230), (0, 90)),
    "CAP": (292, 33, (58, 58, 56), (0, 80)),
    "SEAM": (292, 55, (26, 26, 24), (0, 40)),
    "SCREW": (292, 77, (120, 115, 100), (200, 150)),
    "LIP": (987, 35, (208, 208, 204), (130, 150)),
}
SIZE = 2048
COUNTS = dict(booster_rotor=72, booster_stator=96, hpc_rotor=56, hpc_stator=64, hpt_ngv=44, hpt_blade=76, lpt_rotor=96, lpt_stator=92, nozzles=24, tcf=10, tec=13)
PID = [0]
MATERIALS = ["titanium", "stainless", "casing", "case", "bronze", "black", "hose", "blue", "grey_harness", "canister", "eec", "casting", "brass", "blade", "turbine", "liner",
             "primer", "label", "bolt", "wire", "duct", "frame", "gearbox", "clamp", "yellow"]


def mark(mesh, name):
    """Every triangle is tagged material|part|local index; the bake gives one UV set per part and every copy of a replicated part reuses it."""
    PID[0] += 1
    mesh.tag = [f"{name}|{PID[0]}|{i}" for i in range(len(mesh.T))]
    mesh.UV = np.zeros((len(mesh.P), 2))
    return mesh


tagged = flat = mark


def replicate(proto, n, name):
    out = Mesh()
    P0, N0, T0 = proto.P, proto.N, proto.T
    Ps, Ns, Ts = [], [], []
    for k in range(n):
        a = 2 * np.pi * k / n
        ca, sa = np.cos(a), np.sin(a)
        R = np.array([[ca, sa, 0], [-sa, ca, 0], [0, 0, 1.0]])
        Ps.append(P0 @ R.T)
        Ns.append(N0 @ R.T)
        Ts.append(T0 + k * len(P0))
    out.add(np.vstack(Ps), np.vstack(Ns), np.zeros((n * len(P0), 2)), np.vstack(Ts), name)
    out.tag = list(proto.tag) * n
    return out


def revolve(profile, name, kind="out", nphi=64, cb=0.0):
    pr = np.asarray(profile, float)
    ph = np.arange(nphi) * 2 * np.pi / nphi
    P = np.stack([np.outer(pr[:, 1], np.sin(ph)), cb + np.outer(pr[:, 1], np.cos(ph)), np.repeat(pr[:, 0:1], nphi, axis=1)], axis=-1)
    return tagged(surface(P, True, kind, tag=name, centre_b=cb), name)


def rev_pair(s0, s1, r_hub, r_tip, name_h, name_o, n=2):
    st = np.linspace(s0, s1, n + 1)
    rh = np.interp(st, [s0, s1], r_hub)
    ro = np.interp(st, [s0, s1], r_tip)
    return [revolve(np.stack([st, rh], 1), name_h, "out"), revolve(np.stack([st, ro], 1), name_o, "in")]


def annulus(s, r0, r1, name, kind="aft", nphi=64):
    ph = np.arange(nphi) * 2 * np.pi / nphi
    P = np.stack([np.outer([r0, r1], np.sin(ph)), np.outer([r0, r1], np.cos(ph)), np.full((2, nphi), s)], axis=-1)
    return tagged(surface(P, True, kind, tag=name), name)


def airfoil_blade(theta, s_c, chord, r0, r1, thick, stagger, name, nr=2, nc=4, lean=0.0, twist=0.0):
    f = (1 - np.cos(np.linspace(0, np.pi, nc))) / 2
    faces = []
    for sign in (1, -1):
        P = np.zeros((nr, nc, 3))
        for i, t in enumerate(np.linspace(0, 1, nr)):
            r = r0 + (r1 - r0) * t
            g = stagger + twist * t
            half = 5 * thick * chord * (0.2969 * np.sqrt(f) - 0.126 * f - 0.3516 * f ** 2 + 0.2843 * f ** 3 - 0.1015 * f ** 4)
            ax, tg = np.cos(g), np.sin(g)
            off = sign * half
            ds = f * chord * ax - off * tg - chord * ax / 2
            dt = f * chord * tg + off * ax - chord * tg / 2
            ang = theta + (dt + lean * t) / r
            P[i, :, 0] = r * np.sin(ang)
            P[i, :, 1] = r * np.cos(ang)
            P[i, :, 2] = s_c + ds
        faces.append(P)
    out = Mesh()
    for P in faces:
        out.extend(surface(P, False, "free", tag=name))
    for P in (np.stack([faces[0][:, 0], faces[1][:, 0]], 1), np.stack([faces[0][:, -1], faces[1][:, -1]], 1)):
        out.extend(surface(P, False, "free", tag=name))
    return out


def blade_row(n, s_c, chord, r0, r1, thick, stagger, name, nr=2, nc=4, twist=0.0, jitter=0.0):
    blade = mark(airfoil_blade(0.0, s_c, chord, r0, r1, thick, stagger, name, nr, nc, twist=twist), name)
    return replicate(blade, n, name)


def radial_strut(theta, s0, s1, r0, r1, w, name, h=None):
    h = h if h is not None else (s1 - s0)
    return airfoil_blade(theta, (s0 + s1) / 2, s1 - s0, r0, r1, w / (s1 - s0) / 0.8, 0.0, name, nr=2, nc=5)


def strut_ring(n, s0, s1, r0, r1, w, name, offset=0.0):
    return replicate(mark(radial_strut(offset, s0, s1, r0, r1, w, name), name), n, name)


def flow(name):
    rows = {r[0]: r for r in CORE["flowpath"]["rows"]}
    return rows[name]


def frame_tubes(points, r, name, seg=8):
    out = Mesh()
    pts = np.asarray(points, float)
    for a, b in zip(pts[:-1], pts[1:]):
        out.extend(tube(a, b, r, seg, name))
    return flat(out, name)


def polar(r, th_deg, s):
    t = np.radians(th_deg)
    return np.array([r * np.sin(t), r * np.cos(t), s])


def torus_ring(s, r, rt, name, nu=48, nv=8):
    u = np.arange(nu) * 2 * np.pi / nu
    v = np.arange(nv) * 2 * np.pi / nv
    uu, vv = np.meshgrid(u, v, indexing="ij")
    R = r + rt * np.cos(vv)
    P = np.stack([R * np.sin(uu), R * np.cos(uu), s + rt * np.sin(vv)], axis=-1)
    m = surface(P.transpose(1, 0, 2), True, "free", tag=name)
    Nn = np.stack([np.cos(vv) * np.sin(uu), np.cos(vv) * np.cos(uu), np.sin(vv)], axis=-1).transpose(1, 0, 2).reshape(-1, 3)
    m.N = Nn
    m.T = wind(m.P, m.T, m.N)
    return flat(m, name)


def casing_profile():
    rows = np.array(CORE["casing_outer_envelope"]["rows"])
    out = [(r[0], (r[1] + r[2]) / 2) for r in rows if r[0] > 2.1]
    return np.array(out)


def fan_case_profile():
    rows = np.array(CORE["casing_outer_envelope"]["rows"])
    sel = rows[(rows[:, 0] >= 0.2) & (rows[:, 0] <= 2.11)]
    return np.stack([sel[:, 0], (sel[:, 1] + sel[:, 2]) / 2], 1)


def build_core():
    """Returns (always, rare, rotor_always, rotor_rare) Meshes in engine-local coordinates."""
    always, rare, rot_a, rot_r = Mesh(), Mesh(), Mesh(), Mesh()
    add = lambda dst, m: dst.extend(m) if isinstance(m, Mesh) else [dst.extend(x) for x in m]
    fl = {r[0]: r for r in CORE["flowpath"]["rows"]}
    spl = CORE["fan_hub_and_splitter"]
    s_spl, r_spl = spl["splitter_lip"]["station"], spl["splitter_lip"]["radius"]

    splitter = revolve([(s_spl, r_spl), (s_spl + 0.02, r_spl + 0.012), (1.15, 0.80), (1.50, 0.85)], "titanium", "out")
    core_lip = revolve([(s_spl, r_spl), (s_spl + 0.02, r_spl - 0.012), (1.0, 0.74), (1.55, 0.66), (1.95, 0.55), (2.38, 0.37)], "titanium", "in")
    add(always, [splitter, core_lip])
    hub_prof = np.array([[0.66, 0.505], [1.0, 0.56], [1.55, 0.58], [1.95, 0.42], [2.38, 0.24], [2.97, 0.32], [3.08, 0.29], [3.43, 0.33], [3.68, 0.34], [3.85, 0.44], [4.2, 0.48], [4.54, 0.52], [4.93, 0.55]])
    st = np.unique(np.concatenate([np.linspace(0.66, 4.93, 56), hub_prof[:, 0]]))
    hub = revolve(np.stack([st, np.interp(st, hub_prof[:, 0], hub_prof[:, 1])], 1), "titanium", "out", 48)
    outer_prof = np.array([[1.0, 0.74], [1.55, 0.66], [1.95, 0.55], [2.38, 0.37], [2.97, 0.36], [3.08, 0.42], [3.43, 0.40], [3.68, 0.43], [3.85, 0.58], [4.2, 0.70], [4.54, 0.80], [4.93, 0.77]])
    ost = np.unique(np.concatenate([np.linspace(1.0, 4.93, 56), outer_prof[:, 0]]))
    wall = revolve(np.stack([ost, np.interp(ost, outer_prof[:, 0], outer_prof[:, 1])], 1), "liner", "in", 48)
    add(rare, hub)
    add(rare, wall)

    def r_hub(s):
        return float(np.interp(s, hub_prof[:, 0], hub_prof[:, 1]))

    def r_out(s):
        return float(np.interp(s, outer_prof[:, 0], outer_prof[:, 1]))

    for i in range(5):
        s_r = 1.06 + 0.105 * i
        rr = blade_row(COUNTS["booster_rotor"], s_r, 0.055, r_hub(s_r) + 0.004, r_out(s_r) - 0.006, 0.07, np.radians(38), "blade", twist=np.radians(8))
        sr = blade_row(COUNTS["booster_stator"], s_r + 0.055, 0.045, r_hub(s_r) + 0.004, r_out(s_r) - 0.002, 0.07, np.radians(-30), "stainless")
        (add(rot_a, rr) if i < 2 else add(rot_r, rr))
        (add(always, sr) if i < 2 else add(rare, sr))
        ring = revolve([(s_r - 0.03, r_hub(s_r) + 0.004), (s_r + 0.04, r_hub(s_r) + 0.004)], "blade", "out", 48)
        (add(rot_a, ring) if i < 2 else add(rot_r, ring))
    add(always, blade_row(COUNTS["booster_stator"], 1.0, 0.04, r_hub(1.0) + 0.004, r_out(1.0) - 0.002, 0.07, np.radians(-10), "stainless"))

    add(rare, strut_ring(8, 1.88, 2.30, r_hub(2.0), r_out(2.0), 0.05, "casting"))
    add(rare, revolve([(1.88, 0.62), (2.0, 0.60), (2.3, 0.44)], "casting", "out", 48))

    for i in range(9):
        s_r = 2.40 + 0.065 * i
        h0, h1 = np.interp(s_r, [2.38, 2.97], [0.245, 0.318]), np.interp(s_r, [2.38, 2.97], [0.365, 0.358])
        add(rot_r, blade_row(COUNTS["hpc_rotor"], s_r, 0.034, h0 + 0.002, h1 - 0.003, 0.06, np.radians(35), "blade", twist=np.radians(6)))
        add(rare, blade_row(COUNTS["hpc_stator"], s_r + 0.034, 0.03, h0 + 0.002, h1, 0.06, np.radians(-30), "stainless"))
    add(rare, revolve([(2.34, 0.28), (2.97, 0.32)], "blade", "out", 48))

    add(rare, [revolve([(2.978, 0.36), (3.08, 0.42), (3.08, 0.40)], "titanium", "in", 48), revolve([(2.978, 0.32), (3.08, 0.29)], "titanium", "out", 48)])
    liner_o = revolve([(3.08, 0.40), (3.25, 0.395), (3.43, 0.385)], "liner", "out", 64)
    liner_i = revolve([(3.08, 0.30), (3.25, 0.305), (3.43, 0.31)], "liner", "in", 64)
    dome = revolve([(3.08, 0.30), (3.075, 0.35), (3.08, 0.40)], "bronze", "fwd", 64)
    add(rare, [liner_o, liner_i, dome])
    for k in range(COUNTS["nozzles"]):
        a = 360 * k / COUNTS["nozzles"]
        add(rare, frame_tubes([polar(0.50, a, 3.02), polar(0.40, a, 3.02), polar(0.355, a, 3.08)], 0.012, "brass", 6))

    for i, (s_n, kind) in enumerate([(3.485, "ngv"), (3.545, "blade"), (3.605, "ngv"), (3.655, "blade")]):
        n = COUNTS["hpt_ngv"] if kind == "ngv" else COUNTS["hpt_blade"]
        h0, h1 = np.interp(s_n, [3.47, 3.68], [0.335, 0.34]), np.interp(s_n, [3.47, 3.68], [0.40, 0.43])
        row = blade_row(n, s_n, 0.045, h0, h1 - 0.003, 0.09, np.radians(55 if kind == "ngv" else -50), "turbine")
        add(rare, row)
    add(rare, strut_ring(COUNTS["tcf"], 3.69, 3.84, 0.345, 0.575, 0.045, "casting"))

    for i in range(6):
        s0 = 3.87 + 0.115 * i
        h = lambda s: (r_hub(s) + 0.004, r_out(s) - 0.012)
        stator = blade_row(COUNTS["lpt_stator"], s0, 0.04, *h(s0), 0.08, np.radians(50), "turbine")
        rotor = blade_row(COUNTS["lpt_rotor"], s0 + 0.057, 0.045, *h(s0 + 0.057), 0.07, np.radians(-48), "turbine")
        ring = revolve([(s0 + 0.03, r_hub(s0 + 0.057) + 0.004), (s0 + 0.09, r_hub(s0 + 0.057) + 0.004)], "turbine", "out", 48)
        shroud = revolve([(s0 + 0.035, r_out(s0 + 0.057) - 0.012), (s0 + 0.085, r_out(s0 + 0.057) - 0.012)], "turbine", "in", 48)
        last = i >= 4
        add(always if last else rare, stator)
        add(rot_a if last else rot_r, [rotor, ring, shroud])
    add(always, strut_ring(COUNTS["tec"], 4.60, 4.88, 0.54, 0.775, 0.04, "casing"))
    add(always, [revolve([(4.54, 0.80), (4.93, 0.77)], "liner", "in", 64), annulus(4.931, 0.55, 0.77, "casing", "aft")])

    shaft = revolve([(1.05, 0.06), (4.75, 0.06)], "stainless", "out", 24)
    add(rare, shaft)
    drum = revolve([(2.34, 0.16), (3.0, 0.2), (3.47, 0.24), (3.68, 0.28)], "case", "out", 40)
    add(rare, drum)
    for s_b in (1.05, 1.25, 2.25, 3.76, 4.75):
        add(rare, revolve([(s_b - 0.03, 0.095), (s_b + 0.03, 0.095)], "casing", "out", 24))

    fc = fan_case_profile()
    add(rare, revolve(fc, "primer", "out", 72))
    for s_f, r_f in ((0.213, 1.575), (0.325, 1.559), (1.288, 1.568), (1.496, 1.569), (1.861, 1.587), (2.044, 1.585)):
        add(rare, revolve([(s_f - 0.012, r_f - 0.015), (s_f - 0.012, r_f + 0.012), (s_f + 0.012, r_f + 0.012), (s_f + 0.012, r_f - 0.015)], "case", "out", 64))
    add(rare, annulus(2.13, 0.9, 1.549, "primer", "aft", 96))
    cs_ = casing_profile()
    st = np.unique(np.concatenate([cs_[:, 0], np.linspace(2.16, 4.94, 90)]))
    add(rare, revolve(np.stack([st, np.interp(st, cs_[:, 0], cs_[:, 1])], 1), "case", "out", 56))
    for s_f, r_f in ((2.757, 0.42), (2.837, 0.42), (2.996, 0.438), (3.093, 0.42), (3.518, 0.472), (3.678, 0.503), (3.848, 0.59), (4.056, 0.727), (4.541, 0.85), (4.931, 0.79)):
        add(rare, revolve([(s_f - 0.01, r_f - 0.01), (s_f - 0.01, r_f + 0.025), (s_f + 0.01, r_f + 0.025), (s_f + 0.01, r_f - 0.01)], "casing", "out", 56))

    gb = Mesh()
    gb.extend(flat(box(np.array([0.0, -0.64, 2.10]), (0.30, 0.24, 0.34)), "casting"))
    gb.extend(flat(box(np.array([0.0, -0.62, 2.50]), (0.34, 0.30, 0.52)), "casting"))
    gb.extend(flat(tube(np.array([0.0, -0.42, 2.651]), np.array([0.0, -0.58, 2.651]), 0.035, 10, "stainless"), "stainless"))
    gb.extend(flat(tube(np.array([0.0, -0.80, 2.50]), np.array([0.0, -0.62, 2.50]), 0.09, 14, "stainless"), "stainless"))
    for x, z, r in ((-0.22, 2.40, 0.07), (0.22, 2.40, 0.06), (-0.22, 2.62, 0.08), (0.22, 2.62, 0.075)):
        gb.extend(flat(tube(np.array([x * 0.5, -0.62, z]), np.array([x, -0.62, z]), r, 12, "casting"), "casting"))
    gb.extend(flat(tube(np.array([0.0, -0.62, 2.12]), np.array([0.0, -0.76, 2.12]), 0.05, 10, "titanium"), "titanium"))
    add(rare, gb)
    add(rare, frame_tubes([polar(0.50, -35, 2.62), polar(0.60, -50, 2.80), polar(0.66, -80, 3.05), polar(0.60, -120, 3.35), polar(0.52, -150, 3.60)], 0.10, "stainless", 12))
    for a in (-60, 40, 120):
        add(rare, frame_tubes([polar(0.44, a, 2.55), polar(0.50, a + 15, 2.75), polar(0.46, a + 30, 2.95)], 0.022, "hose", 8))
    eec = flat(box(np.array([-0.52, 0.04, 2.14]), (0.10, 0.34, 0.30)), "eec")
    eec.extend(flat(box(np.array([-0.52, 0.04, 2.14]), (0.12, 0.38, 0.34)), "label"))
    add(rare, eec)
    for k, a in enumerate((-100, -80, 80, 100, 150)):
        col = "grey_harness" if k % 2 else "blue"
        add(rare, frame_tubes([polar(0.46, a, 2.20), polar(0.45, a + 4, 2.7), polar(0.46, a + 8, 3.2), polar(0.50, a + 10, 3.62)], 0.012, col, 6))
    add(rare, flat(tube(np.array([-0.10, -0.52, 2.15]), np.array([0.10, -0.52, 2.15]), 0.06, 12, "canister"), "canister"))
    for k in range(10):
        add(rare, torus_ring(3.88 + 0.065 * k, float(np.interp(3.88 + 0.065 * k, [3.85, 4.54], [0.67, 0.83])), 0.007, "stainless", 40, 5))
    for a in range(0, 360, 45):
        add(rare, frame_tubes([polar(0.67, a, 3.88), polar(0.86, a, 4.53)], 0.009, "stainless", 6))
    for a in range(0, 360, 30):
        add(rare, flat(box(polar(0.80, a, 4.78) + np.array([0, 0, 0]), (0.04, 0.04, 0.05)), "case"))
    add(rare, details())
    return always, rare, rot_a, rot_r


def sweep(points, radius, tag, seg=10, samples=36, caps=True):
    """A tube of constant radius along a smooth path through points (engine-local), rings by parallel transport."""
    p = np.asarray(points, float)
    t = np.linspace(0, 1, len(p))
    u = np.linspace(0, 1, samples)
    path = np.stack([np.interp(u, t, p[:, k]) for k in range(3)], axis=1)
    from scipy.ndimage import gaussian_filter1d
    path = np.stack([gaussian_filter1d(path[:, k], 2.0, mode="nearest") for k in range(3)], axis=1)
    path[0], path[-1] = p[0], p[-1]
    tan = np.gradient(path, axis=0)
    tan /= np.linalg.norm(tan, axis=1, keepdims=True)
    ref = np.array([0.0, 0.0, 1.0]) if abs(tan[0, 2]) < 0.9 else np.array([1.0, 0.0, 0.0])
    nrm = np.cross(tan[0], ref)
    nrm /= np.linalg.norm(nrm)
    ang = np.arange(seg) * 2 * np.pi / seg
    rings, norms = [], []
    for k in range(len(path)):
        nrm = nrm - tan[k] * (nrm @ tan[k])
        nrm /= np.linalg.norm(nrm)
        bn = np.cross(tan[k], nrm)
        d = np.cos(ang)[:, None] * nrm + np.sin(ang)[:, None] * bn
        rings.append(path[k] + radius * d)
        norms.append(d)
    P = np.array(rings)
    m = surface(P, True, "free", tag=tag)
    m.N = np.array(norms).reshape(-1, 3)
    m.T = wind(m.P, m.T, m.N)
    if caps:
        m.extend(cap_ring(P[0], path[0], -tan[0], tag))
        m.extend(cap_ring(P[-1], path[-1], tan[-1], tag))
    return mark(m, tag)


def bolt_ring(s, r, n, head_r=0.0085, height=0.009, nseg=5, front=True):
    """n bolt heads (pentagonal, one cap) round a flange face at station s, radius r; the heads stand on the forward face."""
    a = np.arange(nseg) * 2 * np.pi / nseg
    base, top = s - 0.0005, s - 0.0005 - height
    xs, ys = head_r * np.cos(a), head_r * np.sin(a)
    P = np.vstack([np.stack([xs, ys + r, np.full(nseg, base)], 1), np.stack([xs, ys + r, np.full(nseg, top)], 1), [[0.0, r, top]]])
    Nn = np.vstack([np.stack([np.cos(a), np.sin(a), np.zeros(nseg)], 1), np.stack([np.cos(a), np.sin(a), np.zeros(nseg)], 1), [[0.0, 0.0, -1.0]]])
    T = []
    for i in range(nseg):
        j = (i + 1) % nseg
        T += [(i, nseg + i, j), (j, nseg + i, nseg + j), (2 * nseg, nseg + j, nseg + i)]
    T = wind(P, np.array(T), Nn)
    m = Mesh()
    m.add(P, Nn, np.zeros((len(P), 2)), T, "bolt")
    mark(m, "bolt")
    return replicate(m, n, "bolt")


def details():
    """Bolt rows and safety wire on the core flanges, the bleed duct, silicone hoses with clamps, the EEC frame, labels, access plugs, the aft mount."""
    out = Mesh()
    for s_f, r_f in ((2.757, 0.42), (2.837, 0.42), (2.996, 0.438), (3.093, 0.42), (3.518, 0.472), (3.678, 0.503), (3.848, 0.59), (4.056, 0.727), (4.541, 0.85), (4.931, 0.79)):
        n = int(round(2 * np.pi * (r_f + 0.012) / 0.048))
        out.extend(bolt_ring(s_f - 0.01, r_f + 0.014, n))
    for s_f, r_f in ((2.996, 0.438), (3.518, 0.472), (3.848, 0.59), (4.541, 0.85), (4.931, 0.79)):
        out.extend(torus_ring(s_f - 0.012, r_f + 0.014, 0.0025, "wire", 56, 4))
    duct = [polar(0.50, 28, 2.42), polar(0.58, 38, 2.52), polar(0.66, 62, 2.66), polar(0.70, 95, 2.9), polar(0.64, 122, 3.2), polar(0.57, 140, 3.52)]
    out.extend(sweep(duct, 0.085, "duct", 14, 40))
    for u in (0.2, 0.45, 0.7):
        k = np.interp(u * (len(duct) - 1), np.arange(len(duct)), np.arange(len(duct)))
        c = np.array([np.interp(k, np.arange(len(duct)), np.array(duct)[:, d]) for d in range(3)])
        out.extend(sweep([c - np.array([0, 0, 0.012]), c + np.array([0, 0, 0.012])], 0.092, "clamp", 14, 4))
    for pts in ([polar(0.47, -140, 2.30), polar(0.58, -120, 2.40), polar(0.62, -100, 2.55), polar(0.57, -80, 2.7)], [polar(0.46, 150, 2.4), polar(0.56, 165, 2.6), polar(0.60, 190, 2.85), polar(0.5, 205, 3.1)]):
        out.extend(sweep(pts, 0.034, "hose", 10, 30))
        out.extend(sweep([pts[1] - np.array([0, 0, 0.012]), pts[1] + np.array([0, 0, 0.012])], 0.041, "clamp", 10, 4))
    eec_c = np.array([-0.52, 0.04, 2.14])
    for dy, sx, sy in ((0.205, 0.14, 0.02), (-0.205, 0.14, 0.02)):
        out.extend(mark(box(eec_c + np.array([0.0, dy, 0.0]), (sx, sy, 0.36)), "frame"))
    for dz, sz in ((0.185, 0.02), (-0.185, 0.02)):
        out.extend(mark(box(eec_c + np.array([0.0, 0.0, dz]), (0.14, 0.40, sz)), "frame"))
    for a, s in ((8, 2.62), (-60, 2.9), (-120, 3.3), (60, 3.0)):
        out.extend(mark(box(polar(float(np.interp(s, [2.3, 3.0, 3.5], [0.40, 0.42, 0.46])) + 0.003, a, s), (0.13, 0.045, 0.004)), "label"))
    for a, s, r in ((100, 4.2, 0.79), (260, 4.3, 0.80), (80, 3.1, 0.43), (280, 3.15, 0.43)):
        c = polar(r, a, s)
        out.extend(sweep([c, c + polar(0.025, a, 0.0) * np.array([1, 1, 0])], 0.032, "casing", 14, 3))
    top = polar(0.80, 0, 4.78)
    out.extend(mark(box(top + np.array([0.0, 0.06, 0.0]), (0.16, 0.12, 0.2)), "bronze"))
    out.extend(sweep([top + np.array([0.0, 0.12, 0.0]), top + np.array([0.0, 0.2, 0.0])], 0.035, "bronze", 10, 3))
    return out


def lod1():
    out = Mesh()
    out.extend(annulus(1.0, 0.0, 0.74, "black", "fwd", 24))
    out.extend(annulus(4.7, 0.52, 0.80, "black", "aft", 24))
    return flat(out, "black")


def overlay_uv(tag):
    x, y = OVERLAY[tag][:2]
    return ((x + 0.5) / 2048, 1 - (y + 0.5) / 2048)


def fan_overlay():
    import gp7200_fan as fan
    import gp7200_nacelle as nac
    fixed, turning = Mesh(), Mesh()
    if not fan.DETAIL:
        return fixed, turning
    for out, m, tag in ((fixed, nac.band_overlay(), "BAND"), (fixed, nac.band_line(), "LINE"), *((fixed, m, tag) for m, tag in fan.cone_parts()), (turning, fan.swirl(), "DASH"), (turning, fan.screws(), "SCREW")):
        m.UV = np.tile(overlay_uv(tag), (len(m.P), 1))
        out.extend(m)
    return fixed, turning


def lip_overlay():
    import gp7200_fan as fan
    import gp7200_nacelle as nac
    m = nac.lip_overlay() if fan.DETAIL else Mesh()
    m.UV = np.tile(overlay_uv("LIP"), (len(m.P), 1))
    return m


def paint_overlay(stage):
    from PIL import Image
    for kind in ("ALB", "MAT"):
        path = stage / f"{TEX}_{kind}.png"
        a = np.array(Image.open(path).convert("RGB"))
        for x, y, colour, material in OVERLAY.values():
            a[y - 11:y + 11, x - 45:x + 45] = colour if kind == "ALB" else (*material, 0)
        Image.fromarray(a).save(path)


def to_world(mesh, e):
    return FN[e].point(mesh.P), unit(FN[e].vec(mesh.N)), mesh.UV, mesh.T


def obj_lines(parts, chains, pivots):
    """parts[e] = dict(always, rare, rot_a, rot_r) world arrays; returns vt rows, idx, post lines."""
    vts, idx, body = [], [], []
    nv = 0
    batches = {}
    for e in (1, 2, 3, 4):
        for key in ("always", "rare", "rot_a", "rot_r", "lip"):
            P, N, UV, T = parts[e][key]
            idx_start = len(idx)
            idx += list((np.asarray(T) + nv).ravel())
            vts.append(np.hstack([P, N, UV]))
            nv += len(P)
            batches[(e, key)] = (idx_start, len(idx) - idx_start)
    lod1_b = {}
    for e in (1, 2, 3, 4):
        P, N, UV, T = parts[e]["lod1"]
        idx_start = len(idx)
        idx += list((np.asarray(T) + nv).ravel())
        vts.append(np.hstack([P, N, UV]))
        nv += len(P)
        lod1_b[e] = (idx_start, len(idx) - idx_start)
    gate = ["ANIM_begin", "ANIM_hide -0.5 0.5 fbw/engine/gp7200", "ANIM_hide 0.5 1.5 fbw/anim/eye_in_flight_deck"]
    MOTION = ("ANIM_trans", "ANIM_rotate")

    def engine_open(e):
        return ["ANIM_begin"] + chains[e] + ["ANIM_begin", f"ANIM_hide 5.5 6.5 sim/operation/failures/rel_engsep{e - 1}"] + pivots[e]

    body = ["ATTR_LOD 0 60", "ATTR_cull"] + gate
    for e in (1, 2, 3, 4):
        body += engine_open(e)
        for key, rot, bay in (("always", False, False), ("rare", False, True), ("rot_a", True, False), ("rot_r", True, True), ("lip", False, False)):
            s, c = batches[(e, key)]
            if not c:
                continue
            body += ["ANIM_begin"]
            if key == "lip":
                body += [f"ANIM_hide 0.5 1.5 fbw/fail/sep/cowl_{e}"]
            if bay:
                body += [f"ANIM_hide -99999 99999 fbw/anim/engine_bay_{e}", f"ANIM_show 0.5 1.5 fbw/anim/engine_bay_{e}"]
            if rot:
                p = FN[e].point(np.array([0.0, 0.0, 2.0]))
                d = -FN[e].d
                ps = " ".join(f"{v:.6f}" for v in [*p, *p])
                pn = " ".join(f"{-v:.6f}" for v in [*p, *p])
                body += [f"ANIM_trans {ps}", f"ANIM_rotate_begin {d[0]:.6f} {d[1]:.6f} {d[2]:.6f} fbw/anim/N1_{e}_anim", "ANIM_rotate_key 0 0", "ANIM_rotate_key 1 -360.00001", "ANIM_rotate_end", f"ANIM_trans {pn}"]
            body += [f"TRIS {s} {c}", "ANIM_end"]
        body += ["ANIM_end", "ANIM_end"]
    body += ["ANIM_end", "ATTR_LOD 60 250", "ATTR_cull"] + gate
    for e in (1, 2, 3, 4):
        s, c = lod1_b[e]
        ls, lc = batches[(e, "lip")]
        body += engine_open(e) + [f"TRIS {s} {c}", "ANIM_begin", f"ANIM_hide 0.5 1.5 fbw/fail/sep/cowl_{e}", f"TRIS {ls} {lc}", "ANIM_end", "ANIM_end", "ANIM_end"]
    body += ["ANIM_end", ""]
    return np.vstack(vts), np.array(idx), body


def nacelle_chain(stage, e):
    """The wing-bend and engine-wobble ANIM lines in force on engine e's skin in the staged 022/023 twin: the bucket chain around the
    engine block, and the pitch/yaw pivots inside its rel_engsep block, copied verbatim."""
    from gp7200_obj import ENGSEP, Obj, engine_of_batch
    path = stage / ("a380_exterior_022_gp.obj" if e < 3 else "a380_exterior_023_gp.obj")
    obj = Obj(path)
    target = next(b for b in obj.tris() if engine_of_batch(b) == e - 1 and not any("thrust_rev" in t for t in b["anim"]))
    stack = []
    for i, l in enumerate(obj.post):
        t = l.strip()
        if i == target["line"]:
            break
        if t == "ANIM_begin":
            stack.append([])
        elif t == "ANIM_end":
            stack.pop()
        elif stack:
            stack[-1].append(t)
    keep = lambda lines: [t for t in lines if t.startswith(("ANIM_trans", "ANIM_rotate"))]
    chain, pivots = [], []
    for level in stack:
        if any(ENGSEP.match(t) for t in level):
            pivots = keep(level)
        else:
            chain += keep(level)
    return chain, pivots


def write_obj_file(path, vt, idx, body):
    lines = ["I", "800", "OBJ", "", f"TEXTURE {TEX}_ALB.png", f"TEXTURE_MAP normal {TEX}_NML.png", f"TEXTURE_MAP material_gloss {TEX}_MAT.png", "GLOBAL_specular 1.0",
             f"POINT_COUNTS {len(vt)} 0 0 {len(idx)}", ""]
    lines += ["VT " + " ".join(f"{v:.5f}" for v in r[:3]) + " " + " ".join(f"{v:.4f}" for v in r[3:6]) + " " + " ".join(f"{v:.5f}" for v in r[6:8]) for r in vt]
    full = len(idx) - len(idx) % 10
    lines += ["IDX10 " + " ".join(map(str, idx[k:k + 10])) for k in range(0, full, 10)]
    lines += [f"IDX {i}" for i in idx[full:]]
    lines += [""] + body
    Path(path).write_text("\n".join(lines), encoding="utf-8", newline="\n")


def detach_copy(path, e, mesh_world, centre):
    P, N, UV, T = mesh_world
    lines = ["I", "800", "OBJ", "", f"TEXTURE {TEX}_ALB.png", f"TEXTURE_MAP normal {TEX}_NML.png", f"TEXTURE_MAP material_gloss {TEX}_MAT.png", "GLOBAL_specular 1.0",
             f"POINT_COUNTS {len(P)} 0 0 {len(T) * 3}", ""]
    lines += ["VT " + " ".join(f"{v:.5f}" for v in (p - centre)) + " " + " ".join(f"{v:.4f}" for v in n) + " " + " ".join(f"{v:.5f}" for v in u) for p, n, u in zip(P, N, UV)]
    flat_idx = np.asarray(T).ravel()
    full = len(flat_idx) - len(flat_idx) % 10
    lines += ["IDX10 " + " ".join(map(str, flat_idx[k:k + 10])) for k in range(0, full, 10)]
    lines += [f"IDX {i}" for i in flat_idx[full:]]
    lines += ["", "ATTR_LOD 0 4000", "ATTR_cull", f"TRIS 0 {len(flat_idx)}", ""]
    Path(path).write_text("\n".join(lines), encoding="utf-8", newline="\n")


def main(installed, stage, only_obj=False):
    import gp7200_corebake as cb

    installed, stage = Path(installed), Path(stage)
    PID[0] = 0
    always, rare, rot_a, rot_r = build_core()
    lod = lod1()
    uvs = cb.load_uv()
    lod_uv = uvs[(cb.WORK / "lod_pid.txt").read_text().strip()][0, 0]
    always, rare, rot_a, rot_r = (cb.apply(m, uvs) for m in (always, rare, rot_a, rot_r))
    fixed, turning = fan_overlay()
    always.extend(fixed)
    rot_a.extend(turning)
    lod.UV = np.tile(lod_uv, (len(lod.P), 1))
    lip = lip_overlay()
    parts = {}
    for e in (1, 2, 3, 4):
        parts[e] = dict(always=to_world(always, e), rare=to_world(rare, e), rot_a=to_world(rot_a, e), rot_r=to_world(rot_r, e), lod1=to_world(lod, e), lip=to_world(lip, e))
    chains, pivots = {}, {}
    for e in (1, 2, 3, 4):
        chains[e], pivots[e] = nacelle_chain(stage, e)
    vt, idx, body = obj_lines(parts, chains, pivots)
    write_obj_file(stage / "a380_engcore_gp.obj", vt, idx, body)
    if only_obj:
        print("a380_engcore_gp.obj", len(vt), "vertices", len(idx) // 3, "triangles")
        return {}
    for kind in ("ALB", "NML", "MAT"):
        shutil.copy2(cb.WORK / "tex" / f"{TEX}_{kind}.png", stage / f"{TEX}_{kind}.png")
    paint_overlay(stage)
    tri_engine = (always.tri_count() + rare.tri_count() + rot_a.tri_count() + rot_r.tri_count())
    print("a380_engcore_gp.obj", len(vt), "vertices", len(idx) // 3, "triangles,", tri_engine, "per engine at LOD 0,", lod.tri_count(), "at LOD 1")
    manifest_path = stage / "a380_engdetach.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    for e in (1, 2, 3, 4):
        centre = np.array(manifest["gp"]["engines"][str(e - 1)]["centre"])
        full = Mesh()
        for m in (always, rare, rot_a, rot_r):
            full.extend(m)
        name = f"a380_engdetach_{e - 1}_a380_engcore_gp.obj"
        detach_copy(stage / name, e, to_world(full, e), centre)
        names = manifest["gp"]["engines"][str(e - 1)]["objects"]
        if name not in names:
            names.append(name)
    manifest_path.write_text(json.dumps(manifest, indent=1), encoding="utf-8")
    return dict(always=always.tri_count(), rare=rare.tri_count(), rot_a=rot_a.tri_count(), rot_r=rot_r.tri_count(), lod1=lod.tri_count())


if __name__ == "__main__":
    print(main(sys.argv[1], sys.argv[2], "--only-obj" in sys.argv))
