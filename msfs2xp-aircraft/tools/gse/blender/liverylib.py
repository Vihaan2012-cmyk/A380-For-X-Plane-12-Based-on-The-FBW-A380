import json
import os
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
DRAW = os.path.join(HERE, 'liverydraw.py')
OPS = ('dnata', 'swissport', 'menzies')

OPERATORS = {
    'default': dict(label='', main=(78, 84, 92), accent=(232, 168, 0)),
    'dnata': dict(label='dnata', main=(0, 72, 152), accent=(214, 36, 48)),
    'swissport': dict(label='SWISSPORT', main=(214, 28, 40), accent=(70, 72, 76)),
    'menzies': dict(label='MENZIES', main=(20, 40, 92), accent=(238, 112, 28)),
}


def current():
    key = os.environ.get('GSE_OPERATOR', 'default')
    return key if key in OPERATORS else 'default'


def spec(key=None):
    return OPERATORS[key or current()]


def draw(kind, path, key, **extra):
    s = dict(spec(key), **extra)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    subprocess.run(['python', '-I', DRAW, kind, path, json.dumps(s)], check=True)
    return path


def art(folder, kinds):
    out = {}
    for key in OPERATORS:
        for kind, extra in kinds.items():
            out[(key, kind)] = draw(kind, os.path.join(folder, f'g2_{key}_{kind}.png'), key, **extra)
    return out


def art_path(folder, kind, key=None):
    return os.path.join(folder, f'g2_{key or current()}_{kind}.png')


def baked_albedos(out, name):
    found = {}
    for key in OPS:
        path = os.path.join(out, f'{name}_{key}_ALB.png')
        if os.path.exists(path):
            found[key] = path
    return found


def cyl_decal(f, name, x0, x1, dz0, dz1, radius, zc, side, material, coll, steps=12):
    import math

    import bmesh

    import gselib as g

    bm = bmesh.new()
    uv = bm.loops.layers.uv.new('UVMap')
    a0, a1 = math.asin(dz0 / radius), math.asin(dz1 / radius)
    xa, xb = (x0, x1) if side > 0 else (x1, x0)
    r = radius + 2.5
    rows = []
    for i in range(steps + 1):
        a = a0 + (a1 - a0) * i / steps
        y, z = side * r * math.cos(a), zc + r * math.sin(a)
        rows.append((bm.verts.new(f.v(xa, y, z)), bm.verts.new(f.v(xb, y, z))))
    for i in range(steps):
        face = bm.faces.new([rows[i][0], rows[i][1], rows[i + 1][1], rows[i + 1][0]])
        v0, v1 = i / steps, (i + 1) / steps
        for loop, (u, v) in zip(face.loops, [(0, v0), (1, v0), (1, v1), (0, v1)]):
            loop[uv].uv = (u, v)
    return g._object(name, bm, material, coll)
