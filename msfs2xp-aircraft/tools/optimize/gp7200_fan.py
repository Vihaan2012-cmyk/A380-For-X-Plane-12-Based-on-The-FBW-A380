import numpy as np

from gp7200_geom import SPEC, Mesh, cap_ring, grid_tris_fast, surface, unit, wind

FAN = SPEC["fan"]
BLADES = FAN["blade_count"]
TIP_R = FAN["tip_radius"]
HUB_R = FAN["hub_radius_at_le"]
SPIN_NOSE, SPIN_END = -0.28, 0.45
HUB_END = 0.66
STAGGER_ROOT, STAGGER_TIP = 26.0, 34.0
LEAN_TIP_DEG, SWEEP_TIP, ROOT_EXT = 0.0, 0.0, -0.06
LEAN_POW, HOOK_START, HOOK_DEG = 1.0, 0.65, 13.0
DETAIL = True
SWIRL_R, SWIRL_W, SWIRL_ARC = 0.215, 0.026, (235.0, 305.0)
CAP_R, SEAM_W, CONE_G, DASH_G, FAR_DASH_G = 0.215, 0.006, 0.002, 0.0045, 0.0006
DASH_IN, DASH_OUT, DASH_WIDE, DASH_CORNER = 0.03, 0.225, 0.08, 0.015
SCREW_R, SCREW_RAD, SCREW_H, SCREW_ANG = 0.2, 0.011, 0.006, (0.0, 60.0, 120.0, 180.0, 240.0, 300.0)


def cone_r(s):
    u = np.clip((np.asarray(s, float) - SPIN_NOSE) / (SPIN_END - SPIN_NOSE), 0, 1)
    return HUB_R * np.sqrt(1 - (1 - u) ** 2)


def cone_s(r):
    return SPIN_NOSE + (SPIN_END - SPIN_NOSE) * (1 - np.sqrt(np.clip(1 - (np.asarray(r, float) / HUB_R) ** 2, 0, 1)))


def on_cone(x, y, g):
    """Points on the spinner ellipsoid above (x, y), lifted g along the surface, with their true normals."""
    x, y = np.asarray(x, float), np.asarray(y, float)
    r = np.hypot(x, y)
    s = cone_s(r)
    nr, ns = r / HUB_R ** 2, (s - SPIN_END) / (SPIN_END - SPIN_NOSE) ** 2
    n = np.hypot(nr, ns)
    nr, ns = nr / n, ns / n
    ux, uy = np.divide(x, r, out=np.zeros_like(x), where=r > 1e-9), np.divide(y, r, out=np.zeros_like(y), where=r > 1e-9)
    return np.stack([ux * (r + g * nr), uy * (r + g * nr), s + g * ns], axis=-1), np.stack([ux * nr, uy * nr, ns], axis=-1)


def cone_grid(x, y, g, closed, tag):
    P, N = on_cone(x, y, g)
    K, J = P.shape[:2]
    Pf, Nf = P.reshape(-1, 3), N.reshape(-1, 3)
    m = Mesh()
    m.add(P, N, np.zeros((K * J, 2)), wind(Pf, grid_tris_fast(K, J, closed), Nf), tag)
    return m


def spinner_profile(n=26):
    t = np.linspace(0.02, np.pi / 2, n)
    return np.stack([cone_s(HUB_R * np.sin(t)), HUB_R * np.sin(t)], axis=1)


def spinner():
    prof = spinner_profile()
    ph = np.arange(96) * 2 * np.pi / 96
    P = np.stack([np.outer(prof[:, 1], np.sin(ph)), np.outer(prof[:, 1], np.cos(ph)), np.repeat(prof[:, 0:1], len(ph), axis=1)], axis=-1)
    m = surface(P, True, "out", tag="SPIN")
    ring = P[0]
    m.extend(cap_ring(ring, np.array([0.0, 0.0, SPIN_NOSE]), np.array([0, 0, -1.0]), "SPIN"))
    return m


def cone_parts():
    """The painted spinner on its own colours in the GP-only core atlas: tip cap, the dark seam round it, and the cone behind, all on one surface."""
    ph = np.arange(96) * 2 * np.pi / 96
    a = np.arcsin((CAP_R - SEAM_W / 2) / HUB_R), np.arcsin((CAP_R + SEAM_W / 2) / HUB_R)

    def band(t0, t1, n, tag):
        r = HUB_R * np.sin(np.linspace(t0, t1, n))
        return cone_grid(np.outer(r, np.sin(ph)), np.outer(r, np.cos(ph)), CONE_G, True, tag)

    cap = band(0.02, a[0], 7, "CAP")
    ring = on_cone(HUB_R * np.sin(0.02) * np.sin(ph), HUB_R * np.sin(0.02) * np.cos(ph), CONE_G)[0]
    cap.extend(cap_ring(ring, on_cone(np.zeros(1), np.zeros(1), CONE_G)[0][0], np.array([0, 0, -1.0]), "CAP"))
    return (cap, "CAP"), (band(a[0], a[1], 2, "SEAM"), "SEAM"), (band(a[1], np.pi / 2, 15, "CONE"), "CONE")


def swirl_arc(n=24):
    """Round-5 mark: a tapered arc concentric with the axis (kept for --no-fan)."""
    u = np.linspace(0, 1, n)
    th = np.radians(SWIRL_ARC[0] + (SWIRL_ARC[1] - SWIRL_ARC[0]) * u)
    w = SWIRL_W * np.sqrt(np.sin(np.pi * np.clip(u, 0.02, 0.98)))
    rows = []
    for k in (-1.0, 0.0, 1.0):
        r = SWIRL_R + k * w / 2 * np.cos(2.2 * (u - 0.5)) + 0.04 * (u - 0.5) ** 2
        s = cone_s(r)
        rr = r + 0.0015
        rows.append(np.stack([rr * np.sin(th), rr * np.cos(th), s], axis=1))
    return surface(np.stack(rows, axis=0), False, "out", tag="SWIRL")


def dash_half_width(x):
    """Half width along the dash: round on the inner end, slightly eased corners on the outer one."""
    hw = DASH_WIDE / 2
    x = np.asarray(x, float)
    inner = np.sqrt(np.clip(hw ** 2 - np.clip(x - (DASH_IN - hw), 0, hw) ** 2, 0, None))
    d = np.clip(-DASH_OUT + DASH_CORNER - x, 0, DASH_CORNER)
    outer = hw - DASH_CORNER + np.sqrt(np.clip(DASH_CORNER ** 2 - d ** 2, 0, None))
    return np.minimum(inner, outer)


def swirl(g=None, n=26, k=9, tag="DASH"):
    """The white dash (photos A380_ENGINE.jpg, F-WWDD 248-26): radial, 80 mm wide, from just past the axis out to the cap's rim, along the surface."""
    hw = DASH_WIDE / 2
    x = np.concatenate([DASH_IN - hw * (1 - np.cos(np.linspace(0, np.pi / 2, 7)))[:-1], np.linspace(DASH_IN - hw, DASH_CORNER - DASH_OUT, n), DASH_CORNER - DASH_OUT - DASH_CORNER * np.sin(np.linspace(0, np.pi / 2, 7))[1:]])
    w = dash_half_width(x)
    v = np.linspace(-1, 1, k)
    return cone_grid(np.tile(-x[:, None], (1, k)).T, np.outer(v, w), DASH_G if g is None else g, False, tag)


def screws():
    """Six round screw heads on the tip cap (photo F-WWDD 248-26), 60 degrees apart and either side of the dash: octagonal pillboxes standing proud of the cap."""
    out = Mesh()
    for a in np.radians(SCREW_ANG):
        c, nrm = on_cone(np.array([SCREW_R * np.sin(a)]), np.array([SCREW_R * np.cos(a)]), 0.0)
        c, nrm = c[0], nrm[0]
        t1 = unit(np.array([np.cos(a), -np.sin(a), 0.0]))
        t2 = unit(np.cross(nrm, t1))
        ph = np.arange(8) * 2 * np.pi / 8
        rows = []
        for rad, lift in ((SCREW_RAD, 0.0), (SCREW_RAD, SCREW_H), (SCREW_RAD * 0.7, SCREW_H + 0.002)):
            rows.append(np.array([c + nrm * lift + rad * (np.cos(p) * t1 + np.sin(p) * t2) for p in ph]))
        m = surface(np.stack(rows, axis=0), True, "out", tag="SCREW")
        top = rows[-1]
        m.extend(cap_ring(top, c + nrm * (SCREW_H + 0.002), nrm, "SCREW"))
        out.extend(m)
    return out


def hub():
    s = np.array([SPIN_END, 0.53, 0.60, HUB_END])
    r = np.array([HUB_R, 0.485, 0.50, 0.505])
    ph = np.arange(96) * 2 * np.pi / 96
    P = np.stack([np.outer(r, np.sin(ph)), np.outer(r, np.cos(ph)), np.repeat(s[:, None], len(ph), axis=1)], axis=-1)
    m = surface(P, True, "out", tag="HUB")
    ring = P[-1]
    m.extend(cap_ring(ring, np.array([0.0, 0.0, HUB_END]), np.array([0, 0, 1.0]), "HUB"))
    return m


def blade_station(t):
    r = HUB_R + (TIP_R - HUB_R) * t
    t = np.clip(t, 0.0, 1.0)
    s_le = 0.45 - 0.11 * t + (SWEEP_TIP if DETAIL else 0.07) * (np.clip((t - 0.55) / 0.45, 0, 1) ** 2 * (3 - 2 * np.clip((t - 0.55) / 0.45, 0, 1)))
    chord = 0.35 + (0.55 - 0.35) * t ** 1.3
    stagger = np.radians(STAGGER_ROOT + STAGGER_TIP * t ** 1.2)
    thick = 0.06 - 0.03 * t
    camber = 0.06 - 0.02 * t
    return r, s_le, chord, stagger, thick, camber


def blade_lean(t):
    t = float(np.clip(t, 0, 1))
    u = np.clip((t - HOOK_START) / (1 - HOOK_START), 0, 1)
    return LEAN_TIP_DEG * t ** LEAN_POW + HOOK_DEG * u ** 2


def blade(theta0=0.0, nr=16, nc=13):
    f = (1 - np.cos(np.linspace(0, np.pi, nc))) / 2
    ts = np.concatenate([[ROOT_EXT], 1 - (1 - np.linspace(0, 1, nr)) ** 1.4]) if DETAIL else np.linspace(0, 1, nr)
    nr = len(ts)
    faces = []
    for sign in (1, -1):
        P = np.zeros((nr, nc, 3))
        for i, t in enumerate(ts):
            r, s_le, c, g, th, cam = blade_station(t)
            half = 5 * th * c * (0.2969 * np.sqrt(f) - 0.126 * f - 0.3516 * f ** 2 + 0.2843 * f ** 3 - 0.1015 * f ** 4)
            mid = cam * c * 4 * f * (1 - f)
            off = mid + sign * half
            ax = np.array([np.cos(g), np.sin(g)])
            nrm = np.array([-np.sin(g), np.cos(g)])
            ds = f * c * ax[0] + off * nrm[0]
            dt = f * c * ax[1] + off * nrm[1]
            ang = theta0 + np.radians(blade_lean(t) if DETAIL else 0.0) + dt / r
            P[i, :, 0] = r * np.sin(ang)
            P[i, :, 1] = r * np.cos(ang)
            P[i, :, 2] = s_le + ds
        faces.append(P)
    out = Mesh()
    for k, P in enumerate(faces):
        m = surface(P, False, "free", tag="BLADE%d" % k)
        out.extend(m)
    rim = np.concatenate([faces[0][:, 0, :][:, None, :], faces[1][:, 0, :][:, None, :]], axis=1)
    te = np.concatenate([faces[0][:, -1, :][:, None, :], faces[1][:, -1, :][:, None, :]], axis=1)
    tip = np.concatenate([faces[0][-1][:, None, :], faces[1][-1][:, None, :]], axis=1)
    for P, tag in ((rim, "BLADE0"), (te, "BLADE0"), (tip, "BLADE0")):
        out.extend(surface(P, False, "free", tag=tag))
    return out


def fan_assembly():
    out = Mesh()
    out.extend(spinner())
    out.extend(swirl(FAR_DASH_G, 8, 3, "SWIRL") if DETAIL else swirl_arc())
    out.extend(hub())
    b = blade(0.0)
    for k in range(BLADES):
        a = 2 * np.pi * k / BLADES
        ca, sa = np.cos(a), np.sin(a)
        R = np.array([[ca, sa, 0], [-sa, ca, 0], [0, 0, 1.0]])
        m = Mesh()
        m.add(b.P @ R.T, b.N @ R.T, b.UV, b.T, "BLADE")
        m.tag = list(b.tag)
        out.extend(m)
    return out
