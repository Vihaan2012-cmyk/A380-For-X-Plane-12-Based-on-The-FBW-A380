import numpy as np

from gp7200_geom import (
    BAND, grid_tris_fast, wind, HL_BOT, HL_R, HL_TOP, HL_Y0, S_LIP_END, S_NOSE, S_PLUG, S_TE, S_TR, SPEC, Mesh, box, cap_ring, shear, smooth, surface, unit,
    rsec, y0, s_highlight,
)

NPHI = 120
PHI = np.arange(NPHI) * 2 * np.pi / NPHI
THROAT_R = SPEC["inlet"]["throat_r"]
S_THROAT = -0.80
CASE_R = 1.482
CASE_BAND_S, CASE_AFT_S = 0.10, 1.0
DOOR_T = 0.10
STEP_AFT = 0.006
GROOVE = 0.005
LIP_INNER = np.array([[0.0, HL_R], [0.02, 1.535], [0.05, 1.515], [0.10, 1.495], [0.20, 1.462], [0.35, 1.425], [S_THROAT + 1.24, THROAT_R]])
BARREL = np.array([[S_THROAT, THROAT_R], [-0.5, 1.412], [-0.2, 1.434], [0.1, 1.468], [S_NOSE, CASE_R], [0.6, CASE_R], [1.50, CASE_R]])


def fan_case_r(s):
    return np.interp(s, BARREL[:, 0], BARREL[:, 1])


def sec_points(sp, phi, rho=None, dr=0.0):
    """Points of the outer circular sections: station sp (K,) in the unsheared coordinate, angles phi (J,)."""
    sp = np.asarray(sp, float)[:, None]
    ph = phi[None, :]
    s = shear(np.broadcast_to(sp, (sp.shape[0], ph.shape[1])), ph)
    r = rsec(sp) - dr if rho is None else rho
    c = y0(sp)
    P = np.stack([np.broadcast_to(r, s.shape) * np.sin(ph), c + np.broadcast_to(r, s.shape) * np.cos(ph), s], axis=-1)
    return P


def stations(a, b, step):
    n = max(1, int(round((b - a) / step)))
    return np.linspace(a, b, n + 1)


def _raw_outer_skin():
    pre = np.concatenate([stations(HL_TOP + BAND, S_LIP_END, 0.05), stations(S_LIP_END, 0.0, 0.1)[1:], stations(0.0, S_NOSE, 0.13)[1:]])
    mid = stations(S_NOSE, 2.40, 0.12)
    door = np.concatenate([pre, mid[1:]])
    door = np.concatenate([door, [2.44]])
    P = sec_points(door, PHI)
    P[-1] = sec_points(np.array([2.40]), PHI)[0] * 0 + sec_points(np.array([2.41]), PHI)[0]
    P[-1, :, 2] = 2.44
    fan = surface(P, True, "out", tag="OUT")
    return fan, door


def outer_skin():
    fan, door = _raw_outer_skin()
    lip = inlet_lip(raw=True)
    n = fan.N[:NPHI] + lip.N[:NPHI]
    fan.N[:NPHI] = n / np.linalg.norm(n, axis=1, keepdims=True)
    tr_st = np.concatenate([[2.46], stations(2.5, 4.9, 0.15), [4.95]])
    Q = sec_points(tr_st, PHI)
    Q[0, :, 2] = 2.44 + STEP_AFT
    te = sec_points(np.array([S_TE]), PHI)
    Q = np.concatenate([Q, te], axis=0)
    Q[-1, :, 2] = S_TE
    tr = surface(Q, True, "out", tag="OUT")
    return fan, tr, door


def step_band():
    a = sec_points(np.array([2.41]), PHI)[0].copy()
    a[:, 2] = 2.44
    m1 = sec_points(np.array([2.435]), PHI, dr=GROOVE)[0].copy()
    m1[:, 2] = 2.44 + STEP_AFT * 0.3
    m2 = m1.copy()
    m2[:, 2] = 2.44 + STEP_AFT * 0.7
    b = sec_points(np.array([2.46]), PHI)[0].copy()
    b[:, 2] = 2.44 + STEP_AFT
    P = np.stack([a, m1, m2, b])
    m = surface(P, True, "out", tag="OUT")
    rad = np.stack([np.sin(PHI), np.cos(PHI), np.zeros(NPHI)], 1)
    axial = np.array([0.0, 0.0, 1.0])
    tilt = np.sin(np.radians(55)), np.cos(np.radians(55))
    rows = [rad, rad * tilt[1] + axial * tilt[0], rad * tilt[1] - axial * tilt[0], rad]
    m.N = np.vstack(rows)
    m.T = wind(m.P, m.T, m.N)
    return m


def lip_strip(u_out, u_in):
    rows = []
    for u in u_out:
        rows.append(sec_points(np.array([HL_TOP + u]), PHI)[0])
    for u in u_in:
        sp = HL_TOP + u
        rho = np.interp(u, LIP_INNER[:, 0], LIP_INNER[:, 1]) if u <= LIP_INNER[-1, 0] + 1e-9 else fan_case_r(sp)
        s = shear(np.full(NPHI, sp), PHI)
        cb = y0(np.array([sp]))[0] * (1 - smooth(u, 0.0, 0.6))
        rows.append(np.stack([rho * np.sin(PHI), cb + rho * np.cos(PHI), s], axis=-1))
    return np.array(rows)


def inlet_lip(raw=False):
    u_out = np.array([BAND, 0.14, 0.09, 0.05, 0.025, 0.01, 0.0])
    u_in = np.array([0.01, 0.025, 0.05, 0.10, 0.20, 0.35, S_THROAT + 1.24, 0.60, 0.75])
    P = lip_strip(u_out, u_in)
    m = surface(P, True, "free", tag="LIPM")
    N = m.N.reshape(P.shape)
    sign = np.sign(np.einsum("ij,ij->i", N[0, :, :2], P[0, :, :2] - np.array([0.0, float(y0(np.array([HL_TOP + BAND]))[0])])).mean())
    m.N = (N * (1 if sign >= 0 else -1)).reshape(-1, 3)
    if not raw:
        fan, _ = _raw_outer_skin()
        n = fan.N[:NPHI] + m.N[:NPHI]
        m.N[:NPHI] = n / np.linalg.norm(n, axis=1, keepdims=True)
    m.T = wind(m.P, m.T, m.N)
    return m


def lip_rows():
    """Row counts of inlet_lip's grid up to the end of the silver skin, at the throat (photos A380_ENGINE.jpg, A6-EDS)."""
    return 14


def lip_parts():
    """inlet_lip split at the end of the silver skin: the skin, and the short run aft of it that keeps the liner's texture."""
    m = inlet_lip()
    K, J, k = m.K, m.J, lip_rows()
    P, N = m.P.reshape(K, J, 3), m.N.reshape(K, J, 3)
    out = []
    for r, tag in ((slice(0, k), "LIPS"), (slice(k - 1, K), "LIPM")):
        part = Mesh()
        p, n = P[r], N[r]
        part.add(p, n, np.zeros(p.shape[:2] + (2,)), wind(p.reshape(-1, 3), grid_tris_fast(p.shape[0], J, True), n.reshape(-1, 3)), tag)
        out.append(part)
    return out


def lip_overlay():
    """The silver lip skin on its own metal colour in the GP-only core atlas, 1.5 mm above the lip so nothing of the lip below shows through."""
    m = inlet_lip()
    k = lip_rows()
    P, N = m.P.reshape(m.K, m.J, 3), m.N.reshape(m.K, m.J, 3)
    p, n = P[:k] + N[:k] * 0.0015, N[:k]
    out = Mesh()
    out.add(p, n, np.zeros(p.shape[:2] + (2,)), wind(p.reshape(-1, 3), grid_tris_fast(p.shape[0], p.shape[1], True), n.reshape(-1, 3)), "LIP")
    return out


def barrel_stations():
    sp = np.concatenate([[HL_TOP + 0.75], stations(-0.35, S_NOSE, 0.15)[0:], stations(S_NOSE, 1.50, 0.12)[1:]])
    return np.unique(np.round(sp, 5))


def case_rows(sp, dr=0.0):
    s = shear(np.repeat(sp[:, None], NPHI, axis=1), PHI[None, :])
    cb = y0(sp)[:, None] * (1 - smooth(sp - HL_TOP, 0.0, 0.6))[:, None]
    r = fan_case_r(sp)[:, None] - dr
    return np.stack([r * np.sin(PHI)[None, :] + 0 * s, cb + r * np.cos(PHI)[None, :], s], axis=-1)


def band_overlay():
    """Photo A380_ENGINE.jpg: the pale olive fan-case band around the blade tips, on its own colour in the GP-only core atlas."""
    sp = barrel_stations()
    a, b = np.searchsorted(sp, [CASE_BAND_S, CASE_AFT_S])
    return surface(case_rows(sp[a:b + 1], 0.002), True, "in", tag="BAND")


def band_line():
    """Photo A380_ENGINE.jpg: the thin pale line across the band."""
    return surface(case_rows(np.array([0.30, 0.325]), 0.003), True, "in", tag="LINE")


def barrel_inner():
    sp = barrel_stations()
    P = case_rows(sp)
    a, b = np.searchsorted(sp, [CASE_BAND_S, CASE_AFT_S])
    out = Mesh()
    for rows, tag in ((P[:a + 1], "CASE"), (P[a:b + 1], "CASEBAND"), (P[b:], "CASEAFT")):
        out.extend(surface(rows, True, "in", tag=tag))
    return out


def door_inner(door_stations):
    st = np.array([s for s in door_stations if s >= S_NOSE - 1e-9])
    P = sec_points(st, PHI, dr=DOOR_T)
    return surface(P, True, "in", tag="DOORIN")


def duct_radius(s):
    s = np.asarray(s, float)
    t = smooth(s, S_TR, S_TE)
    return rsec(s) - 0.19 * (1 - t) - 0.04 * t


def tr_inner():
    st = np.concatenate([[S_TR + STEP_AFT], stations(2.6, 4.9, 0.15), [S_TE]])
    r = duct_radius(st)
    P = np.stack([np.outer(r, np.sin(PHI)), np.outer(r, np.cos(PHI)), np.repeat(st[:, None], NPHI, axis=1)], axis=-1)
    return surface(P, True, "in", tag="TRIN")


def te_ring():
    out = sec_points(np.array([4.95]), PHI)[0]
    out[:, 2] = S_TE
    r = duct_radius(np.array([S_TE]))[0]
    inn = np.stack([r * np.sin(PHI), r * np.cos(PHI), np.full(NPHI, S_TE)], axis=-1)
    P = np.stack([out, inn], axis=0)
    return surface(P, True, "aft", tag="TE")


def revolve(profile, nphi=96, kind="out", tag="", centre_b=0.0):
    pr = np.asarray(profile, float)
    ph = np.arange(nphi) * 2 * np.pi / nphi
    P = np.stack([np.outer(pr[:, 1], np.sin(ph)), centre_b + np.outer(pr[:, 1], np.cos(ph)), np.repeat(pr[:, 0:1], nphi, axis=1)], axis=-1)
    return surface(P, True, kind, tag=tag, centre_b=centre_b)


def core_cowl_profile():
    inside = np.array(SPEC["core_cowl"]["inside_bypass_duct_estimate"])
    vis = np.array(SPEC["core_cowl"]["visible_part"]["bottom"])
    vis = vis[np.argsort(vis[:, 0])]
    keep = vis[vis[:, 0] > 5.0]
    return inside, keep


def core_cowl():
    inside, vis = core_cowl_profile()
    st = np.array([1.9, 2.4, 3.2, 4.0, 4.6, 4.967])
    r = np.interp(st, inside[:, 0], inside[:, 1])
    prof = [(s_, r_) for s_, r_ in zip(st, r)]
    prof += [(5.04, 0.916), (5.127, 0.893), (5.215, 0.871), (5.303, 0.849), (5.3035, 0.816)]
    return revolve(prof, kind="out", tag="CCOW")


def nozzle():
    top = np.array(SPEC["core_nozzle"]["top"])
    bot = np.array(SPEC["core_nozzle"]["bottom"])
    outer = [(5.304, 0.848)] + [tuple(p) for p in top[1:]] + [(5.917, 0.688)]
    inner = [(5.917, 0.67), (5.80, 0.704), (5.62, 0.745), (5.45, 0.785), (5.304, 0.82)]
    o = revolve(outer, kind="out", tag="NOZO")
    i = revolve(inner, kind="in", tag="NOZI")
    lip = revolve([(5.917, 0.688), (5.9185, 0.679), (5.917, 0.67)], kind="aft", tag="NOZO")
    return o, i, lip


def plug():
    prof = [(4.93, 0.55), (5.30, 0.52), (5.60, 0.472), (5.917, 0.433)] + [tuple(p) for p in SPEC["plug"]["profile"][1:]] + [(S_PLUG, 0.036)]
    m = revolve(prof, kind="out", tag="PLUG")
    ring = np.stack([0.036 * np.sin(PHI), 0.036 * np.cos(PHI), np.full(NPHI, S_PLUG)], axis=-1)
    m.extend(cap_ring(ring, np.array([0.0, 0.0, S_PLUG]), np.array([0, 0, 1.0]), "PLUG"))
    return m


def upper_fairing():
    from gp7200_geom import rt
    pts = np.array(SPEC["outer_nacelle"]["upper_aft_fairing"]["points"])
    up = pts[pts[:, 0] >= 5.151][:7]
    s_top = np.concatenate([np.linspace(S_TE, 5.12, 4), up[:, 0]])
    r_top = np.concatenate([rt(np.linspace(S_TE, 5.12, 4)), up[:, 1]])
    half = np.radians(16)
    ph = np.linspace(-half, half, 9)
    taper = 1 - 0.35 * smooth(s_top, S_TE, 5.22)

    def shell(r, kind):
        P = np.stack([np.outer(r, np.sin(ph)) * 1.0, np.outer(r, np.cos(ph)), np.repeat(s_top[:, None], len(ph), axis=1)], axis=-1)
        return surface(P, False, kind, tag="OUT")

    out = shell(r_top, "out")
    out.extend(shell(r_top - 0.09, "in"))
    edge = np.stack([np.outer(r_top[-1:], np.sin(ph)), np.outer(r_top[-1:], np.cos(ph)), np.repeat(s_top[-1:, None], len(ph), axis=1)], axis=-1)[0]
    edge2 = edge.copy()
    edge2[:, 1] = (edge2[:, 1] / r_top[-1]) * (r_top[-1] - 0.09)
    edge2[:, 0] = (edge2[:, 0] / r_top[-1]) * (r_top[-1] - 0.09)
    out.extend(surface(np.stack([edge, edge2], axis=0), False, "aft", tag="OUT"))
    return out


def oil_door(side_sign):
    s0, s1 = SPEC["panels"]["oil_tank_access_door"]["s"]
    half = SPEC["panels"]["oil_tank_access_door"]["size"][1] / 2
    st = np.linspace(s0, s1, 5)
    r = float(rsec(np.array([(s0 + s1) / 2]))[0])
    th = np.radians(270.0) if side_sign < 0 else np.radians(90.0)
    ph = th + np.linspace(-half, half, 9) / r
    P = sec_points(st, ph, dr=-0.003)
    return surface(P, False, "out", tag="OUT")


def red_line():
    st = np.array([0.38, 0.47])
    P = sec_points(st, PHI, dr=-0.007)
    return surface(P, True, "out", tag="RED")


def lower_panel(side):
    ph = np.radians(np.linspace(195, 235, 11))
    if side > 0:
        ph = 2 * np.pi - ph
    st = np.linspace(HL_TOP + BAND, 0.2, 13)
    P = sec_points(st, ph, dr=-0.005)
    panel = surface(P, False, "out", tag="PANEL")
    louv = Mesh()
    a = np.radians(215 if side < 0 else -215)
    for s0 in np.linspace(-0.84, -0.62, 8):
        r = float(rsec(np.array([s0]))[0])
        slat = sec_points(np.array([s0 - 0.005, s0 + 0.005]), a + np.linspace(-0.085, 0.085, 7) / r, dr=-0.0075)
        louv.extend(surface(slat, False, "out", tag="LOUV"))
    return panel, louv


def _strip(st, ph, tag="SLAT", dr=-0.0045):
    return surface(sec_points(np.asarray(st, float), np.asarray(ph, float), dr=dr), False, "out", tag=tag)


def door_details(side_sign=-1):
    """Photo ILA 8207 / F-WWDD 248-27: outline groove of the oil tank access door and its four latch slots, as flush dark strips on the skin."""
    s0, s1 = SPEC["panels"]["oil_tank_access_door"]["s"]
    half = SPEC["panels"]["oil_tank_access_door"]["size"][1] / 2
    r = float(rsec(np.array([(s0 + s1) / 2]))[0])
    th = np.radians(270.0)
    w = 0.0035
    out = Mesh()
    hp = half / r
    wp = w / (2 * r)
    for s in (s0, s1):
        out.extend(_strip([s - w / 2, s + w / 2], np.linspace(th - hp, th + hp, 9)))
    for sgn in (-1, 1):
        out.extend(_strip(np.linspace(s0, s1, 5), [th + sgn * hp - wp, th + sgn * hp + wp]))
    # (distance from the forward edge, distance from the top edge, size along s, size across)
    for fs, ft, ls, la in ((0.12, 0.03, 0.010, 0.032), (0.107, 0.35, 0.010, 0.032), (0.35, 0.13, 0.032, 0.010), (0.34, 0.26, 0.032, 0.010)):
        sc = s0 + fs
        pc = th + hp - ft / r
        out.extend(_strip([sc - ls / 2, sc + ls / 2], np.linspace(pc - la / (2 * r), pc + la / (2 * r), 4)))
    return out


def tr_vent():
    """Photo ILA 8207: small dark vent slot on the TR split line low on the outboard side."""
    r = float(rsec(np.array([2.45]))[0])
    pc = np.radians(239.0)
    return _strip([2.44 - 0.0085, 2.44 + 0.0085], np.linspace(pc - 0.025 / r, pc + 0.025 / r, 5))


def latch_dots(side):
    """Photo ILA 8207 (bottom of the cowl, x 530 to 610): three round latch fasteners about 28 mm across on the lower fan cowl joint."""
    out = Mesh()
    pc = np.radians(191.0 if side < 0 else 169.0)
    for s in (0.51, 0.62, 0.73):
        r = float(rsec(np.array([s]))[0])
        ang = np.linspace(0, 2 * np.pi, 9)[:-1]
        rows = [np.array([[0.0, 0.0]] * 8), np.stack([np.cos(ang), np.sin(ang)], 1) * 0.014]
        pts = []
        for rr, k in ((0.0, 0), (0.014, 1)):
            pts.append([sec_points(np.array([s + rr * np.sin(a)]), np.array([pc + rr * np.cos(a) / r]), dr=-0.0045)[0, 0] for a in ang])
        out.extend(surface(np.array(pts), True, "out", tag="SLAT"))
    return out


def lip_joint():
    """Photo A380_ENGINE.jpg and Emirates 5994066593: dark joint line between the silver lip and the nose cowl."""
    sp = HL_TOP + BAND
    return _strip([sp - 0.003, sp + 0.003], np.append(PHI, 2 * np.pi), dr=-0.002)


