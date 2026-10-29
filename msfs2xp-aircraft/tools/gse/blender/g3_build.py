import importlib
import json
import math
import os
import subprocess
import sys
import time

BLENDER_DIR = os.path.dirname(os.path.abspath(__file__))
GSE_DIR = os.path.dirname(BLENDER_DIR)
sys.path.insert(0, BLENDER_DIR)
sys.path.insert(0, GSE_DIR)

EXPORT = 'E:/fbw-gse/export'
ART = os.path.join(EXPORT, '_g3', 'art')
os.environ['G3_ART'] = ART

import bpy
import numpy as np
from mathutils import Matrix, Vector

import bake as bk
import export_obj8 as ex
import g3_obj8 as ob
import g3_paint
import gselib as g

OBJDECIMATE = 'E:/fbw-gse/tools/objdecimate.exe'
LOD_ARGS = ['0:80:0.0008:keep', '80:250:0.02:prune', '250:1500:0.06:sloppy:0.12']
AXES = 'X-Plane object axes in metres: x right, y up, z towards the rear (the front is -Z)'
OPS = list(g3_paint.OPS)

CFG = {
    'bag_tractor': dict(
        decals={'decal_bag_hood': 'bag_hood'},
        tint={'gse_bt_accent': 'main'},
        front=('headlight_L', 'hitch_pin'),
        origin='ground point on the centreline, 1.45 m ahead of the rear end (the tow pin is 1.21 m behind it, the front hitch pad 1.51 m ahead of it)',
        anims=[],
        attach=[
            ('rear_tow_pin_top', 'hitch_pin', 'top', {}),
            ('rear_tow_pin_base', 'hitch_pin', 'bottom', {}),
            ('front_hitch_pad', 'front_hitch_pad', 'center', {}),
            ('seat_top', 'seat_base', 'top', {}),
        ],
    ),
    'bulk_cart': dict(
        decals={'decal_cart_curtain': 'cart_curtain'},
        tint={'gse_cart_canvas': 'main', 'gse_cart_canvas_dark': 'dark'},
        front=('tow_eye', 'rear_hitch_pin'),
        origin='ground point on the centreline, 2.00 m ahead of the rear hitch end (between the axles; the drawbar eye is 2.01 m ahead of it, the rear hitch pin 0.04 m behind the rear end plane)',
        anims=[('fbw/gse/bulk_cart/curtain', 'rolled-up curtain bundle and its ties on the right side', 0.0, 1.0, 'translates up 0 to 0.9 m')],
        attach=[
            ('drawbar_tow_eye', 'tow_eye', 'center', {}),
            ('rear_hitch_pin_top', 'rear_hitch_pin', 'top', {}),
            ('deck_top', 'deck', 'top', {}),
            ('curtain_roll_down', 'curtain_roll', 'center', {}),
            ('curtain_roll_up', 'curtain_roll', 'center', {'fbw/gse/bulk_cart/curtain': 1}),
        ],
    ),
    'belt_loader': dict(
        decals={'decal_belt_boom': 'belt_boom', 'decal_belt_chassis': 'belt_chassis'},
        tint={},
        front=('cab_windscreen', 'tow_pintle'),
        origin='ground point on the centreline at the rear end of the chassis (tow pintle 0.15 m behind it, boom hinge 0.70 m ahead of it)',
        anims=[('fbw/gse/belt_loader/boom', 'belt boom elevation about the hinge (axis X at attach.boom_hinge), the lift cylinder follows; the value is the boom angle in degrees', 0.0, 20.0, 'rotates the boom up 0 to 20 degrees; X-Plane rotation sign verified only by the exporter matrix model, not in the sim')],
        attach=[
            ('boom_hinge', 'boom_beam_L', 'hingepoint', {}),
            ('belt_tip_top_min', 'belt_surface', 'frontTop', {}),
            ('belt_tip_top_max', 'belt_surface', 'frontTop', {'fbw/gse/belt_loader/boom': 20}),
            ('tip_bumper_front_min', 'tip_bumper', 'frontCenter', {}),
            ('tip_bumper_front_max', 'tip_bumper', 'frontCenter', {'fbw/gse/belt_loader/boom': 20}),
            ('belt_rear_top_min', 'belt_surface', 'rearTop', {}),
        ],
        sweep=('fbw/gse/belt_loader/boom', 'belt_surface', 'frontTop', [0, 4, 8, 12, 14, 16, 18, 20]),
    ),
    'ldcl_loader': dict(
        decals={'decal_ldcl_side': 'ldcl_side'},
        tint={'gse_rail_red': 'main'},
        front=('bridge_bumper', 'bumper_rear'),
        origin='ground point on the centreline at the rear end of the chassis (the bridge platform front lip is 7.54 m ahead of it)',
        anims=[
            ('fbw/gse/ldcl_loader/rear_platform', 'rear (feeder) platform with rails and scissors; the value is the rise in metres', 0.0, 2.3, 'rises 0 to 2.3 m'),
            ('fbw/gse/ldcl_loader/bridge_platform', 'bridge platform with rails, lip, bumper and scissors; the value is the rise in metres', 0.0, 2.3, 'rises 0 to 2.3 m'),
        ],
        attach=[
            ('bridge_roller_top_min', 'bridge_roller_1_0', 'top', {}),
            ('bridge_roller_top_max', 'bridge_roller_1_0', 'top', {'fbw/gse/ldcl_loader/bridge_platform': 2.3}),
            ('bridge_deck_front_edge_min', 'bridge_deck', 'frontTop', {}),
            ('bridge_deck_front_edge_max', 'bridge_deck', 'frontTop', {'fbw/gse/ldcl_loader/bridge_platform': 2.3}),
            ('bridge_lip_front_min', 'bridge_lip', 'frontTop', {}),
            ('bridge_lip_front_max', 'bridge_lip', 'frontTop', {'fbw/gse/ldcl_loader/bridge_platform': 2.3}),
            ('rear_roller_top_min', 'rear_roller_1_0', 'top', {}),
            ('rear_roller_top_max', 'rear_roller_1_0', 'top', {'fbw/gse/ldcl_loader/rear_platform': 2.3}),
            ('rear_deck_rear_edge_min', 'rear_deck', 'rearTop', {}),
            ('rear_deck_rear_edge_max', 'rear_deck', 'rearTop', {'fbw/gse/ldcl_loader/rear_platform': 2.3}),
        ],
    ),
    'uld_set': dict(
        decals={'decal_dolly_short': 'dolly_short', 'decal_dolly_long': 'dolly_long'},
        tint={'uld_dolly_blue': 'main'},
        front=('ld3dolly_tow_eye', 'ld3dolly_rear_beam'),
        origin='ground point under the rear centreline of the LD3 dolly; the AKE container stands 2.50 m to the right (+X) and the PMC pallet dolly 3.30 m to the left (-X), 0.40 m behind it',
        anims=[],
        attach=[
            ('ld3_dolly_tow_eye', 'ld3dolly_tow_eye', 'center', {}),
            ('ld3_dolly_deck_top', 'ld3dolly_deck_plate', 'top', {}),
            ('ake_floor_top', 'ake_base_deck', 'top', {}),
            ('pmc_dolly_tow_eye', 'pmcdolly_tow_eye', 'center', {}),
            ('pmc_dolly_deck_top', 'pmcdolly_deck_plate', 'top', {}),
            ('pmc_pallet_top', 'pmc_pallet_plate', 'top', {}),
        ],
        parts=[
            {'name': 'ld3_dolly', 'prefix': 'ld3dolly_', 'centre_xz_m': [0.0, -0.85], 'note': 'LD3 dolly with drawbar, tow eye at the front'},
            {'name': 'ake_container', 'prefix': 'ake_', 'centre_xz_m': [2.5, -0.9], 'note': 'AKE (LD3) container standing on the ground beside the dolly'},
            {'name': 'pmc_dolly', 'prefix': 'pmcdolly_', 'centre_xz_m': [-3.3, -1.3], 'note': 'pallet dolly'},
            {'name': 'pmc_load', 'prefix': 'pmc_', 'centre_xz_m': [-3.3, -1.2], 'note': 'netted PMC pallet load on the pallet dolly'},
        ],
    ),
}


def log(*a):
    print('G3', *a, flush=True)


def world_bbox(obj):
    pts = [obj.matrix_world @ Vector(c) for c in obj.bound_box]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def anchor_point(obj, anchor):
    lo, hi = world_bbox(obj)
    c = (lo + hi) / 2
    if anchor == 'hingepoint':
        return Vector(json.loads(obj['gse_anim'])[0]['pivot'])
    return {
        'center': c,
        'top': Vector((c.x, c.y, hi.z)),
        'bottom': Vector((c.x, c.y, lo.z)),
        'frontTop': Vector((c.x, hi.y, hi.z)),
        'rearTop': Vector((c.x, lo.y, hi.z)),
        'frontCenter': Vector((c.x, hi.y, c.z)),
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


def role_color(role, spec):
    main = spec['main']
    return {'main': main, 'dark': tuple(int(c * 0.62) for c in main), 'accent': spec['accent']}[role]


def set_base(mat, rgb):
    p = mat.node_tree.nodes.get('Principled BSDF')
    p.inputs['Base Color'].default_value = (*g.srgb_to_linear(rgb), 1.0)


def swap_decal(mat_name, path):
    mat = bpy.data.materials[mat_name]
    node = next(n for n in mat.node_tree.nodes if n.type == 'TEX_IMAGE')
    node.image = bpy.data.images.load(path, check_existing=False)


def build_target_fast(coll, name):
    parts = bk.export_objects(coll)
    copies = []
    for i, o in enumerate(parts):
        me = o.data.copy()
        me.transform(o.matrix_world)
        attr = me.attributes.new('gse_part', 'INT', 'FACE')
        for k in range(len(me.polygons)):
            attr.data[k].value = i
        c = bpy.data.objects.new(f'_bake_{i}', me)
        bpy.context.scene.collection.objects.link(c)
        copies.append(c)
    bk._select(copies, copies[0])
    bpy.ops.object.join()
    target = bpy.context.view_layer.objects.active
    target.name = f'{name}_target'
    target.data.materials.clear()
    for uv in list(target.data.uv_layers):
        target.data.uv_layers.remove(uv)
    target.data.uv_layers.new(name='UVMap')
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_all(action='SELECT')
    t0 = time.time()
    bpy.ops.uv.smart_project(angle_limit=1.15, island_margin=0.003, area_weight=0.0, scale_to_bounds=False)
    log('smart_project', round(time.time() - t0, 1), 's')
    t0 = time.time()
    bpy.ops.uv.pack_islands(margin=0.003, rotate=True, shape_method='AABB')
    log('pack_islands', round(time.time() - t0, 1), 's')
    bpy.ops.object.mode_set(mode='OBJECT')
    return target, parts


def composite(albedo, dcol, dalpha):
    a = bk._pixels(dalpha)[..., :1]
    return albedo * (1 - a) + bk._pixels(dcol)[..., :3] * a


def bake_decals(name, tag, objs, size):
    dcol = bk._image(f'{name}_{tag}_dcol', size, (0, 0, 0, 1), False)
    dalpha = bk._image(f'{name}_{tag}_dalpha', size, (0, 0, 0, 1), True)
    mats = {m for o in objs for m in o.data.materials if m is not None}
    for m in mats:
        bk._emission_from(m, image_color=True)
    bk._bake('EMIT', objs, TARGET[0], dcol)
    for m in mats:
        bk._emission_from(m, image_alpha=True)
    bk._bake('EMIT', objs, TARGET[0], dalpha)
    for m in mats:
        bk._clear_emission(m)
    return dcol, dalpha


TARGET = [None]


def bake_set(coll, out, name, cfg, size):
    scene = bpy.context.scene
    scene.render.engine = 'CYCLES'
    if scene.world is None:
        scene.world = bpy.data.worlds.new('World')
    scene.world.use_nodes = True
    scene.cycles.use_denoising = False
    try:
        prefs = bpy.context.preferences.addons['cycles'].preferences
        prefs.compute_device_type = 'OPTIX'
        prefs.get_devices()
        for d in prefs.devices:
            d.use = d.type == 'OPTIX'
        scene.cycles.device = 'GPU'
    except Exception:
        pass
    t0 = time.time()
    target, parts = build_target_fast(coll, name)
    TARGET[0] = target
    log('target built', round(time.time() - t0, 1), 's', len(parts), 'parts', len(target.data.polygons), 'polys')
    decals = bk.decal_objects(coll)
    op_mats = set(cfg['decals'])
    op_decals = [o for o in decals if any(m is not None and m.name in op_mats for m in o.data.materials)]
    static_decals = [o for o in decals if o not in op_decals]
    mats = {m for o in parts for m in o.data.materials if m is not None}

    t0 = time.time()
    base = bk._image(f'{name}_base', size, (0.5, 0.5, 0.5, 1.0), False)
    bk._bake('DIFFUSE', parts, target, base, pass_filter={'COLOR'})
    albedo0 = bk._pixels(base)[..., :3].copy()
    log('diffuse', round(time.time() - t0, 1), 's')

    static_pair = bake_decals(name, 'static', static_decals, size) if static_decals else None
    metal_img = bk._image(f'{name}_metal', size, (0, 0, 0, 1), True)
    for m in mats:
        bk._emission_from(m, value=float(m.get('gse_metallic', 0.0)))
    bk._bake('EMIT', parts, target, metal_img)
    rough_img = bk._image(f'{name}_rough', size, (0.5, 0.5, 0.5, 1), True)
    for m in mats:
        bk._emission_from(m, value=float(m.get('gse_roughness', 0.5)))
    bk._bake('EMIT', parts, target, rough_img)
    for m in mats:
        bk._clear_emission(m)
    log('metal/rough done')

    ao_img = bk._image(f'{name}_ao', size, (1, 1, 1, 1), True)
    for o in parts + decals:
        o.hide_render = True
    scene.world.light_settings.distance = 0.35
    bk._bake('AO', [], target, ao_img, selected_to_active=False, samples=32)
    for o in parts + decals:
        o.hide_render = False
    ao = bk._pixels(ao_img)[..., :1].copy()
    ao_mul = 1 - 0.75 + 0.75 * ao
    log('ao done')

    def finish(alb, tag):
        if static_pair:
            alb = composite(alb, *static_pair)
        return alb

    albedos = {None: finish(albedo0, 'default') * ao_mul}
    originals = {m: list(m['gse_color']) for m in mats if 'gse_color' in m}
    for op in OPS:
        spec = g3_paint.OPS[op]
        alb = albedo0
        if cfg['tint']:
            for mn, role in cfg['tint'].items():
                set_base(bpy.data.materials[mn], role_color(role, spec))
            tb = bk._image(f'{name}_{op}_base', size, (0.5, 0.5, 0.5, 1.0), False)
            bk._bake('DIFFUSE', parts, target, tb, pass_filter={'COLOR'})
            alb = bk._pixels(tb)[..., :3].copy()
            for mn in cfg['tint']:
                set_base(bpy.data.materials[mn], originals[bpy.data.materials[mn]])
        alb = finish(alb, op)
        for mn, kind in cfg['decals'].items():
            swap_decal(mn, os.path.join(ART, f'{kind}_{op}.png'))
        if op_decals:
            dc, da = bake_decals(name, op, op_decals, size)
            alb = composite(alb, dc, da)
        albedos[op] = alb * ao_mul
        log('livery', op, 'done')

    metal = bk._pixels(metal_img)[..., 0]
    gloss = 1.0 - bk._pixels(rough_img)[..., 0]
    mat = np.zeros(albedo0.shape, dtype=np.float32)
    mat[..., 0] = metal
    mat[..., 1] = gloss
    normal = np.zeros(albedo0.shape, dtype=np.float32)
    normal[..., 0] = 0.5
    normal[..., 1] = 0.5
    normal[..., 2] = 1.0
    paths = {'albedo': os.path.join(out, f'{name}_ALB.png'), 'normal': os.path.join(out, f'{name}_NML.png'), 'material': os.path.join(out, f'{name}_MAT.png')}
    bk._save(albedos[None], paths['albedo'], False)
    bk._save(normal, paths['normal'], True)
    bk._save(mat, paths['material'], True)
    variants = {}
    for op in OPS:
        p = os.path.join(out, f'{name}_{op}_ALB.png')
        bk._save(albedos[op], p, False)
        variants[op] = p
    return target, parts, paths, variants


def decimate(path):
    tmp = path + '.lod'
    res = subprocess.run([OBJDECIMATE, path, tmp] + LOD_ARGS, capture_output=True, text=True, check=True)
    os.replace(tmp, path)
    return [l for l in res.stderr.splitlines() if l.startswith('LOD')]


def run(name):
    t_start = time.time()
    cfg = CFG[name]
    out = os.path.join(EXPORT, name)
    os.makedirs(out, exist_ok=True)
    subprocess.run(['python', '-I', os.path.join(GSE_DIR, 'g3_paint.py'), ART], check=True)
    g._materials.clear()
    model = importlib.import_module(name)
    model.build()
    coll = bpy.data.collections[name]
    for other in list(bpy.data.collections):
        if other is not coll and other.name.startswith(name) and other.name not in [c.name for c in coll.children]:
            coll.children.link(other)
    objs = {o.name: o for o in coll.all_objects if o.type == 'MESH'}
    f_obj, r_obj = (objs[n] for n in cfg['front'])
    fy = sum(world_bbox(f_obj)[i].y for i in (0, 1)) / 2
    ry = sum(world_bbox(r_obj)[i].y for i in (0, 1)) / 2
    front_ok = fy > ry
    log('front check', cfg['front'], front_ok)
    attach = {}
    for aname, oname, anchor, values in cfg['attach']:
        attach[aname] = xp_point(posed(objs[oname], anchor_point(objs[oname], anchor), values))
    sweep = None
    if cfg.get('sweep'):
        dr, oname, anchor, vals = cfg['sweep']
        sweep = {'dataref': dr, 'point': oname + ' ' + anchor, 'values_vs_xyz': [[v, xp_point(posed(objs[oname], anchor_point(objs[oname], anchor), {dr: v}))] for v in vals]}
    size = getattr(model, 'TEXTURE_SIZE', 2048)
    target, parts, paths, variants = bake_set(coll, out, name, cfg, size)
    written, nv, nt = ex.export(target, parts, out, name, paths, variants)
    log('export', name, nv, 'vertices', nt, 'triangles')
    lods = {os.path.basename(p): decimate(p) for p in written}
    for k, v in lods.items():
        log(k, v)
    problems = []
    info = {}
    for p in written:
        errors, notes, obj = ob.validate(p)
        info[os.path.basename(p)] = (errors, notes)
        problems += [f'{os.path.basename(p)}: {e}' for e in errors]
    default_obj = ob.parse(written[0])
    pts, _ = ob.posed_points(default_obj, lod_filter=(0, 1))
    lo, hi = pts.min(axis=0), pts.max(axis=0)
    maxvals = {}
    for a in cfg['anims']:
        maxvals[a[0]] = a[3]
    pts_max, _ = ob.posed_points(default_obj, maxvals, lod_filter=(0, 1))
    alo = np.minimum(lo, pts_max.min(axis=0))
    ahi = np.maximum(hi, pts_max.max(axis=0))
    for aname, v in attach.items():
        if not all(alo[i] - 0.01 <= v[i] <= ahi[i] + 0.01 for i in range(3)):
            problems.append(f'attach {aname} {v} outside the exported geometry bounds')
    cmds = ' '.join(default_obj['commands'])
    for a in cfg['anims']:
        if a[0] not in cmds:
            problems.append(f'dataref {a[0]} not in OBJ')
    objects = {'default': f'{name}.obj', **{op: f'{name}_{op}.obj' for op in OPS}}
    tri_lods = {}
    for p in written:
        o = ob.parse(p)
        counts, lds = ob.lod_triangles(o)
        tri_lods[os.path.basename(p)] = {str(k): v for k, v in counts.items()}
    manifest = {
        'vehicle': name,
        'objects': objects,
        'length_m': round(float(hi[2] - lo[2]), 2),
        'width_m': round(float(hi[0] - lo[0]), 2),
        'height_m': round(float(hi[1]), 2),
        'origin': cfg['origin'],
        'front': '-Z',
        'axes': AXES,
        'extent_m': {'x': [round(float(lo[0]), 3), round(float(hi[0]), 3)], 'y': [round(float(lo[1]), 3), round(float(hi[1]), 3)], 'z': [round(float(lo[2]), 3), round(float(hi[2]), 3)]},
        'anims': [{'dataref': d, 'meaning': meaning, 'min': mn, 'max': mx, 'motion': motion} for d, meaning, mn, mx, motion in cfg['anims']],
        'attach': attach,
        'lods': [dict(zip(('near_m', 'far_m', 'max_error_m', 'mode'), a.split(':')[:4])) for a in LOD_ARGS],
        'triangles': tri_lods,
        'textures': [f'{name}_ALB.png', f'{name}_NML.png', f'{name}_MAT.png'] + [f'{name}_{op}_ALB.png' for op in OPS],
    }
    if sweep:
        manifest['sweep'] = sweep
    if cfg.get('parts'):
        manifest['parts'] = cfg['parts']
    with open(os.path.join(out, 'manifest.json'), 'w', encoding='utf-8', newline='\n') as fh:
        json.dump(manifest, fh, indent=1)
        fh.write('\n')
    log('RESULT', name, 'front_ok', front_ok, 'problems', problems)
    for k, (e, notes) in info.items():
        log(k, 'FAIL' if e else 'ok', notes)
    log('total seconds', round(time.time() - t_start))


def main():
    argv = sys.argv[sys.argv.index('--') + 1:]
    run(argv[0])


main()
