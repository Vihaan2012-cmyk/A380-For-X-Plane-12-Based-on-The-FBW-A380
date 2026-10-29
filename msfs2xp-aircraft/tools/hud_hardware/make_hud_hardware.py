"""usage: make_hud_hardware.py <out dir> <font.ttf> [<aircraft objects dir, default out dir> [<originals dir>]]

The HUD hardware for both seats (a380_hud.obj, textures a380_hud_ALB/NML/MAT.png):
the overhead projection unit, the combiner (glass in a slim frame hung from a hinge 0.32 m ahead of the
design eye, stowing up and aft under the projector), and the HUD control panel on the glareshield strip
between AUTOLAND and MASTER CAUT, lettered with FlyByWire's own glyphs (fbw_hud_words.png), its NORM/X WIND
switch, OFF-BRT knob and DECLUTTER pushbutton being FlyByWire's own parts, added to the FlyByWire objects
they come from (fbw_parts.py).
Each seat hides with fbw/hud/<l|r>/installed; the combiner turns with fbw/hud/<l|r>/deploy."""

import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

import fbw_parts

EYE = np.array([-0.524, 3.158, -32.07])
GLASS_OFFSET = np.array([0., -0.008, -0.37])
GLASS_HALF = (0.135, 0.1075)
HINGE_Y = 3.262
HINGE_Z = -32.434
STOW_DEG = -95.0
STRIP_NORMAL = np.array([0., 0.35, 0.94])
STRIP_POINT = np.array([-0.522, 2.946, -32.733])
PANEL_X, PANEL_Y = -0.470, 2.953
PANEL_HALF = (0.065, 0.0170)
PANEL_PROUD = 0.0002
XWIND_U, DECLUTTER_U, CONTROL_V = -0.043, 0.043, -0.002
STRIP_OBJ = "a380_cockpit_030.obj"
STRIP_TEXTURE = "A380_COCKPIT_FCU_4K_ALBEDO_KFFFFFE.dds"
TEX = 1024

REGIONS = {
    "panel_face": (0, 0, 768, 201),
    "panel_side": (0, 300, 64, 364),
    "knob": (80, 300, 144, 364),
    "knurl": (880, 300, 944, 364),
    "housing": (160, 300, 224, 364),
    "frame": (240, 300, 304, 364),
    "metal": (320, 300, 384, 364),
    "lens": (400, 300, 464, 364),
    "glass": (480, 300, 544, 364),
    "white": (560, 300, 624, 364),
    "pb": (640, 300, 704, 364),
    "pb_face": (720, 300, 848, 428),
}
ALBEDO = {
    "panel_side": (58, 66, 76, 255),
    "knob": (34, 35, 38, 255),
    "knurl": (150, 152, 156, 255),
    "housing": (170, 178, 186, 255),
    "frame": (38, 40, 44, 255),
    "metal": (140, 146, 152, 255),
    "lens": (12, 16, 20, 255),
    "glass": (150, 205, 175, 30),
    "white": (236, 238, 240, 255),
    "pb": (46, 50, 56, 255),
}
MATERIAL = {
    "panel_face": (0, 90),
    "panel_side": (0, 90),
    "knob": (0, 120),
    "knurl": (170, 150),
    "housing": (0, 70),
    "frame": (0, 110),
    "metal": (200, 150),
    "lens": (0, 235),
    "glass": (0, 250),
    "white": (0, 80),
    "pb": (0, 130),
    "pb_face": (0, 150),
}
X, Y, Z = np.array([1., 0., 0.]), np.array([0., 1., 0.]), np.array([0., 0., 1.])


def unit(v):
    v = np.asarray(v, float)
    return v / np.linalg.norm(v)


class Mesh:
    def __init__(self):
        self.verts = []
        self.groups = {}

    def vert(self, p, n, uv):
        self.verts.append((*np.asarray(p, float), *unit(n), *uv))
        return len(self.verts) - 1

    def tri(self, group, a, b, c):
        pa, pb, pc = (np.array(self.verts[i][:3]) for i in (a, b, c))
        face = np.cross(pb - pa, pc - pa)
        if np.linalg.norm(face) < 1e-12:
            return
        normal = sum(np.array(self.verts[i][3:6]) for i in (a, b, c))
        if np.dot(face, normal) < 0:
            b, c = c, b
        self.groups.setdefault(group, []).append((a, b, c))


def region_uv(name, u=0.5, v=0.5):
    x0, y0, x1, y1 = REGIONS[name]
    return ((x0 + (x1 - x0) * u) / TEX, 1 - (y0 + (y1 - y0) * v) / TEX)


def rounded_rect(hw, hh, r, seg=6):
    pts = []
    for cx, cy, a0 in ((hw - r, hh - r, 0), (-hw + r, hh - r, 90), (-hw + r, -hh + r, 180), (hw - r, -hh + r, 270)):
        for k in range(seg + 1):
            a = math.radians(a0 + 90 * k / seg)
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts


def prism(mesh, group, origin, U, V, W, profile, depth, mat, face_mat=None, face_uv=None):
    o, U, V, W = (np.asarray(a, float) for a in (origin, U, V, W))
    n = len(profile)
    cu = sum(p[0] for p in profile) / n
    cv = sum(p[1] for p in profile) / n
    back = [o + U * pu + V * pv for pu, pv in profile]
    front = [p + W * depth for p in back]
    centre = o + U * cu + V * cv
    fm = face_mat or mat

    def fuv(pu, pv):
        return face_uv(pu, pv) if face_uv else region_uv(fm)

    c = mesh.vert(centre + W * depth, W, fuv(cu, cv))
    ring = [mesh.vert(front[i], W, fuv(*profile[i])) for i in range(n)]
    cb = mesh.vert(centre, -W, region_uv(mat))
    ringb = [mesh.vert(back[i], -W, region_uv(mat)) for i in range(n)]
    for i in range(n):
        j = (i + 1) % n
        mesh.tri(group, c, ring[i], ring[j])
        mesh.tri(group, cb, ringb[i], ringb[j])
        nrm = unit(np.cross(back[j] - back[i], W))
        if np.dot(nrm, back[i] - centre) < 0:
            nrm = -nrm
        ids = [mesh.vert(p, nrm, region_uv(mat)) for p in (front[i], front[j], back[j], back[i])]
        mesh.tri(group, ids[0], ids[1], ids[2])
        mesh.tri(group, ids[0], ids[2], ids[3])


def ring_prism(mesh, group, origin, U, V, W, outer, inner, depth, mat):
    origin, U, V, W = (np.asarray(a, float) for a in (origin, U, V, W))
    n = len(outer)
    for w, nrm in ((depth, W), (0., -W)):
        for i in range(n):
            j = (i + 1) % n
            ids = [mesh.vert(origin + U * q[0] + V * q[1] + W * w, nrm, region_uv(mat)) for q in (outer[i], outer[j], inner[j], inner[i])]
            mesh.tri(group, ids[0], ids[1], ids[2])
            mesh.tri(group, ids[0], ids[2], ids[3])
    for loop, out in ((outer, 1), (inner, -1)):
        for i in range(n):
            j = (i + 1) % n
            a = origin + U * loop[i][0] + V * loop[i][1]
            b = origin + U * loop[j][0] + V * loop[j][1]
            nrm = unit(np.cross(b - a, W))
            if np.dot(nrm, a - origin) * out < 0:
                nrm = -nrm
            ids = [mesh.vert(p, nrm, region_uv(mat)) for p in (a + W * depth, b + W * depth, b, a)]
            mesh.tri(group, ids[0], ids[1], ids[2])
            mesh.tri(group, ids[0], ids[2], ids[3])


def cylinder(mesh, group, base, axis, radius, length, mat, seg=24, knurl=False):
    axis = unit(axis)
    base = np.asarray(base, float)
    ref = Y if abs(axis[1]) < 0.9 else X
    u = unit(np.cross(axis, ref))
    v = np.cross(axis, u)
    top = base + axis * length
    rim = []
    for k in range(seg):
        a = 2 * math.pi * k / seg
        rim.append((u * math.cos(a) + v * math.sin(a), radius * (0.94 if knurl and k % 2 else 1.)))
    for k in range(seg):
        (d0, r0), (d1, r1) = rim[k], rim[(k + 1) % seg]
        ids = [mesh.vert(base + d0 * r0, d0, region_uv(mat)), mesh.vert(base + d1 * r1, d1, region_uv(mat)),
               mesh.vert(top + d1 * r1, d1, region_uv(mat)), mesh.vert(top + d0 * r0, d0, region_uv(mat))]
        mesh.tri(group, ids[0], ids[1], ids[2])
        mesh.tri(group, ids[0], ids[2], ids[3])
    for centre, nrm in ((top, axis), (base, -axis)):
        c = mesh.vert(centre, nrm, region_uv(mat))
        ids = [mesh.vert(centre + d * r, nrm, region_uv(mat)) for d, r in rim]
        for k in range(seg):
            mesh.tri(group, c, ids[k], ids[(k + 1) % seg])


def box(mesh, group, centre, U, V, W, half, mat):
    centre, W = np.asarray(centre, float), np.asarray(W, float)
    prism(mesh, group, centre - W * half[2], U, V, W,
          [(half[0], half[1]), (-half[0], half[1]), (-half[0], -half[1]), (half[0], -half[1])], 2 * half[2], mat)


def panel_frame(side):
    s = 1 if side == "r" else -1
    n = unit(STRIP_NORMAL)
    a = unit(np.cross(Y, n))
    b = np.cross(n, a)
    z = STRIP_POINT[2] - (PANEL_Y - STRIP_POINT[1]) * n[1] / n[2]
    centre = np.array([PANEL_X * -s, PANEL_Y, z]) + n * 0.0003
    return n, a, b, centre


def build_seat(mesh, side):
    s = 1 if side == "r" else -1
    eye = EYE * np.array([-s, 1, 1])
    st, cb = f"{side}_static", f"{side}_combiner"

    glass_c = eye + GLASS_OFFSET
    outer = rounded_rect(GLASS_HALF[0] + 0.008, GLASS_HALF[1] + 0.008, 0.042)
    inner = rounded_rect(GLASS_HALF[0], GLASS_HALF[1], 0.036)
    ring_prism(mesh, cb, glass_c - Z * 0.005, X, Y, Z, outer, inner, 0.010, "frame")
    bar_y = glass_c[1] + GLASS_HALF[1] + 0.018
    box(mesh, cb, np.array([eye[0], bar_y, glass_c[2]]), X, Y, Z, (0.09, 0.012, 0.010), "frame")
    hinge = np.array([eye[0], HINGE_Y, HINGE_Z])
    cylinder(mesh, cb, hinge - X * 0.105, X, 0.0095, 0.21, "metal", seg=18)
    link_half = (HINGE_Y - bar_y) / 2
    for dx in (-0.06, 0.06):
        box(mesh, cb, np.array([eye[0] + dx, bar_y + link_half, glass_c[2]]), X, Y, Z, (0.008, link_half + 0.004, 0.006), "metal")
    glass = rounded_rect(GLASS_HALF[0] + 0.002, GLASS_HALF[1] + 0.002, 0.037, seg=8)
    for w, nrm in ((0.0015, Z), (-0.0015, -Z)):
        cen = mesh.vert(glass_c + Z * w, nrm, region_uv("glass"))
        ids = [mesh.vert(glass_c + X * p[0] + Y * p[1] + Z * w, nrm, region_uv("glass")) for p in glass]
        for i in range(len(ids)):
            mesh.tri(f"{side}_glass", cen, ids[i], ids[(i + 1) % len(ids)])

    hz0, hz1 = eye[2] - 0.17, eye[2] + 0.21
    hy0, hy1 = 3.385, 3.76
    ch = 0.07
    profile = [(hz1, hy1), (hz0, hy1), (hz0, hy0 + ch), (hz0 + ch, hy0), (hz1, hy0)]
    prism(mesh, st, np.array([eye[0] - 0.15, 0., 0.]), Z, Y, X, profile, 0.30, "housing")
    lens_n = unit(np.array([0., -1., -1.]))
    lens_c = np.array([eye[0], hy0 + ch / 2, hz0 + ch / 2])
    prism(mesh, st, lens_c, X, unit(np.cross(lens_n, X)), lens_n, rounded_rect(0.095, 0.022, 0.01), 0.002, "lens")
    for dx in (-0.11, 0.11):
        a = np.array([eye[0] + dx, HINGE_Y, glass_c[2]])
        b = np.array([eye[0] + dx, hy0 + 0.02, hz0 + 0.03])
        d = unit(b - a)
        box(mesh, st, (a + b) / 2, X, unit(np.cross(d, X)), d, (0.008, 0.014, np.linalg.norm(b - a) / 2 + 0.012), "metal")
    for dx in (-0.112, 0.112):
        cylinder(mesh, st, hinge + X * dx - X * 0.006, X, 0.016, 0.012, "metal", seg=18)

    n, a, b, centre = panel_frame(side)
    hw, hh = PANEL_HALF
    prism(mesh, f"{side}_panel", centre, a, b, n, rounded_rect(hw, hh, 0.0015), PANEL_PROUD, "panel_side", face_mat="panel_face",
          face_uv=lambda pu, pv: region_uv("panel_face", (pu + hw) / (2 * hw), (hh - pv) / (2 * hh)))
    top = centre + n * PANEL_PROUD
    controls = {"xwind": top + a * XWIND_U + b * CONTROL_V, "bright": top + b * CONTROL_V, "declutter": top + a * DECLUTTER_U + b * CONTROL_V}

    xw = controls["xwind"]
    cylinder(mesh, f"{side}_panel", xw, n, 0.0050, 0.0015, "metal", seg=6)
    cylinder(mesh, f"{side}_panel", xw + n * 0.0015, n, 0.0034, 0.0030, "metal", seg=16)
    dc = controls["declutter"]
    prism(mesh, f"{side}_panel", dc - n * 0.0004, a, b, n, rounded_rect(0.0090, 0.0090, 0.0012), 0.0010, "frame")
    return {"hinge": hinge, "controls": controls, "normal": n, "a": a, "glass": glass_c}


def strip_samples(objects_dir, w, h):
    """The glareshield strip's own paint, normals and material under the captain's panel, texel for texel."""
    objects = Path(objects_dir)
    lines = (objects / STRIP_OBJ).read_text(encoding="utf-8", errors="replace").splitlines()
    vt = np.array([[float(x) for x in l.split()[1:9]] for l in lines if l.startswith("VT")])
    idx = np.array([int(x) for l in lines if l.startswith(("IDX10", "IDX ")) for x in l.split()[1:]], dtype=np.int64).reshape(-1, 3)
    maps = {}
    for l in lines:
        words = l.split()
        if words[:1] == ["TEXTURE"]:
            maps["alb"] = words[1]
        elif words[:2] == ["TEXTURE_MAP", "normal"]:
            maps["nml"] = words[2]
        elif words[:2] == ["TEXTURE_MAP", "material_gloss"]:
            maps["mat"] = words[2]
        elif words[:1] == ["POINT_COUNTS"]:
            break
    n, a, b, centre = panel_frame("l")
    T = vt[idx]
    normals = np.cross(T[:, 1, :3] - T[:, 0, :3], T[:, 2, :3] - T[:, 0, :3])
    normals /= np.maximum(np.linalg.norm(normals, axis=1), 1e-12)[:, None]
    near = (np.abs(normals @ n) > 0.98) & (np.linalg.norm(T[:, :, :3].mean(1) - centre, axis=1) < 0.15)
    T = T[near]
    q = T[:, :, :3] - centre
    tu, tv = q @ a, q @ b
    hw, hh = PANEL_HALF
    gu = -hw + (np.arange(w) + 0.5) / w * 2 * hw
    gv = hh - (np.arange(h) + 0.5) / h * 2 * hh
    U, V = np.meshgrid(gu, gv)
    U, V = U.ravel(), V.ravel()
    uv = np.full((len(U), 2), np.nan)
    depth = np.full(len(U), -np.inf)
    jac = []
    for k in range(len(T)):
        x0, y0 = tu[k, 0], tv[k, 0]
        e1 = np.array([tu[k, 1] - x0, tv[k, 1] - y0])
        e2 = np.array([tu[k, 2] - x0, tv[k, 2] - y0])
        den = e1[0] * e2[1] - e2[0] * e1[1]
        if abs(den) < 1e-14:
            continue
        px, py = U - x0, V - y0
        l1 = (px * e2[1] - e2[0] * py) / den
        l2 = (e1[0] * py - px * e1[1]) / den
        inside = (l1 >= -1e-6) & (l2 >= -1e-6) & (l1 + l2 <= 1 + 1e-6)
        h_k = q[k, 0] @ n + l1 * ((q[k, 1] - q[k, 0]) @ n) + l2 * ((q[k, 2] - q[k, 0]) @ n)
        better = inside & (h_k > depth)
        if better.any():
            t_uv = T[k, :, 6:8]
            uv[better] = t_uv[0] + l1[better, None] * (t_uv[1] - t_uv[0]) + l2[better, None] * (t_uv[2] - t_uv[0])
            depth[better] = h_k[better]
            m = np.linalg.solve(np.array([e1, e2]).T, np.eye(2))
            jac.append(np.array([t_uv[1] - t_uv[0], t_uv[2] - t_uv[0]]).T @ m)
    ok = ~np.isnan(uv[:, 0])
    print("strip: sampled", int(ok.sum()), "of", len(U), "texels from", maps)

    def sample(name, mode):
        tex = np.asarray(Image.open(objects / maps[name]).convert(mode))
        th, tw = tex.shape[:2]
        out = np.zeros((len(U), tex.shape[2]), np.uint8)
        out[ok] = tex[np.clip(((1 - uv[ok, 1]) * th).astype(int), 0, th - 1), np.clip((uv[ok, 0] * tw).astype(int), 0, tw - 1)]
        if ok.any():
            out[~ok] = out[ok].mean(0).astype(np.uint8)
        return out

    alb = sample("alb", "RGBA")
    alb[:, 3] = 255
    out = {"alb": Image.fromarray(alb.reshape(h, w, 4), "RGBA")}
    if "nml" in maps and jac:
        J = np.mean(jac, axis=0)
        g_u = J[0] / np.linalg.norm(J[0])
        g_v = J[1] / np.linalg.norm(J[1])
        raw = sample("nml", "RGB").astype(float) / 255 * 2 - 1
        na = raw[:, 0] * g_u[0] + raw[:, 1] * g_v[0]
        nb = raw[:, 0] * g_u[1] + raw[:, 1] * g_v[1]
        nml = np.stack([na, nb, raw[:, 2]], 1)
        out["nml"] = Image.fromarray(np.clip((nml + 1) / 2 * 255 + 0.5, 0, 255).astype(np.uint8).reshape(h, w, 3), "RGB")
    if "mat" in maps:
        out["mat"] = Image.fromarray(sample("mat", "RGB").reshape(h, w, 3), "RGB")
    return out


FBW_WORDS = Path(__file__).with_name("fbw_hud_words.png")
FBW_LETTER_M = 0.0049 / 78
FBW_BANDS = [(20, 72), (115, 167), (210, 262)]
FBW_SPANS = {"OFF": (0, 484, 599), "NORM": (1, 27, 194), "X": (1, 221, 258), "WINF": (1, 281, 435), "BRT": (1, 461, 571),
             "HUD": (1, 597, 715), "DECLUTTER": (2, 27, 358)}


def fbw_letters(mask):
    a = np.asarray(mask) > 60
    cols = np.where(a.any(axis=0))[0]
    out, start = [], cols[0]
    for c0, c1 in zip(cols, cols[1:]):
        if c1 > c0 + 1:
            out.append((start, c0 + 1))
            start = c1
    out.append((start, cols[-1] + 1))
    return out


def fbw_word(word):
    sheet = Image.open(FBW_WORDS).getchannel("A")

    def span(name):
        band, x0, x1 = FBW_SPANS[name]
        y0, y1 = FBW_BANDS[band]
        return sheet.crop((x0, y0, x1, y1))

    if word != "X WIND":
        return span(word)
    x, winf, hud = span("X"), span("WINF"), span("HUD")
    win = winf.crop((0, 0, fbw_letters(winf)[-1][0], winf.height))
    d_ = hud.crop((fbw_letters(hud)[-1][0], 0, hud.width, hud.height))
    gap, kern = 24, 6
    out = Image.new("L", (x.width + gap + win.width + kern + d_.width, x.height))
    out.paste(x, (0, 0))
    out.paste(win, (x.width + gap, 0))
    out.paste(d_, (x.width + gap + win.width + kern, 0))
    return out


def bake_textures(out_dir, font_path, source_dir):
    alb = Image.new("RGBA", (TEX, TEX), (0, 0, 0, 0))
    d = ImageDraw.Draw(alb)
    for name, col in ALBEDO.items():
        d.rectangle(REGIONS[name], fill=col)
    x0, y0, x1, y1 = REGIONS["panel_face"]
    strip = strip_samples(source_dir, x1 - x0, y1 - y0)
    alb.paste(strip["alb"], (x0, y0))
    lit = Image.new("RGB", (TEX, TEX), (0, 0, 0))
    dl = ImageDraw.Draw(lit)
    lit_col = (255, 246, 226)
    px = (x1 - x0) / (2 * PANEL_HALF[0])
    white = (236, 239, 242, 255)
    line_w = max(2, round(0.0005 * px))

    def at(u, v):
        return (x0 + (u + PANEL_HALF[0]) * px, y0 + (PANEL_HALF[1] - v) * px)

    def label(u, v, word):
        mask = fbw_word(word)
        k = FBW_LETTER_M * px
        mask = mask.resize((max(1, round(mask.width * k)), max(1, round(mask.height * k))), Image.LANCZOS)
        cx, cy = at(u, v)
        corner = (round(cx - mask.width / 2), round(cy - mask.height / 2))
        alb.paste(Image.new("RGBA", mask.size, white), corner, mask)
        lit.paste(Image.new("RGB", mask.size, lit_col), corner, mask)

    label(0.0, 0.0128, "HUD")
    label(-0.0165, -0.0130, "OFF")
    label(0.0165, -0.0130, "BRT")
    label(XWIND_U, 0.0128, "NORM")
    label(XWIND_U, -0.0130, "X WIND")
    label(DECLUTTER_U, 0.0128, "DECLUTTER")
    cx, cy = at(0.0, CONTROL_V)
    r = 0.0128 * px
    box_ = (cx - r, cy - r, cx + r, cy + r)
    d.arc(box_, 150, 390, fill=white, width=line_w)
    dl.arc(box_, 150, 390, fill=lit_col, width=line_w)
    end = math.radians(30)
    tip = (cx + r * math.cos(end), cy + r * math.sin(end))
    head = 0.0016 * px
    tangent = (-math.sin(end), math.cos(end))
    normal = (math.cos(end), math.sin(end))
    arrow = [(tip[0] + tangent[0] * head, tip[1] + tangent[1] * head),
             (tip[0] - normal[0] * head * 0.8, tip[1] - normal[1] * head * 0.8),
             (tip[0] + normal[0] * head * 0.8, tip[1] + normal[1] * head * 0.8)]
    d.polygon(arrow, fill=white)
    dl.polygon(arrow, fill=lit_col)
    px0, py0, px1, py1 = REGIONS["pb_face"]
    d.rectangle((px0, py0, px1, py1), fill=(52, 56, 62, 255))
    d.rectangle((px0 + 6, py0 + 6, px1 - 6, py1 - 6), outline=(92, 98, 106, 255), width=3)
    alb.save(out_dir / "a380_hud_ALB.png")
    lit.save(out_dir / "a380_hud_LIT.png")
    nml = Image.new("RGB", (TEX, TEX), (128, 128, 255))
    if "nml" in strip:
        nml.paste(strip["nml"], (x0, y0))
    nml.save(out_dir / "a380_hud_NML.png")
    mat = Image.new("RGB", (TEX, TEX), (0, 80, 0))
    md = ImageDraw.Draw(mat)
    for name, (metal, gloss) in MATERIAL.items():
        md.rectangle(REGIONS[name], fill=(metal, gloss, 0))
    if "mat" in strip:
        mat.paste(strip["mat"], (x0, y0))
    mat.save(out_dir / "a380_hud_MAT.png")


def pivot(p, axis, r1, r2, dataref):
    a = [f"{c:.5f}" for c in p]
    b = [f"{-c:.5f}" for c in p]
    return [f"ANIM_trans {a[0]} {a[1]} {a[2]} {a[0]} {a[1]} {a[2]} 0 0 none",
            f"ANIM_rotate {axis[0]:.6f} {axis[1]:.6f} {axis[2]:.6f} {r1} {r2} 0 1 {dataref}",
            f"ANIM_trans {b[0]} {b[1]} {b[2]} {b[0]} {b[1]} {b[2]} 0 0 none"]


def write_obj(path, mesh, seats):
    idx = []
    spans = {}
    for name, tris in mesh.groups.items():
        start = len(idx)
        for a, b, c in tris:
            idx.extend([a, c, b])
        spans[name] = f"TRIS {start} {len(idx) - start}"
    lines = ["I", "800", "OBJ", "", "GLOBAL_cockpit_lit", "TEXTURE a380_hud_ALB.png", "TEXTURE_LIT a380_hud_LIT.png", "TEXTURE_MAP normal a380_hud_NML.png",
             "TEXTURE_MAP material_gloss a380_hud_MAT.png", "GLOBAL_specular 1.0", f"POINT_COUNTS {len(mesh.verts)} 0 0 {len(idx)}", ""]
    lines += [f"VT {x:.5f} {y:.5f} {z:.5f} {a:.4f} {b:.4f} {c:.4f} {u:.5f} {v:.5f}" for x, y, z, a, b, c, u, v in mesh.verts]
    full = len(idx) - len(idx) % 10
    lines += ["IDX10 " + " ".join(map(str, idx[k:k + 10])) for k in range(0, full, 10)]
    lines += [f"IDX {i}" for i in idx[full:]]
    lines += [""]
    for side in seats:
        lines += ["ANIM_begin", f"ANIM_hide -0.5 0.5 fbw/hud/{side}/installed", spans[f"{side}_panel"], "ANIM_end"]
    Path(path).write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    return len(mesh.verts), len(idx) // 3


def main(objects_dir, font_path, source_dir=None, originals=None):
    out = Path(objects_dir)
    mesh = Mesh()
    seats = {side: build_seat(mesh, side) for side in ("l", "r")}
    bake_textures(out, font_path, Path(source_dir) if source_dir else out)
    v, t = write_obj(out / "a380_hud.obj", mesh, seats)
    print("a380_hud.obj", v, "vertices", t, "triangles")
    for name, count in fbw_parts.build(Path(source_dir) if source_dir else out, out, seats, originals).items():
        print(name, count, "HUD parts added")
    spots = fbw_parts.add_click_spots(Path(source_dir) if source_dir else out, out, seats, STOW_DEG, originals)
    print(fbw_parts.CLICK_OBJ, spots, "HUD click spots added")
    for side, info in seats.items():
        print(side, "hinge", np.round(info["hinge"], 3).tolist(), {k: np.round(p, 3).tolist() for k, p in info["controls"].items()})


if __name__ == "__main__":
    main(*sys.argv[1:5])
