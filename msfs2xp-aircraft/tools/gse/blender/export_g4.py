import json
import os

import bmesh

import export_obj8 as base

HARD = 'concrete'


def anim_lines(steps, indent):
    out = []
    for st in steps:
        k = st['kind']
        if k == 'trans_keys':
            out.append(f"{indent}ANIM_trans_begin {st['dataref']}")
            for v, p in st['keys']:
                out.append(f"{indent}\tANIM_trans_key {base._fmt(v)} {' '.join(base._fmt(c) for c in base.xp_vec(p))}")
            out.append(f'{indent}ANIM_trans_end')
        elif k == 'rotate_keys':
            pivot = base.xp_vec(st['pivot'])
            axis = base.xp_vec(st['axis'])
            out.append(f"{indent}ANIM_trans {' '.join(base._fmt(c) for c in pivot)} {' '.join(base._fmt(c) for c in pivot)} 0 0 none")
            out.append(f"{indent}ANIM_rotate_begin {' '.join(base._fmt(c) for c in axis)} {st['dataref']}")
            for v, deg in st['keys']:
                out.append(f"{indent}\tANIM_rotate_key {base._fmt(v)} {base._fmt(deg)}")
            out.append(f'{indent}ANIM_rotate_end')
            out.append(f"{indent}ANIM_trans {' '.join(base._fmt(-c) for c in pivot)} {' '.join(base._fmt(-c) for c in pivot)} 0 0 none")
        else:
            out.extend(base._anim_lines([st], indent))
    return out


def export(target, parts, out_dir, name, textures, variants=None):
    me = target.data
    bm = bmesh.new()
    bm.from_mesh(me)
    bmesh.ops.triangulate(bm, faces=bm.faces)
    uv = bm.loops.layers.uv.active
    part = bm.faces.layers.int.get('gse_part')
    groups = {}
    for f in bm.faces:
        i = f[part] if part else 0
        p = parts[i]
        key = (p.get('gse_anim') or '', bool(p.get('gse_hard')))
        groups.setdefault(key, []).append(f)
    order = sorted(groups, key=lambda k: (k[0] != '', k[0], k[1]))
    vt, index, keyed, batches = [], [], {}, []
    for key in order:
        start = len(index)
        for f in groups[key]:
            corners = []
            for loop in f.loops:
                n = loop.vert.normal if f.smooth else f.normal
                k = (tuple(round(c, 5) for c in loop.vert.co), tuple(round(c, 3) for c in n), tuple(round(c, 5) for c in loop[uv].uv))
                idx = keyed.get(k)
                if idx is None:
                    idx = len(vt)
                    keyed[k] = idx
                    vt.append((base.xp(loop.vert.co), base.xp(n), tuple(loop[uv].uv)))
                corners.append(idx)
            index.extend(corners)
        batches.append((key, start, len(index) - start))
    bm.free()

    body = []
    for (x, y, z), (nx, ny, nz), (s, t) in vt:
        body.append(f'VT {base._fmt(x)} {base._fmt(y)} {base._fmt(z)} {base._fmt(nx)} {base._fmt(ny)} {base._fmt(nz)} {base._fmt(s)} {base._fmt(t)}')
    full = len(index) // 10
    for k in range(full):
        body.append('IDX10 ' + ' '.join(str(v) for v in index[k * 10:k * 10 + 10]))
    for v in index[full * 10:]:
        body.append(f'IDX {v}')
    body.append('')
    for (anim, hard), s, c in batches:
        indent = ''
        if anim:
            body.append('ANIM_begin')
            body.extend(anim_lines(json.loads(anim), '\t'))
            indent = '\t'
        if hard:
            body.append(f'{indent}ATTR_hard {HARD}')
        body.append(f'{indent}TRIS {s} {c}')
        if hard:
            body.append(f'{indent}ATTR_no_hard')
        if anim:
            body.append('ANIM_end')

    written = []
    for variant, albedo in [(None, textures['albedo'])] + sorted((variants or {}).items()):
        head = ['I', '800', 'OBJ', '', f'TEXTURE {os.path.basename(albedo)}']
        if textures.get('normal'):
            head.append(f"TEXTURE_MAP normal {os.path.basename(textures['normal'])}")
        if textures.get('material'):
            head.append(f"TEXTURE_MAP material_gloss {os.path.basename(textures['material'])}")
        head += ['GLOBAL_specular 1.0', f'POINT_COUNTS {len(vt)} 0 0 {len(index)}', '']
        path = os.path.join(out_dir, f'{name}.obj' if variant is None else f'{name}_{variant}.obj')
        with open(path, 'w', encoding='utf-8', newline='\n') as fh:
            fh.write('\n'.join(head + body) + '\n')
        written.append(path)
    return written, len(vt), len(index) // 3
