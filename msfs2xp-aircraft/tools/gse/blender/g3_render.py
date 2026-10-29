import json
import os
import sys

BLENDER_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, BLENDER_DIR)
sys.path.insert(0, os.path.dirname(BLENDER_DIR))

import bpy
import numpy as np
from mathutils import Vector

import g3_obj8 as ob
import gselib as g

VIEW_DIRS = {'front_right': (0.80, 0.65, 0.38), 'rear_left': (-0.80, -0.65, 0.38), 'side_right': (1.0, 0.0, 0.12)}


def build_mesh(path, values, scene):
    obj = ob.parse(path)
    bs, _ = ob.batches(obj)
    pos = obj['vt'][:, :3]
    nrm = obj['vt'][:, 3:6]
    uvs = obj['vt'][:, 6:8]
    verts, norms, faces, uvl = [], [], [], []
    for b in bs:
        if b['lod'] not in (0, 1):
            continue
        ids = obj['idx'][b['start']:b['start'] + b['count']].reshape(-1, 3)
        m = ob.anim_matrix(b['anim'], values)
        used = np.unique(ids)
        remap = {int(u): len(verts) + i for i, u in enumerate(used)}
        p = np.ones((len(used), 4))
        p[:, :3] = pos[used]
        q = (m @ p.T).T[:, :3]
        n = (m[:3, :3] @ nrm[used].T).T
        for i, u in enumerate(used):
            verts.append((q[i][0], -q[i][2], q[i][1]))
            norms.append((n[i][0], -n[i][2], n[i][1]))
            uvl.append(uvs[u])
        for t in ids:
            faces.append(tuple(remap[int(x)] for x in t))
    me = bpy.data.meshes.new('chk')
    me.from_pydata(verts, [], faces)
    uv = me.uv_layers.new(name='UVMap')
    loop_v = np.array([l.vertex_index for l in me.loops])
    uv.data.foreach_set('uv', np.array(uvl)[loop_v].ravel())
    me.shade_smooth()
    me.normals_split_custom_set_from_vertices(norms)
    o = bpy.data.objects.new('chk', me)
    scene.collection.objects.link(o)
    base = os.path.dirname(path)
    tex = {}
    for line in obj['header']:
        w = line.split()
        if w[0] == 'TEXTURE':
            tex['albedo'] = w[1]
        elif w[0] == 'TEXTURE_MAP':
            tex[w[1]] = w[2]
    m = bpy.data.materials.new('chk')
    m.use_nodes = True
    nt = m.node_tree
    p = nt.nodes.get('Principled BSDF')
    alb = nt.nodes.new('ShaderNodeTexImage')
    alb.image = bpy.data.images.load(os.path.join(base, tex['albedo']), check_existing=False)
    nt.links.new(alb.outputs['Color'], p.inputs['Base Color'])
    mat = nt.nodes.new('ShaderNodeTexImage')
    mat.image = bpy.data.images.load(os.path.join(base, tex['material_gloss']), check_existing=False)
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
    return np.array(verts)


def main():
    argv = sys.argv[sys.argv.index('--') + 1:]
    jobs = json.loads(open(argv[0], encoding='utf-8').read())
    for job in jobs:
        g.reset_scene()
        g._materials.clear()
        scene = bpy.context.scene
        g.setup_render(scene, job.get('w', 900), job.get('h', 600), samples=job.get('samples', 24))
        verts = build_mesh(job['obj'], job.get('values', {}), scene)
        lo, hi = verts.min(axis=0), verts.max(axis=0)
        c = Vector(((lo + hi) / 2).tolist())
        diag = float(np.linalg.norm(hi - lo))
        d = Vector(VIEW_DIRS[job.get('view', 'front_right')]).normalized()
        cam = g.camera(scene, 'cam', c + d * diag * job.get('dist', 1.15), c - Vector((0, 0, diag * 0.04)), lens=job.get('lens', 35.0))
        g.render(scene, cam, job['png'])
        print('G3R', job['png'], flush=True)


main()
