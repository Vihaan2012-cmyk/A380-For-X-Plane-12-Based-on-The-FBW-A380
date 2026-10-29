import json
import math
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from gse_manifest_data import ANIM_RANGES, VEHICLES

BLENDER = 'C:/Program Files/Blender Foundation/Blender 5.2/blender.exe'
DECIMATE = os.environ.get('GSE_OBJDECIMATE', 'E:/fbw-gse/export/_tools/objdecimate.exe')
EXPORT = 'E:/fbw-gse/export'
WORK = os.path.join(EXPORT, '_work')
OPERATORS = ('dnata', 'swissport', 'menzies')
LODS = ['0:80:0.0003:keep', '80:250:0.015:prune', '250:3000:0.05:sloppy:0.08']


def object_names(name):
    return {'default': f'{name}.obj', **{op: f'{name}_{op}.obj' for op in OPERATORS}}


def parse_obj(path):
    o = {'vt': [], 'idx': [], 'tris': [], 'lods': [], 'textures': [], 'anim': [], 'counts': None, 'balance': 0, 'max_depth': 0, 'lod_tris': []}
    depth = 0
    lod = -1
    for line in open(path, encoding='utf-8').read().splitlines():
        t = line.strip()
        if t.startswith('VT '):
            o['vt'].append([float(x) for x in t.split()[1:]])
        elif t.startswith('IDX10') or t.startswith('IDX '):
            o['idx'].extend(int(x) for x in t.split()[1:])
        elif t.startswith('POINT_COUNTS'):
            o['counts'] = [int(x) for x in t.split()[1:]]
        elif t.startswith('TEXTURE_MAP'):
            o['textures'].append(t.split()[2])
        elif t.startswith('TEXTURE_LIT') or t.startswith('TEXTURE_NORMAL'):
            o['textures'].append(t.split()[-1])
        elif t.startswith('TEXTURE '):
            o['textures'].append(t.split()[1])
        elif t.startswith('ATTR_LOD'):
            _, a, b = t.split()
            o['lods'].append((float(a), float(b)))
            o['lod_tris'].append(0)
            lod += 1
        elif t.startswith('TRIS'):
            _, a, b = t.split()
            o['tris'].append((int(a), int(b), lod, depth))
            if lod >= 0:
                o['lod_tris'][lod] += int(b) // 3
        elif t.startswith('ANIM_begin'):
            depth += 1
            o['max_depth'] = max(o['max_depth'], depth)
        elif t.startswith('ANIM_end'):
            depth -= 1
            if depth < 0:
                o['balance'] = -1
        elif t.startswith(('ANIM_trans', 'ANIM_rotate', 'ANIM_hide', 'ANIM_show')):
            o['anim'].append(t.split())
    if depth != 0:
        o['balance'] = depth
    return o


def validate(path, expect=None):
    problems = []
    o = parse_obj(path)
    base = os.path.dirname(path)
    n_vt, n_idx = len(o['vt']), len(o['idx'])
    if o['counts'] is None or o['counts'][0] != n_vt or o['counts'][3] != n_idx:
        problems.append(f'POINT_COUNTS {o["counts"]} vs VT {n_vt} IDX {n_idx}')
    if n_idx % 3:
        problems.append('IDX count not a multiple of 3')
    if o['idx'] and max(o['idx']) >= n_vt:
        problems.append('IDX index out of range')
    for a, b, lod, depth in o['tris']:
        if a + b > n_idx or b % 3:
            problems.append(f'TRIS {a} {b} out of range')
    if o['balance']:
        problems.append(f'ANIM_begin/ANIM_end unbalanced ({o["balance"]})')
    if o['max_depth'] > 1:
        problems.append('nested ANIM_begin')
    for tex in dict.fromkeys(o['textures']):
        p = os.path.join(base, tex)
        if not os.path.exists(p):
            problems.append(f'texture missing {tex}')
            continue
        from PIL import Image
        w, h = Image.open(p).size
        if (w & (w - 1)) or (h & (h - 1)):
            problems.append(f'texture {tex} is {w}x{h}, not a power of two')
    prev = 0.0
    for a, b in o['lods']:
        if abs(a - prev) > 1e-6 and not (prev == 0.0 and a == 0.0):
            problems.append(f'LOD gap {prev} -> {a}')
        prev = b
    bad = 0
    total = 0
    for a, b, lod, depth in o['tris']:
        if lod > 0:
            continue
        for i in range(a, a + b, 3):
            v = [o['vt'][o['idx'][i + k]] for k in range(3)]
            e1 = [v[1][k] - v[0][k] for k in range(3)]
            e2 = [v[2][k] - v[0][k] for k in range(3)]
            n = (e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0])
            vn = [sum(v[k][3 + j] for k in range(3)) for j in range(3)]
            ln = math.sqrt(sum(c * c for c in n))
            if ln < 1e-12:
                continue
            total += 1
            if sum(n[j] * vn[j] for j in range(3)) < 0:
                bad += 1
    if total and bad / total > 0.02:
        problems.append(f'{bad}/{total} triangles wind against their normals')
    xs = [v[0] for v in o['vt']]
    ys = [v[1] for v in o['vt']]
    zs = [v[2] for v in o['vt']]
    box = {'min': [min(xs), min(ys), min(zs)], 'max': [max(xs), max(ys), max(zs)]}
    if abs(box['min'][1]) > 0.02:
        problems.append(f'lowest point is {box["min"][1]:.3f} m, not on the ground')
    o['bbox'] = box
    o['winding_bad'] = (bad, total)
    return o, problems


def clamp_ground(src, dst, limit=0.03):
    out = []
    for line in open(src, encoding='utf-8').read().splitlines():
        if line.startswith('VT '):
            p = line.split()
            y = float(p[2])
            if -limit < y < 0:
                p[2] = '0'
                line = ' '.join(p)
        out.append(line)
    with open(dst, 'w', encoding='utf-8', newline=chr(10)) as fh:
        fh.write(chr(10).join(out) + chr(10))


def run_lods(name):
    names = object_names(name)
    os.makedirs(os.path.join(EXPORT, name), exist_ok=True)
    stats = {}
    for key, fn in names.items():
        raw = os.path.join(WORK, name, fn)
        src = os.path.join(WORK, name, 'clamped_' + fn)
        clamp_ground(raw, src)
        dst = os.path.join(EXPORT, name, fn)
        r = subprocess.run([DECIMATE, src, dst] + LODS, capture_output=True, text=True)
        if r.returncode:
            raise SystemExit(f'objdecimate failed on {fn}: {r.stderr}')
        stats[key] = [l for l in r.stderr.splitlines() if l.startswith('LOD')]
    for f in os.listdir(os.path.join(WORK, name)):
        if f.lower().endswith('.png') and re.match(re.escape(name) + r'_(ALB|NML|MAT)', f):
            with open(os.path.join(WORK, name, f), 'rb') as a, open(os.path.join(EXPORT, name, f), 'wb') as b:
                b.write(a.read())
    return stats


def part_dims(name):
    cfg = VEHICLES[name]
    parts = json.load(open(os.path.join(WORK, name, f'{name}_parts.json')))
    pat = re.compile(cfg['exclude']) if cfg['exclude'] else None
    keep = [p for p in parts if not (pat and pat.search(p['name']))]
    lo = [min(p['min'][i] for p in keep) for i in range(3)]
    hi = [max(p['max'][i] for p in keep) for i in range(3)]
    lo[1] = max(lo[1], 0.0)
    return lo, hi


def xp_point(cfg, p):
    x, y, z = p
    return [round(y / 1000.0, 3), round(z / 1000.0, 3), round(-(x - cfg['x0']) / 1000.0, 3)]


def manifest(name, parsed):
    cfg = VEHICLES[name]
    lo, hi = part_dims(name)
    o = parsed['default']
    anims = {}
    for a in o['anim']:
        if a[0] == 'ANIM_trans' and a[-1] != 'none':
            anims.setdefault(a[-1], ('trans', []))[1].append((float(a[7]), float(a[8])))
        elif a[0] == 'ANIM_rotate':
            anims.setdefault(a[-1], ('rotate', []))[1].append((float(a[-3]), float(a[-2])))
        elif a[0] in ('ANIM_hide', 'ANIM_show'):
            anims.setdefault(a[-1], (a[0][5:], []))[1].append((float(a[1]), float(a[2])))
    problems = []
    for dr in anims:
        if dr not in cfg['anims']:
            problems.append(f'dataref {dr} in the OBJ has no meaning in the config')
        if dr not in ANIM_RANGES:
            problems.append(f'dataref {dr} has no range')
    for dr in cfg['anims']:
        if dr not in anims:
            problems.append(f'configured dataref {dr} missing from the OBJ')
    out = {
        'vehicle': name,
        'objects': object_names(name),
        'length_m': round(hi[2] - lo[2], 3),
        'width_m': round(hi[0] - lo[0], 3),
        'height_m': round(hi[1] - lo[1], 3),
        'origin': cfg['origin'],
        'front': '-Z',
        'front_desc': cfg['front_desc'],
        'axes': 'X-Plane object axes: +X right of the vehicle, +Y up, +Z backwards, metres',
        'body_extent_m': {'min': [round(c, 3) for c in lo], 'max': [round(c, 3) for c in hi]},
        'bbox_m': {'min': [round(c, 3) for c in o['bbox']['min']], 'max': [round(c, 3) for c in o['bbox']['max']]},
        'anims': [{'dataref': dr, 'meaning': cfg['anims'][dr], 'min': ANIM_RANGES[dr][0], 'max': ANIM_RANGES[dr][1]} for dr in cfg['anims'] if dr in ANIM_RANGES],
        'attach': {k: {'x': xp_point(cfg, v)[0], 'y': xp_point(cfg, v)[1], 'z': xp_point(cfg, v)[2], 'note': cfg['attach_notes'][k]} for k, v in cfg['attach'].items()},
        'lod_ranges_m': [[float(s.split(':')[0]), float(s.split(':')[1])] for s in LODS],
        'triangles': {'lod0': parsed['default']['lod_tris'][0], 'lod1': parsed['default']['lod_tris'][1], 'lod2': parsed['default']['lod_tris'][2]},
    }
    with open(os.path.join(EXPORT, name, 'manifest.json'), 'w') as fh:
        json.dump(out, fh, indent=2)
    return out, problems


def axis_check(name, parsed):
    cfg = VEHICLES[name]
    lo, hi = part_dims(name)
    exp_front = -(cfg['front_x'] - cfg['x0']) / 1000.0
    exp_rear = -(cfg['rear_x'] - cfg['x0']) / 1000.0
    return {'z_front_actual': round(lo[2], 3), 'z_front_expected': round(exp_front, 3), 'z_rear_actual': round(hi[2], 3), 'z_rear_expected': round(exp_rear, 3)}


def post(name):
    stats = run_lods(name)
    parsed, problems = {}, {}
    for key, fn in object_names(name).items():
        parsed[key], problems[key] = validate(os.path.join(EXPORT, name, fn))
    man, mp = manifest(name, parsed)
    problems['manifest'] = mp
    report = {'name': name, 'lod_stats': stats['default'], 'lod_tris_per_object': {k: v['lod_tris'] for k, v in parsed.items()}, 'problems': problems, 'axis': axis_check(name, parsed),
              'winding': {k: v['winding_bad'] for k, v in parsed.items()}, 'manifest': {k: man[k] for k in ('length_m', 'width_m', 'height_m')}}
    print(json.dumps(report, indent=1))
    return report


def render(name, views=('front_right',)):
    outs = []
    for key, fn in object_names(name).items():
        for view in views:
            if view != 'front_right' and key != 'default':
                continue
            png = os.path.join(EXPORT, name, f'{name}_{key}_{view}.png' if view != 'front_right' else f'{name}_{key}.png')
            subprocess.run([BLENDER, '--background', '--python', os.path.join(HERE, 'blender', 'render_export.py'), '--', png, os.path.join(EXPORT, name, fn), view], capture_output=True, text=True)
            outs.append(png)
    return outs


def sheet(names):
    from PIL import Image, ImageDraw
    cols = 4
    tw, th = 480, 320
    rows = len(names)
    im = Image.new('RGB', (cols * tw, rows * (th + 22)), (24, 24, 26))
    d = ImageDraw.Draw(im)
    for r, name in enumerate(names):
        for c, key in enumerate(object_names(name)):
            p = os.path.join(EXPORT, name, f'{name}_{key}.png' if key != 'default' else f'{name}_default.png')
            if not os.path.exists(p):
                p = os.path.join(EXPORT, name, f'{name}_{key}.png')
            y = r * (th + 22)
            d.text((c * tw + 6, y + 4), f'{name} / {key}', fill=(255, 220, 80))
            if os.path.exists(p):
                t = Image.open(p).convert('RGB').resize((tw, th))
                im.paste(t, (c * tw, y + 22))
    im.save(os.path.join(EXPORT, 'group1_contact.png'))


if __name__ == '__main__':
    cmd = sys.argv[1]
    if cmd == 'post':
        post(sys.argv[2])
    elif cmd == 'render':
        render(sys.argv[2], tuple(sys.argv[3].split(',')) if len(sys.argv) > 3 else ('front_right',))
    elif cmd == 'sheet':
        sheet(sys.argv[2:])
