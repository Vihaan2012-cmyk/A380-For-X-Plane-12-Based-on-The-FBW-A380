"""3D paint kit for A380 for X-Plane 12: the exterior with both engine types, the cabin,
the flight deck placards, the IFE look and the cabin mood lighting, in one Blender file.

blender -b --factory-startup --python build_kit.py -- <aircraft folder> <kit folder>
"""
import hashlib
import json
import math
import os
import re
import shutil
import sys

import bmesh
import bpy
import numpy as np
from mathutils import Vector

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
sys.path.insert(0, os.path.join(here, '..', 'paintkit'))
import kit_dds
import kit_overlay

COCKPIT = ['a380_cockpit_011.obj', 'a380_cockpit_082.obj']
IFE_IMAGES = {'ife_logo': (512, 256, (0.0, 0.0, 0.0, 0.0)), 'ife_background': (512, 256, (0.04, 0.09, 0.15, 1.0))}
GP_OBJECT = re.compile(r'^a380_(exterior_\d+|engine_covers)_gp\.obj$')
ALPHA_SHEET = re.compile(r'DECAL|_A\d+\.|MASK\d+\.')
MAX_PAINT = 4096
WHITE_SHARE = 0.5
LINE_COLOR = (0.0, 0.85, 1.0)
ISLAND_COLOR = (1.0, 0.55, 0.0)

aircraft, kit = sys.argv[sys.argv.index('--') + 1:][:2]
objects_dir = os.path.join(aircraft, 'objects')
stored = os.path.join(objects_dir, 'cabin_on')
liveries_dir = os.path.join(aircraft, 'liveries')
tex_dir = os.path.join(kit, 'textures')
paint_dir = os.path.join(tex_dir, 'paint')
base_dir = os.path.join(tex_dir, 'base')
overlay_dir = os.path.join(tex_dir, 'overlay')
ife_dir = os.path.join(tex_dir, 'ife')
for d in (paint_dir, base_dir, overlay_dir, ife_dir):
    os.makedirs(d, exist_ok=True)

bpy.ops.wm.read_factory_settings(use_empty=True)
acf = next(os.path.join(aircraft, f) for f in os.listdir(aircraft) if f.endswith('.acf') and 'Clean' not in f)
acf_objects = re.findall(r'^P _obja/\d+/_v10_att_file_stl (\S+\.obj)', open(acf, encoding='utf-8', errors='replace').read(), re.M)
on_disk = set(os.listdir(objects_dir))


def first_texture(path):
    with open(path, encoding='latin-1') as f:
        for line in f:
            p = line.split()
            if len(p) > 1 and p[0] == 'TEXTURE':
                return p[1]
    return None


liveries = sorted(d for d in os.listdir(liveries_dir) if os.path.isdir(os.path.join(liveries_dir, d, 'objects')))
overrides = {}
for d in liveries:
    folder = os.path.join(liveries_dir, d, 'objects')
    for f in os.listdir(folder):
        if f.lower().endswith('.dds'):
            w, h, _, _ = kit_dds.read_header(os.path.join(folder, f))
            o = overrides.setdefault(f, [0, 0])
            o[0] += 1
            o[1] = max(o[1], w, h)

gp_engine = sorted(n for n in set(acf_objects) | on_disk if GP_OBJECT.match(n) and n[:-7] + '.obj' in on_disk)
trent_engine = [n[:-7] + '.obj' for n in gp_engine]
engine_names = set(gp_engine) | set(trent_engine)
exterior = sorted(n for n in set(acf_objects) if n.startswith('a380_exterior') and n not in engine_names and '_hail' not in n)
cabin = sorted(os.listdir(os.path.join(objects_dir, 'cabin_off')))
cabin += sorted(f for f in on_disk if (f.startswith('a380_cabin_mood_') and f.endswith('_0.obj')) or (f.startswith('a380_cabin_ife_') and f.endswith('.obj')))


def source_of(name):
    full = os.path.join(stored, name)
    return full if os.path.exists(full) else os.path.join(objects_dir, name)


plan = [(n, os.path.join(objects_dir, n), 'exterior') for n in exterior]
plan += [(n, os.path.join(objects_dir, n), 'trent') for n in trent_engine]
plan += [(n, os.path.join(objects_dir, n), 'gp') for n in gp_engine]
plan += [(n, source_of(n), 'cabin') for n in cabin]
plan += [(n, os.path.join(objects_dir, n), 'deck') for n in COCKPIT]
textures_of = {n: first_texture(path) for n, path, _ in plan}

exterior_textures = {textures_of[n] for n, _, group in plan if group in ('exterior', 'trent', 'gp') and textures_of[n]}
interior_textures = {textures_of[n] for n, _, group in plan if group in ('cabin', 'deck') and textures_of[n]}
deck_textures = {textures_of[n] for n, _, group in plan if group == 'deck'}

sheets = {}
for t in sorted(exterior_textures):
    if t in overrides:
        sheets[t] = {'role': 'exterior'}
for t in sorted(interior_textures - set(sheets)):
    if t.lower().endswith('.dds') and not t.startswith('solid_') and os.path.exists(os.path.join(objects_dir, t)):
        sheets[t] = {'role': 'placard' if t in deck_textures else 'cabin'}

total = len(liveries)
bases = {}
for t, s in sheets.items():
    base = kit_dds.read_dds(os.path.join(objects_dir, t))
    bases[t] = base
    h, w = base.shape[:2]
    s['stem'] = os.path.splitext(t)[0]
    s['file'] = t
    s['base_size'] = [w, h]
    s['alpha'] = bool(ALPHA_SHEET.search(t)) or float((base[..., 3] < 200).mean()) > 0.05
    s['overridden'] = overrides.get(t, [0, 0])[0]
    s['white'] = s['role'] == 'exterior' and s['overridden'] >= WHITE_SHARE * total and not s['alpha'] and int(base[..., 3].min()) >= 250
    scale = max(1, min(MAX_PAINT, max(overrides[t][1], w, h)) // max(w, h)) if s['white'] else 1
    s['size'] = [w * scale, h * scale]

print(f'paintable sheet rule: overridden by at least one of {total} installed liveries, used by an exterior object;')
print('cabin and flight deck sheets: every DDS the interior objects use')
for s in sheets.values():
    print(f"  {s['role']:9s} {s['file']:52s} {s['overridden']:3d}/{total} liveries  base {s['base_size'][0]} canvas {s['size'][0]}x{s['size'][1]}" + (' white' if s['white'] else ''))
not_used = sorted(f for f in overrides if f not in sheets)
print('overridden by liveries but not on any kit object:', ', '.join(not_used) or 'none')

images = {}
materials = {}
originals = {}
segments = {}


def to_image(name, rgba8, path):
    h, w = rgba8.shape[:2]
    img = bpy.data.images.new(name, w, h, alpha=True)
    img.alpha_mode = 'STRAIGHT'
    img.pixels.foreach_set((rgba8[::-1].astype(np.float32) / 255.0).ravel())
    img.file_format = 'PNG'
    img.filepath_raw = path
    img.save()
    return img


def paint_state(sheet):
    base = bases[sheet['file']]
    if sheet['role'] != 'exterior':
        originals[sheet['stem']] = hashlib.md5(np.ascontiguousarray(base).tobytes()).hexdigest()
        return base
    shutil.copy2(os.path.join(objects_dir, sheet['file']), os.path.join(base_dir, sheet['file']))
    if sheet['white']:
        return np.full((sheet['size'][1], sheet['size'][0], 4), 255, np.uint8)
    return base


def image_for(texture):
    if texture in images:
        return images[texture]
    sheet = sheets.get(texture)
    if sheet:
        img = to_image(sheet['stem'], paint_state(sheet), os.path.join(paint_dir, sheet['stem'] + '.png'))
        print(f"sheet {sheet['stem']} {img.size[0]}x{img.size[1]}" + (' white' if sheet.get('white') else ''))
    else:
        src = os.path.join(objects_dir, texture)
        if not os.path.exists(src):
            alt = [src[:-4] + e for e in ('.dds', '.png') if os.path.exists(src[:-4] + e)]
            if not alt:
                images[texture] = None
                return None
            src = alt[0]
        dst = os.path.join(tex_dir, os.path.basename(src))
        if not os.path.exists(dst):
            shutil.copy2(src, dst)
        img = bpy.data.images.load(dst)
    images[texture] = img
    return img


def material_for(texture):
    if texture in materials:
        return materials[texture]
    mat = bpy.data.materials.new(os.path.splitext(texture)[0] if texture else 'untextured')
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = nodes['Principled BSDF']
    img = image_for(texture) if texture else None
    if img is not None:
        node = nodes.new('ShaderNodeTexImage')
        node.name = 'KIT_PAINT'
        node.image = img
        node.interpolation = 'Linear'
        links.new(node.outputs['Color'], bsdf.inputs['Base Color'])
        if texture in sheets and sheets[texture]['alpha'] or (texture not in sheets and ALPHA_SHEET.search(texture)):
            links.new(node.outputs['Alpha'], bsdf.inputs['Alpha'])
            try:
                mat.surface_render_method = 'BLENDED'
            except AttributeError:
                mat.blend_method = 'BLEND'
        nodes.active = node
    materials[texture] = mat
    return mat


def collection(name, parent=None):
    c = bpy.data.collections.new(name)
    (parent or bpy.context.scene.collection).children.link(c)
    return c


paint_col = collection('Exterior - paintable')
other_col = collection('Exterior - reference')
trent_col = collection('Engines - Trent 900')
gp_col = collection('Engines - GP7200')
cabin_col = collection('Cabin')
deck_col = collection('Flight deck placards')


def load(path, group):
    vt, idx, ranges, texture, lods = [], [], [], None, 0
    for line in open(path, encoding='latin-1'):
        p = line.split()
        if not p:
            continue
        k = p[0]
        if k == 'VT':
            vt.append([float(x) for x in p[1:9]])
        elif k in ('IDX', 'IDX10'):
            idx.extend(int(x) for x in p[1:])
        elif k == 'TEXTURE' and len(p) > 1:
            texture = p[1]
        elif k == 'ATTR_LOD':
            lods += 1
        elif k == 'TRIS' and lods <= 1:
            ranges.append((int(p[1]), int(p[2])))
    if not ranges:
        return
    name = os.path.basename(path)[:-4]
    vt = np.array(vt, dtype=np.float32)
    idx = np.array(idx, dtype=np.int64)
    tri = np.concatenate([idx[s:s + n] for s, n in ranges]).reshape(-1, 3)[:, [0, 2, 1]].reshape(-1)
    part = np.repeat(np.arange(len(ranges)), [n // 3 for _, n in ranges])
    used, local = np.unique(tri, return_inverse=True)
    local = local.reshape(-1)
    pos = vt[used, 0:3]
    co = np.stack([pos[:, 0], -pos[:, 2], pos[:, 1]], axis=1)
    mesh = bpy.data.meshes.new(name)
    mesh.vertices.add(len(co))
    mesh.vertices.foreach_set('co', co.ravel())
    mesh.loops.add(len(local))
    mesh.loops.foreach_set('vertex_index', local.astype(np.int32))
    mesh.polygons.add(len(local) // 3)
    mesh.polygons.foreach_set('loop_start', np.arange(0, len(local), 3, dtype=np.int32))
    uv = mesh.uv_layers.new(name='UVMap')
    uv.data.foreach_set('uv', vt[used[local], 6:8].ravel())
    mesh.update(calc_edges=True)
    mesh.polygons.foreach_set('use_smooth', np.ones(len(mesh.polygons), dtype=bool))
    if group in ('exterior', 'trent', 'gp'):
        mesh.validate()
        nrm = vt[used, 3:6]
        try:
            mesh.normals_split_custom_set_from_vertices(np.stack([nrm[:, 0], -nrm[:, 2], nrm[:, 1]], axis=1).tolist())
        except (AttributeError, RuntimeError, TypeError):
            pass
    mesh.materials.append(material_for(texture))
    obj = bpy.data.objects.new(name, mesh)
    if group in ('exterior', 'trent', 'gp'):
        target = {'trent': trent_col, 'gp': gp_col}.get(group) or (paint_col if texture in sheets else other_col)
    else:
        target = cabin_col if group == 'cabin' else deck_col
    target.objects.link(obj)
    if 'engine_covers' in name:
        obj.hide_viewport = True
        obj.hide_render = True
    if texture in sheets:
        uvs = vt[used, 6:8]
        local_tri = local.astype(np.int64)
        segments.setdefault(texture, []).append((kit_overlay.panel_segments(pos, uvs, local_tri, part), kit_overlay.island_segments(uvs, local_tri)))
    print(f'{name}: {len(mesh.polygons)} triangles, {texture}')


for name, path, group in plan:
    load(path, group)

lines_nodes = {}
for texture, sheet in sheets.items():
    mat = materials.get(texture)
    if mat is None:
        continue
    w, h = sheet['size']
    thick = max(w, h) >= 4096
    parts = segments.get(texture, [])
    panel = np.concatenate([p for p, _ in parts]) if parts else np.zeros((0, 4))
    island = np.concatenate([i for _, i in parts]) if parts else np.zeros((0, 4))
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = nodes['Principled BSDF']
    paint = nodes['KIT_PAINT']
    colour = paint.outputs['Color']
    for kind, seg, rgb in (('lines', panel, LINE_COLOR), ('islands', island, ISLAND_COLOR)):
        mask = kit_overlay.raster(seg, w, h, thick)
        img = bpy.data.images.new(f"{sheet['stem']}_{kind}", w, h, alpha=True)
        img.alpha_mode = 'STRAIGHT'
        img.pixels.foreach_set(kit_overlay.to_rgba(mask, rgb).ravel())
        img.file_format = 'PNG'
        img.filepath_raw = os.path.join(overlay_dir, f"{sheet['stem']}_{kind}.png")
        img.save()
        img.use_fake_user = True
        tex = nodes.new('ShaderNodeTexImage')
        tex.name = f'KIT_{kind.upper()}'
        tex.image = img
        tex.interpolation = 'Closest'
        on = nodes.new('ShaderNodeValue')
        on.name = f'KIT_{kind.upper()}_ON'
        on.outputs[0].default_value = 0.0
        mul = nodes.new('ShaderNodeMath')
        mul.operation = 'MULTIPLY'
        links.new(tex.outputs['Alpha'], mul.inputs[0])
        links.new(on.outputs[0], mul.inputs[1])
        mix = nodes.new('ShaderNodeMix')
        mix.data_type = 'RGBA'
        links.new(mul.outputs[0], mix.inputs[0])
        links.new(colour, mix.inputs[6])
        links.new(tex.outputs['Color'], mix.inputs[7])
        colour = mix.outputs[2]
        print(f"overlay {sheet['stem']}_{kind}: {len(seg)} segments")
    links.new(colour, bsdf.inputs['Base Color'])
    nodes.active = paint

for name, (w, h, rgba) in IFE_IMAGES.items():
    img = bpy.data.images.new(name, w, h, alpha=True)
    img.pixels.foreach_set(np.tile(np.array(rgba, dtype=np.float32), w * h))
    img.file_format = 'PNG'
    img.filepath_raw = os.path.join(ife_dir, name + '.png')
    img.save()
    img.use_fake_user = True



def vertices(o):
    co = np.empty(len(o.data.vertices) * 3, np.float32)
    o.data.vertices.foreach_get('co', co)
    return co.reshape(-1, 3)[::50]


all_pos = np.concatenate([vertices(o) for o in list(paint_col.objects) + list(other_col.objects) if len(o.data.vertices)])
lo, hi = all_pos.min(axis=0), all_pos.max(axis=0)
mid = (lo + hi) / 2


def arrow(name, direction, origin, length, color):
    bm = bmesh.new()
    shaft = bmesh.ops.create_cone(bm, cap_ends=True, segments=16, radius1=length * 0.04, radius2=length * 0.04, depth=length * 0.7)
    bmesh.ops.translate(bm, verts=shaft['verts'], vec=(0, 0, length * 0.35))
    head = bmesh.ops.create_cone(bm, cap_ends=True, segments=16, radius1=length * 0.1, radius2=0.0, depth=length * 0.3)
    bmesh.ops.translate(bm, verts=head['verts'], vec=(0, 0, length * 0.85))
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    n = mat.node_tree.nodes
    n.clear()
    em = n.new('ShaderNodeEmission')
    em.inputs['Color'].default_value = (*color, 1.0)
    out = n.new('ShaderNodeOutputMaterial')
    mat.node_tree.links.new(em.outputs[0], out.inputs[0])
    mesh.materials.append(mat)
    obj = bpy.data.objects.new(name, mesh)
    obj.location = origin
    obj.rotation_mode = 'QUATERNION'
    obj.rotation_quaternion = Vector((0, 0, 1)).rotation_difference(Vector(direction))
    obj.hide_render = True
    return obj


helpers = collection('Kit helpers')
span = float(hi[1] - lo[1])
fwd = arrow('FWD arrow', (0, 1, 0), (0, hi[1] + span * 0.04, mid[2]), span * 0.18, (0.1, 0.9, 0.2))
up = arrow('UP arrow', (0, 0, 1), (0, mid[1], hi[2] + span * 0.03), span * 0.12, (0.2, 0.5, 1.0))
helpers.objects.link(fwd)
helpers.objects.link(up)
for text, at, size, color in (('FWD', (0, hi[1] + span * 0.04 + span * 0.2, mid[2]), span * 0.04, (0.1, 0.9, 0.2)), ('UP', (0, mid[1], hi[2] + span * 0.03 + span * 0.14), span * 0.04, (0.2, 0.5, 1.0))):
    curve = bpy.data.curves.new(text, 'FONT')
    curve.body = text
    curve.size = size
    curve.align_x = 'CENTER'
    curve.align_y = 'CENTER'
    label = bpy.data.objects.new(text + ' label', curve)
    label.location = at
    label.rotation_euler = (math.radians(90), 0, math.radians(-90))
    mat = bpy.data.materials.new(text + ' label')
    mat.use_nodes = True
    mat.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (*color, 1.0)
    curve.materials.append(mat)
    label.hide_render = True
    helpers.objects.link(label)
helpers.hide_viewport = True
helpers.hide_render = True
gp_col.hide_viewport = True
gp_col.hide_render = True

scripts = os.path.join(kit, 'scripts')
os.makedirs(scripts, exist_ok=True)
shutil.copy2(os.path.join(here, '..', 'paintkit', 'dds.py'), scripts)
for f in ('kit_dds.py', 'kit_panel.py', 'export_livery.py'):
    shutil.copy2(os.path.join(here, f), scripts)
for f in ('HOW TO PAINT.txt', 'NOTICE.txt', 'cabin.json'):
    shutil.copy2(os.path.join(here, f), kit)
os.makedirs(os.path.join(kit, 'liveries'), exist_ok=True)

for s in sheets.values():
    s['path'] = f"textures/paint/{s['stem']}.png"
    s['base'] = f"textures/base/{s['file']}" if s['role'] == 'exterior' else None
info = {'sheets': list(sheets.values()), 'liveries_seen': total, 'engines': {'trent': trent_engine, 'gp': gp_engine}}
bpy.data.texts.new('kit_sheets.json').write(json.dumps(info, indent=1))
bpy.data.texts.new('kit_originals.json').write(json.dumps(originals, indent=1))
bpy.data.texts.load(os.path.join(kit, 'cabin.json'))
bpy.data.texts.load(os.path.join(scripts, 'export_livery.py'))
panel = bpy.data.texts.load(os.path.join(scripts, 'kit_panel.py'))
panel.use_module = True
blend = os.path.join(kit, 'A380 3D Paint Kit.blend')
bpy.ops.wm.save_as_mainfile(filepath=blend)
bpy.ops.file.make_paths_relative()
bpy.ops.wm.save_mainfile(filepath=blend)
print('saved', len(exterior), 'exterior,', len(trent_engine), 'Trent and', len(gp_engine), 'GP7200 engine and', len(cabin) + len(COCKPIT), 'interior objects;', len(sheets), 'paint sheets')
