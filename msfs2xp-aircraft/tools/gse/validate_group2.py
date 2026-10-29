import json
import os
import struct
import sys
import zlib

import numpy as np


def png_size(path):
    with open(path, 'rb') as fh:
        head = fh.read(24)
    assert head[:8] == b'\x89PNG\r\n\x1a\n', path
    return struct.unpack('>II', head[16:24])


def pow2(n):
    return n > 0 and n & (n - 1) == 0


def check(path, folder):
    errors, info = [], {}
    vt, idx, lods, anim_depth, textures, counts = [], [], [], 0, [], None
    cur = None
    batches = []
    with open(path, encoding='utf-8') as fh:
        for line in fh:
            t = line.strip()
            if t.startswith('VT '):
                vt.append([float(x) for x in t.split()[1:]])
            elif t.startswith('IDX'):
                idx.extend(int(x) for x in t.split()[1:])
            elif t.startswith('POINT_COUNTS'):
                counts = [int(x) for x in t.split()[1:]]
            elif t.startswith('TEXTURE'):
                textures.append(t.split()[-1])
            elif t.startswith('ANIM_begin'):
                anim_depth += 1
                if anim_depth > 1:
                    errors.append('nested ANIM_begin')
            elif t.startswith('ANIM_end'):
                anim_depth -= 1
                if anim_depth < 0:
                    errors.append('ANIM_end without begin')
            elif t.startswith('ATTR_LOD'):
                cur = {'range': t.split()[1:], 'tris': 0}
                lods.append(cur)
            elif t.startswith('TRIS'):
                _, a, b = t.split()
                batches.append((len(lods), int(a), int(b)))
                if cur is None:
                    cur = {'range': None, 'tris': 0}
                    lods.append(cur)
                cur['tris'] += int(b) // 3
    if anim_depth != 0:
        errors.append(f'unbalanced ANIM blocks ({anim_depth})')
    if counts is None or counts[0] != len(vt) or counts[3] != len(idx):
        errors.append(f'POINT_COUNTS {counts} vs VT {len(vt)} IDX {len(idx)}')
    vt = np.array(vt, dtype=np.float64)
    idx_a = np.array(idx, dtype=np.int64)
    if idx_a.size and idx_a.max() >= len(vt):
        errors.append('IDX out of range')
    for lod, a, b in batches:
        if a + b > len(idx) or b % 3:
            errors.append(f'TRIS {a} {b} out of range')
    for name in textures:
        full = os.path.join(folder, name)
        if not os.path.exists(full):
            errors.append(f'missing texture {name}')
        else:
            w, h = png_size(full)
            if not (pow2(w) and pow2(h)):
                errors.append(f'texture {name} is {w}x{h}')
    first = [(a, b) for lod, a, b in batches if lod == 1]
    tri = np.concatenate([idx_a[a:a + b] for a, b in first]).reshape(-1, 3)
    p = vt[:, 0:3]
    n = vt[:, 3:6]
    fn = np.cross(p[tri[:, 1]] - p[tri[:, 0]], p[tri[:, 2]] - p[tri[:, 0]])
    area = np.linalg.norm(fn, axis=1)
    ok = area > 1e-12
    fnn = fn[ok] / area[ok, None]
    vn = n[tri[ok]].sum(axis=1)
    dots = (fnn * vn).sum(axis=1)
    info['winding_outward'] = round(float((dots > 0).mean()), 4)
    info['degenerate_tris'] = int((~ok).sum())
    if info['winding_outward'] < 0.97:
        errors.append(f"winding agrees with normals for only {info['winding_outward']}")
    nl = np.linalg.norm(n[np.unique(tri[ok])], axis=1)
    if nl.min() < 0.9 or nl.max() > 1.1:
        errors.append('non-unit normals')
    uv = vt[:, 6:8]
    info['uv_range'] = [round(float(uv.min()), 3), round(float(uv.max()), 3)]
    if uv.min() < -0.001 or uv.max() > 1.001:
        errors.append('uv outside 0..1')
    info['lods'] = [(l['range'], l['tris']) for l in lods]
    info['extent'] = {
        'x': [round(float(p[:, 0].min()), 3), round(float(p[:, 0].max()), 3)],
        'y': [round(float(p[:, 1].min()), 3), round(float(p[:, 1].max()), 3)],
        'z': [round(float(p[:, 2].min()), 3), round(float(p[:, 2].max()), 3)],
    }
    return errors, info


def main():
    root = sys.argv[1]
    bad = 0
    for vehicle in sys.argv[2:]:
        folder = os.path.join(root, vehicle)
        manifest = json.load(open(os.path.join(folder, 'manifest.json')))
        ext = manifest['extent_m']
        for key, fn in manifest['objects'].items():
            errors, info = check(os.path.join(folder, fn), folder)
            ground = abs(info['extent']['y'][0]) < 0.01
            if not ground:
                errors.append(f"lowest point y={info['extent']['y'][0]}, not on the ground")
            if abs(sum(info['extent']['x'])) > 0.05:
                errors.append(f"not centred in x: {info['extent']['x']}")
            close = all(abs(a - b) < 0.02 for ax in 'xyz' for a, b in zip(info['extent'][ax], ext[ax]))
            if not close:
                errors.append(f'extent differs from manifest: {info["extent"]} vs {ext}')
            print(vehicle, key, fn, 'OK' if not errors else 'FAIL', info['lods'], 'winding', info['winding_outward'], 'extent', info['extent'])
            for e in errors:
                print('   ', e)
            bad += bool(errors)
    sys.exit(1 if bad else 0)


main()
