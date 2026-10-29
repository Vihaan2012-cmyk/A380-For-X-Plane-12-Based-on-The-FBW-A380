import json
import os
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import g4_obj


def pow2(n):
    return n > 0 and n & (n - 1) == 0


def validate(path, expect=None):
    problems = []
    stats = {}
    o = g4_obj.parse(path)
    base = os.path.dirname(path)
    n_vt, n_idx = len(o['vt']), len(o['idx'])
    if o['counts'] is None or o['counts'][0] != n_vt or o['counts'][3] != n_idx:
        problems.append(f'POINT_COUNTS {o["counts"]} vs VT {n_vt} IDX {n_idx}')
    if n_idx % 3:
        problems.append('IDX count is not a multiple of 3')
    if n_idx and max(o['idx']) >= n_vt:
        problems.append('IDX value beyond VT count')
    for c in o['calls']:
        if c['start'] + c['count'] > n_idx or c['count'] % 3:
            problems.append(f'TRIS {c["start"]} {c["count"]} out of range')
    if o['anim_balance'] != 0:
        problems.append(f'ANIM_begin/ANIM_end unbalanced ({o["anim_balance"]})')
    if o['key_balance'] != 0:
        problems.append(f'ANIM key blocks unbalanced ({o["key_balance"]})')
    if o['hard_balance'] != 0:
        problems.append(f'ATTR_hard/ATTR_no_hard unbalanced ({o["hard_balance"]})')
    problems += o['errors']
    for tex in o['textures']:
        tp = os.path.join(base, tex)
        if not os.path.exists(tp):
            problems.append(f'texture missing: {tex}')
        else:
            w, h = Image.open(tp).size
            if not (pow2(w) and pow2(h)):
                problems.append(f'texture {tex} is {w}x{h}, not power of two')
    if not o['lods']:
        problems.append('no ATTR_LOD')
    stats['lod_triangles'] = {str(k): v for k, v in sorted(g4_obj.lod_triangles(o).items())}
    stats['lods'] = o['lods']
    stats['vertices'] = n_vt
    tris = g4_obj.posed_triangles(o, {}, lod=0)
    p = np.concatenate([t[0].reshape(-1, 3) for t in tris])
    lo, hi = p.min(axis=0), p.max(axis=0)
    stats['bbox_default'] = [lo.tolist(), hi.tolist()]
    if abs(lo[1]) > 0.05:
        problems.append(f'lowest point y={lo[1]:.3f}, origin is not on the ground')
    agree = total = 0
    for pts, nrm, _, call in tris:
        g = np.cross(pts[:, 1] - pts[:, 0], pts[:, 2] - pts[:, 0])
        area = np.linalg.norm(g, axis=1)
        ok = area > 1e-10
        vn = nrm.mean(axis=1)
        d = (g * vn).sum(axis=1)
        agree += int((d[ok] > 0).sum())
        total += int(ok.sum())
    stats['winding_agree'] = agree / max(total, 1)
    if stats['winding_agree'] < 0.99:
        problems.append(f'winding agrees with normals for only {stats["winding_agree"]:.3f}')
    if expect:
        ebb = expect.get('expected_bbox_xp')
        if ebb:
            err = max(np.abs(np.array(ebb[0]) - lo).max(), np.abs(np.array(ebb[1]) - hi).max())
            stats['bbox_error_m'] = float(err)
            if err > 0.03:
                problems.append(f'bbox differs from the Blender model by {err:.3f} m')
        fm, rm = expect.get('front_marker_xp'), expect.get('rear_marker_xp')
        if fm and rm:
            stats['front_z'], stats['rear_z'] = fm[0][2], rm[0][2]
            if not fm[0][2] < rm[0][2]:
                problems.append(f'front marker z={fm[0][2]:.2f} is not ahead (-Z) of rear marker z={rm[0][2]:.2f}')
    return problems, stats


if __name__ == '__main__':
    bad = 0
    for path in sys.argv[1:]:
        meta_path = os.path.join(os.path.dirname(path), 'raw_meta.json')
        expect = json.load(open(meta_path)) if os.path.exists(meta_path) else None
        problems, stats = validate(path, expect)
        print(os.path.basename(path), 'OK' if not problems else problems, stats['lod_triangles'])
        bad += bool(problems)
    sys.exit(1 if bad else 0)
