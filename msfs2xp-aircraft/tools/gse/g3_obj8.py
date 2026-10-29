import json
import math
import os
import struct
import sys

import numpy as np


def png_size(path):
    with open(path, 'rb') as fh:
        head = fh.read(24)
    if head[:8] != b'\x89PNG\r\n\x1a\n':
        raise ValueError(f'{path}: not a PNG')
    return struct.unpack('>II', head[16:24])


def is_pow2(n):
    return n > 0 and (n & (n - 1)) == 0


def parse(path):
    vt, idx = [], []
    header, commands = [], []
    seen = False
    for raw in open(path, encoding='utf-8').read().split('\n'):
        t = raw.strip()
        if not t:
            continue
        w = t.split()
        if w[0] == 'VT':
            vt.append([float(x) for x in w[1:]])
            seen = True
        elif w[0] in ('IDX', 'IDX10'):
            idx.extend(int(x) for x in w[1:])
            seen = True
        elif seen:
            commands.append(t)
        else:
            header.append(t)
    return {'header': header, 'vt': np.array(vt, dtype=np.float64).reshape(-1, 8), 'idx': np.array(idx, dtype=np.int64), 'commands': commands, 'path': path}


def batches(obj):
    out = []
    stack = []
    lod = 0
    lods = []
    for cmd in obj['commands']:
        w = cmd.split()
        if w[0] == 'ATTR_LOD':
            lods.append((float(w[1]), float(w[2])))
            lod = len(lods)
        elif w[0] == 'ANIM_begin':
            stack.append([])
        elif w[0] == 'ANIM_end':
            if not stack:
                raise ValueError('ANIM_end without ANIM_begin')
            stack.pop()
        elif w[0].startswith('ANIM_'):
            if not stack:
                raise ValueError(f'{w[0]} outside ANIM_begin')
            stack[-1].append(w)
        elif w[0] == 'TRIS':
            out.append({'start': int(w[1]), 'count': int(w[2]), 'lod': lod, 'anim': [list(s) for s in stack]})
    return out, lods


def rotation(axis, deg):
    a = np.array(axis, dtype=np.float64)
    n = np.linalg.norm(a)
    if n == 0:
        return np.eye(4)
    x, y, z = a / n
    r = math.radians(deg)
    c, s = math.cos(r), math.sin(r)
    m = np.eye(4)
    m[:3, :3] = [[c + x * x * (1 - c), x * y * (1 - c) - z * s, x * z * (1 - c) + y * s],
                 [y * x * (1 - c) + z * s, c + y * y * (1 - c), y * z * (1 - c) - x * s],
                 [z * x * (1 - c) - y * s, z * y * (1 - c) + x * s, c + z * z * (1 - c)]]
    return m


def translation(v):
    m = np.eye(4)
    m[:3, 3] = v
    return m


def lerp_value(v, v1, v2, a, b):
    if v2 == v1:
        return a
    t = min(1.0, max(0.0, (v - v1) / (v2 - v1)))
    return a + (b - a) * t


def anim_matrix(stack, values):
    m = np.eye(4)
    for level in stack:
        for w in level:
            if w[0] == 'ANIM_trans':
                p1 = np.array([float(x) for x in w[1:4]])
                p2 = np.array([float(x) for x in w[4:7]])
                v1, v2, dr = float(w[7]), float(w[8]), w[9]
                val = values.get(dr, 0.0)
                t = 0.0 if dr == 'none' else (min(1.0, max(0.0, (val - v1) / (v2 - v1))) if v2 != v1 else 0.0)
                m = m @ translation(p1 + (p2 - p1) * t)
            elif w[0] == 'ANIM_rotate':
                axis = [float(x) for x in w[1:4]]
                r1, r2, v1, v2, dr = float(w[4]), float(w[5]), float(w[6]), float(w[7]), w[8]
                val = values.get(dr, 0.0)
                m = m @ rotation(axis, lerp_value(val, v1, v2, r1, r2))
    return m


def posed_triangles(obj, values=None, lod=0):
    values = values or {}
    bs, _ = batches(obj)
    pos = obj['vt'][:, :3]
    nrm = obj['vt'][:, 3:6]
    tris = []
    for b in bs:
        if b['lod'] != lod and not (lod == 0 and b['lod'] in (0, 1)):
            continue
        ids = obj['idx'][b['start']:b['start'] + b['count']].reshape(-1, 3)
        m = anim_matrix(b['anim'], values)
        used = np.unique(ids)
        p = np.ones((len(used), 4))
        p[:, :3] = pos[used]
        q = (m @ p.T).T[:, :3]
        lookup = {int(u): i for i, u in enumerate(used)}
        for t in ids:
            tris.append((q[lookup[int(t[0])]], q[lookup[int(t[1])]], q[lookup[int(t[2])]], b))
    return tris


def posed_points(obj, values=None, lod_filter=None):
    values = values or {}
    bs, _ = batches(obj)
    pos = obj['vt'][:, :3]
    pts = []
    tags = []
    for b in bs:
        if lod_filter is not None and b['lod'] not in lod_filter:
            continue
        ids = np.unique(obj['idx'][b['start']:b['start'] + b['count']])
        m = anim_matrix(b['anim'], values)
        p = np.ones((len(ids), 4))
        p[:, :3] = pos[ids]
        pts.append((m @ p.T).T[:, :3])
        tags.append(np.full(len(ids), 1 if b['anim'] else 0))
    return np.concatenate(pts), np.concatenate(tags)


def lod_triangles(obj):
    bs, lods = batches(obj)
    counts = {}
    for b in bs:
        counts[b['lod']] = counts.get(b['lod'], 0) + b['count'] // 3
    return counts, lods


def validate(path):
    errors, notes = [], []
    obj = parse(path)
    d = os.path.dirname(path)
    h = obj['header']
    if h[:3] != ['I', '800', 'OBJ']:
        errors.append(f'bad header {h[:3]}')
    pc = next((l for l in h if l.startswith('POINT_COUNTS')), None)
    if pc is None:
        errors.append('no POINT_COUNTS')
    else:
        a = [int(x) for x in pc.split()[1:]]
        if a[0] != len(obj['vt']):
            errors.append(f'POINT_COUNTS VT {a[0]} != {len(obj["vt"])}')
        if a[3] != len(obj['idx']):
            errors.append(f'POINT_COUNTS IDX {a[3]} != {len(obj["idx"])}')
    texs = []
    for l in h:
        w = l.split()
        if w[0] == 'TEXTURE':
            texs.append(w[1])
        elif w[0] == 'TEXTURE_MAP':
            texs.append(w[2])
    if not texs:
        errors.append('no TEXTURE')
    for t in texs:
        p = os.path.join(d, t)
        if not os.path.exists(p):
            errors.append(f'missing texture {t}')
        else:
            wd, ht = png_size(p)
            if not (is_pow2(wd) and is_pow2(ht)):
                errors.append(f'{t} is {wd}x{ht}, not power of two')
    begins = sum(1 for c in obj['commands'] if c == 'ANIM_begin')
    ends = sum(1 for c in obj['commands'] if c == 'ANIM_end')
    if begins != ends:
        errors.append(f'ANIM_begin {begins} != ANIM_end {ends}')
    try:
        bs, lods = batches(obj)
    except ValueError as e:
        errors.append(str(e))
        return errors, notes, obj
    if len(obj['idx']) and (obj['idx'].max() >= len(obj['vt']) or obj['idx'].min() < 0):
        errors.append('index out of range')
    for b in bs:
        if b['count'] % 3 or b['start'] + b['count'] > len(obj['idx']):
            errors.append(f'bad TRIS {b["start"]} {b["count"]}')
    nan = ~np.isfinite(obj['vt']).all()
    if nan:
        errors.append('non-finite vertex data')
    uv = obj['vt'][:, 6:8]
    if uv.min() < -0.001 or uv.max() > 1.001:
        errors.append(f'UV out of 0..1: {uv.min():.3f}..{uv.max():.3f}')
    pos = obj['vt'][:, :3]
    nrm = obj['vt'][:, 3:6]
    ids = obj['idx'].reshape(-1, 3)
    e1 = pos[ids[:, 1]] - pos[ids[:, 0]]
    e2 = pos[ids[:, 2]] - pos[ids[:, 0]]
    fn = np.cross(e1, e2)
    area = np.linalg.norm(fn, axis=1)
    ok = area > 1e-12
    avg = nrm[ids].sum(axis=1)
    agree = (fn[ok] * avg[ok]).sum(axis=1) > 0
    frac = float(agree.mean()) if ok.any() else 1.0
    notes.append(f'winding agrees with normals on {frac * 100:.1f}% of triangles')
    if frac < 0.97:
        errors.append(f'winding/normal disagreement {100 - frac * 100:.1f}%')
    counts, lods_ = lod_triangles(obj)
    notes.append(f'triangles per LOD {counts} lods {lods_}')
    pts, _ = posed_points(obj, lod_filter=(0, 1))
    lo, hi = pts.min(axis=0), pts.max(axis=0)
    notes.append(f'bbox min {np.round(lo, 3).tolist()} max {np.round(hi, 3).tolist()}')
    if lo[1] < -0.01:
        errors.append(f'geometry below ground y={lo[1]:.3f}')
    return errors, notes, obj


if __name__ == '__main__':
    bad = 0
    for p in sys.argv[1:]:
        e, n, _ = validate(p)
        print(os.path.basename(p), 'FAIL' if e else 'ok')
        for x in e:
            print('  ERROR', x)
        for x in n:
            print('  ', x)
        bad += bool(e)
    sys.exit(1 if bad else 0)
