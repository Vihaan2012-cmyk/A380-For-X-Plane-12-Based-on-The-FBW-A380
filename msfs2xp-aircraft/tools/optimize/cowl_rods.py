"""usage: cowl_rods.py <objects dir> [<out dir>]

Hold-open rods for the opened nacelles (a380_cowl_rods.obj + a380_cowl_rods_ALB/MAT.png): two per fan
cowl door from the fan case to the door, one per thrust reverser C-duct half from the core cowl to the
C-duct, Trent and GP sets. The crew fits them once a door is fully up, so the door rods show at
fbw/anim/cowl_open_N >= 0.5 and the C-duct rods at 1. The door attachment points are found on the
nacelle skin and swung through the same hinge turns cowl_hinges.py and reverser_hinges.py write.
Run after both."""

import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

import cowl_hinges as ch
import cowl_separation as cs
import reverser_hinges as rh
from gp7200_variant import LIFT

DOOR_STATIONS = (-0.95, -1.40)
DOOR_ATTACH_DEG = 122.0
CASE_ATTACH_DEG = 94.0
CASE_R = 1.60
CDUCT_STATION = -4.15
CDUCT_ATTACH_DEG = 116.0
CORE_ATTACH_DEG = 100.0
CORE_R = 1.16
CDUCT_INNER_R = 1.88
ROD_R = 0.016
END_R = 0.028
TEX = 64
COLOURS = {"rod": ((168, 172, 176), 200, 170), "end": ((48, 50, 54), 60, 120), "band": ((214, 40, 32), 0, 90)}


def rotate(v, axis, deg):
    k = axis / np.linalg.norm(axis)
    t = math.radians(deg)
    return v * math.cos(t) + np.cross(k, v) * math.sin(t) + k * (k @ v) * (1 - math.cos(t))


def at(fr, along, deg, r):
    p, d, up, side, tip = fr
    a = math.radians(deg)
    return p + d * (tip + along) + (up * math.cos(a) + side * math.sin(a)) * r


def skin_radius(verts, fr, along, deg):
    p, d, up, side, tip = fr
    q = verts - p
    ax = q @ d - tip
    radial = q - np.outer(q @ d, d)
    r = np.linalg.norm(radial, axis=1)
    ang = np.degrees(np.arctan2(radial @ side, radial @ up))
    m = (np.abs(ax - along) < 0.08) & (np.abs((ang - deg + 180) % 360 - 180) < 6) & (r > 1.5) & (r < 2.4)
    if not m.any():
        return None
    return float(np.max(r[m]))


def uv(name):
    i = list(COLOURS).index(name)
    return ((i * 16 + 8) / TEX, 0.5)


class Mesh:
    def __init__(self):
        self.verts, self.idx = [], []

    def tube(self, a, b, r, name, seg=10):
        d = b - a
        L = np.linalg.norm(d)
        d = d / L
        ref = np.array([0., 1., 0.]) if abs(d[1]) < 0.9 else np.array([1., 0., 0.])
        u = np.cross(d, ref)
        u /= np.linalg.norm(u)
        v = np.cross(d, u)
        base = len(self.verts)
        for k in range(seg):
            t = 2 * math.pi * k / seg
            n = u * math.cos(t) + v * math.sin(t)
            self.verts.append((*(a + n * r), *n, *uv(name)))
            self.verts.append((*(b + n * r), *n, *uv(name)))
        for k in range(seg):
            i0, i1 = base + 2 * k, base + 2 * ((k + 1) % seg)
            self.idx += [i0, i1 + 1, i1, i0, i0 + 1, i1 + 1]
        for end, nrm, off in ((a, -d, 0), (b, d, 1)):
            c = len(self.verts)
            self.verts.append((*end, *nrm, *uv(name)))
            for k in range(seg):
                i0, i1 = base + 2 * k + off, base + 2 * ((k + 1) % seg) + off
                self.idx += [c, i1, i0] if off else [c, i0, i1]

    def rod(self, a, b):
        d = (b - a) / np.linalg.norm(b - a)
        self.tube(a - d * 0.02, a + d * 0.07, END_R, "end")
        self.tube(b - d * 0.07, b + d * 0.02, END_R, "end")
        self.tube(a + d * 0.07, b - d * 0.07, ROD_R, "rod")
        mid = (a + b) / 2
        self.tube(mid - d * 0.04, mid + d * 0.04, ROD_R * 1.25, "band")

    def span(self, start):
        return start, len(self.idx) - start


def engine_rods(mesh, verts, fr, e):
    p, d, up, side, tip = fr
    door, cduct = [], []
    for sign in (-1, 1):
        door_hinge = p + d * (tip - 2.0) + up * ch.HINGE_UP_M + side * (sign * ch.HINGE_SIDE_M)
        for station in DOOR_STATIONS:
            r = skin_radius(verts, fr, station, sign * DOOR_ATTACH_DEG)
            if r is None:
                raise SystemExit(f"engine {e + 1}: no cowl skin at {station} m, {sign * DOOR_ATTACH_DEG} deg")
            closed = at(fr, station, sign * DOOR_ATTACH_DEG, r - 0.035)
            opened = door_hinge + rotate(closed - door_hinge, d, -sign * ch.OPEN_DEG)
            door.append((at(fr, station, sign * CASE_ATTACH_DEG, CASE_R), opened))
        duct_hinge = p + d * (tip - 5.0) + up * ch.HINGE_UP_M + side * (sign * ch.HINGE_SIDE_M)
        closed = at(fr, CDUCT_STATION, sign * CDUCT_ATTACH_DEG, CDUCT_INNER_R)
        opened = duct_hinge + rotate(closed - duct_hinge, d, -sign * rh.OPEN_DEG)
        cduct.append((at(fr, CDUCT_STATION, sign * CORE_ATTACH_DEG, CORE_R), opened))
    s = len(mesh.idx)
    for a, b in door:
        mesh.rod(a, b)
    door_span = mesh.span(s)
    s = len(mesh.idx)
    for a, b in cduct:
        mesh.rod(a, b)
    return door_span, mesh.span(s), [float(np.linalg.norm(b - a)) for a, b in door + cduct]


def skin_vertices(objects, suffix):
    rows = []
    for stem in ("022", "023"):
        for line in (objects / f"a380_exterior_{stem}{suffix}.obj").read_text(encoding="utf-8").split("\n"):
            if line.startswith("VT"):
                rows.append([float(x) for x in line.split()[1:4]])
    return np.array(rows)


def main(objects_dir, out_dir=None):
    objects = Path(objects_dir)
    out = Path(out_dir) if out_dir else objects
    axes, tip, _ = cs.main(objects, preview=True)
    gp_axes = {e: (p + np.array([0.0, LIFT[e + 1], 0.0]), d) for e, (p, d) in axes.items()}
    mesh = Mesh()
    sets = []
    for suffix, chosen, gate in (("", axes, "ANIM_hide 0.5 1.5 fbw/engine/gp7200"), ("_gp", gp_axes, "ANIM_hide -0.5 0.5 fbw/engine/gp7200")):
        verts = skin_vertices(objects, suffix)
        engines = []
        for e in sorted(chosen):
            door, cduct, lengths = engine_rods(mesh, verts, ch.frame(chosen[e], tip[e]), e)
            engines.append((e, door, cduct))
            print(suffix or "trent", "engine", e + 1, "rod lengths", [round(x, 2) for x in lengths])
        sets.append((gate, engines))
    lines = ["I", "800", "OBJ", "", "TEXTURE a380_cowl_rods_ALB.png", "TEXTURE_MAP material_gloss a380_cowl_rods_MAT.png", "GLOBAL_specular 1.0",
             f"POINT_COUNTS {len(mesh.verts)} 0 0 {len(mesh.idx)}", ""]
    lines += [f"VT {' '.join(f'{x:.5f}' for x in v)}" for v in mesh.verts]
    full = len(mesh.idx) - len(mesh.idx) % 10
    lines += [f"IDX10 {' '.join(map(str, mesh.idx[k:k + 10]))}" for k in range(0, full, 10)]
    lines += [f"IDX {i}" for i in mesh.idx[full:]]
    lines += ["", "ATTR_LOD 0 400", "ANIM_begin", "ANIM_hide 0.5 1.5 fbw/anim/eye_in_flight_deck"]
    for gate, engines in sets:
        lines += ["ANIM_begin", gate]
        for e, door, cduct in engines:
            n = e + 1
            lines += ["ANIM_begin", f"ANIM_hide 5.5 6.5 sim/operation/failures/rel_engsep{e}", f"ANIM_hide 0.5 1.5 fbw/fail/sep/cowl_{n}",
                      "ANIM_begin", f"ANIM_hide -1 0.495 fbw/anim/cowl_open_{n}", f"TRIS {door[0]} {door[1]}", "ANIM_end", "ANIM_end"]
        lines += ["ANIM_end"]
    lines += ["ANIM_end"]
    (out / "a380_cowl_rods.obj").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    alb = Image.new("RGB", (TEX, 16))
    mat = Image.new("RGB", (TEX, 16))
    da, dm = ImageDraw.Draw(alb), ImageDraw.Draw(mat)
    for i, (rgb, metal, gloss) in enumerate(COLOURS.values()):
        da.rectangle((i * 16, 0, i * 16 + 15, 15), fill=rgb)
        dm.rectangle((i * 16, 0, i * 16 + 15, 15), fill=(metal, gloss, 0))
    alb.save(out / "a380_cowl_rods_ALB.png")
    mat.save(out / "a380_cowl_rods_MAT.png")
    print("a380_cowl_rods.obj", len(mesh.verts), "vertices", len(mesh.idx) // 3, "triangles")


if __name__ == "__main__":
    main(*sys.argv[1:3])
