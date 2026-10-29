"""usage: windshield_cracks.py <objects dir>

Cracked cockpit windows: writes a380_windshield_cracks.obj, the cockpit glass
(a380_cockpit_088.obj) again with a procedural crack texture, one pane per
group, each shown by fbw/damage/windshield_crack_<n> (deep::environment's
pane numbering: 1 L windshield, 2 R windshield, 3 L sliding, 4 R sliding,
5 L fixed, 6 R fixed), and A380_WINDSHIELD_CRACKS.png."""

import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

SOURCE = "a380_cockpit_088.obj"
OUT_OBJ = "a380_windshield_cracks.obj"
OUT_TEX = "A380_WINDSHIELD_CRACKS.png"
SIZE = 2048
SUPERSAMPLE = 4
SEED = 380
RADIAL = (11, 17)
RINGS = (3, 5)
CRACK_RGBA = (236, 240, 244, 210)
SPALL_RGBA = (240, 244, 248, 150)


def gate(n):
    return ["ANIM_begin", f"ANIM_hide -1000000 1000000 fbw/damage/windshield_crack_{n}", f"ANIM_show 0.5 1.5 fbw/damage/windshield_crack_{n}"]


def read_obj(path):
    raw = path.read_bytes().decode("utf-8", errors="replace")
    lines = raw.replace("\r\n", "\n").split("\n")
    vt = np.array([[float(x) for x in l.split()[1:9]] for l in lines if l.startswith("VT")])
    idx = np.array([int(x) for l in lines if l.startswith(("IDX10", "IDX ")) for x in l.split()[1:]], dtype=np.int64)
    body = lines[max(i for i, l in enumerate(lines) if l.startswith(("IDX10", "IDX "))) + 1:]
    return lines, vt, idx, body


def static_panes(vt, idx, count):
    tris = idx[:count].reshape(-1, 3)
    parent = list(range(len(vt)))

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    seen = {}
    for i, key in enumerate(map(tuple, np.round(vt[:, :3], 4))):
        if key in seen:
            parent[find(i)] = find(seen[key])
        else:
            seen[key] = i
    for t in tris:
        parent[find(t[1])] = find(t[0])
        parent[find(t[2])] = find(t[0])
    groups = {}
    for i, t in enumerate(tris):
        groups.setdefault(find(t[0]), []).append(i)
    panes = {}
    for members in groups.values():
        centre = vt[np.unique(tris[members].ravel()), :3].mean(axis=0)
        front = centre[2] < -32.5
        left = centre[0] < 0
        n = (1 if left else 2) if front else (5 if left else 6)
        panes[n] = tris[members]
    assert sorted(panes) == [1, 2, 5, 6], sorted(panes)
    return panes


def pane_mask(vt, tris):
    img = Image.new("L", (SIZE * SUPERSAMPLE, SIZE * SUPERSAMPLE), 0)
    draw = ImageDraw.Draw(img)
    for t in tris:
        uv = vt[t, 6:8]
        draw.polygon([(u * SIZE * SUPERSAMPLE, (1 - v) * SIZE * SUPERSAMPLE) for u, v in uv], fill=255)
    return img


def draw_cracks(draw, rng, centre, reach, width):
    cx, cy = centre
    n = rng.integers(*RADIAL)
    for k in range(n):
        angle = 2 * np.pi * (k + rng.uniform(-0.35, 0.35)) / n
        length = reach * rng.uniform(0.45, 1.0)
        x, y, a, travelled = cx, cy, angle, 0.0
        pts = [(x, y)]
        while travelled < length:
            step = reach * rng.uniform(0.03, 0.07)
            a += rng.normal(0, 0.18)
            x += np.cos(a) * step
            y += np.sin(a) * step
            travelled += step
            pts.append((x, y))
            if rng.random() < 0.08:
                b = a + rng.choice([-1, 1]) * rng.uniform(0.4, 0.9)
                bx, by = x, y
                branch = [(bx, by)]
                for _ in range(rng.integers(2, 6)):
                    b += rng.normal(0, 0.2)
                    bx += np.cos(b) * step
                    by += np.sin(b) * step
                    branch.append((bx, by))
                draw.line(branch, fill=CRACK_RGBA, width=max(1, width - 1))
        draw.line(pts, fill=CRACK_RGBA, width=width)
    for r in np.sort(rng.uniform(0.08, 0.55, rng.integers(*RINGS))):
        radius = reach * r
        start = rng.uniform(0, 360)
        span = rng.uniform(200, 340)
        segs = int(span / 6)
        arc = []
        for s in range(segs + 1):
            t = np.radians(start + span * s / segs)
            rr = radius * (1 + rng.normal(0, 0.04))
            arc.append((cx + np.cos(t) * rr, cy + np.sin(t) * rr))
        draw.line(arc, fill=CRACK_RGBA, width=max(1, width - 1))
    spall = reach * 0.022
    corners = rng.integers(9, 14)
    draw.polygon([(cx + np.cos(2 * np.pi * c / corners) * spall * rng.uniform(0.35, 1.0), cy + np.sin(2 * np.pi * c / corners) * spall * rng.uniform(0.35, 1.0)) for c in range(corners)], fill=SPALL_RGBA)


def texture(vt, panes, rng):
    s = SIZE * SUPERSAMPLE
    out = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    for n in sorted(panes):
        tris = panes[n]
        uv = vt[np.unique(tris.ravel()), 6:8]
        lo, hi = uv.min(axis=0), uv.max(axis=0)
        span = (hi - lo) * s
        centre_uv = lo + (hi - lo) * rng.uniform(0.35, 0.65, 2)
        centre = (centre_uv[0] * s, (1 - centre_uv[1]) * s)
        layer = Image.new("RGBA", (s, s), (0, 0, 0, 0))
        draw_cracks(ImageDraw.Draw(layer), rng, centre, 0.75 * max(span), max(2, SUPERSAMPLE // 2 + 1))
        mask = pane_mask(vt, tris)
        alpha = Image.fromarray(np.minimum(np.array(layer.getchannel("A")), np.array(mask)))
        layer.putalpha(alpha)
        out = Image.alpha_composite(out, layer)
    return out.resize((SIZE, SIZE), Image.LANCZOS).filter(ImageFilter.SMOOTH)


def idx_lines(idx):
    out = []
    for i in range(0, len(idx) - len(idx) % 10, 10):
        out.append("IDX10 " + " ".join(str(x) for x in idx[i:i + 10]))
    for x in idx[len(idx) - len(idx) % 10:]:
        out.append(f"IDX {x}")
    return out


def main(obj_dir):
    obj_dir = Path(obj_dir)
    lines, vt, idx, body = read_obj(obj_dir / SOURCE)
    static_count = int(next(l for l in body if l.startswith("TRIS 0 ")).split()[2])
    panes = static_panes(vt, idx, static_count)
    order = [1, 2, 5, 6]
    new_idx = np.concatenate([panes[n].ravel() for n in order] + [idx[static_count:]])
    starts, at = {}, 0
    for n in order:
        starts[n] = (at, panes[n].size)
        at += panes[n].size

    sliding = {"CPT_SLIDING_WINDOW_ANIM": 3, "FO_SLIDING_WINDOW_ANIM": 4}
    out_body, current = [], None
    for l in body:
        if l.startswith("ANIM_trans_begin fbw/cockpit/"):
            current = sliding.get(l.split("/")[-1])
        if l == "ATTR_cull":
            out_body.append("ATTR_no_cull")
        elif l.startswith("ATTR_poly_os"):
            out_body.append("ATTR_poly_os 2")
        elif l == f"TRIS 0 {static_count}":
            for n in order:
                s, c = starts[n]
                out_body += gate(n) + [f"TRIS {s} {c}", "ANIM_end"]
        elif l.startswith("TRIS ") and current:
            out_body += gate(current) + [l, "ANIM_end"]
        else:
            out_body.append(l)

    vt_lines = [l for l in lines if l.startswith("VT")]
    header = ["I", "800", "OBJ", "", "GLOBAL_cockpit_lit", f"TEXTURE {OUT_TEX}", f"POINT_COUNTS {len(vt)} 0 0 {len(new_idx)}", ""]
    obj = header + vt_lines + [""] + idx_lines(new_idx) + out_body
    (obj_dir / OUT_OBJ).write_text("\n".join(obj), encoding="utf-8", newline="\n")
    texture(vt, panes | {3: idx[static_count + 111:static_count + 222].reshape(-1, 3), 4: idx[static_count:static_count + 111].reshape(-1, 3)}, np.random.default_rng(SEED)).save(obj_dir / OUT_TEX)
    print(f"{OUT_OBJ}: 6 panes, {len(new_idx) // 3} triangles; {OUT_TEX} {SIZE}x{SIZE}")


if __name__ == "__main__":
    main(sys.argv[1])
