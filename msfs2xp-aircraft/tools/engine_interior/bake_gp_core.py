"""Blender script: unwrap the GP7200 core's parts, pack them into one 2048 sheet and bake its X-Plane textures.

    blender -b --factory-startup -P bake_gp_core.py -- <parts.obj> <out dir> [size]

parts.obj holds one object per part, named <material>__<part id>; one copy of every replicated row (blades, vanes, bolts) carries the UV
set all its copies share, and objects named ctx__* are neighbouring copies that only shade the bake. Writes GPCORE_{col,ao,nrm,rough,metal}_raw.png
(finish_textures.py turns them into ALB/NML/MAT) and uv.npz (the packed UV of every part, per face corner)."""

import os
import sys

import bpy
import numpy as np

ARGS = sys.argv[sys.argv.index("--") + 1:]
OBJ, OUT = ARGS[0], ARGS[1]
SIZE = int(ARGS[2]) if len(ARGS) > 2 else 2048
STEM = "GPCORE"

CASE_RAMP = [(0.0, (0.27, 0.30, 0.33)), (0.20, (0.29, 0.31, 0.33)), (0.30, (0.26, 0.26, 0.26)), (0.45, (0.22, 0.19, 0.16)), (0.65, (0.27, 0.21, 0.15)), (1.0, (0.31, 0.24, 0.17))]

# name: (base colour, tint colour, metallic, roughness, grime, streak strength, edge wear, bump, heat-tint amount)
MATS = {
    "titanium": ((0.46, 0.44, 0.41), (0.30, 0.27, 0.22), 1.0, 0.38, 0.25, 0.45, 0.45, 0.12, 0.25),
    "stainless": ((0.66, 0.66, 0.64), (0.42, 0.38, 0.30), 1.0, 0.28, 0.15, 0.35, 0.5, 0.08, 0.2),
    "casing": ((0.36, 0.36, 0.35), (0.20, 0.17, 0.14), 0.9, 0.45, 0.3, 0.3, 0.4, 0.3, 0.3),
    "case": ((0.27, 0.30, 0.33), (0.18, 0.14, 0.10), 0.85, 0.5, 0.3, 0.35, 0.3, 0.25, 0.0),
    "bronze": ((0.24, 0.15, 0.10), (0.10, 0.12, 0.20), 0.85, 0.5, 0.35, 0.2, 0.2, 0.2, 0.6),
    "black": ((0.02, 0.02, 0.02), (0.01, 0.01, 0.01), 0.0, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0),
    "hose": ((0.78, 0.26, 0.10), (0.5, 0.15, 0.06), 0.0, 0.55, 0.15, 0.0, 0.0, 0.25, 0.0),
    "blue": ((0.10, 0.24, 0.50), (0.05, 0.10, 0.25), 0.0, 0.4, 0.1, 0.0, 0.0, 0.3, 0.0),
    "grey_harness": ((0.28, 0.28, 0.27), (0.12, 0.12, 0.12), 0.0, 0.7, 0.1, 0.0, 0.0, 0.6, 0.0),
    "canister": ((0.82, 0.82, 0.80), (0.5, 0.5, 0.48), 0.6, 0.35, 0.2, 0.1, 0.3, 0.05, 0.0),
    "eec": ((0.62, 0.63, 0.64), (0.35, 0.35, 0.35), 0.8, 0.4, 0.25, 0.1, 0.3, 0.1, 0.0),
    "casting": ((0.40, 0.41, 0.40), (0.15, 0.14, 0.13), 0.6, 0.55, 0.35, 0.2, 0.3, 0.45, 0.0),
    "brass": ((0.65, 0.52, 0.25), (0.35, 0.25, 0.10), 1.0, 0.4, 0.2, 0.2, 0.3, 0.05, 0.0),
    "blade": ((0.58, 0.59, 0.60), (0.35, 0.34, 0.32), 1.0, 0.22, 0.1, 0.4, 0.5, 0.0, 0.1),
    "turbine": ((0.20, 0.18, 0.17), (0.30, 0.20, 0.12), 0.85, 0.45, 0.2, 0.3, 0.2, 0.05, 0.7),
    "liner": ((0.19, 0.17, 0.15), (0.34, 0.24, 0.14), 0.8, 0.5, 0.3, 0.2, 0.0, 0.1, 0.6),
    "primer": ((0.30, 0.32, 0.33), (0.18, 0.18, 0.17), 0.5, 0.55, 0.3, 0.1, 0.2, 0.15, 0.0),
    "label": ((0.88, 0.88, 0.84), (0.1, 0.1, 0.1), 0.0, 0.6, 0.05, 0.0, 0.0, 0.0, 0.0),
    "bolt": ((0.55, 0.54, 0.52), (0.30, 0.27, 0.22), 1.0, 0.35, 0.15, 0.0, 0.5, 0.0, 0.2),
    "wire": ((0.70, 0.70, 0.68), (0.5, 0.5, 0.5), 1.0, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0),
    "duct": ((0.62, 0.62, 0.60), (0.40, 0.34, 0.26), 1.0, 0.3, 0.2, 0.45, 0.3, 0.08, 0.35),
    "frame": ((0.62, 0.16, 0.12), (0.30, 0.07, 0.05), 0.0, 0.7, 0.2, 0.0, 0.0, 0.2, 0.0),
    "clamp": ((0.55, 0.55, 0.55), (0.3, 0.3, 0.3), 1.0, 0.4, 0.15, 0.0, 0.4, 0.0, 0.0),
    "yellow": ((0.80, 0.62, 0.08), (0.5, 0.35, 0.05), 0.0, 0.45, 0.1, 0.0, 0.0, 0.1, 0.0),
    "gearbox": ((0.07, 0.07, 0.07), (0.03, 0.03, 0.03), 0.4, 0.55, 0.3, 0.1, 0.3, 0.4, 0.0),
}


def node(tree, kind, **inputs):
    n = tree.nodes.new(kind)
    for k, v in inputs.items():
        n.inputs[k].default_value = v
    return n


def make_material(name):
    base, tint, metal, rough, grime_amt, streak_amt, wear_amt, bump_amt, heat = MATS[name]
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    tree = mat.node_tree
    links = tree.links
    bsdf = tree.nodes.get("Principled BSDF")
    coord = tree.nodes.new("ShaderNodeTexCoord")
    sep = tree.nodes.new("ShaderNodeSeparateXYZ")
    links.new(coord.outputs["Object"], sep.inputs["Vector"])
    ao = node(tree, "ShaderNodeAmbientOcclusion", Distance=0.03)
    ao.samples = 16
    cavity = node(tree, "ShaderNodeMapRange", **{"From Min": 0.3, "From Max": 1.0, "To Min": 1.0, "To Max": 0.0})
    links.new(ao.outputs["AO"], cavity.inputs["Value"])
    blotch = node(tree, "ShaderNodeTexNoise", Scale=7.0, Detail=8.0, Roughness=0.6)
    links.new(coord.outputs["Object"], blotch.inputs["Vector"])
    fine = node(tree, "ShaderNodeTexNoise", Scale=140.0, Detail=4.0, Roughness=0.6)
    links.new(coord.outputs["Object"], fine.inputs["Vector"])
    smap = tree.nodes.new("ShaderNodeMapping")
    smap.inputs["Scale"].default_value = (30.0, 2.0, 30.0)
    links.new(coord.outputs["Object"], smap.inputs["Vector"])
    streak = node(tree, "ShaderNodeTexNoise", Scale=3.0, Detail=10.0, Roughness=0.7)
    links.new(smap.outputs["Vector"], streak.inputs["Vector"])

    color = None
    if name == "case":
        ramp = tree.nodes.new("ShaderNodeValToRGB")
        pos = node(tree, "ShaderNodeMapRange", **{"From Min": 2.2, "From Max": 5.0, "To Min": 0.0, "To Max": 1.0})
        neg = node(tree, "ShaderNodeMath")
        neg.operation = "MULTIPLY"
        neg.inputs[1].default_value = -1.0
        links.new(sep.outputs["Y"], neg.inputs[0])
        links.new(neg.outputs["Value"], pos.inputs["Value"])
        links.new(pos.outputs["Result"], ramp.inputs["Fac"])
        el = ramp.color_ramp.elements
        while len(el) < len(CASE_RAMP):
            el.new(0.5)
        for e, (p, c) in zip(el, CASE_RAMP):
            e.position = p
            e.color = c + (1.0,)
        color = ramp.outputs["Color"]
    else:
        flat = node(tree, "ShaderNodeRGB")
        flat.outputs[0].default_value = base + (1.0,)
        color = flat.outputs[0]
    if heat > 0 and name != "case":
        tintmix = node(tree, "ShaderNodeMix")
        tintmix.data_type = "RGBA"
        tintmap = node(tree, "ShaderNodeMapRange", **{"From Min": 0.35, "From Max": 0.7, "To Min": 0.0, "To Max": heat})
        links.new(blotch.outputs["Fac"], tintmap.inputs["Value"])
        links.new(tintmap.outputs["Result"], tintmix.inputs["Factor"])
        links.new(color, tintmix.inputs["A"])
        tintmix.inputs["B"].default_value = tint + (1.0,)
        color = tintmix.outputs["Result"]
    if grime_amt > 0:
        gamt = tree.nodes.new("ShaderNodeMath")
        gamt.operation = "MULTIPLY_ADD"
        links.new(cavity.outputs["Result"], gamt.inputs[0])
        gamt.inputs[1].default_value = 0.8
        links.new(blotch.outputs["Fac"], gamt.inputs[2])
        grime = tree.nodes.new("ShaderNodeMath")
        grime.operation = "MULTIPLY"
        links.new(gamt.outputs["Value"], grime.inputs[0])
        grime.inputs[1].default_value = grime_amt
        dirty = node(tree, "ShaderNodeMix")
        dirty.data_type = "RGBA"
        links.new(grime.outputs["Value"], dirty.inputs["Factor"])
        links.new(color, dirty.inputs["A"])
        dirty.inputs["B"].default_value = (0.045, 0.038, 0.03, 1.0)
        color = dirty.outputs["Result"]
    if streak_amt > 0:
        st = node(tree, "ShaderNodeMix")
        st.data_type = "RGBA"
        st.blend_type = "MULTIPLY"
        sr = node(tree, "ShaderNodeMapRange", **{"From Min": 0.35, "From Max": 0.75, "To Min": 0.0, "To Max": streak_amt})
        links.new(streak.outputs["Fac"], sr.inputs["Value"])
        links.new(sr.outputs["Result"], st.inputs["Factor"])
        links.new(color, st.inputs["A"])
        st.inputs["B"].default_value = (0.55, 0.52, 0.47, 1.0)
        color = st.outputs["Result"]
    if wear_amt > 0 and metal > 0.5:
        edge = tree.nodes.new("ShaderNodeBevel")
        edge.inputs["Radius"].default_value = 0.004
        geo = tree.nodes.new("ShaderNodeNewGeometry")
        dot = tree.nodes.new("ShaderNodeVectorMath")
        dot.operation = "DOT_PRODUCT"
        links.new(edge.outputs["Normal"], dot.inputs[0])
        links.new(geo.outputs["Normal"], dot.inputs[1])
        wr = node(tree, "ShaderNodeMapRange", **{"From Min": 0.97, "From Max": 0.85, "To Min": 0.0, "To Max": wear_amt})
        links.new(dot.outputs["Value"], wr.inputs["Value"])
        worn = node(tree, "ShaderNodeMix")
        worn.data_type = "RGBA"
        links.new(wr.outputs["Result"], worn.inputs["Factor"])
        links.new(color, worn.inputs["A"])
        worn.inputs["B"].default_value = (0.82, 0.8, 0.76, 1.0)
        color = worn.outputs["Result"]
    if name == "label":
        wave = node(tree, "ShaderNodeTexWave", Scale=90.0, Distortion=4.0)
        wave.wave_type = "BANDS"
        wave.bands_direction = "Y"
        links.new(coord.outputs["Object"], wave.inputs["Vector"])
        ink = node(tree, "ShaderNodeMix")
        ink.data_type = "RGBA"
        im = node(tree, "ShaderNodeMapRange", **{"From Min": 0.55, "From Max": 0.7, "To Min": 0.0, "To Max": 0.85})
        links.new(wave.outputs["Fac"], im.inputs["Value"])
        links.new(im.outputs["Result"], ink.inputs["Factor"])
        links.new(color, ink.inputs["A"])
        ink.inputs["B"].default_value = (0.05, 0.05, 0.05, 1.0)
        color = ink.outputs["Result"]
    if name == "duct":
        wave = node(tree, "ShaderNodeTexWave", Scale=9.0, Distortion=0.5)
        wave.wave_type = "BANDS"
        wave.bands_direction = "Z"
        links.new(coord.outputs["Object"], wave.inputs["Vector"])
        seam = node(tree, "ShaderNodeMix")
        seam.data_type = "RGBA"
        sm = node(tree, "ShaderNodeMapRange", **{"From Min": 0.92, "From Max": 1.0, "To Min": 0.0, "To Max": 0.5})
        links.new(wave.outputs["Fac"], sm.inputs["Value"])
        links.new(sm.outputs["Result"], seam.inputs["Factor"])
        links.new(color, seam.inputs["A"])
        seam.inputs["B"].default_value = (0.1, 0.09, 0.08, 1.0)
        color = seam.outputs["Result"]
    links.new(color, bsdf.inputs["Base Color"])
    bsdf.inputs["Metallic"].default_value = metal
    rmix = node(tree, "ShaderNodeMapRange", **{"To Min": rough, "To Max": min(1.0, rough + 0.28)})
    links.new(fine.outputs["Fac"], rmix.inputs["Value"])
    links.new(rmix.outputs["Result"], bsdf.inputs["Roughness"])
    if bump_amt > 0:
        bump = node(tree, "ShaderNodeBump", Strength=bump_amt, Distance=0.002)
        links.new(fine.outputs["Fac"], bump.inputs["Height"])
        links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
    mat["metal"] = metal
    return mat


def bake(objs, kind, image, pass_filter=None):
    mats = {m for o in objs for m in o.data.materials}
    for mat in mats:
        tree = mat.node_tree
        tex = tree.nodes.get("bake_target") or tree.nodes.new("ShaderNodeTexImage")
        tex.name = "bake_target"
        tex.image = image
        tree.nodes.active = tex
    if kind == "EMIT":
        saved = []
        for mat in mats:
            tree = mat.node_tree
            out = tree.nodes.get("Material Output")
            em = tree.nodes.new("ShaderNodeEmission")
            em.inputs["Color"].default_value = (mat["metal"],) * 3 + (1,)
            saved.append((mat, out.inputs["Surface"].links[0].from_socket, em))
            tree.links.new(em.outputs["Emission"], out.inputs["Surface"])
        bpy.ops.object.bake(type="EMIT", margin=8)
        for mat, sock, em in saved:
            mat.node_tree.links.new(sock, mat.node_tree.nodes.get("Material Output").inputs["Surface"])
            mat.node_tree.nodes.remove(em)
        return
    kw = {"type": kind, "margin": 8}
    if pass_filter:
        kw["pass_filter"] = pass_filter
    bpy.ops.object.bake(**kw)


def new_image(name, colorspace):
    img = bpy.data.images.new(name, SIZE, SIZE, alpha=False, float_buffer=False)
    img.colorspace_settings.name = colorspace
    return img


def main():
    os.makedirs(OUT, exist_ok=True)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.wm.obj_import(filepath=OBJ)
    made = {}
    parts, ctx = [], []
    for o in list(bpy.data.objects):
        if o.type != "MESH":
            continue
        mat_name = o.name.split("__")[0] if not o.name.startswith("ctx__") else o.name.split("__")[1]
        if mat_name not in made:
            made[mat_name] = make_material(mat_name)
        o.data.materials.clear()
        o.data.materials.append(made[mat_name])
        for p in o.data.polygons:
            p.use_smooth = True
        (ctx if o.name.startswith("ctx__") else parts).append(o)
    bpy.ops.object.select_all(action="DESELECT")
    for o in parts:
        o.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=1.15, island_margin=0.002, scale_to_bounds=False)
    bpy.ops.uv.select_all(action="SELECT")
    bpy.ops.uv.average_islands_scale()
    bpy.ops.uv.pack_islands(rotate=True, margin=0.0015, shape_method="CONCAVE")
    bpy.ops.object.mode_set(mode="OBJECT")
    uv = {}
    for o in parts:
        layer = o.data.uv_layers.active.data
        arr = np.array([c.uv[:] for c in layer], dtype=np.float32).reshape(-1, 3, 2)
        uv[o.name.split("__")[1]] = arr
    np.savez(os.path.join(OUT, "uv.npz"), **uv)
    s = bpy.context.scene
    world = bpy.data.worlds.new("w")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs["Strength"].default_value = 1.0
    try:
        world.light_settings.distance = 0.25
    except Exception:
        pass
    s.world = world
    s.render.engine = "CYCLES"
    s.cycles.device = "CPU"
    s.cycles.samples = 24
    s.render.bake.use_selected_to_active = False
    col = new_image("col", "sRGB")
    mats = list(made.values())
    for m in mats:
        m.node_tree.nodes.get("Principled BSDF").inputs["Metallic"].default_value = 0.0
    bake(parts, "DIFFUSE", col, {"COLOR"})
    for m in mats:
        m.node_tree.nodes.get("Principled BSDF").inputs["Metallic"].default_value = m["metal"]
    ao = new_image("ao", "Non-Color")
    s.cycles.samples = 48
    bake(parts, "AO", ao)
    s.cycles.samples = 8
    nrm = new_image("nrm", "Non-Color")
    bake(parts, "NORMAL", nrm)
    rough = new_image("rough", "Non-Color")
    bake(parts, "ROUGHNESS", rough)
    metal = new_image("metal", "Non-Color")
    bake(parts, "EMIT", metal)
    for img, name in ((col, "col"), (ao, "ao"), (nrm, "nrm"), (rough, "rough"), (metal, "metal")):
        img.filepath_raw = os.path.join(OUT, f"{STEM}_{name}_raw.png")
        img.file_format = "PNG"
        img.save()
    print("BAKED", STEM, SIZE, len(parts), "parts", len(ctx), "context copies")


main()
