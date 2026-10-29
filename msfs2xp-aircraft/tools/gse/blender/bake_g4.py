import os
import time

import bpy

import bake as bk


def _decal_mats(decals):
    return {m for o in decals for m in o.data.materials if m is not None}


def _set_decal_image(mats, path):
    img = bpy.data.images.load(path, check_existing=True)
    for m in mats:
        for n in m.node_tree.nodes:
            if n.type == 'TEX_IMAGE':
                n.image = img


T0 = time.time()


def log(msg):
    print(f'G4 {time.time() - T0:7.1f}s {msg}', flush=True)


def bake(coll, out_dir, name, variants, size=2048, samples=32, ao_strength=0.75):
    os.makedirs(out_dir, exist_ok=True)
    scene = bpy.context.scene
    scene.render.engine = 'CYCLES'
    if scene.world is None:
        scene.world = bpy.data.worlds.new('World')
    scene.world.use_nodes = True
    scene.cycles.use_denoising = False
    if os.environ.get('GSE_CPU'):
        scene.cycles.device = 'CPU'
        scene.render.threads_mode = 'FIXED'
        scene.render.threads = int(os.environ.get('GSE_THREADS', '4'))
    else:
        try:
            prefs = bpy.context.preferences.addons['cycles'].preferences
            prefs.compute_device_type = 'OPTIX'
            prefs.get_devices()
            for d in prefs.devices:
                d.use = d.type == 'OPTIX'
            scene.cycles.device = 'GPU'
        except Exception:
            pass
    log('uv unwrap')
    target, parts = bk.build_target(coll, name)
    log('unwrapped')
    decals = bk.decal_objects(coll)
    mats = {m for o in parts for m in o.data.materials if m is not None}
    dmats = _decal_mats(decals)

    base = bk._image(f'{name}_base', size, (0.5, 0.5, 0.5, 1.0), False)
    log('diffuse')
    bk._bake('DIFFUSE', parts, target, base, pass_filter={'COLOR'})
    albedo = bk._pixels(base)[..., :3].copy()

    log('metal')
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

    log('ao')
    ao_img = bk._image(f'{name}_ao', size, (1, 1, 1, 1), True)
    for o in parts + decals:
        o.hide_render = True
    scene.world.light_settings.distance = 0.35
    bk._bake('AO', [], target, ao_img, selected_to_active=False, samples=min(samples, 48))
    for o in parts + decals:
        o.hide_render = False
    ao = bk._pixels(ao_img)[..., :1]
    shade = 1 - ao_strength + ao_strength * ao

    log('decals')
    outputs = {}
    for key, path in variants.items():
        a = albedo
        if decals:
            _set_decal_image(dmats, path)
            dcol = bk._image(f'{name}_dcol_{key}', size, (0, 0, 0, 1), False)
            dalpha = bk._image(f'{name}_dalpha_{key}', size, (0, 0, 0, 1), True)
            for m in dmats:
                bk._emission_from(m, image_color=True)
            bk._bake('EMIT', decals, target, dcol)
            for m in dmats:
                bk._emission_from(m, image_alpha=True)
            bk._bake('EMIT', decals, target, dalpha)
            for m in dmats:
                bk._clear_emission(m)
            al = bk._pixels(dalpha)[..., :1]
            a = albedo * (1 - al) + bk._pixels(dcol)[..., :3] * al
        outputs[key] = a * shade

    import numpy as np
    metal = bk._pixels(metal_img)[..., 0]
    gloss = 1.0 - bk._pixels(rough_img)[..., 0]
    mat = np.zeros(albedo.shape, dtype=np.float32)
    mat[..., 0] = metal
    mat[..., 1] = gloss
    normal = np.zeros(albedo.shape, dtype=np.float32)
    normal[..., 0] = 0.5
    normal[..., 1] = 0.5
    normal[..., 2] = 1.0

    paths = {'normal': os.path.join(out_dir, f'{name}_NML.png'), 'material': os.path.join(out_dir, f'{name}_MAT.png')}
    bk._save(normal, paths['normal'], True)
    bk._save(mat, paths['material'], True)
    albedos = {}
    for key, arr in outputs.items():
        p = os.path.join(out_dir, f'{name}_ALB.png' if key == 'default' else f'{name}_{key}_ALB.png')
        bk._save(arr, p, False)
        albedos[key] = p
    return target, parts, paths, albedos
