import math

EXPORT = 'E:/fbw-gse/export'
WORK = EXPORT + '/_work4'
OPERATORS = ('dnata', 'swissport', 'menzies')
KEYS = [i / 10.0 for i in range(11)]


def livery_path(vehicle, key):
    return f'{WORK}/liveries/{vehicle}/{key}.png'


def mark_hard(objs):
    for o in objs if isinstance(objs, (list, tuple)) else [objs]:
        o['gse_hard'] = 1
    return objs


def trans(dataref, offset, v0=0.0, v1=1.0):
    return {'kind': 'trans', 'offset0': (0.0, 0.0, 0.0), 'offset1': tuple(offset), 'v0': v0, 'v1': v1, 'dataref': dataref}


def trans_keys(dataref, keys):
    return {'kind': 'trans_keys', 'keys': [(v, tuple(p)) for v, p in keys], 'dataref': dataref}


def rotate_keys(dataref, pivot, axis, keys):
    return {'kind': 'rotate_keys', 'pivot': tuple(pivot), 'axis': tuple(axis), 'keys': [(v, d) for v, d in keys], 'dataref': dataref}


def set_all(objs, steps):
    import gselib as g
    for o in objs if isinstance(objs, (list, tuple)) else [objs]:
        g.set_anim(o, steps)
    return objs


def interp(keys, v):
    if v <= keys[0][0]:
        return keys[0][1]
    for (a, pa), (b, pb) in zip(keys, keys[1:]):
        if v <= b:
            t = (v - a) / (b - a)
            if isinstance(pa, (tuple, list)):
                return tuple(x + (y - x) * t for x, y in zip(pa, pb))
            return pa + (pb - pa) * t
    return keys[-1][1]


def rigid_flight_keys(top, foot_rel, rise_total, axis_sign=1.0):
    tx, tz = top
    rx0, rz0 = foot_rel
    length = math.hypot(rx0, rz0)
    a0 = math.atan2(rz0, rx0)
    keys = []
    for v in KEYS:
        dz = rise_total * v
        rz = rz0 - dz
        rx = -math.sqrt(length * length - rz * rz)
        keys.append((v, math.degrees(math.atan2(rz, rx) - a0) * axis_sign))
    return keys
