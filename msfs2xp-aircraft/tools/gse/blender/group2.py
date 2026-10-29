import importlib
import json
import math
import os
import shutil
import subprocess
import sys

BLENDER_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, BLENDER_DIR)

import bpy
import numpy as np
from mathutils import Matrix, Vector

import bake as bk
import export_obj8 as ex
import gselib as g
import liverylib as L

EXPORT = 'E:/fbw-gse/export'
OBJDECIMATE = 'E:/fbw-gse/tools/objdecimate.exe'
LOD_ARGS = ['0:80:0.0008:keep', '80:250:0.02:prune', '250:1500:0.06:sloppy:0.12']
AXES = 'X-Plane object axes in metres: x right, y up, z towards the rear (the front is -Z)'

SPECS = {
    'fuel_tanker': dict(
        origin='ground point on the centreline, 4.70 m ahead of the rear end of the rear cabinet (between the tag and front axles)',
        anims=[
            ('fbw/gse/fuel_tanker/deck', 'elevating operator deck, with the hose and coupling riding on it', 'translates up 0 to 1.5 m'),
            ('fbw/gse/fuel_tanker/hose', 'hose and coupling swing off the deck towards the aircraft', 'translates 0.9 m aft and 0.55 m down (on top of the deck lift)'),
        ],
        attach=[
            ('hose_coupling_stowed', 'hose_coupling_body', 'top', {}),
            ('hose_coupling_deployed', 'hose_coupling_body', 'top', {'fbw/gse/fuel_tanker/deck': 1, 'fbw/gse/fuel_tanker/hose': 1}),
            ('deck_top_stowed', 'deck_floor', 'top', {}),
            ('deck_top_raised', 'deck_floor', 'top', {'fbw/gse/fuel_tanker/deck': 1}),
            ('bottom_load_port_0', 'bottom_load_port_0', 'maxX', {}),
            ('bottom_load_port_1', 'bottom_load_port_1', 'maxX', {}),
            ('bottom_load_port_2', 'bottom_load_port_2', 'maxX', {}),
            ('bottom_load_port_3', 'bottom_load_port_3', 'maxX', {}),
            ('earth_reel', 'earth_reel', 'center', {}),
        ],
    ),
    'water_truck': dict(
        origin='ground point on the centreline, 3.30 m ahead of the rear end of the chassis frame (the platform and hose reach about 1.8 m behind it)',
        anims=[
            ('fbw/gse/water/platform', 'scissor lift platform at the rear, with rails and control post', 'translates up 0 to 2.3 m'),
            ('fbw/gse/water/hose', 'service hose and coupling pulled away from the truck', 'translates 0.6 m to the left (-X)'),
        ],
        attach=[
            ('platform_deck_top_stowed', 'platform_deck', 'top', {}),
            ('platform_deck_top_raised', 'platform_deck', 'top', {'fbw/gse/water/platform': 1}),
            ('hose_coupling_stowed', 'hose_cap', 'minY', {}),
            ('hose_coupling_deployed', 'hose_cap', 'minY', {'fbw/gse/water/hose': 1}),
            ('tank_fill_manway_top', 'manway_lid', 'top', {}),
        ],
    ),
    'lav_truck': dict(
        origin='ground point on the centreline, 2.80 m ahead of the rear bumper plane (between the rear and front axles)',
        anims=[
            ('fbw/gse/lav/platform', 'rear scissor platform with rails, control pod, dock pad and legs', 'translates up 0 to 1.15 m'),
            ('fbw/gse/lav/hose', 'service hose and coupling raised from the tray', 'translates up 0 to 2.2 m'),
        ],
        attach=[
            ('platform_floor_top_stowed', 'platform_floor', 'top', {}),
            ('platform_floor_top_raised', 'platform_floor', 'top', {'fbw/gse/lav/platform': 1}),
            ('platform_dock_pad_stowed', 'platform_dock_pad', 'minY', {}),
            ('platform_dock_pad_raised', 'platform_dock_pad', 'minY', {'fbw/gse/lav/platform': 1}),
            ('hose_coupling_stowed', 'hose_cap', 'minY', {}),
            ('hose_coupling_raised', 'hose_cap', 'minY', {'fbw/gse/lav/hose': 1}),
        ],
    ),
    'clean_van': dict(
        origin='ground point on the centreline at the rear bumper face (the rear step protrudes 0.09 m behind it)',
        anims=[
            ('fbw/gse/clean_van/door', 'sliding cargo door on the right side with its window, handle and trim', 'translates 1.2 m towards the rear'),
        ],
        attach=[
            ('sliding_door_centre_closed', 'sliding_door', 'center', {}),
            ('sliding_door_centre_open', 'sliding_door', 'center', {'fbw/gse/clean_van/door': 1}),
            ('doorway_sill_centre', 'sliding_door', 'bottom', {}),
        ],
    ),
    'deice_truck': dict(
        origin='ground point on the centreline, 4.70 m ahead of the rear bumper (between the rear axles and the front axle)',
        anims=[
            ('fbw/gse/deice/boom', 'boom raise about the hinge on the turntable (axis +X, pivot in attach.boom_hinge); the sign in X-Plane is not verified in the sim', 'rotates 0 to 72 degrees nose up'),
            ('fbw/gse/deice/extend', 'telescoping travel along the boom, applied before the raise: middle section 0 to 3.6 m, top section, basket mount and basket 0 to 7.2 m', 'translates along the boom'),
            ('fbw/gse/deice/basket', 'basket yaw about its vertical pivot; 0.5 is straight ahead along the boom', 'rotates -90 to +90 degrees'),
        ],
        attach=[
            ('boom_hinge', 'boom_lower', 'pivot', {}),
            ('basket_floor_stowed', 'basket_floor_grid', 'top', {'fbw/gse/deice/basket': 0.5}),
            ('basket_floor_raised', 'basket_floor_grid', 'top', {'fbw/gse/deice/boom': 1, 'fbw/gse/deice/extend': 1, 'fbw/gse/deice/basket': 0.5}),
            ('nozzle_tip_stowed', 'nozzle_tip', 'maxY', {'fbw/gse/deice/basket': 0.5}),
            ('nozzle_tip_raised', 'nozzle_tip', 'maxY', {'fbw/gse/deice/boom': 1, 'fbw/gse/deice/extend': 1, 'fbw/gse/deice/basket': 0.5}),
        ],
    ),
}


def build(model, op):
    os.environ['GSE_OPERATOR'] = op
    g._materials.clear()
    model.build()
    for m in bpy.data.materials:
        if m.use_nodes and m.node_tree.nodes.get('Principled BSDF'):
            m.node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 0.0
    return bpy.data.collections[model.NAME]


def world_bbox(obj):
    pts = [obj.matrix_world @ Vector(c) for c in obj.bound_box]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def anchor_point(obj, anchor):
    lo, hi = world_bbox(obj)
    c = (lo + hi) / 2
    if anchor == 'pivot':
        return Vector(json.loads(obj['gse_anim'])[0]['pivot'])
    return {
        'center': c,
        'top': Vector((c.x, c.y, hi.z)),
        'bottom': Vector((c.x, c.y, lo.z)),
        'minY': Vector((c.x, lo.y, c.z)),
        'maxY': Vector((c.x, hi.y, c.z)),
        'maxX': Vector((hi.x, c.y, c.z)),
    }[anchor]


def posed(obj, p, values):
    steps = json.loads(obj['gse_anim']) if obj.get('gse_anim') else []
    for st in reversed(steps):
        v = values.get(st['dataref'], 0.0)
        span = st['v1'] - st['v0']
        t = min(max((v - st['v0']) / span, 0.0), 1.0) if span else 0.0
        if st['kind'] == 'trans':
            p = p + Vector(st['offset0']).lerp(Vector(st['offset1']), t)
        elif st['kind'] == 'rotate':
            pivot = Vector(st['pivot'])
            ang = math.radians(st['deg0'] + (st['deg1'] - st['deg0']) * t)
            p = pivot + Matrix.Rotation(ang, 3, Vector(st['axis']).normalized()) @ (p - pivot)
    return p


def xp_point(p):
    return [round(p.x, 3), round(p.z, 3), round(-p.y, 3)]


def attach_points(spec):
    out = {}
    for name, obj_name, anchor, values in spec['attach']:
        obj = bpy.data.objects[obj_name]
        out[name] = xp_point(posed(obj, anchor_point(obj, anchor), values))
    return out


def front_check(coll):
    wind = bpy.data.objects['windscreen']
    wc = (world_bbox(wind)[0] + world_bbox(wind)[1]) / 2
    pts = [world_bbox(o) for o in coll.all_objects if o.type == 'MESH']
    lo_y = min(b[0].y for b in pts)
    hi_y = max(b[1].y for b in pts)
    return xp_point(wc)[2] < (-(lo_y + hi_y) / 2)


def decimate(path):
    tmp = path + '.lod'
    res = subprocess.run([OBJDECIMATE, path, tmp] + LOD_ARGS, capture_output=True, text=True, check=True)
    os.replace(tmp, path)
    return [l for l in res.stderr.splitlines() if l.startswith('LOD')]


def parse_obj(path):
    vt, idx, lods, cur = [], [], [], None
    textures = {}
    with open(path, encoding='utf-8') as fh:
        for line in fh:
            t = line.strip()
            if t.startswith('VT '):
                vt.append([float(x) for x in t.split()[1:]])
            elif t.startswith('IDX'):
                idx.extend(int(x) for x in t.split()[1:])
            elif t.startswith('TEXTURE_MAP'):
                _, kind, name = t.split()
                textures[kind] = name
            elif t.startswith('TEXTURE '):
                textures['albedo'] = t.split()[1]
            elif t.startswith('ATTR_LOD'):
                cur = []
                lods.append(cur)
            elif t.startswith('TRIS'):
                _, a, b = t.split()
                if cur is None:
                    cur = []
                    lods.append(cur)
                cur.append((int(a), int(b)))
    return np.array(vt, dtype=np.float32), idx, lods, textures


def render_obj(path, png, view_dir):
    vt, idx, lods, textures = parse_obj(path)
    tris = []
    for a, b in lods[0] if lods else [(0, len(idx))]:
        tris.extend(idx[a:a + b])
    tris = np.array(tris, dtype=np.int64).reshape(-1, 3)
    tris = tris[(tris[:, 0] != tris[:, 1]) & (tris[:, 1] != tris[:, 2]) & (tris[:, 0] != tris[:, 2])]
    used = np.unique(tris)
    remap = np.full(len(vt), -1, dtype=np.int64)
    remap[used] = np.arange(len(used))
    v = vt[used]
    to_b = lambda a: np.stack([a[:, 0], -a[:, 2], a[:, 1]], axis=1)
    pos, nrm = to_b(v[:, 0:3]), to_b(v[:, 3:6])
    g.reset_scene()
    g._materials.clear()
    scene = bpy.context.scene
    me = bpy.data.meshes.new('chk')
    me.from_pydata(pos.tolist(), [], remap[tris].tolist())
    uv = me.uv_layers.new(name='UVMap')
    loop_v = np.array([l.vertex_index for l in me.loops])
    uv.data.foreach_set('uv', v[loop_v][:, 6:8].ravel())
    me.normals_split_custom_set_from_vertices(nrm.tolist())
    for poly in me.polygons:
        poly.use_smooth = True
    obj = bpy.data.objects.new('chk', me)
    scene.collection.objects.link(obj)
    folder = os.path.dirname(path)
    m = bpy.data.materials.new('chk')
    m.use_nodes = True
    nt = m.node_tree
    p = nt.nodes.get('Principled BSDF')
    alb = nt.nodes.new('ShaderNodeTexImage')
    alb.image = bpy.data.images.load(os.path.join(folder, textures['albedo']))
    nt.links.new(alb.outputs['Color'], p.inputs['Base Color'])
    mat = nt.nodes.new('ShaderNodeTexImage')
    mat.image = bpy.data.images.load(os.path.join(folder, textures['material_gloss']))
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
    g.setup_render(scene, 960, 600, samples=24)
    lo, hi = pos.min(axis=0), pos.max(axis=0)
    c = Vector(((lo + hi) / 2).tolist())
    diag = float(np.linalg.norm(hi - lo))
    d = Vector(view_dir).normalized()
    cam = g.camera(scene, 'cam', c + d * diag * 1.15, c - Vector((0, 0, diag * 0.04)), lens=35.0)
    g.render(scene, cam, png)


def pixels(path):
    img = bpy.data.images.load(path, check_existing=False)
    a = np.empty(len(img.pixels), dtype=np.float32)
    img.pixels.foreach_get(a)
    bpy.data.images.remove(img)
    return a


def export_vehicle(name):
    model = importlib.import_module(name)
    spec = SPECS[name]
    out = os.path.join(EXPORT, name)
    os.makedirs(out, exist_ok=True)
    tmp = f'E:/fbw-gse/out/{name}/g2_bake'
    size = getattr(model, 'TEXTURE_SIZE', 2048)
    for op in L.OPS:
        coll = build(model, op)
        _, _, paths, _ = bk.bake(coll, tmp, f'{name}_{op}', size=size)
        shutil.copy(paths['albedo'], os.path.join(out, f'{name}_{op}_ALB.png'))
    coll = build(model, 'default')
    target, parts, paths, _ = bk.bake(coll, out, name, size=size)
    variants = {op: os.path.join(out, f'{name}_{op}_ALB.png') for op in L.OPS}
    written, nv, nt = ex.export(target, parts, out, name, paths, variants)
    ref_mat = pixels(paths['material'])
    same_uv = all(np.array_equal(pixels(os.path.join(tmp, f'{name}_{op}_MAT.png')), ref_mat) for op in L.OPS)
    lo = Vector((1e9, 1e9, 1e9))
    hi = Vector((-1e9, -1e9, -1e9))
    for o in coll.all_objects:
        if o.type == 'MESH':
            a, b = world_bbox(o)
            lo = Vector((min(lo.x, a.x), min(lo.y, a.y), min(lo.z, a.z)))
            hi = Vector((max(hi.x, b.x), max(hi.y, b.y), max(hi.z, b.z)))
    attach = attach_points(spec)
    front_ok = front_check(coll)
    lods = {}
    for path in written:
        lods[os.path.basename(path)] = decimate(path)
    objects = {'default': f'{name}.obj', 'dnata': f'{name}_dnata.obj', 'swissport': f'{name}_swissport.obj', 'menzies': f'{name}_menzies.obj'}
    manifest = {
        'vehicle': name,
        'objects': objects,
        'length_m': round(hi.y - lo.y, 2),
        'width_m': round(hi.x - lo.x, 2),
        'height_m': round(hi.z, 2),
        'origin': spec['origin'],
        'front': '-Z',
        'axes': AXES,
        'extent_m': {'x': [round(lo.x, 3), round(hi.x, 3)], 'y': [round(lo.z, 3), round(hi.z, 3)], 'z': [round(-hi.y, 3), round(-lo.y, 3)]},
        'anims': [{'dataref': d, 'meaning': meaning, 'min': 0.0, 'max': 1.0, 'motion': motion} for d, meaning, motion in spec['anims']],
        'attach': attach,
        'lods': [dict(zip(('near_m', 'far_m', 'max_error_m', 'mode'), a.split(':')[:4])) for a in LOD_ARGS],
        'textures': [f'{name}_ALB.png', f'{name}_NML.png', f'{name}_MAT.png'] + [f'{name}_{op}_ALB.png' for op in L.OPS],
    }
    with open(os.path.join(out, 'manifest.json'), 'w', encoding='utf-8', newline='\n') as fh:
        json.dump(manifest, fh, indent=1)
        fh.write('\n')
    print('GSE2', name, 'vertices', nv, 'triangles_before_lod', nt, 'front_ok', front_ok, 'same_uv_all_ops', same_uv)
    for k, v in lods.items():
        print('GSE2', k, v)
    return out, objects


def render_vehicle(name):
    out = os.path.join(EXPORT, name)
    shots = os.path.join(EXPORT, '_checks')
    os.makedirs(shots, exist_ok=True)
    objects = {'default': f'{name}.obj', 'dnata': f'{name}_dnata.obj', 'swissport': f'{name}_swissport.obj', 'menzies': f'{name}_menzies.obj'}
    for op, fn in objects.items():
        render_obj(os.path.join(out, fn), os.path.join(shots, f'{name}_{op}.png'), (0.85, 0.75, 0.40))


def main():
    argv = sys.argv[sys.argv.index('--') + 1:]
    name, mode = argv[0], argv[1]
    if mode in ('export', 'all'):
        export_vehicle(name)
    if mode == 'render':
        render_vehicle(name)


main()
