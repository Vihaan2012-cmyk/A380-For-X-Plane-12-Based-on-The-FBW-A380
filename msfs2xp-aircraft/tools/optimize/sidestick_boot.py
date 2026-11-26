"""Makes the sidesticks' leather boots bend with the stick in an installed aircraft.

sidestick_boot.py <aircraft dir> <a380_cockpit.gltf> [--dry-run]

In MSFS the boot (YOKE_LEFT/RIGHT_LEATHER) is skinned across the stick's joints: YOKE_*_DUMMY2 never moves,
DUMMY1 and YOKE_* carry the stick's two clips. X-Plane has no skinning, so the converter hangs each triangle
on its heaviest joint (model/glb.rs) and the boot tears in two: the top half rides the stick, the base half
stays frozen and the stick cuts through it. This lifts each frozen boot triangle out of its static block and
rotates it with the stick by its share of the stick's weight (mean over its vertices, in sixths), the nearest
X-Plane gets to a bending boot. Weights are read from the MSFS glTF. Records itself in
objects/optimize_state.json and runs once.
shortcut: in-place twin of nothing yet; the converter should do this per skinned control (glb.rs) on the next
full rebuild.
"""
import json
import os
import struct
import sys

STEP = 'sidestick_boot'
OBJS = {'a380_cockpit_045.obj': 'LEFT', 'a380_cockpit_045_r9.obj': 'RIGHT'}
STEPS = 6
STICK = 'yoke_lever_stick'


def key(p):
    return (round(p[0], 3), round(p[1], 3), round(p[2], 3))


def boot_weights(gltf, side):
    """OBJ-space position -> how much of the stick's motion the boot vertex there follows (0..1)."""
    j = json.load(open(gltf))
    nodes, bufs = j['nodes'], {}

    def rows(i):
        a = j['accessors'][i]
        bv = j['bufferViews'][a['bufferView']]
        b = bv['buffer']
        if b not in bufs:
            bufs[b] = open(os.path.join(os.path.dirname(gltf), j['buffers'][b]['uri']), 'rb').read()
        fmt, scale = {5121: ('B', 255), 5123: ('H', 65535), 5126: ('f', 1)}[a['componentType']]
        n = {'VEC3': 3, 'VEC4': 4}[a['type']]
        width = struct.calcsize('<' + fmt * n)
        stride = bv.get('byteStride', width)
        off = bv.get('byteOffset', 0) + a.get('byteOffset', 0)
        div = scale if a.get('normalized') else 1
        return [[x / div for x in struct.unpack_from('<' + fmt * n, bufs[b], off + r * stride)] for r in range(a['count'])]

    out = {}
    for node in nodes:
        if node.get('name') != f'YOKE_{side}_LEATHER':
            continue
        joints = [nodes[k]['name'] for k in j['skins'][node['skin']]['joints']]
        for prim in j['meshes'][node['mesh']]['primitives']:
            at = prim['attributes']
            for p, js, ws in zip(rows(at['POSITION']), rows(at['JOINTS_0']), rows(at['WEIGHTS_0'])):
                total = sum(ws)
                base = sum(w for jj, w in zip(js, ws) if joints[int(jj)].endswith('DUMMY2'))
                out[key((-p[0], p[1], -p[2]))] = 1 - base / total if total else 0  # glTF -> OBJ flips x and z
    if not out:
        raise SystemExit(f'no YOKE_{side}_LEATHER in {gltf}')
    return out


def idx_lines(idx):
    lines = [f'IDX10 ' + ' '.join(map(str, idx[k:k + 10])) for k in range(0, len(idx) - len(idx) % 10, 10)]
    return lines + [f'IDX {i}' for i in idx[len(idx) - len(idx) % 10:]]


def patch(lines, weights):
    """Returns (new lines, triangles moved per sixth)."""
    verts, idx, first_idx = [], [], None
    for n, line in enumerate(lines):
        if line.startswith('VT '):
            verts.append(tuple(map(float, line.split()[1:4])))
        elif line.startswith('IDX'):
            first_idx = n if first_idx is None else first_idx
            idx.extend(map(int, line.split()[1:]))
    last_idx = max(n for n, line in enumerate(lines) if line.startswith('IDX'))
    # TRIS ranges that are static (bucket 0) or ride the stick alone (bucket STEPS); the buffet shake wraps
    # everything and is kept by inserting next to the stick block. Also the stick block's ANIM_begin line and
    # the anim lines before its TRIS.
    stack, ranges, stick_at, stick_anim = [[]], [], None, None
    for n, line in enumerate(lines):
        t = line.split()
        if not t:
            continue
        if t[0] == 'ANIM_begin':
            stack.append([])
            if stick_at is None and any(STICK in lines[m] for m in range(n, n + 12)):
                stick_at = n
        elif t[0] == 'ANIM_end':
            stack.pop()
        elif t[0].startswith('ANIM_') and '/' in t[-1] and 'buffet' not in t[-1]:
            stack[-1].append(t[-1])
        elif t[0] == 'TRIS':
            anims = [d for s in stack for d in s]
            if anims and all(STICK in d for d in anims):
                ranges.append((int(t[1]), int(t[2]), STEPS))
                if stick_anim is None:
                    stick_anim = [lines[m] for m in range(stick_at + 1, n) if lines[m].startswith('ANIM_')]
            elif not anims:
                ranges.append((int(t[1]), int(t[2]), 0))
    assert stick_at is not None and stick_anim, 'no sidestick block'
    buckets = {}
    for a, c, now in ranges:
        for k in range(a, a + c, 3):
            ws = [weights.get(key(verts[idx[k + d]])) for d in range(3)]
            if all(w is None for w in ws):
                continue
            # a vertex off the boot sits on whatever the triangle already rides
            b = round(sum(now / STEPS if w is None else w for w in ws) / 3 * STEPS)
            if b != now:
                buckets.setdefault(b, []).extend(idx[k:k + 3])
                idx[k + 1] = idx[k + 2] = idx[k]  # degenerate: drawn nowhere, counts unchanged
    blocks, start = [], len(idx)
    for b in sorted(buckets):
        tris = buckets[b]
        anim = [f'ANIM_rotate_key {s[1]} {float(s[2]) * b / STEPS:.6f}' if s[0] == 'ANIM_rotate_key' else ' '.join(s)
                for s in (line.split() for line in stick_anim)]
        blocks += ['ANIM_begin', *anim, f'TRIS {start} {len(tris)}', 'ANIM_end']
        idx += tris
        start += len(tris)
    out = lines[:first_idx] + idx_lines(idx) + lines[last_idx + 1:]
    shift = len(out) - len(lines)
    at = stick_at + shift
    out = out[:at] + blocks + out[at:]
    for n, line in enumerate(out):
        if line.startswith('POINT_COUNTS'):
            p = line.split()
            out[n] = f'POINT_COUNTS {p[1]} {p[2]} {p[3]} {len(idx)}'
            break
    return out, {b: len(t) // 3 for b, t in buckets.items()}


def self_check():
    obj = ['POINT_COUNTS 4 0 0 6',
           'VT 0 0 0 0 1 0 0 0', 'VT 1 0 0 0 1 0 0 0', 'VT 0 1 0 0 1 0 0 0', 'VT 5 5 5 0 1 0 0 0',
           'IDX 0', 'IDX 1', 'IDX 2', 'IDX 2', 'IDX 3', 'IDX 0',
           'TRIS 0 3', 'TRIS 3 3',
           'ANIM_begin', 'ANIM_trans 1 2 3 1 2 3', f'ANIM_rotate_begin 0 0 1 fbw/cockpit/{STICK}1_l_r',
           'ANIM_rotate_key 0 -24', 'ANIM_rotate_key 1 24', 'ANIM_rotate_end', 'ATTR_shadow', 'TRIS 0 3', 'ANIM_end']
    w = {key((0, 0, 0)): 1.0, key((1, 0, 0)): 0.5, key((0, 1, 0)): 0.0}
    out, moved = patch(obj, w)
    assert moved == {3: 1, 2: 1}, moved  # means 0.5 -> 3/6 and 1/3 -> 2/6 (the vertex off the boot counts 0)
    assert out[0] == 'POINT_COUNTS 4 0 0 12', out[0]
    idx = [int(x) for line in out if line.startswith('IDX') for x in line.split()[1:]]
    assert idx == [0, 0, 0, 2, 2, 2, 2, 3, 0, 0, 1, 2], idx  # originals degenerate; bucket 2 then bucket 3 appended
    blk = out.index('ANIM_begin')
    assert out[blk:blk + 8] == ['ANIM_begin', 'ANIM_trans 1 2 3 1 2 3', f'ANIM_rotate_begin 0 0 1 fbw/cockpit/{STICK}1_l_r',
                                'ANIM_rotate_key 0 -8.000000', 'ANIM_rotate_key 1 8.000000', 'ANIM_rotate_end',
                                'TRIS 6 3', 'ANIM_end'], out[blk:blk + 8]
    assert out[blk + 14] == 'TRIS 9 3' and out[blk + 16:blk + 18] == ['ANIM_begin', 'ANIM_trans 1 2 3 1 2 3'], out[blk:]


def main():
    self_check()
    aircraft, gltf = sys.argv[1], sys.argv[2]
    dry = '--dry-run' in sys.argv
    objects = os.path.join(aircraft, 'objects')
    state_path = os.path.join(objects, 'optimize_state.json')
    state = json.load(open(state_path)) if os.path.exists(state_path) else {}
    if state.get(STEP):
        print(f'{STEP}: already done')
        return
    for name, side in OBJS.items():
        path = os.path.join(objects, name)
        out, moved = patch(open(path, encoding='utf-8', errors='replace').read().split('\n'), boot_weights(gltf, side))
        print(f'{STEP}: {name}: {sum(moved.values())} boot triangles moved, by sixth {dict(sorted(moved.items()))}'
              + (' (dry run)' if dry else ''))
        if not dry:
            open(path, 'w', encoding='utf-8', newline='\n').write('\n'.join(out))
    if not dry:
        state[STEP] = True
        json.dump(state, open(state_path, 'w'), indent=1)


if __name__ == '__main__':
    main()
