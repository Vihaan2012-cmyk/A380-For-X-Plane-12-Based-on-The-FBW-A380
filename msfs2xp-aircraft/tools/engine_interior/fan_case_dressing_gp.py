"""Blender script: the equipment on the GP7200's fan case, as seen with the fan cowl doors off.

    blender -b --factory-startup -P fan_case_dressing_gp.py -- <out.obj> [<out.blend>]

Engine-local axes as fan_case_dressing.py (X starboard, Y forward, Z up, clock angle 0 = top, 90 = starboard) but Y = -s with s the station from
the spinner tip, so the fan case runs from Y -0.213 to -2.143 (gp7200_spec.json, core.module_stations). The accessory gearbox is core-mounted
(core.gearbox_and_externals) and lives in a380_engcore_gp.obj; the fan case carries the oil tank at 9 o'clock between s 1.6 and 2.1, the
ignition exciters, fuel / hydraulic / bleed / lube lines, harness trunks, fire loops, sensors and the flange bolt rows. Everything follows the
measured case radius along the axis and is held inside the closed door's inner skin from gp7200_nacelle (door_inner)."""

import json
import math
import os
import random
import sys

import bpy
import numpy as np
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fan_case_dressing as fcd  # noqa: E402

SPEC = json.load(open("E:/fbw-int/msfs2xp-aircraft/tools/optimize/gp7200_spec.json", encoding="utf-8"))
CASE_R = fcd.CASE_R
DOOR_T = 0.10
CLEAR = 0.010
S_FRONT, S_REAR = 0.213, 2.143
FLANGES = [(0.213, 1.575, 0.030, 150), (0.325, 1.559, 0.022, 140), (1.288, 1.568, 0.030, 150), (1.496, 1.569, 0.040, 150), (1.861, 1.587, 0.034, 160), (2.044, 1.585, 0.040, 170)]


_finish, _tube = fcd.finish, fcd.tube


def finish_lo(obj, mat, bevel=0.0, smooth=True):
    out = _finish(obj, mat, bevel if bevel >= 0.004 else 0.0, smooth)
    for m in obj.modifiers:
        if m.type == "BEVEL":
            m.segments = 1
    return out


def tube_lo(name, points, radius, mat, resolution=4, bevel_resolution=None):
    return _tube(name, points, radius, mat, resolution=2, bevel_resolution=0 if radius < 0.012 else 1)


fcd.finish, fcd.tube = finish_lo, tube_lo


def y_of_s(s):
    return -s


def remap_y(y_old):
    """The older GP layout was drawn for a case from Y -0.51 to -2.40; its stations map onto the real case 0.213-2.143."""
    return -(S_FRONT + 0.06 + (-y_old - 0.51) * (S_REAR - S_FRONT - 0.12) / (2.40 - 0.51))


def env_table():
    rows = np.array(SPEC["core"]["casing_outer_envelope"]["rows"])
    sel = rows[(rows[:, 0] >= 0.2) & (rows[:, 0] <= 2.11)]
    s = np.arange(0.2, 2.12, 0.01)
    r = np.interp(s, sel[:, 0], (sel[:, 1] + sel[:, 2]) / 2)
    k = 9
    pad = np.concatenate([np.full(k, r[0]), r, np.full(k, r[-1])])
    return s, np.convolve(pad, np.ones(2 * k + 1) / (2 * k + 1), mode="valid")


ENV_S, ENV_R = env_table()


def case_r(s):
    return np.interp(s, ENV_S, ENV_R)


def door_limit(theta, s):
    top = np.array(SPEC["outer_nacelle"]["top"])
    bot = np.array(SPEC["outer_nacelle"]["bottom"])
    rt = np.interp(s, top[:, 0], top[:, 1])
    rb = np.interp(s, bot[:, 0], bot[:, 1])
    c, rs = (rt - rb) / 2, (rt + rb) / 2 - DOOR_T
    cu = c * np.cos(theta)
    return cu + np.sqrt(np.maximum(rs * rs - (c * c - cu * cu), 0.0)) - CLEAR


def oil_tank_gp(rng):
    th, s_mid, length, rad = 270.0, 1.85, 0.50, 0.092
    y = y_of_s(s_mid)
    r_c = CASE_R + 0.016 + rad
    parts = []
    m = fcd.frame_at(th, y, r_c) @ Matrix.Rotation(math.radians(90), 4, "X")
    parts.append(fcd.cylinder("oil_tank", rad, length - 2 * rad * 0.6, m, "aluminium", 32, 0.006))
    for sgn in (-1, 1):
        bm = fcd.new_bm()
        fcd.bmesh.ops.create_uvsphere(bm, u_segments=24, v_segments=12, radius=1.0, calc_uvs=True)
        fcd.bmesh.ops.scale(bm, vec=Vector((rad, rad, rad * 0.6)), verts=bm.verts)
        fcd.bmesh.ops.transform(bm, matrix=fcd.frame_at(th, y + sgn * (length / 2 - rad * 0.6), r_c) @ Matrix.Rotation(math.radians(90), 4, "X"), verts=bm.verts)
        parts.append(fcd.finish(fcd.mesh_object(f"oil_tank_end{sgn}", bm), "aluminium", 0.0))
    parts.append(fcd.cylinder("oil_tank_neck", 0.03, 0.07, fcd.frame_at(th - 14, y + 0.12, r_c + rad * 0.9) @ Matrix.Identity(4), "casting", 18, 0.004))
    parts.append(fcd.cylinder("oil_tank_cap", 0.038, 0.022, fcd.frame_at(th - 14, y + 0.12, r_c + rad * 0.9 + 0.045), "stainless", 18, 0.003))
    parts.append(fcd.box("oil_tank_sight", (0.045, 0.1, 0.008), fcd.frame_at(th + 28, y - 0.08, r_c + rad * 0.93), "label", 0.002))
    for k in (-1, 1):
        parts.append(fcd.box(f"oil_tank_strap{k}", (0.12, 0.03, 0.01), fcd.frame_at(th, y + k * length * 0.3, r_c + rad * 0.98), "titanium", 0.002))
        parts.append(fcd.box(f"oil_tank_foot{k}", (0.07, 0.04, r_c - CASE_R - rad * 0.4), fcd.frame_at(th, y + k * length * 0.3, CASE_R + (r_c - CASE_R - rad * 0.4) / 2), "casting", 0.003))
    for dy, mat in ((0.2, "firesleeve"), (-0.2, "stainless")):
        pts = fcd.arc_points(th - 10, th - 55, y + dy, CASE_R + 0.03, step=6.0)
        parts.append(fcd.tube(f"oil_line{dy}", pts, 0.012, mat))
        parts.append(fcd.fitting(f"oil_line_end{dy}", pts[0], pts[1] - pts[0], 0.012))
    return parts


def flanges_gp(rng):
    parts = []
    for k, (s, r_f, width, bolts) in enumerate(FLANGES):
        parts += fcd.flange(f"flange{k}", y_of_s(s), CASE_R, r_f + 0.012 - float(case_r(max(s, 0.2))) + CASE_R, width, bolts, rng)
    return parts


LAYOUT = {
    "agb": (166, 194, -1.80, -2.08),
    "oil_filter": (147, -1.38),
    "fuel_filters": (131, -1.30),
    "oil_tank": (270, -1.85, 0.5),
    "eec": (58, -1.35),
    "exciters": [(78, remap_y(-1.12)), (88, remap_y(-1.82))],
    "pipes": [
        {"mat": "stainless", "radius": 0.015, "r": 1.62, "path": [("arc", 18, 150, remap_y(-0.70)), ("axial", 150, remap_y(-0.7), remap_y(-1.2))], "clamps": 0.27},
        {"mat": "firesleeve", "radius": 0.02, "r": 1.65, "path": [("arc", 130, 165, remap_y(-1.85)), ("axial", 165, remap_y(-1.85), remap_y(-2.3))], "clamps": 0.28},
        {"mat": "titanium", "radius": 0.018, "r": 1.66, "path": [("axial", 120, remap_y(-0.6), remap_y(-2.3))], "clamps": 0.3},
        {"mat": "stainless", "radius": 0.011, "r": 1.59, "path": [("arc", 205, 250, remap_y(-0.82))], "clamps": 0.25},
        {"mat": "firesleeve", "radius": 0.024, "r": 1.69, "path": [("arc", 125, 175, remap_y(-2.0))], "clamps": 0.26},
        {"mat": "stainless", "radius": 0.013, "r": 1.63, "path": [("axial", 305, remap_y(-0.7), remap_y(-2.25)), ("arc", 305, 335, remap_y(-2.25))], "clamps": 0.3},
        {"mat": "titanium", "radius": 0.012, "r": 1.605, "path": [("axial", 80, remap_y(-0.65), remap_y(-2.4))], "clamps": 0.29},
        {"mat": "stainless", "radius": 0.009, "r": 1.585, "path": [("arc", 25, 95, remap_y(-2.25)), ("axial", 95, remap_y(-2.25), remap_y(-2.4))], "clamps": 0.22},
    ],
    "harness": [
        {"r": 1.615, "radius": 0.024, "arcs": [(28, 155, remap_y(-1.9))]},
        {"r": 1.605, "radius": 0.017, "arcs": [(285, 345, remap_y(-1.6))]},
        {"r": 1.62, "radius": 0.022, "arcs": [(45, 115, remap_y(-0.95))]},
    ],
    "fire_loops": [remap_y(-0.92), remap_y(-2.1)],
    "banks": [(20, 152, remap_y(-2.28), 3, 1.585, 0.01), (290, 345, remap_y(-2.3), 3, 1.585, 0.009), (35, 105, remap_y(-1.1), 3, 1.59, 0.009), (295, 345, remap_y(-1.25), 3, 1.588, 0.008),
              (125, 148, remap_y(-0.72), 2, 1.59, 0.012), (45, 140, remap_y(-1.75), 3, 1.62, 0.009)],
    "drops": [(100, remap_y(-2.15), remap_y(-1.45), 1.6, 0.013, "stainless"), (122, remap_y(-0.8), remap_y(-1.25), 1.63, 0.012, "firesleeve"), (140, remap_y(-1.95), remap_y(-1.6), 1.6, 0.01, "stainless"),
              (232, remap_y(-1.4), remap_y(-1.95), 1.64, 0.016, "firesleeve")],
    "sensors": [(45, remap_y(-0.66)), (80, remap_y(-1.95)), (110, remap_y(-0.9)), (236, remap_y(-2.0)), (322, remap_y(-1.45)), (90, remap_y(-1.6)), (305, remap_y(-2.15)), (20, remap_y(-2.25))],
    "junctions": [(32, remap_y(-1.85)), (335, remap_y(-1.0)), (102, remap_y(-1.3))],
    "routes": 40, "looms": 12, "gadgets": 44,
}


def build():
    rng = random.Random(7295)
    fcd.Y_FRONT, fcd.Y_REAR = y_of_s(S_FRONT + 0.16), y_of_s(S_REAR - 0.12)
    layout = LAYOUT
    parts = []
    parts += flanges_gp(rng)
    parts += oil_tank_gp(rng)
    parts += fcd.exciters(layout)
    parts += fcd.pipe_runs(layout, rng)
    parts += fcd.harnesses(layout, rng)
    parts += fcd.fire_loops(layout)
    parts += fcd.tube_banks(layout, rng)
    parts += fcd.drops(layout, rng)
    parts += fcd.sensors(layout, rng)
    parts += fcd.routed_lines(layout, rng)
    parts += fcd.looms(layout, rng)
    parts += fcd.gadgets(layout, rng)
    return parts


def conform(obj):
    """Follow the measured case radius along the axis, then hold every vertex inside the closed door's inner skin."""
    me = obj.data
    n = len(me.vertices)
    co = np.empty(n * 3)
    me.vertices.foreach_get("co", co)
    co = co.reshape(-1, 3)
    x, z, s = co[:, 0], co[:, 2], -co[:, 1]
    r = np.hypot(x, z)
    th = np.arctan2(x, z)
    dr = case_r(np.clip(s, 0.2, 2.11)) - CASE_R
    r2 = np.where(r > CASE_R - 0.05, r + dr, r)
    lim = door_limit(th, np.clip(s, 0.3, 2.4))
    over = r2 - lim
    print("DRESSING over the door skin before clamp: max", round(float(over.max()), 4), "m on", int((over > 0).sum()), "vertices")
    r3 = np.minimum(r2, lim)
    k = np.where(r > 1e-6, r3 / np.maximum(r, 1e-6), 1.0)
    co[:, 0] = x * k
    co[:, 2] = z * k
    me.vertices.foreach_set("co", co.ravel())
    me.update()
    r_f = np.hypot(co[:, 0], co[:, 2])
    print("DRESSING radius range", round(float(r_f.min()), 3), round(float(r_f.max()), 3), "s range", round(float(s.min()), 3), round(float(s.max()), 3))
    print("DRESSING min clearance to the door skin", round(float((door_limit(np.arctan2(co[:, 0], co[:, 2]), np.clip(s, 0.3, 2.4)) - r_f).min() + CLEAR), 4), "m")


def main():
    args = sys.argv[sys.argv.index("--") + 1:]
    out_obj = args[0]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    build()
    bpy.ops.object.select_all(action="SELECT")
    for o in list(bpy.context.selected_objects):
        if o.type == "CURVE":
            bpy.context.view_layer.objects.active = o
    bpy.ops.object.convert(target="MESH")
    fcd.unwrap_parts()
    bpy.ops.object.select_all(action="SELECT")
    bpy.context.view_layer.objects.active = bpy.context.selected_objects[0]
    bpy.ops.object.join()
    joined = bpy.context.view_layer.objects.active
    joined.name = "fan_case_gp"
    for mod in list(joined.modifiers):
        bpy.ops.object.modifier_apply(modifier=mod.name)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    conform(joined)
    print("DRESSING gp faces", len(joined.data.polygons))
    bpy.ops.wm.obj_export(filepath=out_obj, export_selected_objects=True, export_uv=True, export_normals=True, export_materials=True, forward_axis="Y", up_axis="Z")
    if len(args) > 1:
        bpy.ops.wm.save_as_mainfile(filepath=args[1])


if __name__ == "__main__":
    main()
