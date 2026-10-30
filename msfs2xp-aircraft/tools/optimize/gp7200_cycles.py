"""usage: blender -b --factory-startup -P gp7200_cycles.py -- <spec.json>

Cycles render of X-Plane OBJ8 objects with their real albedo, normal map (X-Plane _NML: red and green hold the tangent normal, blue is empty) and material
map (_MAT: red metalness, green gloss), so that normal and material detail can be judged without X-Plane. The spec holds
  objects: [{path, engine (1-4, selects the engine's rel_engsep batches; omit for everything), albedo_dir (optional override folder for the albedo)}],
  views: [{name, eye, at, fov}], out_dir, size, samples, nml_override / mat_override ({texture file name: replacement path}).
Coordinates are X-Plane (x right, y up, z aft) and are turned into Blender's (x, -z, y)."""

import json
import math
import re
import sys
from pathlib import Path

import bpy
import numpy as np
from mathutils import Vector

ENGSEP = re.compile(r"ANIM_hide 5\.5 6\.5 sim/operation/failures/rel_engsep(\d)")


def parse(path):
    lines = Path(path).read_text(encoding="utf-8", errors="replace").split("\n")
    vt = np.array([[float(v) for v in l.split()[1:9]] for l in lines if l.startswith("VT")]).reshape(-1, 8)
    idx = np.array([int(v) for l in lines if l.startswith(("IDX10", "IDX ")) for v in l.split()[1:]], dtype=np.int64)
    first = next(i for i, l in enumerate(lines) if l.startswith("VT"))
    last = max(i for i, l in enumerate(lines) if l.startswith(("VT", "IDX")))
    tex = {}
    for l in lines[:first]:
        t = l.split()
        if t[:1] == ["TEXTURE"]:
            tex["albedo"] = t[1]
        elif t[:2] == ["TEXTURE_MAP", "normal"]:
            tex["normal"] = t[2]
        elif t[:2] == ["TEXTURE_MAP", "material_gloss"]:
            tex["mat"] = t[2]
    batches, stack = [], []
    for l in lines[last + 1:]:
        t = l.strip()
        if t == "ANIM_begin":
            stack.append(None)
        elif t == "ANIM_end":
            if stack:
                stack.pop()
        else:
            m = ENGSEP.match(t)
            if m and stack:
                stack[-1] = int(m.group(1))
            elif t.startswith("TRIS "):
                s, c = map(int, t.split()[1:3])
                eng = next((e for e in reversed(stack) if e is not None), None)
                batches.append((s, c, eng))
    return vt, idx, tex, batches


def conv(p):
    return (p[0], -p[2], p[1])


def build(spec_obj, nml_over, mat_over):
    path = Path(spec_obj["path"])
    vt, idx, tex, batches = parse(path)
    want = spec_obj.get("engine")
    tris = []
    for s, c, eng in batches:
        if want is None or eng is None or eng == want - 1:
            tris.append(idx[s:s + c].reshape(-1, 3))
    if not tris:
        return None
    T = np.concatenate(tris)
    p3 = vt[:, :3][T]
    area = np.linalg.norm(np.cross(p3[:, 1] - p3[:, 0], p3[:, 2] - p3[:, 0]), axis=1)
    T = T[(area > 1e-9) & (T[:, 0] != T[:, 1]) & (T[:, 1] != T[:, 2]) & (T[:, 0] != T[:, 2])]
    used, inv = np.unique(T.ravel(), return_inverse=True)
    T = inv.reshape(-1, 3)
    P = vt[used, :3]
    N = vt[used, 3:6]
    UV = vt[used, 6:8]
    Pb = np.stack([P[:, 0], -P[:, 2], P[:, 1]], axis=1)
    Nb = np.stack([N[:, 0], -N[:, 2], N[:, 1]], axis=1)
    mesh = bpy.data.meshes.new(path.stem)
    mesh.from_pydata(Pb.tolist(), [], T[:, ::-1].tolist())
    mesh.update()
    uv = mesh.uv_layers.new(name="UV")
    loop_v = np.array([l.vertex_index for l in mesh.loops])
    uv.data.foreach_set("uv", UV[loop_v].ravel().tolist())
    mesh.normals_split_custom_set_from_vertices(Nb.tolist())
    obj = bpy.data.objects.new(path.stem, mesh)
    bpy.context.scene.collection.objects.link(obj)
    for poly in mesh.polygons:
        poly.use_smooth = True
    mat = bpy.data.materials.new(path.stem)
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])

    def image(name, folders, colour):
        for d in folders:
            p = Path(d) / name
            if p.is_file():
                img = bpy.data.images.load(str(p))
                img.colorspace_settings.name = "sRGB" if colour else "Non-Color"
                img.alpha_mode = "NONE"
                node = nt.nodes.new("ShaderNodeTexImage")
                node.image = img
                node.interpolation = "Smart"
                return node
        return None

    folders = [spec_obj.get("albedo_dir"), path.parent]
    folders = [f for f in folders if f]
    alb = image(tex["albedo"], folders, True)
    if alb and not spec_obj.get("plain"):
        nt.links.new(alb.outputs["Color"], bsdf.inputs["Base Color"])
    nml_name = nml_over.get(tex.get("normal"), tex.get("normal"))
    nml = image(nml_name, [path.parent], False) if nml_name else None
    if nml is None and nml_name and Path(nml_name).is_file():
        img = bpy.data.images.load(nml_name)
        img.colorspace_settings.name = "Non-Color"
        img.alpha_mode = "NONE"
        nml = nt.nodes.new("ShaderNodeTexImage")
        nml.image = img
    if nml and not spec_obj.get("skip_normal"):
        sep = nt.nodes.new("ShaderNodeSeparateColor")
        nt.links.new(nml.outputs["Color"], sep.inputs["Color"])
        mx = nt.nodes.new("ShaderNodeMath"); mx.operation = "MULTIPLY_ADD"; mx.inputs[1].default_value = 2.0; mx.inputs[2].default_value = -1.0
        my = nt.nodes.new("ShaderNodeMath"); my.operation = "MULTIPLY_ADD"; my.inputs[1].default_value = 2.0; my.inputs[2].default_value = -1.0
        nt.links.new(sep.outputs["Red"], mx.inputs[0])
        nt.links.new(sep.outputs["Green"], my.inputs[0])
        sx = nt.nodes.new("ShaderNodeMath"); sx.operation = "POWER"; sx.inputs[1].default_value = 2.0
        sy = nt.nodes.new("ShaderNodeMath"); sy.operation = "POWER"; sy.inputs[1].default_value = 2.0
        nt.links.new(mx.outputs[0], sx.inputs[0])
        nt.links.new(my.outputs[0], sy.inputs[0])
        add = nt.nodes.new("ShaderNodeMath"); add.operation = "ADD"
        nt.links.new(sx.outputs[0], add.inputs[0])
        nt.links.new(sy.outputs[0], add.inputs[1])
        one = nt.nodes.new("ShaderNodeMath"); one.operation = "SUBTRACT"; one.inputs[0].default_value = 1.0
        nt.links.new(add.outputs[0], one.inputs[1])
        clamp = nt.nodes.new("ShaderNodeMath"); clamp.operation = "MAXIMUM"; clamp.inputs[1].default_value = 0.0
        nt.links.new(one.outputs[0], clamp.inputs[0])
        rt = nt.nodes.new("ShaderNodeMath"); rt.operation = "SQRT"
        nt.links.new(clamp.outputs[0], rt.inputs[0])
        enc = nt.nodes.new("ShaderNodeMath"); enc.operation = "MULTIPLY_ADD"; enc.inputs[1].default_value = 0.5; enc.inputs[2].default_value = 0.5
        nt.links.new(rt.outputs[0], enc.inputs[0])
        comb = nt.nodes.new("ShaderNodeCombineColor")
        nt.links.new(sep.outputs["Red"], comb.inputs["Red"])
        nt.links.new(sep.outputs["Green"], comb.inputs["Green"])
        nt.links.new(enc.outputs[0], comb.inputs["Blue"])
        nm = nt.nodes.new("ShaderNodeNormalMap")
        nm.inputs["Strength"].default_value = 1.0
        nt.links.new(comb.outputs["Color"], nm.inputs["Color"])
        nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    mat_name = mat_over.get(tex.get("mat"), tex.get("mat"))
    mt = image(mat_name, [path.parent], False) if mat_name else None
    if mt is None and mat_name and Path(mat_name).is_file():
        img = bpy.data.images.load(mat_name)
        img.colorspace_settings.name = "Non-Color"
        img.alpha_mode = "NONE"
        mt = nt.nodes.new("ShaderNodeTexImage")
        mt.image = img
    if mt and not spec_obj.get("skip_mat"):
        sep2 = nt.nodes.new("ShaderNodeSeparateColor")
        nt.links.new(mt.outputs["Color"], sep2.inputs["Color"])
        nt.links.new(sep2.outputs["Red"], bsdf.inputs["Metallic"])
        inv = nt.nodes.new("ShaderNodeMath"); inv.operation = "SUBTRACT"; inv.inputs[0].default_value = 1.0
        nt.links.new(sep2.outputs["Green"], inv.inputs[1])
        nt.links.new(inv.outputs[0], bsdf.inputs["Roughness"])
    obj.data.materials.append(mat)
    return obj


def main(spec):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    sc = bpy.context.scene
    sc.render.engine = "CYCLES"
    sc.cycles.device = "CPU"
    sc.cycles.samples = spec.get("samples", 48)
    sc.cycles.use_denoising = spec.get("denoise", True)
    sc.render.resolution_x, sc.render.resolution_y = spec.get("size", [1000, 640])
    sc.render.image_settings.file_format = "PNG"
    sc.view_settings.view_transform = "Standard"
    world = bpy.data.worlds.new("w")
    world.use_nodes = True
    sc.world = world
    bg = world.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = (0.55, 0.6, 0.68, 1.0)
    bg.inputs["Strength"].default_value = spec.get("bg_strength", 0.45)
    sun = bpy.data.lights.new("sun", "SUN")
    sun.energy = 2.4
    sun.angle = math.radians(4)
    so = bpy.data.objects.new("sun", sun)
    suns = spec.get("suns") or [[50, 10, -35]]
    so.rotation_euler = tuple(math.radians(x) for x in suns[0])
    sc.collection.objects.link(so)
    for o in spec["objects"]:
        build(o, spec.get("nml_override", {}), spec.get("mat_override", {}))
    cam_data = bpy.data.cameras.new("cam")
    cam = bpy.data.objects.new("cam", cam_data)
    sc.collection.objects.link(cam)
    sc.camera = cam
    cam_data.sensor_fit = "VERTICAL"
    for v, si in [(v, i) for i in range(len(suns)) for v in spec["views"]]:
        so.rotation_euler = tuple(math.radians(x) for x in suns[si])
        vname = v["name"] + (f"_s{si}" if len(suns) > 1 else "")
        eye, at = Vector(conv(v["eye"])), Vector(conv(v["at"]))
        cam.location = eye
        cam.rotation_euler = (at - eye).to_track_quat("-Z", "Y").to_euler()
        cam_data.angle_y = math.radians(v["fov"])
        cam_data.clip_start, cam_data.clip_end = 0.05, 500
        sc.render.filepath = str(Path(spec["out_dir"]) / f"{vname}.png")
        bpy.ops.render.render(write_still=True)
        print("rendered", sc.render.filepath, flush=True)


if __name__ == "__main__":
    main(json.loads(Path(sys.argv[sys.argv.index("--") + 1]).read_text()))
