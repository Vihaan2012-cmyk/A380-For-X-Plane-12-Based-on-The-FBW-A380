import json
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import bpy
import numpy as np
from mathutils import Vector

import g4_obj

EXPORT = 'E:/fbw-gse/export'
RES = (900, 560)


def clear():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def make_scene():
    scene = bpy.context.scene
    scene.render.engine = 'BLENDER_WORKBENCH'
    scene.render.resolution_x, scene.render.resolution_y = RES
    scene.display.shading.light = 'STUDIO'
    scene.display.shading.color_type = 'TEXTURE'
    scene.display.shading.show_shadows = True
    scene.display.shading.shadow_intensity = 0.35
    scene.display.shading.show_cavity = False
    scene.display.shading.show_object_outline = False
    scene.view_settings.view_transform = 'Standard'
    scene.world = bpy.data.worlds.new('W')
    scene.world.color = (0.62, 0.72, 0.85)
    me = bpy.data.meshes.new('ground')
    s = 40.0
    me.from_pydata([(-s, -s, -0.002), (s, -s, -0.002), (s, s, -0.002), (-s, s, -0.002)], [], [(0, 1, 2, 3)])
    ob = bpy.data.objects.new('ground', me)
    mat = bpy.data.materials.new('gm')
    mat.diffuse_color = (0.42, 0.42, 0.4, 1)
    me.materials.append(mat)
    scene.collection.objects.link(ob)
    return scene


def add_vehicle(o, vals, png, lod=0):
    tris = g4_obj.posed_triangles(o, vals, lod)
    pts = np.concatenate([t[0].reshape(-1, 3) for t in tris])
    nrm = np.concatenate([t[1].reshape(-1, 3) for t in tris])
    uv = np.concatenate([t[2].reshape(-1, 2) for t in tris])
    bl = np.stack([pts[:, 0], -pts[:, 2], pts[:, 1]], axis=1)
    bn = np.stack([nrm[:, 0], -nrm[:, 2], nrm[:, 1]], axis=1)
    n = len(bl) // 3
    me = bpy.data.meshes.new('veh')
    me.vertices.add(len(bl))
    me.vertices.foreach_set('co', bl.astype(np.float32).ravel())
    me.loops.add(len(bl))
    me.loops.foreach_set('vertex_index', np.arange(len(bl), dtype=np.int32))
    me.polygons.add(n)
    me.polygons.foreach_set('loop_start', np.arange(0, len(bl), 3, dtype=np.int32))
    me.polygons.foreach_set('loop_total', np.full(n, 3, dtype=np.int32))
    me.update(calc_edges=True)
    layer = me.uv_layers.new(name='UVMap')
    layer.data.foreach_set('uv', uv.astype(np.float32).ravel())
    ln = bn / np.maximum(np.linalg.norm(bn, axis=1, keepdims=True), 1e-9)
    try:
        me.normals_split_custom_set(ln.tolist())
    except Exception:
        pass
    mat = bpy.data.materials.new('vm')
    mat.use_nodes = True
    nt = mat.node_tree
    tex = nt.nodes.new('ShaderNodeTexImage')
    tex.image = bpy.data.images.load(png)
    nt.links.new(tex.outputs['Color'], nt.nodes['Principled BSDF'].inputs['Base Color'])
    nt.nodes.active = tex
    me.materials.append(mat)
    ob = bpy.data.objects.new('veh', me)
    bpy.context.scene.collection.objects.link(ob)
    return ob, bl


def shoot(scene, name, loc, target, path, lens=32):
    cam_data = bpy.data.cameras.new(name)
    cam_data.lens = lens
    cam = bpy.data.objects.new(name, cam_data)
    cam.location = Vector(loc)
    cam.rotation_euler = (Vector(target) - Vector(loc)).to_track_quat('-Z', 'Y').to_euler()
    scene.collection.objects.link(cam)
    scene.camera = cam
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)


def views(bb_lo, bb_hi, front_edge_h):
    c = (bb_lo + bb_hi) / 2
    size = bb_hi - bb_lo
    d = max(size.x, size.y, size.z) * 1.35
    cx, cy = c.x, c.y
    ch = size.z * 0.42
    v = {
        'front34': ((cx - d * 0.75, cy + d * 0.85, ch + size.z * 0.25), (cx, cy, ch)),
        'rear34': ((cx + d * 0.75, cy - d * 0.9, ch + size.z * 0.25), (cx, cy, ch)),
        'side': ((cx + d * 1.35, cy, ch), (cx, cy, ch)),
    }
    if front_edge_h:
        h = front_edge_h
        v['door'] = ((3.6, 5.0, h + 1.4), (0.0, -1.2, h - 0.4))
    return v


def main():
    argv = sys.argv[sys.argv.index('--') + 1:]
    name = argv[0]
    what = argv[1].split(',') if len(argv) > 1 else ['min', 'max']
    out = os.path.join(EXPORT, name)
    rend = os.path.join(out, 'renders')
    os.makedirs(rend, exist_ok=True)
    manifest = json.load(open(os.path.join(out, 'manifest.json')))
    o = g4_obj.parse(os.path.join(out, manifest['objects']['default']))
    drs = [a['dataref'] for a in manifest['anims']]
    poses = {'min': {}, 'max': {drs[0]: 1.0}, 'maxbridge': {d: 1.0 for d in drs}}
    h = manifest['anims'][0]['platform_height_at_min_m']
    for pose in [w for w in what if w in poses]:
        clear()
        scene = make_scene()
        ob, bl = add_vehicle(o, poses[pose], os.path.join(out, f'{name}_ALB.png'))
        lo, hi = Vector(bl.min(axis=0)), Vector(bl.max(axis=0))
        hh = h + (manifest['anims'][0]['platform_height_at_max_m'] - h) * poses[pose].get(drs[0], 0.0)
        fe = manifest['attach']['platform_front_edge']
        for vn, (loc, tgt) in views(lo, hi, hh).items():
            shoot(scene, vn, loc, tgt, os.path.join(rend, f'{name}_{pose}_{vn}.png'))
    for op in [w for w in what if w in ('dnata', 'swissport', 'menzies')]:
        clear()
        scene = make_scene()
        ob, bl = add_vehicle(o, {}, os.path.join(out, f'{name}_{op}_ALB.png'))
        lo, hi = Vector(bl.min(axis=0)), Vector(bl.max(axis=0))
        for vn, (loc, tgt) in views(lo, hi, None).items():
            if vn in ('front34', 'side'):
                shoot(scene, vn, loc, tgt, os.path.join(rend, f'{name}_{op}_{vn}.png'))


main()
