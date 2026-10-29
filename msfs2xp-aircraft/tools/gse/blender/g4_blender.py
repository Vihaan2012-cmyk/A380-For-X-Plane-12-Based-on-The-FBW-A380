import faulthandler
faulthandler.enable()
import importlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bpy

import bake_g4
import export_g4
import g4


def main():
    argv = sys.argv[sys.argv.index('--') + 1:]
    name, out = argv[0], argv[1]
    mode = argv[2] if len(argv) > 2 else 'bake'
    model = importlib.import_module(name)
    model.build()
    coll = bpy.data.collections[name]
    os.makedirs(out, exist_ok=True)
    if mode == 'count':
        n = 0
        for o in coll.all_objects:
            if o.type == 'MESH' and not any(m is not None and 'gse_decal' in m for m in o.data.materials):
                me = o.to_mesh()
                me.calc_loop_triangles()
                n += len(me.loop_triangles)
        print(f'G4COUNT {name} objects {len(coll.all_objects)} triangles {n}')
        return
    variants = {'default': g4.livery_path(name, 'neutral')}
    for op in g4.OPERATORS:
        variants[op] = g4.livery_path(name, op)
    if mode == 'quick':
        import bake as bk
        target, parts = bk.build_target(coll, name)
        paths = {'normal': os.path.join(out, f'{name}_NML.png'), 'material': os.path.join(out, f'{name}_MAT.png')}
        albedos = {k: os.path.join(out, f'{name}_ALB.png' if k == 'default' else f'{name}_{k}_ALB.png') for k in variants}
        import numpy as np
        for pth in list(paths.values()) + list(albedos.values()):
            if not os.path.exists(pth):
                bk._save(np.full((64, 64, 3), 0.6, dtype=np.float32), pth, False)
    else:
        target, parts, paths, albedos = bake_g4.bake(coll, out, name, variants, size=getattr(model, 'TEXTURE_SIZE', 2048))
    textures = {'albedo': albedos['default'], 'normal': paths['normal'], 'material': paths['material']}
    written, nv, nt = export_g4.export(target, parts, out, name + '_raw', textures, {})
    meta = model.meta()
    import numpy as np
    co = np.empty(len(target.data.vertices) * 3, dtype=np.float32)
    target.data.vertices.foreach_get('co', co)
    co = co.reshape(-1, 3)
    xp = np.stack([co[:, 0], co[:, 2], -co[:, 1]], axis=1)
    meta['expected_bbox_xp'] = [xp.min(axis=0).tolist(), xp.max(axis=0).tolist()]
    meta['raw_vertices'], meta['raw_triangles'] = nv, nt
    with open(os.path.join(out, 'raw_meta.json'), 'w') as fh:
        json.dump(meta, fh, indent=1)
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(g4.WORK, f'{name}.blend'))
    print(f'G4 export {name}: {nv} vertices, {nt} triangles')


main()
