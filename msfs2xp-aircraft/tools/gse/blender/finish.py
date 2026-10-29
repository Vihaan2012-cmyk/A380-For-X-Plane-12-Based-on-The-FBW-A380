import importlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bpy
from mathutils import Vector


def main():
    argv = sys.argv[sys.argv.index('--') + 1:]
    name, out = argv[0], argv[1]
    os.makedirs(out, exist_ok=True)
    os.environ['GSE_OUT'] = out
    import bake as bk
    import export_obj8 as ex
    model = importlib.import_module(name)
    model.build()
    coll = bpy.data.collections[name]
    variants = model.livery_variants()
    target, parts, paths, _ = bk.bake(coll, out, name, size=getattr(model, 'TEXTURE_SIZE', 2048), variants=variants)
    written, nv, nt = ex.export(target, parts, out, name, paths, paths['variants'])
    info = []
    for o in parts:
        pts = [o.matrix_world @ Vector(c) for c in o.bound_box]
        xs = [(p.x, p.z, -p.y) for p in pts]
        info.append({'name': o.name, 'anim': bool(o.get('gse_anim')), 'min': [min(p[i] for p in xs) for i in range(3)], 'max': [max(p[i] for p in xs) for i in range(3)]})
    with open(os.path.join(out, f'{name}_parts.json'), 'w') as fh:
        json.dump(info, fh)
    print(f'GSE export {name}: {nv} vertices, {nt} triangles, files {written}')


main()
