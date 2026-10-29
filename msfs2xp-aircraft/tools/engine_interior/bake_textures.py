"""Blender script: unwrap a built engine part and bake its X-Plane textures.

    blender -b <part.blend> -P bake_textures.py -- <out_dir> <stem> [size]

Writes <stem>_ALB.png (colour with grime and ambient occlusion),
<stem>_NML.png (tangent normals, red/green, blue unused as X-Plane reads it)
and <stem>_MAT.png (red metalness, green gloss), plus <stem>.obj with the
baked UVs. Materials keep the names the dressing scripts give them; each gets
a layered shader here: base colour, cavity grime from ambient occlusion,
streaks along the airflow (Y), worn edges on metal, cast grain or braid."""

import os
import sys

import bpy

ARGS = sys.argv[sys.argv.index("--") + 1:]
OUT, STEM = ARGS[0], ARGS[1]
SIZE = int(ARGS[2]) if len(ARGS) > 2 else 2048

GRAIN = {"casting": 2600.0, "case": 1800.0}
BRAID = {"harness"}
STREAKS = {"case", "casting", "stainless", "titanium"}


def node(tree, kind, **inputs):
    n = tree.nodes.new(kind)
    for k, v in inputs.items():
        n.inputs[k].default_value = v
    return n


def dress_material(mat):
    tree = mat.node_tree
    bsdf = tree.nodes.get("Principled BSDF")
    base = tuple(bsdf.inputs["Base Color"].default_value)
    metal = bsdf.inputs["Metallic"].default_value
    rough = bsdf.inputs["Roughness"].default_value
    links = tree.links
    coord = tree.nodes.new("ShaderNodeTexCoord")
    ao = node(tree, "ShaderNodeAmbientOcclusion", Distance=0.04)
    ao.samples = 16
    cavity = node(tree, "ShaderNodeMapRange", **{"From Min": 0.35, "From Max": 1.0, "To Min": 1.0, "To Max": 0.0})
    links.new(ao.outputs["AO"], cavity.inputs["Value"])
    blotch = node(tree, "ShaderNodeTexNoise", Scale=9.0, Detail=8.0, Roughness=0.6)
    links.new(coord.outputs["Object"], blotch.inputs["Vector"])
    streak_map = tree.nodes.new("ShaderNodeMapping")
    streak_map.inputs["Scale"].default_value = (40.0, 2.5, 40.0)
    links.new(coord.outputs["Object"], streak_map.inputs["Vector"])
    streak = node(tree, "ShaderNodeTexNoise", Scale=3.0, Detail=10.0, Roughness=0.7)
    links.new(streak_map.outputs["Vector"], streak.inputs["Vector"])
    dirt_amount = tree.nodes.new("ShaderNodeMath")
    dirt_amount.operation = "MULTIPLY_ADD"
    links.new(cavity.outputs["Result"], dirt_amount.inputs[0])
    dirt_amount.inputs[1].default_value = 0.75
    links.new(blotch.outputs["Fac"], dirt_amount.inputs[2])
    grime = tree.nodes.new("ShaderNodeMath")
    grime.operation = "MULTIPLY"
    links.new(dirt_amount.outputs["Value"], grime.inputs[0])
    grime.inputs[1].default_value = {"case": 0.5, "casting": 0.45, "stainless": 0.18, "titanium": 0.25, "firesleeve": 0.3, "clamp": 0.3}.get(mat.name, 0.3)
    dirty = node(tree, "ShaderNodeMix")
    dirty.data_type = "RGBA"
    dirty.inputs["A"].default_value = base
    dirty.inputs["B"].default_value = (0.09, 0.075, 0.06, 1)
    links.new(grime.outputs["Value"], dirty.inputs["Factor"])
    color = dirty.outputs["Result"]
    if mat.name in STREAKS:
        streaked = node(tree, "ShaderNodeMix")
        streaked.data_type = "RGBA"
        streaked.blend_type = "MULTIPLY"
        streak_ramp = node(tree, "ShaderNodeMapRange", **{"From Min": 0.35, "From Max": 0.75, "To Min": 0.0, "To Max": 0.35})
        links.new(streak.outputs["Fac"], streak_ramp.inputs["Value"])
        links.new(streak_ramp.outputs["Result"], streaked.inputs["Factor"])
        links.new(color, streaked.inputs["A"])
        streaked.inputs["B"].default_value = (0.55, 0.5, 0.44, 1)
        color = streaked.outputs["Result"]
    if metal > 0.5:
        edge = tree.nodes.new("ShaderNodeBevel")
        edge.inputs["Radius"].default_value = 0.004
        geo = tree.nodes.new("ShaderNodeNewGeometry")
        dot = tree.nodes.new("ShaderNodeVectorMath")
        dot.operation = "DOT_PRODUCT"
        links.new(edge.outputs["Normal"], dot.inputs[0])
        links.new(geo.outputs["Normal"], dot.inputs[1])
        wear = node(tree, "ShaderNodeMapRange", **{"From Min": 0.97, "From Max": 0.85, "To Min": 0.0, "To Max": 0.5})
        links.new(dot.outputs["Value"], wear.inputs["Value"])
        worn = node(tree, "ShaderNodeMix")
        worn.data_type = "RGBA"
        links.new(wear.outputs["Result"], worn.inputs["Factor"])
        links.new(color, worn.inputs["A"])
        worn.inputs["B"].default_value = tuple(min(1.0, c * 1.45 + 0.05) for c in base[:3]) + (1,)
        color = worn.outputs["Result"]
    links.new(color, bsdf.inputs["Base Color"])
    rough_mix = node(tree, "ShaderNodeMapRange", **{"To Min": rough, "To Max": min(1.0, rough + 0.3)})
    links.new(grime.outputs["Value"], rough_mix.inputs["Value"])
    links.new(rough_mix.outputs["Result"], bsdf.inputs["Roughness"])
    if mat.name in GRAIN or mat.name in BRAID:
        bump = node(tree, "ShaderNodeBump", Strength=0.35 if mat.name in GRAIN else 0.6, Distance=0.002)
        if mat.name in BRAID:
            wave = node(tree, "ShaderNodeTexWave", Scale=180.0, Distortion=2.0)
            wave.wave_type = "BANDS"
            links.new(coord.outputs["Object"], wave.inputs["Vector"])
            links.new(wave.outputs["Fac"], bump.inputs["Height"])
        else:
            grain = node(tree, "ShaderNodeTexNoise", Scale=GRAIN[mat.name], Detail=2.0)
            links.new(coord.outputs["Object"], grain.inputs["Vector"])
            links.new(grain.outputs["Fac"], bump.inputs["Height"])
        links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
    mat["metal"] = metal


def bake(obj, kind, image, pass_filter=None, emit_from=None):
    for mat in obj.data.materials:
        tree = mat.node_tree
        tex = tree.nodes.get("bake_target") or tree.nodes.new("ShaderNodeTexImage")
        tex.name = "bake_target"
        tex.image = image
        tree.nodes.active = tex
    s = bpy.context.scene
    if kind == "EMIT":
        saved = []
        for mat in obj.data.materials:
            tree = mat.node_tree
            out = tree.nodes.get("Material Output")
            em = tree.nodes.new("ShaderNodeEmission")
            em.inputs["Color"].default_value = (mat["metal"],) * 3 + (1,)
            saved.append((mat, out.inputs["Surface"].links[0].from_socket, em))
            tree.links.new(em.outputs["Emission"], out.inputs["Surface"])
        bpy.ops.object.bake(type="EMIT", margin=6)
        for mat, sock, em in saved:
            mat.node_tree.links.new(sock, mat.node_tree.nodes.get("Material Output").inputs["Surface"])
            mat.node_tree.nodes.remove(em)
        return
    kw = {"type": kind, "margin": 6}
    if pass_filter:
        kw["pass_filter"] = pass_filter
    bpy.ops.object.bake(**kw)


def new_image(name, colorspace):
    img = bpy.data.images.new(name, SIZE, SIZE, alpha=False, float_buffer=False)
    img.colorspace_settings.name = colorspace
    return img


def main():
    os.makedirs(OUT, exist_ok=True)
    obj = next(o for o in bpy.data.objects if o.type == "MESH")
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.select_all(action="SELECT")
    bpy.ops.uv.average_islands_scale()
    bpy.ops.uv.pack_islands(rotate=True, margin=0.0015, shape_method="CONCAVE")
    bpy.ops.object.mode_set(mode="OBJECT")
    for mat in obj.data.materials:
        dress_material(mat)
    s = bpy.context.scene
    s.render.engine = "CYCLES"
    s.cycles.device = "CPU"
    s.cycles.samples = 24
    s.render.bake.use_selected_to_active = False
    col = new_image(f"{STEM}_col", "sRGB")
    for mat in obj.data.materials:
        mat.node_tree.nodes.get("Principled BSDF").inputs["Metallic"].default_value = 0.0
    bake(obj, "DIFFUSE", col, pass_filter={"COLOR"})
    for mat in obj.data.materials:
        mat.node_tree.nodes.get("Principled BSDF").inputs["Metallic"].default_value = mat["metal"]
    ao = new_image(f"{STEM}_ao", "Non-Color")
    s.cycles.samples = 48
    bake(obj, "AO", ao)
    s.cycles.samples = 8
    nrm = new_image(f"{STEM}_nrm", "Non-Color")
    bake(obj, "NORMAL", nrm)
    rough = new_image(f"{STEM}_rough", "Non-Color")
    bake(obj, "ROUGHNESS", rough)
    metal = new_image(f"{STEM}_metal", "Non-Color")
    bake(obj, "EMIT", metal)
    for img, name in ((col, "col"), (ao, "ao"), (nrm, "nrm"), (rough, "rough"), (metal, "metal")):
        img.filepath_raw = os.path.join(OUT, f"{STEM}_{name}_raw.png")
        img.file_format = "PNG"
        img.save()
    bpy.ops.wm.obj_export(filepath=os.path.join(OUT, f"{STEM}.obj"), export_selected_objects=True, export_uv=True, export_normals=True, export_materials=True, forward_axis="Y", up_axis="Z")
    print("BAKED", STEM, SIZE)


main()
