"""Straight-on panel art from the cockpit OBJs, lamps off and lamps on (Blender, Cycles, orthographic).

blender -b --factory-startup -P render.py -- [--panels E:/fbw-build/panels] [--only glareshield,fcu]
    [--aircraft DIR] [--samples 48] [--gpu]
"""
import argparse
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from obj8 import Obj8  # noqa: E402

import bpy  # noqa: E402
from mathutils import Matrix, Vector  # noqa: E402

DEFAULT_AIRCRAFT = "D:/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12"
VISUAL = ["a380_cockpit_000.obj", "a380_cockpit_003.obj"] + [f"a380_cockpit_{i:03d}.obj" for i in range(11, 96)] + [
    "a380_cockpit_039_lamps.obj", "a380_cockpit_042_lamps.obj"]
SKIP = {"a380_cockpit_085.obj", "a380_cockpit_086.obj", "a380_cockpit_087.obj", "a380_cockpit_088.obj"}
CAM_DIST = 0.5


def argv():
    a = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    ap = argparse.ArgumentParser()
    ap.add_argument("--panels", default="E:/fbw-build/panels")
    ap.add_argument("--only", default="")
    ap.add_argument("--aircraft", default=DEFAULT_AIRCRAFT)
    ap.add_argument("--samples", type=int, default=48)
    ap.add_argument("--gpu", action="store_true")
    return ap.parse_args(a)


_images = {}


def image(objs, name):
    if not name:
        return None
    if name in _images:
        return _images[name]
    path = os.path.join(objs, name)
    img = None
    if os.path.exists(path):
        try:
            img = bpy.data.images.load(path, check_existing=True)
        except RuntimeError:
            img = None
    _images[name] = img
    return img


def material(objs, key, lamps_on):
    tex, lit, blend = key
    name = f"{tex}|{lit}|{blend}|{lamps_on}"
    m = bpy.data.materials.get(name)
    if m:
        return m
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.55
    img = image(objs, tex)
    if img:
        t = nt.nodes.new("ShaderNodeTexImage")
        t.image = img
        nt.links.new(t.outputs["Color"], bsdf.inputs["Base Color"])
        if blend:
            nt.links.new(t.outputs["Alpha"], bsdf.inputs["Alpha"])
            m.blend_method = "BLEND" if hasattr(m, "blend_method") else None
    else:
        bsdf.inputs["Base Color"].default_value = (0.2, 0.2, 0.2, 1)
    limg = image(objs, lit) if lamps_on else None
    if limg:
        e = nt.nodes.new("ShaderNodeTexImage")
        e.image = limg
        nt.links.new(e.outputs["Color"], bsdf.inputs["Emission Color"])
        bsdf.inputs["Emission Strength"].default_value = 2.0
    return m


def clear_scene():
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    for me in list(bpy.data.meshes):
        bpy.data.meshes.remove(me)


def build(panel, parsed, objs, lamps_on):
    pl = panel["plane"]
    org, r, u, n = (np.array(pl[k]) for k in ("origin", "right", "up", "normal"))
    w, h = pl["width_m"], pl["height_m"]
    pad = 0.005
    dfront, dback = pl["depth_front_m"] + 0.05, pl["depth_back_m"] - 0.02
    ctr0 = org - u * 0  # top-left origin
    count = 0
    for name, o in parsed:
        buckets = {}
        for d in o.draws:
            if d.lod > 0 or d.hidden or d.count < 3 or d.state.get("disabled"):
                continue
            P, N, UV = o.triangles(d)
            q = P - ctr0
            x, y, z = q @ r, q @ u, q @ n
            nt = len(P) // 3
            x, y, z = x[: nt * 3].reshape(nt, 3), y[: nt * 3].reshape(nt, 3), z[: nt * 3].reshape(nt, 3)
            keep = ((x.max(1) >= -pad) & (x.min(1) <= w + pad) & (y.max(1) >= -h - pad) & (y.min(1) <= pad)
                    & (z.max(1) >= dback) & (z.min(1) <= dfront))
            if not keep.any():
                continue
            if "device" in d.state and name == "a380_cockpit_003.obj":
                continue
            sel = np.repeat(keep, 3)
            key = (o.textures.get("TEXTURE"), o.textures.get("TEXTURE_LIT"), "no_blend" not in d.state)
            b = buckets.setdefault(key, ([], []))
            b[0].append(P[: nt * 3][sel])
            b[1].append(UV[: nt * 3][sel])
        for key, (Ps, UVs) in buckets.items():
            P = np.vstack(Ps)
            UV = np.vstack(UVs)
            nt = len(P) // 3
            me = bpy.data.meshes.new(f"{name}")
            me.from_pydata([tuple(v) for v in P], [], [(3 * i, 3 * i + 1, 3 * i + 2) for i in range(nt)])
            uvl = me.uv_layers.new()
            uvl.data.foreach_set("uv", UV.astype(np.float32).ravel())
            me.materials.append(material(objs, key, lamps_on))
            ob = bpy.data.objects.new(name, me)
            bpy.context.scene.collection.objects.link(ob)
            count += nt
    return count


def camera(panel):
    pl = panel["plane"]
    org, r, u, n = (Vector(pl[k]) for k in ("origin", "right", "up", "normal"))
    w, h = pl["width_m"], pl["height_m"]
    ctr = org + r * (w / 2) - u * (h / 2)
    cd = bpy.data.cameras.new("cam")
    cd.type = "ORTHO"
    cd.ortho_scale = max(w, h)
    cd.clip_start = max(0.001, CAM_DIST - pl["depth_front_m"] - 0.06)
    cd.clip_end = CAM_DIST - pl["depth_back_m"] + 0.03
    cam = bpy.data.objects.new("cam", cd)
    rot = Matrix((r, u, n)).transposed()
    cam.matrix_world = Matrix.Translation(ctr + n * CAM_DIST) @ rot.to_4x4()
    bpy.context.scene.collection.objects.link(cam)
    bpy.context.scene.camera = cam
    ld = bpy.data.lights.new("key", "SUN")
    ld.energy = 2.0
    ld.angle = 0.6
    key = bpy.data.objects.new("key", ld)
    key.matrix_world = Matrix.Translation(ctr + n * CAM_DIST) @ (rot @ Matrix.Rotation(0.25, 3, "X")).to_4x4()
    bpy.context.scene.collection.objects.link(key)


def setup(args, size):
    sc = bpy.context.scene
    sc.render.engine = "CYCLES"
    sc.cycles.samples = args.samples
    sc.cycles.use_denoising = True
    if args.gpu:
        sc.cycles.device = "GPU"
    sc.render.resolution_x, sc.render.resolution_y = size
    sc.render.resolution_percentage = 100
    sc.render.film_transparent = False
    sc.view_settings.view_transform = "Standard"
    sc.render.image_settings.file_format = "PNG"
    world = sc.world or bpy.data.worlds.new("w")
    sc.world = world
    world.use_nodes = True
    bg = world.node_tree.nodes.get("Background")
    bg.inputs["Color"].default_value = (1, 1, 1, 1)
    bg.inputs["Strength"].default_value = 0.9


def main():
    args = argv()
    objs = os.path.join(args.aircraft, "objects")
    only = [s for s in args.only.split(",") if s]
    files = sorted(f for f in os.listdir(args.panels) if f.endswith(".json"))
    panels = [json.load(open(os.path.join(args.panels, f), encoding="utf-8")) for f in files]
    if only:
        panels = sorted((p for p in panels if p["id"] in only), key=lambda p: only.index(p["id"]))
    parsed = [(f, Obj8(os.path.join(objs, f))) for f in VISUAL if f not in SKIP and os.path.exists(os.path.join(objs, f))]
    os.makedirs(os.path.join(args.panels, "art"), exist_ok=True)
    for p in panels:
        for lamps_on in (False, True):
            clear_scene()
            for c in list(bpy.data.cameras):
                bpy.data.cameras.remove(c)
            for l in list(bpy.data.lights):
                bpy.data.lights.remove(l)
            n = build(p, parsed, objs, lamps_on)
            camera(p)
            pl = p["plane"]
            ppm = p.get("px_per_mm", 4.0)
            size = (int(round(pl["width_m"] * 1000 * ppm)), int(round(pl["height_m"] * 1000 * ppm)))
            setup(args, size)
            out = os.path.join(args.panels, p["art_lit"] if lamps_on else p["art"])
            bpy.context.scene.render.filepath = out
            bpy.ops.render.render(write_still=True)
            print(f"panel {p['id']} {'on' if lamps_on else 'off'}: {n} tris -> {out}", flush=True)


main()
