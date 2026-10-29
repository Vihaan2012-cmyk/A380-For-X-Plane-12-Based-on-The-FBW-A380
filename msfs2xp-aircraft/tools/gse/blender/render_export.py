import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bpy
from mathutils import Vector

import gselib as g


def parse_lod0(path):
    vt, idx, textures, tris = [], [], {}, []
    lod_seen = 0
    lines = open(path, encoding='utf-8').read().splitlines()
    for line in lines:
        t = line.strip()
        if t.startswith('VT '):
            vt.append([float(x) for x in t.split()[1:]])
        elif t.startswith('IDX10') or t.startswith('IDX '):
            idx.extend(int(x) for x in t.split()[1:])
        elif t.startswith('TEXTURE_MAP'):
            _, kind, name = t.split()
            textures[kind] = name
        elif t.startswith('TEXTURE '):
            textures['albedo'] = t.split()[1]
        elif t.startswith('ATTR_LOD'):
            lod_seen += 1
        elif t.startswith('TRIS') and lod_seen <= 1:
            _, a, b = t.split()
            tris.append((int(a), int(b)))
    return vt, idx, textures, tris


def build_mesh(path):
    vt, idx, textures, tris = parse_lod0(path)
    verts = [(v[0], -v[2], v[1]) for v in vt]
    faces = []
    for a, b in tris:
        for i in range(a, a + b, 3):
            faces.append(tuple(idx[i:i + 3]))
    me = bpy.data.meshes.new('export')
    me.from_pydata(verts, [], faces)
    uv = me.uv_layers.new(name='UVMap')
    for loop in me.loops:
        v = vt[loop.vertex_index]
        uv.data[loop.index].uv = (v[6], v[7])
    me.shade_smooth()
    me.normals_split_custom_set_from_vertices([(v[3], -v[5], v[4]) for v in vt])
    obj = bpy.data.objects.new('export', me)
    bpy.context.scene.collection.objects.link(obj)
    base = os.path.dirname(path)
    m = bpy.data.materials.new('export')
    m.use_nodes = True
    nt = m.node_tree
    p = nt.nodes.get('Principled BSDF')
    alb = nt.nodes.new('ShaderNodeTexImage')
    alb.image = bpy.data.images.load(os.path.join(base, textures['albedo']), check_existing=False)
    alb.image.colorspace_settings.name = 'sRGB'
    nt.links.new(alb.outputs['Color'], p.inputs['Base Color'])
    if 'material_gloss' in textures:
        mat = nt.nodes.new('ShaderNodeTexImage')
        mat.image = bpy.data.images.load(os.path.join(base, textures['material_gloss']), check_existing=False)
        mat.image.colorspace_settings.name = 'Non-Color'
        sep = nt.nodes.new('ShaderNodeSeparateColor')
        inv = nt.nodes.new('ShaderNodeMath')
        inv.operation = 'SUBTRACT'
        inv.inputs[0].default_value = 1.0
        nt.links.new(mat.outputs['Color'], sep.inputs['Color'])
        nt.links.new(sep.outputs['Red'], p.inputs['Metallic'])
        nt.links.new(sep.outputs['Green'], inv.inputs[1])
        nt.links.new(inv.outputs['Value'], p.inputs['Roughness'])
    me.materials.append(m)
    return obj


def bounds(obj):
    pts = [obj.matrix_world @ Vector(c) for c in obj.bound_box]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def main():
    argv = sys.argv[sys.argv.index('--') + 1:]
    out_png, obj_path = argv[0], argv[1]
    view = argv[2] if len(argv) > 2 else 'front_right'
    scene = bpy.context.scene
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o)
    g.setup_render(scene, 960, 640, samples=int(os.environ.get('GSE_SAMPLES', '32')))
    obj = build_mesh(obj_path)
    lo, hi = bounds(obj)
    c = (lo + hi) / 2
    size = max(hi.x - lo.x, hi.y - lo.y)
    d = size * 1.25 + 2.0
    sign = {'front_right': (1, 1), 'rear_left': (-1, -1), 'front_left': (-1, 1), 'rear_right': (1, -1)}[view]
    loc = (c.x + sign[0] * d * 0.75, c.y + sign[1] * d * 0.75, (hi.z - lo.z) * 0.8 + 1.0)
    cam = g.camera(scene, 'cam', loc, (c.x, c.y, (hi.z - lo.z) * 0.38), lens=32.0)
    g.render(scene, cam, out_png)


main()
