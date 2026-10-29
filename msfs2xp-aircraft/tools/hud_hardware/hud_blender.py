"""Blender script: the captain's HUD hardware, modelled and baked.

    blender -b --factory-startup -P hud_blender.py -- <out dir> [size]

Builds, in X-Plane aircraft coordinates (Blender X = x, Y = -z, Z = y), the overhead projector fairing on
the ceiling: a panel filling FlyByWire's projector recess, its face draped flush with the lining sampled
by ceiling_grid.py (<out dir>/ceiling.json), with an inset seam, countersunk screws, a vent grille and a deep
lens bore with a bezel and an inner ring, the hinge housing under the ceiling (pin caps, access slot), and the combiner: a T-shaped
arm with filleted shoulders into the top bar (parting line, screws), the bar's inboard end running down
into a ribbed rubber grip, a dark rim round a rounded glass. Materials carry grain, worn edges and cavity
grime; colour, ambient occlusion, roughness and a tangent normal map are baked into one atlas.

Writes hud_unit.obj (materials fairing, lens, bezel, housing, arm, grip, screw, rim, glass; glass on its
own UV patch) and col.png, ao.png, rough.png, nml.png for place_hud.py. The first officer's side is the
mirror image, made at placement."""

import json
import math
import os
import sys

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector

ARGS = sys.argv[sys.argv.index("--") + 1:]
OUT = ARGS[0]
SIZE = int(ARGS[1]) if len(ARGS) > 1 else 2048

X0 = -0.524
HINGE = (X0 - 0.124, 3.262, -32.434)
GLASS = (X0, 3.150, -32.44)
GLASS_W, GLASS_H = 0.270, 0.215
CHAMFERS = (0.035, 0.022, 0.035, 0.028)
EDGE = 0.002
OHU_X = (X0 - 0.141, X0 + 0.139)
POCKET_U, POCKET_V = 0.025, -0.080
POCKET_W, POCKET_L, POCKET_R = 0.200, 0.270, 0.042

MATERIALS = {
    "fairing": ((0.165, 0.172, 0.185), 0.58, 0.0, 0.10, 0.012),
    "lens": ((0.035, 0.038, 0.045), 0.05, 0.0, 0.0, 0.0),
    "bezel": ((0.30, 0.30, 0.31), 0.32, 0.8, 0.35, 0.004),
    "housing": ((0.22, 0.22, 0.23), 0.48, 0.0, 0.22, 0.010),
    "arm": ((0.24, 0.24, 0.25), 0.42, 0.0, 0.25, 0.010),
    "grip": ((0.17, 0.17, 0.18), 0.78, 0.0, 0.05, 0.020),
    "screw": ((0.46, 0.46, 0.47), 0.35, 0.9, 0.30, 0.004),
    "screw_arm": ((0.46, 0.46, 0.47), 0.35, 0.9, 0.30, 0.004),
    "rim": ((0.70, 0.72, 0.74), 0.20, 0.0, 0.06, 0.002),
    "bracket": ((0.66, 0.68, 0.70), 0.28, 0.6, 0.10, 0.003),
    "label": ((0.74, 0.77, 0.58), 0.55, 0.0, 0.0, 0.004),
    "labeltext": ((0.06, 0.06, 0.05), 0.60, 0.0, 0.0, 0.0),
    "glass": ((0.62, 0.78, 0.72), 0.05, 0.0, 0.0, 0.0),
}


def bl(p):
    return Vector((p[0], -p[2], p[1]))


def xpdir(d):
    return Vector((d[0], -d[2], d[1]))


def material(name):
    m = bpy.data.materials.get(name)
    if m:
        return m
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes.get("Principled BSDF")
    col, rough, metal, wear, grain = MATERIALS[name]
    col = tuple(c ** 2.2 for c in col)
    bsdf.inputs["Metallic"].default_value = metal
    noise = nt.nodes.new("ShaderNodeTexNoise")
    noise.inputs["Scale"].default_value = 420.0
    noise.inputs["Detail"].default_value = 8.0
    fine = nt.nodes.new("ShaderNodeTexNoise")
    fine.inputs["Scale"].default_value = 2600.0
    fine.inputs["Detail"].default_value = 4.0
    geo = nt.nodes.new("ShaderNodeNewGeometry")
    edge = nt.nodes.new("ShaderNodeMapRange")
    edge.inputs["From Min"].default_value = 0.52
    edge.inputs["From Max"].default_value = 0.62
    nt.links.new(geo.outputs["Pointiness"], edge.inputs["Value"])
    ao = nt.nodes.new("ShaderNodeAmbientOcclusion")
    ao.inputs["Distance"].default_value = 0.012
    grime = nt.nodes.new("ShaderNodeMapRange")
    grime.inputs["From Min"].default_value = 0.35
    grime.inputs["From Max"].default_value = 1.0
    grime.inputs["To Min"].default_value = 0.55
    grime.inputs["To Max"].default_value = 1.0
    nt.links.new(ao.outputs["AO"], grime.inputs["Value"])
    base = nt.nodes.new("ShaderNodeRGB")
    base.outputs[0].default_value = (*col, 1.0)
    worn = nt.nodes.new("ShaderNodeMix")
    worn.data_type = "RGBA"
    worn.inputs["B"].default_value = (min(1, col[0] * 1.9 + 0.06), min(1, col[1] * 1.9 + 0.06), min(1, col[2] * 1.9 + 0.06), 1.0)
    wear_amt = nt.nodes.new("ShaderNodeMath")
    wear_amt.operation = "MULTIPLY"
    wear_amt.inputs[1].default_value = wear
    nt.links.new(edge.outputs["Result"], wear_amt.inputs[0])
    nt.links.new(wear_amt.outputs[0], worn.inputs["Factor"])
    nt.links.new(base.outputs[0], worn.inputs["A"])
    speck = nt.nodes.new("ShaderNodeMapRange")
    speck.inputs["To Min"].default_value = 1.0 - grain * 4
    speck.inputs["To Max"].default_value = 1.0 + grain * 2
    nt.links.new(noise.outputs["Fac"], speck.inputs["Value"])
    shade = nt.nodes.new("ShaderNodeMath")
    shade.operation = "MULTIPLY"
    nt.links.new(speck.outputs["Result"], shade.inputs[0])
    nt.links.new(grime.outputs["Result"], shade.inputs[1])
    tint = nt.nodes.new("ShaderNodeMix")
    tint.data_type = "RGBA"
    tint.blend_type = "MULTIPLY"
    tint.inputs["Factor"].default_value = 1.0
    nt.links.new(worn.outputs["Result"], tint.inputs["A"])
    comb = nt.nodes.new("ShaderNodeCombineColor")
    for k in ("Red", "Green", "Blue"):
        nt.links.new(shade.outputs[0], comb.inputs[k])
    nt.links.new(comb.outputs[0], tint.inputs["B"])
    nt.links.new(tint.outputs["Result"], bsdf.inputs["Base Color"])
    rmap = nt.nodes.new("ShaderNodeMapRange")
    rmap.inputs["To Min"].default_value = max(0.02, rough - 0.08)
    rmap.inputs["To Max"].default_value = min(1.0, rough + 0.10)
    nt.links.new(noise.outputs["Fac"], rmap.inputs["Value"])
    nt.links.new(rmap.outputs["Result"], bsdf.inputs["Roughness"])
    if grain > 0:
        bump = nt.nodes.new("ShaderNodeBump")
        bump.inputs["Strength"].default_value = min(0.6, grain * 18)
        bump.inputs["Distance"].default_value = 0.0004
        nt.links.new(fine.outputs["Fac"], bump.inputs["Height"])
        nt.links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
    return m


def rounded_outline(w, h, r_top, r_bottom, seg=16):
    pts = []
    corners = [((w / 2 - r_bottom, -h / 2 + r_bottom), r_bottom, -90), ((w / 2 - r_top, h / 2 - r_top), r_top, 0),
               ((-w / 2 + r_top, h / 2 - r_top), r_top, 90), ((-w / 2 + r_bottom, -h / 2 + r_bottom), r_bottom, 180)]
    for (cx, cy), r, a0 in corners:
        for k in range(seg + 1):
            a = math.radians(a0 + 90 * k / seg)
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts


def chamfered_outline(w, h, chamfers, r=0.004, seg=5):
    """Clockwise from the inboard (+x) side: chamfers = (top outboard, top inboard, bottom inboard, bottom outboard)."""
    to, ti, bi, bo = chamfers
    corners = [(w / 2, -h / 2 + bi), (w / 2, h / 2 - ti), (w / 2 - ti, h / 2), (-w / 2 + to, h / 2),
               (-w / 2, h / 2 - to), (-w / 2, -h / 2 + bo), (-w / 2 + bo, -h / 2), (w / 2 - bi, -h / 2)]
    pts = []
    n = len(corners)
    for i, c in enumerate(corners):
        a, b = Vector((*corners[i - 1], 0)), Vector((*corners[(i + 1) % n], 0))
        v = Vector((*c, 0))
        p0, p1 = v + (a - v).normalized() * r, v + (b - v).normalized() * r
        for k in range(seg + 1):
            t = k / seg
            q = p0 * (1 - t) ** 2 + v * 2 * t * (1 - t) + p1 * t ** 2
            pts.append((q.x, q.y))
    return pts


def circle(r, seg=48):
    return [(r * math.cos(2 * math.pi * k / seg), r * math.sin(2 * math.pi * k / seg)) for k in range(seg)]


def slab(name, outline, centre, u, v, n, thick, mat):
    bm = bmesh.new()
    c, u, v, n = Vector(centre), Vector(u).normalized(), Vector(v).normalized(), Vector(n).normalized()
    front = [bm.verts.new(c + u * x + v * y + n * thick / 2) for x, y in outline]
    back = [bm.verts.new(c + u * x + v * y - n * thick / 2) for x, y in outline]
    bm.faces.new(front)
    bm.faces.new(list(reversed(back)))
    k = len(outline)
    for i in range(k):
        bm.faces.new((front[i], back[i], back[(i + 1) % k], front[(i + 1) % k]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(ob)
    ob.data.materials.append(material(mat))
    return ob


def frame(axis):
    axis = Vector(axis).normalized()
    up = Vector((0, 0, 1)) if abs(axis.z) < 0.9 else Vector((1, 0, 0))
    u = axis.cross(up).normalized()
    return u, axis.cross(u), axis


def cylinder(name, centre, axis, radius, length, mat, seg=48):
    u, v, n = frame(axis)
    return slab(name, circle(radius, seg), centre, u, v, n, length, mat)


def box(name, centre, size, radius, mat, u=(1, 0, 0), v=(0, 0, 1), n=(0, 1, 0)):
    w, h, d = size
    return slab(name, rounded_outline(w, h, radius, radius), centre, u, v, n, d, mat)


def select(ob):
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob


def boolean(target, cutter, op):
    m = target.modifiers.new("b", "BOOLEAN")
    m.operation = op
    m.object = cutter
    m.solver = "EXACT"
    select(target)
    bpy.ops.object.modifier_apply(modifier=m.name)
    bpy.data.objects.remove(cutter)
    return target


def finish(ob, width, segments=3, angle=35):
    if width > 0:
        m = ob.modifiers.new("bevel", "BEVEL")
        m.width = width
        m.segments = segments
        m.limit_method = "ANGLE"
        m.angle_limit = math.radians(angle)
        m.harden_normals = True
        select(ob)
        bpy.ops.object.modifier_apply(modifier=m.name)
    select(ob)
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(40))
    return ob


def tube(name, points, radii, mat, bevel, kind="NURBS"):
    cu = bpy.data.curves.new(name, "CURVE")
    cu.dimensions = "3D"
    cu.bevel_depth = bevel
    cu.bevel_resolution = 8
    cu.use_fill_caps = True
    sp = cu.splines.new(kind)
    sp.points.add(len(points) - 1)
    for p, co, r in zip(sp.points, points, radii):
        p.co = (*co, 1.0)
        p.radius = r
    if kind == "NURBS":
        sp.order_u = 4
        sp.use_endpoint_u = True
        sp.resolution_u = 32
    ob = bpy.data.objects.new(name, cu)
    bpy.context.collection.objects.link(ob)
    ob.data.materials.append(material(mat))
    select(ob)
    bpy.ops.object.convert(target="MESH")
    return bpy.context.view_layer.objects.active


def screw(name, centre, normal, radius, parts, mat="screw"):
    head = cylinder(name, centre + normal * 0.0006, normal, radius, 0.0014, mat, seg=32)
    finish(head, radius * 0.35, 3, angle=40)
    u, v, n = frame(normal)
    for d in (u, v):
        slot = slab(name + "_slot", rounded_outline(radius * 1.5, radius * 0.28, 0.0002, 0.0002, 2), centre + normal * 0.0012,
                    d, n.cross(d), n, 0.0012, mat)
        boolean(head, slot, "DIFFERENCE")
    parts.append(head)


def build():
    gx, gy, gz = GLASS
    hx, hy, hz = HINGE
    pu, pv, pn = xpdir((1, 0, 0)), xpdir((0, 1, 0)), xpdir((0, 0, 1))
    static, moving = [], []

    inset = tuple(c - EDGE * 0.4 for c in CHAMFERS)
    glass = slab("glass", chamfered_outline(GLASS_W - 2 * EDGE, GLASS_H - 2 * EDGE, inset), bl(GLASS), pu, pv, pn, 0.007, "glass")
    finish(glass, 0.0005, 2)
    rim = slab("rim", chamfered_outline(GLASS_W, GLASS_H, CHAMFERS), bl(GLASS), pu, pv, pn, 0.0075, "rim")
    hole = slab("hole", chamfered_outline(GLASS_W - 2 * EDGE, GLASS_H - 2 * EDGE, inset), bl(GLASS), pu, pv, pn, 0.03, "rim")
    boolean(rim, hole, "DIFFERENCE")
    finish(rim, 0.0006, 2)
    moving.append(rim)

    to, ti, bi, bo = CHAMFERS
    top = gy + GLASS_H / 2
    left = gx - GLASS_W / 2
    right = gx + GLASS_W / 2
    strip = tube("bracket", [bl((hx + 0.016, hy - 0.002, gz)), bl((left + to + 0.012, top + 0.0045, gz)), bl((right - ti - 0.010, top + 0.0045, gz)),
                             bl((right - ti * 0.35, top + 0.0035, gz)), bl((right + 0.0015, top - ti * 0.55, gz)), bl((right + 0.0015, top - ti - 0.006, gz))],
                 [1.2, 1.0, 1.0, 1.0, 0.95, 0.75], "bracket", bevel=0.0042)
    finish(strip, 0.0, 1)
    moving.append(strip)
    rail = tube("rail", [bl((hx - 0.002, hy - 0.010, gz)), bl((left - 0.0018, top - to - 0.002, gz)), bl((left - 0.0018, top - to - 0.040, gz)),
                         bl((left - 0.0018, gy - GLASS_H * 0.20, gz))],
                [1.15, 1.0, 1.0, 0.7], "bracket", bevel=0.0032)
    finish(rail, 0.0, 1)
    moving.append(rail)
    yoke = box("yoke", bl((hx + 0.004, hy - 0.004, gz + 0.001)), (0.026, 0.009, 0.024), 0.004, "bracket", u=pu, v=pn, n=pv)
    finish(yoke, 0.0015, 3)
    moving.append(yoke)
    for cx in (left + 0.075, right - 0.075):
        clip = box("clip", bl((cx, top + 0.003, gz)), (0.020, 0.012, 0.014), 0.003, "arm", u=pu, v=pn, n=pv)
        finish(clip, 0.0012, 3)
        moving.append(clip)
        for side in (-1, 1):
            screw("clipscrew", bl((cx, top + 0.003, gz + side * 0.0072)), xpdir((0, 0, side)), 0.0019, moving, "screw_arm")

    barrel = cylinder("knuckle", bl((hx, hy, hz)), xpdir((1, 0, 0)), 0.0135, 0.040, "housing", seg=64)
    finish(barrel, 0.0022, 4)
    boss = box("boss", bl((hx - 0.027, hy + 0.001, hz + 0.010)), (0.016, 0.034, 0.022), 0.005, "housing", u=pu, v=pn, n=pv)
    finish(boss, 0.0025, 4)
    static.extend([barrel, boss])
    for s_ in (-1, 1):
        cap = cylinder("pincap", bl((hx + s_ * 0.0215, hy, hz)), xpdir((1, 0, 0)), 0.0110, 0.004, "bezel", seg=48)
        finish(cap, 0.0010, 3)
        static.append(cap)
        nut = cylinder("pinnut", bl((hx + s_ * 0.0245, hy, hz)), xpdir((1, 0, 0)), 0.0062, 0.0035, "screw", seg=6)
        finish(nut, 0.0006, 2)
        static.append(nut)
    detent = cylinder("detent", bl((hx - 0.027, hy - 0.004, hz + 0.028)), xpdir((0, 0, 1)), 0.0045, 0.010, "bezel", seg=32)
    finish(detent, 0.0008, 2)
    static.append(detent)
    for k in range(2):
        screw("bossscrew", bl((hx - 0.0355, hy + 0.001, hz + 0.004 + k * 0.013)), xpdir((-1, 0, 0)), 0.0022, static)

    xa, xb = OHU_X
    profile = [(-32.420, 3.312), (-32.205, 3.312), (-32.192, 3.362), (-31.990, 3.585), (-31.990, 3.720), (-32.465, 3.720),
               (-32.465, 3.382), (-32.437, 3.322)]
    bm = bmesh.new()
    left_face = [bm.verts.new(bl((xa, y, z))) for z, y in profile]
    right_face = [bm.verts.new(bl((xb, y, z))) for z, y in profile]
    bm.faces.new(left_face)
    bm.faces.new(list(reversed(right_face)))
    k = len(profile)
    for i in range(k):
        bm.faces.new((left_face[i], left_face[(i + 1) % k], right_face[(i + 1) % k], right_face[i]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new("ohu")
    bm.to_mesh(me)
    bm.free()
    ohu = bpy.data.objects.new("ohu", me)
    bpy.context.collection.objects.link(ohu)
    ohu.data.materials.append(material("fairing"))
    finish(ohu, 0.005, 4, angle=30)
    xm = (xa + xb) / 2
    window = box("window_cut", bl((xm + 0.020, 3.312, -32.330)), (0.120, 0.070, 0.012), 0.006, "fairing", u=pu, v=pn, n=pv)
    boolean(ohu, window, "DIFFERENCE")
    for k in range(6):
        rib = box("rib", bl((xa + 0.026 + k * 0.011, 3.3115, -32.300)), (0.0045, 0.140, 0.004), 0.0018, "fairing", u=pu, v=pn, n=pv)
        boolean(ohu, rib, "DIFFERENCE")
    static.append(ohu)
    lens = box("lens", bl((xm + 0.020, 3.3155, -32.330)), (0.116, 0.066, 0.002), 0.005, "lens", u=pu, v=pn, n=pv)
    static.append(lens)
    bezel = box("bezel", bl((xm + 0.020, 3.3125, -32.330)), (0.128, 0.078, 0.002), 0.007, "bezel", u=pu, v=pn, n=pv)
    bh = box("bezelhole", bl((xm + 0.020, 3.3125, -32.330)), (0.114, 0.064, 0.02), 0.005, "bezel", u=pu, v=pn, n=pv)
    boolean(bezel, bh, "DIFFERENCE")
    finish(bezel, 0.0006, 2)
    static.append(bezel)
    aft_n = Vector((0.0, 0.050, 0.013)).normalized()
    label_c = Vector((xb - 0.060, 3.337, -32.1985)) + aft_n * 0.0012
    lu = xpdir((1, 0, 0))
    ln = xpdir((0.0, -0.250, 0.968))
    lv = ln.cross(lu)
    label = slab("label", rounded_outline(0.078, 0.040, 0.002, 0.002, 2), bl(tuple(label_c)), lu, lv, ln, 0.0016, "label")
    finish(label, 0.0005, 2)
    static.append(label)
    lines = ["HEAD-UP DISPLAY", "OVERHEAD UNIT", "PN 380-HUD-OHU-01", "SN 004127   MOD ABCD"]
    for i, line in enumerate(lines):
        bpy.ops.object.text_add()
        t = bpy.context.view_layer.objects.active
        t.data.body = line
        t.data.size = 0.0058 if i < 2 else 0.0046
        t.data.extrude = 0.00012
        t.data.align_x = "LEFT"
        bpy.ops.object.convert(target="MESH")
        t = bpy.context.view_layer.objects.active
        t.data.materials.clear()
        t.data.materials.append(material("labeltext"))
        origin = bl(tuple(label_c)) + lu * (-0.034) + lv * (0.012 - i * 0.0085) + ln * 0.0009
        t.matrix_world = Matrix(((lu.x, lv.x, ln.x, origin.x), (lu.y, lv.y, ln.y, origin.y), (lu.z, lv.z, ln.z, origin.z), (0, 0, 0, 1)))
        static.append(t)
    for sx in (xa + 0.012, xb - 0.012):
        for sz in (-32.405, -32.220):
            screw("ohuscrew", bl((sx, 3.3117, sz)), xpdir((0, -1, 0)), 0.0028, static)
    arm_top = (hx - 0.022, 3.316, hz + 0.012)
    bracket = box("ohuarm", bl((arm_top[0], (arm_top[1] + hy + 0.004) / 2, hz + 0.016)), (0.020, 0.026, arm_top[1] - hy + 0.004), 0.005, "housing",
                  u=pu, v=pn, n=pv)
    finish(bracket, 0.0025, 4)
    static.append(bracket)
    foot = box("ohufoot", bl((arm_top[0] + 0.006, 3.3135, hz + 0.016)), (0.040, 0.040, 0.004), 0.004, "housing", u=pu, v=pn, n=pv)
    finish(foot, 0.0012, 3)
    static.append(foot)
    for sx in (-1, 1):
        screw("footscrew", bl((arm_top[0] + 0.006 + sx * 0.013, 3.3112, hz + 0.016)), xpdir((0, -1, 0)), 0.0022, static)
    return static, moving, glass


def bake(static, moving, glass):
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = 192
    scene.cycles.device = "CPU"
    scene.view_settings.view_transform = "Standard"
    for ob in static:
        ob["hud_group"] = "static"
    select(static[0])
    for ob in static + moving:
        ob.select_set(True)
    bpy.ops.object.join()
    unit = bpy.context.view_layer.objects.active
    unit.name = "hud_unit"
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=math.radians(50), island_margin=0.003)
    bpy.ops.uv.pack_islands(margin=0.003)
    bpy.ops.object.mode_set(mode="OBJECT")
    for loop in unit.data.uv_layers.active.data:
        loop.uv = loop.uv * 0.92
    select(glass)
    glass.data.uv_layers.new(name="UVMap")
    for loop in glass.data.uv_layers.active.data:
        loop.uv = (0.97, 0.97)
    jobs = (("col", "DIFFUSE", {"COLOR"}, "sRGB"), ("ao", "AO", None, "Non-Color"),
            ("rough", "ROUGHNESS", None, "Non-Color"), ("nml", "NORMAL", None, "Non-Color"))
    for kind, bake_type, passes, space in jobs:
        img = bpy.data.images.new(kind, SIZE, SIZE, alpha=False)
        img.colorspace_settings.name = space
        for m in unit.data.materials:
            nt = m.node_tree
            n = nt.nodes.get("bake_target") or nt.nodes.new("ShaderNodeTexImage")
            n.name = "bake_target"
            n.image = img
            nt.nodes.active = n
        select(unit)
        kw = {"type": bake_type, "margin": 8, "use_clear": True}
        if passes:
            kw["pass_filter"] = passes
        if bake_type == "NORMAL":
            kw["normal_space"] = "TANGENT"
        bpy.ops.object.bake(**kw)
        img.filepath_raw = os.path.join(OUT, f"{kind}.png")
        img.file_format = "PNG"
        img.save()
    for ob in list(bpy.data.objects):
        ob.select_set(ob.name in ("hud_unit", "glass"))
    bpy.ops.wm.obj_export(filepath=os.path.join(OUT, "hud_unit.obj"), export_selected_objects=True, forward_axis="NEGATIVE_Z",
                          up_axis="Y", export_materials=True, export_uv=True, export_normals=True, apply_modifiers=True,
                          export_triangulated_mesh=True, export_material_groups=True)


def main():
    os.makedirs(OUT, exist_ok=True)
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob)
    static, moving, glass = build()
    bake(static, moving, glass)
    print("hud unit written to", OUT)


main()
