import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from objtools import Obj, wrap

SCREENS = {
    'A380X_YC_SEAT_ALBEDO_KE7E7E7.dds': [(678, 897, 1538, 1648, 2048), (678, 897, 1708, 1818, 2048), (680, 900, 1908, 2018, 2048)],
    'A380X_SUITES_ALBEDO_KE7E7E7.dds': [(262, 346, 612, 668, 1024), (386, 470, 724, 778, 1024), (805, 888, 602, 660, 1024)],
}
DEVICE = 'ATTR_cockpit_device SCREEN_IFE 0 0 0 350'
SHOW = ['ANIM_begin', 'ANIM_hide -1000000 1000000 fbw/options/cabin_visible', 'ANIM_show 0.5 1.5 fbw/options/cabin_visible']


def clusters(pos, tris):
    parent = list(range(len(pos)))

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    key = {}
    for i, p in enumerate(pos):
        k = tuple(np.round(p, 3))
        if k in key:
            parent[find(i)] = find(key[k])
        else:
            key[k] = i
    for t in tris:
        a = find(t[0])
        for b in t[1:]:
            parent[find(b)] = a
    groups = {}
    for ti, t in enumerate(tris):
        groups.setdefault(find(t[0]), []).append(ti)
    return list(groups.values())

def planar_uv(path):
    """Each screen's UVs from its own plane, so the IFE page lands on it once, upright and
    unmirrored: the remap above flips and clips triangle by triangle, which folded the map
    across the bezel wherever a screen's triangles disagreed."""
    lines = open(path, encoding='latin-1').read().split('\n')
    vt_lines = [i for i, l in enumerate(lines) if l.startswith('VT ')]
    vt = np.array([[float(x) for x in lines[i].split()[1:9]] for i in vt_lines])
    idx = [int(x) for l in lines if l.startswith(('IDX10 ', 'IDX ')) for x in l.split()[1:]]
    tris = np.array(idx[: len(idx) // 3 * 3]).reshape(-1, 3)
    pos, nrm = vt[:, 0:3], vt[:, 3:6]
    for g in clusters(pos, tris):
        verts = sorted(set(tris[g].ravel().tolist()))
        n = nrm[verts].mean(axis=0)
        n = n / (np.linalg.norm(n) or 1.0)
        right = np.cross([0.0, 1.0, 0.0], n)
        right = right / np.linalg.norm(right) if np.linalg.norm(right) > 1e-6 else np.array([1.0, 0.0, 0.0])
        up = np.cross(n, right)
        u, v = pos[verts] @ right, pos[verts] @ up
        su = (u - u.min()) / max(u.max() - u.min(), 1e-9)
        sv = (v - v.min()) / max(v.max() - v.min(), 1e-9)
        for k, vi in enumerate(verts):
            parts = lines[vt_lines[vi]].split()
            parts[7], parts[8] = f'{su[k]:.6f}', f'{sv[k]:.6f}'
            lines[vt_lines[vi]] = ' '.join(parts)
    open(path, 'w', encoding='latin-1', newline='\n').write('\n'.join(lines))

full_dir, objects_dir = sys.argv[1], sys.argv[2]
made = 0
for name in sorted(os.listdir(full_dir)):
    if not name.endswith('.obj') or name.startswith('a380_cabin_ife'):
        continue
    path = os.path.join(full_dir, name)
    with open(path, encoding='latin-1') as f:
        head = f.read(4000)
    if not any(t in head for t in SCREENS):
        continue
    obj = Obj(path)
    rects = SCREENS.get(obj.texture())
    if not rects or any('SCREEN_IFE' in l for l in obj.lines):
        continue
    tris = obj.triangles()
    uv, pos = obj.vt[:, 6:8], obj.vt[:, 0:3]
    which = []
    for t in tris:
        c = uv[t].mean(axis=1)
        w = np.full(len(t), -1)
        for r, (x0, x1, y0, y1, size) in enumerate(rects):
            u0, u1, v0, v1 = x0 / size, x1 / size, 1 - y1 / size, 1 - y0 / size
            w[(c[:, 0] > u0) & (c[:, 0] < u1) & (c[:, 1] > v0) & (c[:, 1] < v1)] = r
        which.append(w)
    count = sum(int((w >= 0).sum()) for w in which)
    if not count:
        continue

    def remap(b, k):
        t = tris[b][k]
        r = which[b][k]
        x0, x1, y0, y1, size = rects[r]
        u0, u1, v0, v1 = x0 / size, x1 / size, 1 - y1 / size, 1 - y0 / size
        st = (uv[t] - [u0, v0]) / [u1 - u0, v1 - v0]
        p = pos[t]
        d1, d2 = p[1] - p[0], p[2] - p[0]
        e1, e2 = uv[t][1] - uv[t][0], uv[t][2] - uv[t][0]
        det = e1[0] * e2[1] - e2[0] * e1[1]
        if abs(det) > 1e-12:
            tangent = (d1 * e2[1] - d2 * e1[1]) / det
            bitangent = (d2 * e1[0] - d1 * e2[0]) / det
            if tangent[0] < 0:
                st[:, 0] = 1 - st[:, 0]
            if bitangent[1] < 0:
                st[:, 1] = 1 - st[:, 1]
        return np.clip(st, 0, 1)

    made += 1
    ife = os.path.join(objects_dir, f'a380_cabin_ife_{made}.obj')
    shown = obj.write(ife, [w >= 0 for w in which], uv=remap, edit=lambda b, tris_line: [DEVICE, tris_line, 'ATTR_no_cockpit'])
    lines = open(ife, encoding='latin-1').read().split('\n')
    lines = ['TEXTURE solid_000000ff.png' if l.startswith('TEXTURE ') else l for l in lines if not l.startswith(('TEXTURE_LIT', 'TEXTURE_MAP', 'TEXTURE_NORMAL', 'ATTR_light_level'))]
    open(ife, 'w', encoding='latin-1', newline='\n').write('\n'.join(lines))
    planar_uv(ife)
    wrap(ife, SHOW)
    kept = obj.write(path, [w < 0 for w in which])
    print(f'{name}: {shown} screen triangles to {os.path.basename(ife)}, {kept} left in the seats')
