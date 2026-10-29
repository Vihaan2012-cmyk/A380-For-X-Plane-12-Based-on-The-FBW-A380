"""Blender script: the equipment on a turbofan's fan case, as seen with the
fan cowl doors off (Trent 900 and GP7200 layouts).

    blender -b --factory-startup -P fan_case_dressing.py -- <trent|gp> <out.obj> [<out.blend>]

Engine-local axes: X starboard, Z up, Y forward from the spinner tip (the
fan case runs from Y -0.5 to -3.0). Clock angle 0 = top, 90 = starboard.
Every part stays between the fan case (r 1.55) and the closed cowl (r 1.80).
Each part carries a named material the texture bake reads."""

import math
import random
import sys

import bpy
import bmesh
from mathutils import Matrix, Vector

CASE_R = 1.555
ENVELOPE_R = 1.80


def clock(theta, y, r):
    a = math.radians(theta)
    return Vector((r * math.sin(a), y, r * math.cos(a)))


def frame_at(theta, y, r):
    """Local frame on the case: x along the circumference, y along the axis, z out."""
    a = math.radians(theta)
    out = Vector((math.sin(a), 0, math.cos(a)))
    tangent = Vector((math.cos(a), 0, -math.sin(a)))
    m = Matrix.Identity(4)
    m.col[0][:3] = tangent
    m.col[1][:3] = Vector((0, 1, 0))
    m.col[2][:3] = out
    m.col[3][:3] = clock(theta, y, r)
    return m


MATERIALS = {
    "stainless": (0.62, 0.62, 0.60, 1.0, 0.30),
    "titanium": (0.45, 0.43, 0.40, 1.0, 0.40),
    "casting": (0.36, 0.37, 0.35, 0.6, 0.55),
    "case": (0.30, 0.31, 0.30, 0.5, 0.55),
    "firesleeve": (0.62, 0.20, 0.10, 0.0, 0.80),
    "clamp": (0.70, 0.30, 0.12, 0.2, 0.55),
    "yellow": (0.80, 0.62, 0.08, 0.0, 0.45),
    "blue": (0.10, 0.26, 0.52, 0.6, 0.35),
    "harness": (0.08, 0.08, 0.08, 0.0, 0.75),
    "connector": (0.55, 0.47, 0.30, 1.0, 0.35),
    "rubber": (0.04, 0.04, 0.04, 0.0, 0.85),
    "label": (0.85, 0.85, 0.82, 0.0, 0.6),
    "aluminium": (0.72, 0.73, 0.74, 1.0, 0.32),
    "primer": (0.42, 0.47, 0.36, 0.0, 0.6),
}


def material(name):
    m = bpy.data.materials.get(name)
    if m:
        return m
    r, g, b, metal, rough = MATERIALS[name]
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = (r, g, b, 1)
    bsdf.inputs["Metallic"].default_value = metal
    bsdf.inputs["Roughness"].default_value = rough
    return m


def finish(obj, mat, bevel=0.0, smooth=True):
    obj.data.materials.clear()
    obj.data.materials.append(material(mat))
    if bevel > 0:
        b = obj.modifiers.new("bevel", "BEVEL")
        b.width = bevel
        b.segments = 2
        b.limit_method = "ANGLE"
    if smooth:
        for p in obj.data.polygons:
            p.use_smooth = True
        try:
            obj.data.set_sharp_from_angle(angle=math.radians(40))
        except AttributeError:
            pass
    return obj


def mesh_object(name, bm):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(obj)
    return obj


def new_bm():
    bm = bmesh.new()
    bm.loops.layers.uv.new("UVMap")
    return bm


def box(name, size, matrix, mat, bevel=0.006):
    bm = new_bm()
    bmesh.ops.create_cube(bm, size=1.0, calc_uvs=True)
    bmesh.ops.scale(bm, vec=Vector(size), verts=bm.verts)
    bmesh.ops.transform(bm, matrix=matrix, verts=bm.verts)
    return finish(mesh_object(name, bm), mat, bevel)


def cylinder(name, radius, depth, matrix, mat, segments=20, bevel=0.004):
    bm = new_bm()
    bmesh.ops.create_cone(bm, cap_ends=True, segments=segments, radius1=radius, radius2=radius, depth=depth, calc_uvs=True)
    bmesh.ops.transform(bm, matrix=matrix, verts=bm.verts)
    return finish(mesh_object(name, bm), mat, bevel)


def tube(name, points, radius, mat, resolution=4, bevel_resolution=None):
    cu = bpy.data.curves.new(name, "CURVE")
    cu.dimensions = "3D"
    cu.bevel_depth = radius
    cu.bevel_resolution = bevel_resolution if bevel_resolution is not None else (1 if radius < 0.008 else 2)
    cu.resolution_u = resolution
    cu.use_fill_caps = True
    sp = cu.splines.new("BEZIER")
    sp.bezier_points.add(len(points) - 1)
    for bp, p in zip(sp.bezier_points, points):
        bp.co = p
        bp.handle_left_type = "AUTO"
        bp.handle_right_type = "AUTO"
    obj = bpy.data.objects.new(name, cu)
    bpy.context.collection.objects.link(obj)
    obj.data.materials.append(material(mat))
    return obj


def arc_points(theta0, theta1, y, r, step=7.0, wobble=0.0, rng=None):
    n = max(2, int(abs(theta1 - theta0) / step) + 1)
    pts = []
    for k in range(n):
        th = theta0 + (theta1 - theta0) * k / (n - 1)
        dr = (rng.uniform(-wobble, wobble) if rng and 0 < k < n - 1 else 0.0)
        pts.append(clock(th, y, r + dr))
    return pts


def along(direction, at):
    """A matrix at `at` whose Z axis runs along `direction`."""
    rot = Vector((0, 0, 1)).rotation_difference(direction.normalized()).to_matrix().to_4x4()
    return Matrix.Translation(at) @ rot


def clamp_at(name, at, direction, pipe_r, mat="clamp"):
    theta = math.degrees(math.atan2(at.x, at.z))
    r = (at.x ** 2 + at.z ** 2) ** 0.5
    band = cylinder(name, pipe_r + 0.005, 0.016, along(direction, at), mat, segments=12, bevel=0.002)
    foot = box(name + "_foot", (0.026, 0.02, max(0.01, r - CASE_R - pipe_r)), frame_at(theta, at.y, CASE_R + (r - CASE_R - pipe_r) / 2), "titanium", bevel=0.002)
    return [band, foot]


def fitting(name, at, direction, pipe_r):
    return cylinder(name, pipe_r * 1.7, 0.028, along(direction, at), "stainless", segments=6, bevel=0.002)


def flange(name, y, r_in, r_out, width, bolts, rng):
    bm = new_bm()
    bmesh.ops.create_cone(bm, cap_ends=False, segments=144, radius1=r_out, radius2=r_out, depth=width, calc_uvs=True)
    ring = mesh_object(name, bm)
    ring.matrix_world = Matrix.Translation((0, y, 0)) @ Matrix.Rotation(math.radians(90), 4, "X")
    finish(ring, "case", 0.004)
    parts = [ring]
    for k in range(bolts):
        th = 360.0 * k / bolts + rng.uniform(-0.15, 0.15)
        if abs(((th + 180) % 360) - 180) < 9:
            continue
        m = frame_at(th, y, r_out) @ Matrix.Translation((0, 0, 0.006))
        parts.append(cylinder(f"{name}_bolt{k}", 0.0085, 0.012, m, "stainless", segments=6, bevel=0.0))
    return parts


def ribbed_rear_case(y0, y1, pitch, r):
    parts = []
    y = y0
    k = 0
    while y > y1:
        bm = new_bm()
        bmesh.ops.create_cone(bm, cap_ends=False, segments=144, radius1=r + 0.012, radius2=r + 0.012, depth=0.018, calc_uvs=True)
        o = mesh_object(f"rib{k}", bm)
        o.matrix_world = Matrix.Translation((0, y, 0)) @ Matrix.Rotation(math.radians(90), 4, "X")
        parts.append(finish(o, "case", 0.004))
        y -= pitch
        k += 1
    return parts


def gearbox(layout, rng):
    """The accessory gearbox: a cast housing swept round the case bottom,
    with its drives."""
    th0, th1, y0, y1 = layout["agb"]
    parts = []
    segs = 18
    bm = bmesh.new()
    rings = []
    for k in range(segs + 1):
        th = th0 + (th1 - th0) * k / segs
        m = frame_at(th, 0, 0)
        ring = []
        for (dx, dy) in ((0, y0), (0, y1)):
            pass
        corners = [(y0, 0.04), (y1, 0.04), (y1, 0.17), (y0 + 0.04, 0.20), (y0, 0.16)]
        for (yy, dz) in corners:
            ring.append(bm.verts.new(clock(th, yy, CASE_R + dz)))
        rings.append(ring)
    for a, b in zip(rings, rings[1:]):
        for i in range(len(a)):
            j = (i + 1) % len(a)
            bm.faces.new((a[i], a[j], b[j], b[i]))
    bm.faces.new(rings[0][::-1])
    bm.faces.new(rings[-1])
    housing = mesh_object("agb_housing", bm)
    finish(housing, "casting", 0.012)
    parts.append(housing)
    for k in range(5):
        th = th0 + (th1 - th0) * (k + 0.5) / 5
        parts.append(box(f"agb_rib{k}", (0.012, abs(y1 - y0) * 0.9, 0.05), frame_at(th, (y0 + y1) / 2, CASE_R + 0.2), "casting", 0.003))
    for name, (th, y, radius, length, mat, fins) in layout["drives"].items():
        m = frame_at(th, y, CASE_R + 0.11) @ Matrix.Rotation(math.radians(90), 4, "X")
        body = cylinder(f"agb_{name}", radius, length, m, mat, segments=24, bevel=0.006)
        parts.append(body)
        flange_m = frame_at(th, y + length / 2 * (1 if y < y0 else -1) * 0.0, CASE_R + 0.11) @ Matrix.Rotation(math.radians(90), 4, "X") @ Matrix.Translation((0, 0, length / 2))
        parts.append(cylinder(f"agb_{name}_flange", radius * 1.12, 0.018, flange_m, "casting", segments=24, bevel=0.003))
        for f in range(fins):
            fm = frame_at(th, y, CASE_R + 0.11) @ Matrix.Rotation(math.radians(90), 4, "X") @ Matrix.Translation((0, 0, -length / 2 + (f + 1) * length / (fins + 1)))
            parts.append(cylinder(f"agb_{name}_fin{f}", radius * 1.06, 0.008, fm, mat, segments=24, bevel=0.0))
        for b in range(8):
            bt = th + math.degrees((radius * 1.05) / (CASE_R + 0.11)) * math.cos(b * math.pi / 4)
            parts.append(cylinder(f"agb_{name}_bolt{b}", 0.006, 0.02, frame_at(bt, y + length / 2, CASE_R + 0.11 + radius * 1.05 * math.sin(b * math.pi / 4)) @ Matrix.Rotation(math.radians(90), 4, "X"), "stainless", segments=6, bevel=0.0))
    th, y = layout["oil_filter"]
    m = frame_at(th, y, CASE_R + 0.12)
    parts.append(cylinder("oil_filter", 0.05, 0.18, m @ Matrix.Rotation(math.radians(90), 4, "X"), "yellow", 24, 0.008))
    parts.append(cylinder("oil_filter_cap", 0.055, 0.03, m @ Matrix.Rotation(math.radians(90), 4, "X") @ Matrix.Translation((0, 0, 0.1)), "casting", 24, 0.004))
    th, y = layout["fuel_filters"]
    bracket = box("fuel_bracket", (0.30, 0.05, 0.02), frame_at(th, y, CASE_R + 0.06), "blue", 0.004)
    parts.append(bracket)
    for k, dx in enumerate((-0.07, 0.07)):
        m = frame_at(th, y, CASE_R + 0.06) @ Matrix.Translation((dx, -0.05, 0.06))
        parts.append(cylinder(f"fuel_filter{k}", 0.035, 0.16, m @ Matrix.Rotation(math.radians(90), 4, "X"), "stainless", 20, 0.006))
        parts.append(cylinder(f"fuel_filter_head{k}", 0.042, 0.035, m @ Matrix.Rotation(math.radians(90), 4, "X") @ Matrix.Translation((0, 0, 0.08)), "casting", 20, 0.004))
    return parts


def oil_tank(layout):
    th, y, length = layout["oil_tank"]
    m = frame_at(th, y, CASE_R + 0.12) @ Matrix.Rotation(math.radians(90), 4, "X")
    bm = new_bm()
    bmesh.ops.create_uvsphere(bm, u_segments=28, v_segments=16, radius=1.0, calc_uvs=True)
    bmesh.ops.scale(bm, vec=Vector((0.11, 0.10, length / 2)), verts=bm.verts)
    for v in bm.verts:
        if abs(v.co.z) < length / 2 * 0.85:
            v.co.z *= 1.0
    bmesh.ops.transform(bm, matrix=m, verts=bm.verts)
    tank = finish(mesh_object("oil_tank", bm), "stainless", 0.0)
    parts = [tank]
    parts.append(cylinder("oil_tank_cap", 0.03, 0.04, frame_at(th, y + length * 0.3, CASE_R + 0.235), "casting", 16, 0.004))
    parts.append(box("oil_tank_sight", (0.05, 0.09, 0.01), frame_at(th + 4.2, y, CASE_R + 0.215), "label", 0.002))
    for k in (-1, 1):
        parts.append(box(f"oil_tank_strap{k}", (0.26, 0.025, 0.006), frame_at(th, y + k * length * 0.28, CASE_R + 0.12), "titanium", 0.0))
    return parts


def eec(layout):
    th, y = layout["eec"]
    parts = [box("eec", (0.36, 0.26, 0.11), frame_at(th, y, CASE_R + 0.085), "casting", 0.008)]
    for k in range(9):
        parts.append(box(f"eec_fin{k}", (0.34, 0.006, 0.02), frame_at(th, y - 0.11 + k * 0.0275, CASE_R + 0.15), "casting", 0.0))
    for k, dx in enumerate((-0.12, -0.04, 0.04, 0.12)):
        m = frame_at(th, y + 0.13, CASE_R + 0.085) @ Matrix.Translation((dx, 0.02, 0)) @ Matrix.Rotation(math.radians(90), 4, "X")
        parts.append(cylinder(f"eec_plug{k}", 0.022, 0.05, m, "connector", 16, 0.003))
    return parts


def exciters(layout):
    parts = []
    for k, (th, y) in enumerate(layout["exciters"]):
        parts.append(box(f"exciter{k}", (0.16, 0.12, 0.08), frame_at(th, y, CASE_R + 0.06), "casting", 0.006))
        parts.append(box(f"exciter{k}_plate", (0.07, 0.04, 0.003), frame_at(th, y, CASE_R + 0.101), "label", 0.0))
    return parts


def pipe_runs(layout, rng):
    parts = []
    for k, run in enumerate(layout["pipes"]):
        mat, radius, r = run["mat"], run["radius"], run["r"]
        pts = []
        for seg in run["path"]:
            if seg[0] == "arc":
                _, a0, a1, y = seg
                pts += arc_points(a0, a1, y, r, wobble=0.004, rng=rng)
            else:
                _, th, y0, y1 = seg
                n = max(2, int(abs(y1 - y0) / 0.12) + 1)
                pts += [clock(th, y0 + (y1 - y0) * i / (n - 1), r) for i in range(n)]
        clean = [pts[0]]
        for p in pts[1:]:
            if (p - clean[-1]).length > 0.02:
                clean.append(p)
        parts.append(tube(f"pipe{k}", clean, radius, mat))
        parts.append(fitting(f"pipe{k}_end0", clean[0], clean[1] - clean[0], radius))
        parts.append(fitting(f"pipe{k}_end1", clean[-1], clean[-1] - clean[-2], radius))
        for i in range(1, len(clean) - 1):
            a, b = clean[i] - clean[i - 1], clean[i + 1] - clean[i]
            if a.angle(b, 0) > math.radians(35):
                parts.append(fitting(f"pipe{k}_bend{i}", clean[i], b, radius))
        spacing = run.get("clamps", 0.3)
        travelled = rng.uniform(0.05, spacing)
        for i in range(1, len(clean)):
            a, b = clean[i - 1], clean[i]
            seg_len = (b - a).length
            while travelled < seg_len:
                p = a + (b - a) * (travelled / seg_len)
                parts += clamp_at(f"pipe{k}_clamp{i}_{int(travelled * 1000)}", p, b - a, radius)
                travelled += spacing * rng.uniform(0.75, 1.3)
            travelled -= seg_len
    return parts


def harnesses(layout, rng):
    parts = []
    for k, run in enumerate(layout["harness"]):
        r, radius = run["r"], run["radius"]
        pts = []
        for (a0, a1, y) in run["arcs"]:
            pts += arc_points(a0, a1, y, r, step=6.0, wobble=0.006, rng=rng)
        parts.append(tube(f"harness{k}", pts, radius, "harness"))
        for i in range(2, len(pts) - 1, 3):
            parts += clamp_at(f"harness{k}_clamp{i}", pts[i], pts[i + 1] - pts[i - 1], radius, mat="rubber")
    return parts


def tube_banks(layout, rng):
    """Rows of parallel tubes held in shared clamp blocks, the way the
    lube, fuel and air lines run round a real fan case."""
    parts = []
    for k, bank in enumerate(layout["banks"]):
        a0, a1, y, count, r0, radius = bank
        for t in range(count):
            r = r0 + t * (radius * 2 + 0.012)
            pts = arc_points(a0 + t * 1.5, a1 - t * 1.5, y - t * 0.004, r, step=6.0)
            parts.append(tube(f"bank{k}_{t}", pts, radius, ("stainless", "titanium", "stainless")[t % 3]))
            parts.append(fitting(f"bank{k}_{t}_end0", pts[0], pts[1] - pts[0], radius))
            parts.append(fitting(f"bank{k}_{t}_end1", pts[-1], pts[-1] - pts[-2], radius))
        th = a0 + rng.uniform(4, 9)
        while th < a1 - 4:
            mid = r0 + (count - 1) * (radius + 0.006)
            block_h = count * (radius * 2 + 0.012) + 0.012
            parts.append(box(f"bank{k}_block{int(th)}", (0.02, 0.03, block_h), frame_at(th, y, mid), "clamp", 0.003))
            parts.append(box(f"bank{k}_post{int(th)}", (0.016, 0.02, r0 - CASE_R), frame_at(th, y, CASE_R + (r0 - CASE_R) / 2), "titanium", 0.002))
            th += rng.uniform(10, 16)
    return parts


def drops(layout, rng):
    """Lines leaving the circumferential runs and turning down the case to
    the gearbox, each with its own bends and a fitting where it lands."""
    parts = []
    th0, th1, gy0, gy1 = layout["agb"]
    for k, (theta, y_from, y_to, r, radius, mat) in enumerate(layout["drops"]):
        land = th0 + 4 if theta < 180 else th1 - 4
        pts = arc_points(theta, (theta + land) / 2, y_from, r, step=5.0)
        pts += [clock((theta + land) / 2, y_from + (y_to - y_from) * f, r) for f in (0.35, 0.7)]
        pts += arc_points((theta + land) / 2, land, y_to, r, step=5.0)[1:]
        parts.append(tube(f"drop{k}", pts, radius, mat))
        parts.append(fitting(f"drop{k}_land", pts[-1], pts[-1] - pts[-2], radius))
        for i in range(2, len(pts) - 1, 3):
            parts += clamp_at(f"drop{k}_clamp{i}", pts[i], pts[i + 1] - pts[i - 1], radius)
    return parts


def sensors(layout, rng):
    parts = []
    for k, (theta, y) in enumerate(layout["sensors"]):
        m = frame_at(theta, y, CASE_R + 0.035)
        parts.append(cylinder(f"sensor{k}", 0.022, 0.07, m, "stainless", 16, 0.003))
        parts.append(cylinder(f"sensor{k}_plug", 0.017, 0.035, m @ Matrix.Translation((0, 0, 0.05)), "connector", 16, 0.002))
        parts.append(box(f"sensor{k}_boss", (0.06, 0.06, 0.012), frame_at(theta, y, CASE_R + 0.006), "casting", 0.003))
    for k, (theta, y) in enumerate(layout["junctions"]):
        parts.append(box(f"junction{k}", (0.14, 0.1, 0.06), frame_at(theta, y, CASE_R + 0.035), "casting", 0.006))
        for j in range(3):
            m = frame_at(theta, y + 0.055, CASE_R + 0.035) @ Matrix.Translation((-0.04 + j * 0.04, 0, 0)) @ Matrix.Rotation(math.radians(90), 4, "X")
            parts.append(cylinder(f"junction{k}_plug{j}", 0.014, 0.03, m, "connector", 12, 0.002))
    return parts


def case_skin(layout):
    y0, y1 = layout["skin"]
    bm = new_bm()
    bmesh.ops.create_cone(bm, cap_ends=False, segments=160, radius1=CASE_R + 0.0015, radius2=CASE_R + 0.0015, depth=abs(y1 - y0), calc_uvs=True)
    o = mesh_object("case_skin", bm)
    o.matrix_world = Matrix.Translation((0, (y0 + y1) / 2, 0)) @ Matrix.Rotation(math.radians(90), 4, "X")
    return [finish(o, "case", 0.0)]


def fire_loops(layout):
    parts = []
    for k, y in enumerate(layout["fire_loops"]):
        pts = arc_points(15, 345, y, CASE_R + 0.035, step=8)
        parts.append(tube(f"fire_loop{k}", pts, 0.005, "stainless"))
    return parts


def keepouts(layout):
    """Clock/axial boxes of the big equipment that routed lines stop at."""
    th0, th1, y0, y1 = layout["agb"]
    boxes = [(th0 - 3, th1 + 3, y0 + 0.05, y1 - 0.05)]
    th, y, length = layout["oil_tank"]
    boxes.append((th - 6, th + 6, y + length / 2 + 0.04, y - length / 2 - 0.04))
    th, y = layout["eec"]
    boxes.append((th - 8, th + 8, y + 0.16, y - 0.16))
    for th, y in layout["exciters"]:
        boxes.append((th - 4, th + 4, y + 0.08, y - 0.08))
    th, y = layout["fuel_filters"]
    boxes.append((th - 7, th + 7, y + 0.1, y - 0.12))
    boxes.append((345, 375, -0.4, -3.1))
    boxes.append((-15, 15, -0.4, -3.1))
    return boxes


def blocked(theta, y, boxes):
    t = theta % 360
    return any((a <= t <= b or a <= t + 360 <= b or a <= t - 360 <= b) and y1 <= y <= y0 for a, b, y0, y1 in boxes)


Y_FRONT, Y_REAR = -0.62, -2.38


def route(rng, boxes, max_len=4.0):
    """A plausible line run on the case: arcs round it joined by short
    axial jogs, stopping at equipment or the case ends."""
    for _ in range(40):
        theta = rng.uniform(20, 340)
        y = rng.uniform(Y_REAR, Y_FRONT)
        if not blocked(theta, y, boxes):
            break
    pts = [(theta, y)]
    travelled, axial_next = 0.0, rng.random() < 0.4
    while travelled < max_len:
        if axial_next:
            target = y + rng.choice((-1, 1)) * rng.uniform(0.08, 0.5)
            target = min(Y_FRONT, max(Y_REAR, target))
            n = max(2, int(abs(target - y) / 0.05))
            steps = [(theta, y + (target - y) * k / n) for k in range(1, n + 1)]
        else:
            span = rng.choice((-1, 1)) * rng.uniform(12, 85)
            n = max(2, int(abs(span) / 4))
            steps = [(theta + span * k / n, y) for k in range(1, n + 1)]
        hit = False
        for th, yy in steps:
            if blocked(th, yy, boxes) or not (Y_REAR - 0.001 <= yy <= Y_FRONT + 0.001):
                hit = True
                break
            prev = pts[-1]
            travelled += math.hypot(math.radians(th - prev[0]) * 1.6, yy - prev[1])
            pts.append((th, yy))
        theta, y = pts[-1]
        if hit or rng.random() < 0.12:
            break
        axial_next = not axial_next
    return pts if travelled > 0.35 else None


def routed_lines(layout, rng):
    boxes = keepouts(layout)
    parts = []
    mats = ["stainless"] * 5 + ["titanium"] * 3 + ["firesleeve"] * 2 + ["aluminium"] * 2
    for k in range(layout.get("routes", 48)):
        path = route(rng, boxes)
        if not path:
            continue
        radius = rng.choice((0.005, 0.006, 0.008, 0.01, 0.012, 0.014, 0.018))
        r = rng.uniform(1.575, 1.775 - radius)
        mat = rng.choice(mats)
        pts = [clock(th, y, r) for th, y in path]
        clean = [pts[0]]
        for p in pts[1:]:
            if (p - clean[-1]).length > 0.025:
                clean.append(p)
        if len(clean) < 3:
            continue
        parts.append(tube(f"route{k}", clean, radius, mat))
        parts.append(fitting(f"route{k}_a", clean[0], clean[1] - clean[0], radius))
        parts.append(fitting(f"route{k}_b", clean[-1], clean[-1] - clean[-2], radius))
        step = rng.randint(4, 7)
        for i in range(step // 2, len(clean) - 1, step):
            parts += clamp_at(f"route{k}_clamp{i}", clean[i], clean[i + 1] - clean[i - 1], radius)
    return parts


def looms(layout, rng):
    """Wiring looms: several cables run together under cushion clamps,
    breaking out to connectors at the ends."""
    boxes = keepouts(layout)
    parts = []
    for k in range(layout.get("looms", 14)):
        path = route(rng, boxes, max_len=3.0)
        if not path:
            continue
        r = rng.uniform(1.585, 1.72)
        count = rng.randint(3, 6)
        centre = [clock(th, y, r) for th, y in path]
        centre = [centre[0]] + [p for a, p in zip(centre, centre[1:]) if (p - a).length > 0.02]
        if len(centre) < 3:
            continue
        for c in range(count):
            ang = 2 * math.pi * c / count + rng.uniform(-0.3, 0.3)
            rad = rng.choice((0.004, 0.005, 0.006, 0.008))
            offs = []
            for i, p in enumerate(centre):
                d = (centre[min(i + 1, len(centre) - 1)] - centre[max(i - 1, 0)]).normalized()
                out = Vector((p.x, 0, p.z)).normalized()
                side = d.cross(out).normalized()
                offs.append(p + (out * math.cos(ang) + side * math.sin(ang)) * 0.012)
            parts.append(tube(f"loom{k}_{c}", offs, rad, "harness" if c % 3 else "rubber"))
        for i in range(2, len(centre) - 1, 4):
            parts += clamp_at(f"loom{k}_clamp{i}", centre[i], centre[i + 1] - centre[i - 1], 0.018, mat="rubber")
        for end, nxt in ((centre[0], centre[1]), (centre[-1], centre[-2])):
            parts.append(cylinder(f"loom{k}_shell{int(end.y * 1000)}", 0.02, 0.05, along(end - nxt, end), "connector", 14, 0.003))
    return parts


def gadgets(layout, rng):
    """The small equipment that crowds a real fan case: valves, solenoids,
    pressure transducers, manifold blocks and mounting brackets."""
    boxes = keepouts(layout)
    parts = []
    placed = 0
    tries = 0
    while placed < layout.get("gadgets", 50) and tries < 600:
        tries += 1
        th, y = rng.uniform(22, 338), rng.uniform(Y_REAR, Y_FRONT)
        if blocked(th, y, boxes):
            continue
        boxes.append((th - 3, th + 3, y + 0.05, y - 0.05))
        kind = rng.choice(("valve", "valve", "solenoid", "transducer", "manifold", "bracket"))
        base = frame_at(th, y, CASE_R)
        name = f"gadget{placed}_{kind}"
        parts.append(box(name + "_pad", (0.07, 0.07, 0.01), base @ Matrix.Translation((0, 0, 0.006)), "primer", 0.002))
        if kind == "valve":
            parts.append(box(name, (0.075, 0.06, 0.05), base @ Matrix.Translation((0, 0, 0.035)), "casting", 0.006))
            parts.append(cylinder(name + "_act", 0.024, 0.05, base @ Matrix.Translation((0, 0, 0.08)), "aluminium", 16, 0.003))
            for s in (-1, 1):
                parts.append(cylinder(name + f"_port{s}", 0.012, 0.03, base @ Matrix.Translation((s * 0.05, 0, 0.035)) @ Matrix.Rotation(math.radians(90), 4, "Y"), "stainless", 6, 0.0))
        elif kind == "solenoid":
            parts.append(box(name, (0.05, 0.05, 0.04), base @ Matrix.Translation((0, 0, 0.03)), "casting", 0.004))
            parts.append(cylinder(name + "_coil", 0.022, 0.06, base @ Matrix.Translation((0, 0, 0.075)), "rubber", 16, 0.003))
            parts.append(cylinder(name + "_plug", 0.012, 0.025, base @ Matrix.Translation((0, 0, 0.115)), "connector", 12, 0.002))
        elif kind == "transducer":
            parts.append(cylinder(name, 0.018, 0.06, base @ Matrix.Translation((0, 0, 0.04)), "stainless", 14, 0.003))
            parts.append(cylinder(name + "_hex", 0.022, 0.015, base @ Matrix.Translation((0, 0, 0.015)), "stainless", 6, 0.0))
            parts.append(cylinder(name + "_plug", 0.014, 0.03, base @ Matrix.Translation((0, 0, 0.085)), "connector", 12, 0.002))
        elif kind == "manifold":
            parts.append(box(name, (0.16, 0.05, 0.04), base @ Matrix.Translation((0, 0, 0.03)), "aluminium", 0.004))
            for j in range(4):
                parts.append(cylinder(name + f"_fit{j}", 0.01, 0.03, base @ Matrix.Translation((-0.06 + j * 0.04, 0, 0.06)), "stainless", 6, 0.0))
        else:
            parts.append(box(name, (0.12, 0.016, 0.07), base @ Matrix.Translation((0, 0, 0.04)), "primer", 0.003))
            parts.append(box(name + "_web", (0.012, 0.06, 0.06), base @ Matrix.Translation((0, 0, 0.035)), "primer", 0.002))
        placed += 1
    return parts


LAYOUTS = {
    "trent": {
        "agb": (152, 212, -1.05, -1.75),
        "drives": {
            "vfg": (204, -1.40, 0.125, 0.30, "casting", 7),
            "starter": (160, -1.35, 0.085, 0.22, "titanium", 0),
            "edp_a": (184, -1.22, 0.065, 0.17, "casting", 3),
            "edp_b": (192, -1.55, 0.065, 0.17, "casting", 3),
            "fmu": (174, -1.60, 0.07, 0.14, "titanium", 0),
        },
        "oil_filter": (143, -1.45),
        "fuel_filters": (126, -1.35),
        "oil_tank": (252, -1.25, 0.55),
        "eec": (62, -1.45),
        "exciters": [(292, -1.05), (300, -1.75)],
        "pipes": [
            {"mat": "stainless", "radius": 0.016, "r": 1.62, "path": [("arc", 20, 140, -0.72), ("axial", 140, -0.72, -1.25)], "clamps": 0.28},
            {"mat": "stainless", "radius": 0.012, "r": 1.60, "path": [("arc", 25, 150, -0.88)], "clamps": 0.26},
            {"mat": "titanium", "radius": 0.02, "r": 1.66, "path": [("axial", 110, -0.6, -2.3), ("arc", 110, 165, -2.3)], "clamps": 0.32},
            {"mat": "firesleeve", "radius": 0.018, "r": 1.64, "path": [("arc", 215, 330, -1.95), ("axial", 330, -1.95, -2.6)], "clamps": 0.3},
            {"mat": "stainless", "radius": 0.010, "r": 1.59, "path": [("arc", 200, 335, -0.78)], "clamps": 0.24},
            {"mat": "stainless", "radius": 0.014, "r": 1.63, "path": [("axial", 230, -0.7, -2.2), ("arc", 230, 268, -2.2)], "clamps": 0.3},
            {"mat": "firesleeve", "radius": 0.022, "r": 1.68, "path": [("arc", 120, 205, -2.05)], "clamps": 0.25},
            {"mat": "titanium", "radius": 0.013, "r": 1.61, "path": [("axial", 75, -0.65, -2.4)], "clamps": 0.3},
            {"mat": "stainless", "radius": 0.009, "r": 1.585, "path": [("arc", 30, 100, -2.2), ("axial", 100, -2.2, -2.75)], "clamps": 0.22},
        ],
        "harness": [
            {"r": 1.615, "radius": 0.022, "arcs": [(30, 160, -1.88)]},
            {"r": 1.605, "radius": 0.016, "arcs": [(200, 330, -1.62)]},
            {"r": 1.62, "radius": 0.026, "arcs": [(50, 120, -0.98)]},
        ],
        "fire_loops": [-0.95, -2.15],
        "flanges": [(-0.56, 0.035, 150), (-1.50, 0.03, 140), (-2.45, 0.04, 160)],
        "ribs": (-2.5, -3.0, 0.055),
        "skin": (-0.52, -2.43),
        "banks": [(22, 148, -2.3, 4, 1.585, 0.009), (205, 340, -2.32, 3, 1.585, 0.01), (30, 100, -1.15, 3, 1.59, 0.008), (262, 330, -1.3, 4, 1.588, 0.008), (128, 150, -0.75, 3, 1.59, 0.011), (40, 135, -1.72, 3, 1.62, 0.008), (215, 300, -0.66, 3, 1.6, 0.009)],
        "drops": [(95, -2.2, -1.5, 1.6, 0.013, "stainless"), (118, -0.85, -1.3, 1.63, 0.011, "titanium"), (240, -2.25, -1.6, 1.61, 0.012, "stainless"), (226, -0.9, -1.25, 1.64, 0.016, "firesleeve"), (135, -2.0, -1.65, 1.6, 0.01, "stainless"), (250, -1.0, -1.4, 1.6, 0.009, "titanium")],
        "sensors": [(40, -0.68), (75, -2.0), (105, -0.95), (232, -2.05), (275, -0.7), (318, -1.4), (85, -1.62), (300, -2.2)],
        "junctions": [(28, -1.9), (330, -1.05), (98, -1.32)],
    },
    "gp": {
        "agb": (158, 218, -1.0, -1.7),
        "drives": {
            "idg": (210, -1.35, 0.13, 0.32, "casting", 8),
            "starter": (165, -1.30, 0.09, 0.24, "stainless", 0),
            "edp_a": (188, -1.18, 0.065, 0.18, "casting", 3),
            "edp_b": (196, -1.52, 0.065, 0.18, "casting", 3),
            "fuel_pump": (178, -1.58, 0.075, 0.16, "titanium", 0),
        },
        "oil_filter": (147, -1.38),
        "fuel_filters": (131, -1.30),
        "oil_tank": (276, -1.05, 0.5),
        "eec": (58, -1.35),
        "exciters": [(305, -1.15), (312, -1.8)],
        "pipes": [
            {"mat": "stainless", "radius": 0.015, "r": 1.62, "path": [("arc", 18, 150, -0.70), ("axial", 150, -0.7, -1.2)], "clamps": 0.27},
            {"mat": "firesleeve", "radius": 0.02, "r": 1.65, "path": [("arc", 220, 340, -1.85), ("axial", 340, -1.85, -2.5)], "clamps": 0.28},
            {"mat": "titanium", "radius": 0.018, "r": 1.66, "path": [("axial", 120, -0.6, -2.3)], "clamps": 0.3},
            {"mat": "stainless", "radius": 0.011, "r": 1.59, "path": [("arc", 205, 330, -0.82)], "clamps": 0.25},
            {"mat": "firesleeve", "radius": 0.024, "r": 1.69, "path": [("arc", 125, 210, -2.0)], "clamps": 0.26},
            {"mat": "stainless", "radius": 0.013, "r": 1.63, "path": [("axial", 245, -0.7, -2.25), ("arc", 245, 290, -2.25)], "clamps": 0.3},
            {"mat": "titanium", "radius": 0.012, "r": 1.605, "path": [("axial", 80, -0.65, -2.4)], "clamps": 0.29},
            {"mat": "stainless", "radius": 0.009, "r": 1.585, "path": [("arc", 25, 95, -2.25), ("axial", 95, -2.25, -2.8)], "clamps": 0.22},
        ],
        "harness": [
            {"r": 1.615, "radius": 0.024, "arcs": [(28, 155, -1.9)]},
            {"r": 1.605, "radius": 0.017, "arcs": [(205, 335, -1.6)]},
            {"r": 1.62, "radius": 0.022, "arcs": [(45, 115, -0.95)]},
        ],
        "fire_loops": [-0.92, -2.1],
        "flanges": [(-0.55, 0.035, 160), (-1.48, 0.03, 150), (-2.42, 0.04, 170)],
        "ribs": (-2.48, -3.0, 0.05),
        "skin": (-0.51, -2.40),
        "banks": [(20, 152, -2.28, 3, 1.585, 0.01), (210, 345, -2.3, 4, 1.585, 0.009), (35, 105, -1.1, 3, 1.59, 0.009), (290, 340, -1.25, 3, 1.588, 0.008), (125, 148, -0.72, 2, 1.59, 0.012), (45, 140, -1.75, 3, 1.62, 0.009), (220, 305, -0.64, 2, 1.6, 0.01)],
        "drops": [(100, -2.15, -1.45, 1.6, 0.013, "stainless"), (122, -0.8, -1.25, 1.63, 0.012, "firesleeve"), (245, -2.2, -1.55, 1.61, 0.012, "titanium"), (232, -0.85, -1.2, 1.64, 0.016, "firesleeve"), (140, -1.95, -1.6, 1.6, 0.01, "stainless")],
        "sensors": [(45, -0.66), (80, -1.95), (110, -0.9), (236, -2.0), (268, -0.72), (322, -1.45), (90, -1.6), (305, -2.15), (20, -2.25)],
        "junctions": [(32, -1.85), (335, -1.0), (102, -1.3)],
    },
}


def build(kind):
    rng = random.Random(1309 if kind == "trent" else 7270)
    layout = LAYOUTS[kind]
    parts = []
    for k, (y, width, bolts) in enumerate(layout["flanges"]):
        parts += flange(f"flange{k}", y, CASE_R, CASE_R + 0.022, width, bolts, rng)
    parts += ribbed_rear_case(*layout["ribs"], CASE_R + 0.012)
    parts += gearbox(layout, rng)
    parts += oil_tank(layout)
    parts += eec(layout)
    parts += exciters(layout)
    parts += pipe_runs(layout, rng)
    parts += harnesses(layout, rng)
    parts += fire_loops(layout)
    parts += tube_banks(layout, rng)
    parts += drops(layout, rng)
    parts += sensors(layout, rng)
    parts += routed_lines(layout, rng)
    parts += looms(layout, rng)
    parts += gadgets(layout, rng)
    parts += case_skin(layout)
    return parts


def true_proportions(obj):
    """Scale an object's UV axes so a texel covers the same distance along
    and across, whatever its generated unwrap stretched."""
    me = obj.data
    if not me.uv_layers:
        return False
    uv = me.uv_layers[0].data
    along, across = [], []
    mw = obj.matrix_world
    for poly in me.polygons:
        loops = list(poly.loop_indices)
        for a, b in zip(loops, loops[1:] + loops[:1]):
            du = abs(uv[b].uv[0] - uv[a].uv[0])
            dv = abs(uv[b].uv[1] - uv[a].uv[1])
            d3 = ((mw @ me.vertices[me.loops[b].vertex_index].co) - (mw @ me.vertices[me.loops[a].vertex_index].co)).length
            if du > 4 * dv and du > 1e-6:
                along.append(d3 / du)
            elif dv > 4 * du and dv > 1e-6:
                across.append(d3 / dv)
    if not along or not across:
        return True
    su, sv = sum(along) / len(along), sum(across) / len(across)
    for d in uv:
        d.uv = (d.uv[0] * su / max(su, sv), d.uv[1] * sv / max(su, sv))
    return True


def unwrap_parts():
    for o in list(bpy.data.objects):
        if o.type != "MESH":
            continue
        if not true_proportions(o):
            o.data.uv_layers.new(name="UVMap")
            bpy.ops.object.select_all(action="DESELECT")
            bpy.context.view_layer.objects.active = o
            o.select_set(True)
            bpy.ops.object.mode_set(mode="EDIT")
            bpy.ops.mesh.select_all(action="SELECT")
            bpy.ops.uv.smart_project(angle_limit=1.15, island_margin=0.002)
            bpy.ops.object.mode_set(mode="OBJECT")


def main():
    args = sys.argv[sys.argv.index("--") + 1:]
    kind, out_obj = args[0], args[1]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    build(kind)
    bpy.ops.object.select_all(action="SELECT")
    for o in list(bpy.context.selected_objects):
        if o.type == "CURVE":
            bpy.context.view_layer.objects.active = o
    bpy.ops.object.convert(target="MESH")
    unwrap_parts()
    bpy.ops.object.select_all(action="SELECT")
    bpy.context.view_layer.objects.active = bpy.context.selected_objects[0]
    bpy.ops.object.join()
    joined = bpy.context.view_layer.objects.active
    joined.name = f"fan_case_{kind}"
    for mod in list(joined.modifiers):
        bpy.ops.object.modifier_apply(modifier=mod.name)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    out_r = max((v.co.x ** 2 + v.co.z ** 2) ** 0.5 for v in joined.data.vertices)
    print("DRESSING", kind, "faces", len(joined.data.polygons), "max radius", round(out_r, 3))
    bpy.ops.wm.obj_export(filepath=out_obj, export_selected_objects=True, export_uv=True, export_normals=True, export_materials=True, forward_axis="Y", up_axis="Z")
    if len(args) > 2:
        bpy.ops.wm.save_as_mainfile(filepath=args[2])


if __name__ == "__main__":
    main()
