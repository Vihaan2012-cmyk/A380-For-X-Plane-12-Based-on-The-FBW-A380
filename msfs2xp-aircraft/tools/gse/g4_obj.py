import math
import os

import numpy as np


def parse(path):
    o = {'header': [], 'vt': [], 'idx': [], 'counts': None, 'textures': [], 'calls': [], 'lods': [], 'errors': [],
         'anim_balance': 0, 'key_balance': 0, 'hard_balance': 0}
    lod = -1
    stack = None
    key = None
    hard = False
    for raw in open(path, encoding='utf-8').read().splitlines():
        t = raw.strip()
        if not t:
            continue
        w = t.split()
        c = w[0]
        if c == 'VT':
            o['vt'].append([float(x) for x in w[1:]])
        elif c in ('IDX', 'IDX10'):
            o['idx'].extend(int(x) for x in w[1:])
        elif c == 'POINT_COUNTS':
            o['counts'] = [int(x) for x in w[1:]]
        elif c == 'TEXTURE':
            o['textures'].append(w[1])
        elif c == 'TEXTURE_MAP':
            o['textures'].append(w[2])
        elif c == 'ATTR_LOD':
            lod += 1
            o['lods'].append((float(w[1]), float(w[2])))
        elif c == 'ATTR_hard':
            hard = True
            o['hard_balance'] += 1
        elif c == 'ATTR_no_hard':
            hard = False
            o['hard_balance'] -= 1
        elif c == 'ANIM_begin':
            if stack is not None:
                o['errors'].append('nested ANIM_begin')
            stack = []
            o['anim_balance'] += 1
        elif c == 'ANIM_end':
            stack = None
            o['anim_balance'] -= 1
        elif c == 'ANIM_trans' and stack is not None:
            stack.append({'k': 'trans', 'p1': [float(x) for x in w[1:4]], 'p2': [float(x) for x in w[4:7]], 'v1': float(w[7]), 'v2': float(w[8]), 'dr': w[9]})
        elif c == 'ANIM_rotate' and stack is not None:
            stack.append({'k': 'rotate', 'ax': [float(x) for x in w[1:4]], 'a1': float(w[4]), 'a2': float(w[5]), 'v1': float(w[6]), 'v2': float(w[7]), 'dr': w[8]})
        elif c == 'ANIM_trans_begin' and stack is not None:
            key = {'k': 'trans_keys', 'dr': w[1], 'keys': []}
            o['key_balance'] += 1
        elif c == 'ANIM_trans_key' and key is not None:
            key['keys'].append((float(w[1]), [float(x) for x in w[2:5]]))
        elif c == 'ANIM_trans_end':
            stack.append(key)
            key = None
            o['key_balance'] -= 1
        elif c == 'ANIM_rotate_begin' and stack is not None:
            key = {'k': 'rotate_keys', 'ax': [float(x) for x in w[1:4]], 'dr': w[4], 'keys': []}
            o['key_balance'] += 1
        elif c == 'ANIM_rotate_key' and key is not None:
            key['keys'].append((float(w[1]), float(w[2])))
        elif c == 'ANIM_rotate_end':
            stack.append(key)
            key = None
            o['key_balance'] -= 1
        elif c == 'TRIS':
            o['calls'].append({'start': int(w[1]), 'count': int(w[2]), 'lod': lod, 'anim': list(stack) if stack is not None else [], 'hard': hard})
    return o


def _interp(keys, v):
    if v <= keys[0][0]:
        return keys[0][1]
    for (a, pa), (b, pb) in zip(keys, keys[1:]):
        if v <= b:
            t = (v - a) / (b - a) if b != a else 0.0
            if isinstance(pa, list):
                return [x + (y - x) * t for x, y in zip(pa, pb)]
            return pa + (pb - pa) * t
    return keys[-1][1]


def _rot(axis, deg):
    x, y, z = np.array(axis, dtype=float) / np.linalg.norm(axis)
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    m = np.eye(4)
    m[:3, :3] = [[c + x * x * (1 - c), x * y * (1 - c) - z * s, x * z * (1 - c) + y * s],
                 [y * x * (1 - c) + z * s, c + y * y * (1 - c), y * z * (1 - c) - x * s],
                 [z * x * (1 - c) - y * s, z * y * (1 - c) + x * s, c + z * z * (1 - c)]]
    return m


def _trs(p):
    m = np.eye(4)
    m[:3, 3] = p
    return m


def _lerp(a, b, v1, v2, v):
    if v2 == v1:
        return a
    t = min(1.0, max(0.0, (v - v1) / (v2 - v1)))
    return a + (b - a) * t


def anim_matrix(steps, values):
    m = np.eye(4)
    for s in steps:
        v = values.get(s['dr'], 0.0)
        if s['k'] == 'trans':
            p = [_lerp(a, b, s['v1'], s['v2'], v) for a, b in zip(s['p1'], s['p2'])]
            m = m @ _trs(p)
        elif s['k'] == 'rotate':
            m = m @ _rot(s['ax'], _lerp(s['a1'], s['a2'], s['v1'], s['v2'], v))
        elif s['k'] == 'trans_keys':
            m = m @ _trs(_interp(s['keys'], v))
        elif s['k'] == 'rotate_keys':
            m = m @ _rot(s['ax'], _interp(s['keys'], v))
    return m


def posed_triangles(o, values, lod=0):
    vt = np.array(o['vt'], dtype=float)
    idx = np.array(o['idx'], dtype=int)
    pos = vt[:, :3]
    nrm = vt[:, 3:6]
    uv = vt[:, 6:8]
    out = []
    for call in o['calls']:
        if call['lod'] != lod and not (call['lod'] == -1):
            continue
        ids = idx[call['start']:call['start'] + call['count']]
        m = anim_matrix(call['anim'], values)
        p = pos[ids]
        p = (m[:3, :3] @ p.T).T + m[:3, 3]
        n = (m[:3, :3] @ nrm[ids].T).T
        out.append((p.reshape(-1, 3, 3), n.reshape(-1, 3, 3), uv[ids].reshape(-1, 3, 2), call))
    return out


def lod_triangles(o):
    n = {}
    for c in o['calls']:
        n[c['lod']] = n.get(c['lod'], 0) + c['count'] // 3
    return n
