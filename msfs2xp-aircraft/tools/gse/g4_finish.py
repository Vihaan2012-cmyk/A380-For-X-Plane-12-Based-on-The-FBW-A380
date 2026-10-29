import json
import os
import re
import subprocess
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import g4_obj
import g4_test

EXPORT = 'E:/fbw-gse/export'
DECIMATE = 'D:/A380/fbw-build/target/release/objdecimate.exe'
LODS = ['0:80:0.001:keep', '80:250:0.015:prune', '250:3000:0.05:sloppy:0.08']
OPS = ('dnata', 'swissport', 'menzies')
ORIGINS = {
    'stairs_main': 'ground point on the centreline directly below the platform bumper pad face (default pose); +Y up, the front (cab and platform bumper) faces -Z',
    'stairs_upper': 'ground point on the centreline directly below the platform bumper face (default pose); +Y up, the front (cab and platform bumper) faces -Z',
    'cat_highloader': 'ground point on the centreline directly below the bridge seal face (retracted, default pose); +Y up, the front (cab and bridge) faces -Z',
    'udcat_highloader': 'ground point on the centreline directly below the bridge bumper face (retracted, default pose); +Y up, the front (cab and bridge) faces -Z',
}


def finish(name):
    out = os.path.join(EXPORT, name)
    raw = os.path.join(out, f'{name}_raw.obj')
    meta = json.load(open(os.path.join(out, 'raw_meta.json')))
    final = os.path.join(out, f'{name}.obj')
    subprocess.run([DECIMATE, raw, final] + LODS, check=True)
    text = open(final, encoding='utf-8').read()
    m = re.search(r'^TEXTURE (\S+)$', text, re.M)
    assert m
    objects = {'default': f'{name}.obj'}
    for op in OPS:
        t = text.replace(f'TEXTURE {m.group(1)}', f'TEXTURE {name}_{op}_ALB.png', 1)
        with open(os.path.join(out, f'{name}_{op}.obj'), 'w', encoding='utf-8', newline='\n') as fh:
            fh.write(t)
        objects[op] = f'{name}_{op}.obj'
    os.remove(raw)

    results = {}
    all_problems = []
    for key, fn in objects.items():
        problems, stats = g4_test.validate(os.path.join(out, fn), meta)
        results[key] = (problems, stats)
        all_problems += [f'{fn}: {p}' for p in problems]

    o = g4_obj.parse(final)
    datarefs = [a['dataref'] for a in meta['anims']]
    lo_hi = {}
    for label, vals in (('min', {}), ('max', {d: 1.0 for d in datarefs[:1]}), ('max_all', {d: 1.0 for d in datarefs})):
        tris = g4_obj.posed_triangles(o, vals, lod=0)
        p = np.concatenate([t[0].reshape(-1, 3) for t in tris])
        lo_hi[label] = (p.min(axis=0), p.max(axis=0))
    lo, hi = lo_hi['min']
    for label in ('max', 'max_all'):
        if lo_hi[label][0][1] < -0.05:
            all_problems.append(f'lowest point at {label} pose y={lo_hi[label][0][1]:.3f}')

    bridge_dr = datarefs[1] if len(datarefs) > 1 else None
    measured = {}
    if bridge_dr:
        front_z = None
        top_y = None
        for pts, nrm, uv, call in g4_obj.posed_triangles(o, {}, lod=0):
            drs = {s.get('dr') for s in call['anim']}
            if bridge_dr in drs:
                z = pts[..., 2].min()
                front_z = z if front_z is None else min(front_z, z)
        measured['bridge_group_front_z_m'] = round(float(front_z), 4)
    deck_tops = {}
    for lbl, vals in (('min', {}), ('max', {datarefs[0]: 1.0})):
        best = None
        for pts, nrm, uv, call in g4_obj.posed_triangles(o, vals, lod=0):
            if call['hard'] and not any(s['k'].startswith('rotate') for s in call['anim']) and any(s.get('dr') == datarefs[0] for s in call['anim']):
                y = float(pts[..., 1].max())
                best = y if best is None else max(best, y)
        deck_tops[lbl] = best
    if any(v is not None for v in deck_tops.values()):
        measured['hard_platform_top_y_m'] = {k: (round(v, 4) if v is not None else None) for k, v in deck_tops.items()}

    anims = []
    for a in meta['anims']:
        anims.append({
            'dataref': a['dataref'],
            'meaning': a['meaning'],
            'min': a['min'],
            'max': a['max'],
            'platform_height_at_min_m': a['platform_height_at_min_m'],
            'platform_height_at_max_m': a['platform_height_at_max_m'],
        })
    manifest = {
        'vehicle': name,
        'objects': objects,
        'length_m': round(float(hi[2] - lo[2]), 3),
        'width_m': round(float(hi[0] - lo[0]), 3),
        'height_m': round(float(hi[1]), 3),
        'height_at_max_lift_m': round(float(lo_hi['max'][1][1]), 3),
        'origin': ORIGINS[name],
        'front': '-Z',
        'axes': 'X-Plane object axes in metres: x right, y up, z towards the rear',
        'anims': anims,
        'attach': {
            'platform_front_edge': meta['attach']['platform_front_edge'],
            'how': meta['attach']['how'],
            'measured_in_obj': measured,
        },
        'lod_triangles': results['default'][1]['lod_triangles'],
        'lod_ranges_m': results['default'][1]['lods'],
        'textures': sorted(f for f in os.listdir(out) if f.endswith('.png')),
        'walkable': 'ATTR_hard concrete on treads, risers, landings and platform decks' if name.startswith('stairs') else None,
    }
    with open(os.path.join(out, 'manifest.json'), 'w') as fh:
        json.dump(manifest, fh, indent=1)
    print(name, 'problems:', all_problems or 'none')
    print(json.dumps({k: manifest[k] for k in ('length_m', 'width_m', 'height_m', 'height_at_max_lift_m', 'lod_triangles')}))
    print(json.dumps(measured))
    return all_problems


if __name__ == '__main__':
    bad = []
    for n in sys.argv[1:]:
        bad += finish(n)
    sys.exit(1 if bad else 0)
