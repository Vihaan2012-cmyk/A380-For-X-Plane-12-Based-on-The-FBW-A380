import json
import os
import random
import re
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

import structure_separation as ss

SIZE = 2048
COVE = (0, 0, 2048, 512)
BAY = (0, 512, 1024, 1536)
CAP = (1024, 512, 2048, 1536)
NAME = "a380_sep_scars"


def jagged_alpha(w, h, edge, seed):
    rnd = random.Random(seed)
    a = Image.new("L", (w, h), 255)
    d = ImageDraw.Draw(a)
    for side in (0, 1):
        pts, x = [], 0
        while x <= w:
            depth = rnd.uniform(0.15, 1.0) * edge
            pts.append((x, depth if side == 0 else h - depth))
            x += rnd.randint(6, 28)
        poly = ([(0, 0)] + pts + [(w, 0)]) if side == 0 else ([(0, h)] + pts + [(w, h)])
        d.polygon(poly, fill=0)
    return a


def texture(path):
    rnd = random.Random(1812)
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    x0, y0, x1, y1 = COVE
    w, h = x1 - x0, y1 - y0
    cove = Image.new("RGBA", (w, h), (150, 162, 140, 255))
    d = ImageDraw.Draw(cove)
    for x in range(0, w, 96):
        d.rectangle((x, 0, x + 12, h), fill=(172, 182, 162, 255))
        for y in range(48, h - 48, 34):
            d.ellipse((x + 3, y, x + 9, y + 6), fill=(196, 200, 198, 255))
    for yy in (170, 214, 318):
        d.rectangle((0, yy, w, yy + 7), fill=(186, 190, 192, 255))
        d.rectangle((0, yy + 1, w, yy + 2), fill=(222, 226, 228, 255))
    for x in range(140, w, 340):
        d.rectangle((x, 200, x + 84, 300), fill=(176, 180, 184, 255))
        d.rectangle((x + 8, 208, x + 76, 292), outline=(140, 144, 148, 255), width=3)
        d.ellipse((x + 30, 238, x + 54, 262), fill=(120, 124, 128, 255))
        d.polygon([(x + 54, 300), (x + 74, 316), (x + 62, 352), (x + 48, 330)], fill=(206, 208, 206, 255))
    for _ in range(500):
        x, y = rnd.randrange(w), rnd.randrange(h)
        g = rnd.randint(118, 140)
        d.ellipse((x, y, x + rnd.randint(2, 6), y + rnd.randint(2, 6)), fill=(g, g + 8, g - 6, 255))
    edge = Image.new("RGBA", (w, h), (214, 216, 214, 255))
    band = jagged_alpha(w, h, 64, 11)
    cove = Image.composite(cove, edge, band)
    cove.putalpha(jagged_alpha(w, h, 30, 7))
    img.paste(cove, (x0, y0))

    x0, y0, x1, y1 = BAY
    w, h = x1 - x0, y1 - y0
    bay = Image.new("RGBA", (w, h), (140, 156, 128, 255))
    d = ImageDraw.Draw(bay)
    for x in range(0, w, 128):
        d.rectangle((x, 0, x + 14, h), fill=(166, 178, 156, 255))
    for y in range(0, h, 170):
        d.rectangle((0, y, w, y + 10), fill=(166, 178, 156, 255))
    d.rectangle((w * 0.35, h * 0.3, w * 0.65, h * 0.7), fill=(170, 174, 178, 255))
    d.rectangle((w * 0.47, h * 0.1, w * 0.53, h * 0.9), fill=(200, 202, 204, 255))
    for _ in range(300):
        x, y = rnd.randrange(w), rnd.randrange(h)
        d.ellipse((x, y, x + 6, y + 6), fill=(128, 142, 118, 255))
    img.paste(bay, (x0, y0))

    x0, y0, x1, y1 = CAP
    w, h = x1 - x0, y1 - y0
    cap = Image.new("RGBA", (w, h), (150, 152, 150, 255))
    d = ImageDraw.Draw(cap)
    r = 16
    for row in range(0, h // r + 2):
        for col in range(0, w // r + 2):
            cx = col * r * 1.5
            cy = row * r * 1.732 + (col % 2) * r * 0.866
            pts = [(cx + r * np.cos(a), cy + r * np.sin(a)) for a in np.linspace(0, 2 * np.pi, 7)]
            d.line(pts, fill=(108, 110, 108, 255), width=3)
    for x in range(0, w, 180):
        d.rectangle((x, 0, x + 16, h), fill=(70, 72, 72, 255))
    soot = Image.new("L", (w, h), 0)
    sd = ImageDraw.Draw(soot)
    for _ in range(60):
        x, y = rnd.randrange(w), rnd.randrange(h)
        rr = rnd.randint(40, 160)
        sd.ellipse((x - rr, y - rr, x + rr, y + rr), fill=rnd.randint(60, 140))
    soot = soot.filter(ImageFilter.GaussianBlur(40))
    cap = Image.composite(Image.new("RGBA", (w, h), (30, 28, 26, 255)), cap, soot)
    img.paste(cap, (x0, y0))
    img.save(path)


def uv(region, u, v):
    x0, y0, x1, y1 = region
    return (x0 + u * (x1 - x0)) / SIZE, 1.0 - (y0 + v * (y1 - y0)) / SIZE


def axes(points):
    c = points.mean(axis=0)
    _, _, vt = np.linalg.svd(points - c, full_matrices=False)
    span, chord, thick = vt[0], vt[1], vt[2]
    if chord[2] < 0:
        chord = -chord
    if abs(thick[1]) > abs(thick[0]) and thick[1] < 0:
        thick = -thick
    if abs(thick[0]) >= abs(thick[1]) and thick[0] < 0:
        thick = -thick
    return c, span, chord, thick


def cove(points, bins=10):
    c, span, chord, thick = axes(points)
    rel = points - c
    s, ch, th = rel @ span, rel @ chord, rel @ thick
    edges = np.linspace(s.min(), s.max(), bins + 1)
    rows = []
    for k in range(bins + 1):
        lo, hi = edges[max(0, k - 1)], edges[min(bins, k + 1)]
        m = (s >= lo) & (s <= hi)
        if not m.any():
            continue
        le = np.percentile(ch[m], 5)
        near = m & (ch <= le + 0.25)
        rows.append((edges[k], le, th[near].min(), th[near].max(), k / bins))
    rows = np.array(rows)
    fit = np.polyfit(rows[:, 0], rows[:, 1], 1)
    lo_t = np.convolve(np.pad(rows[:, 2], 1, mode="edge"), np.ones(3) / 3, mode="valid")
    hi_t = np.convolve(np.pad(rows[:, 3], 1, mode="edge"), np.ones(3) / 3, mode="valid")
    ring = []
    for (sk, _, _, _, u), t0, t1 in zip(rows, lo_t, hi_t):
        at = c + span * sk + chord * (np.polyval(fit, sk) - 0.015)
        ring.append((at + thick * (t1 + 0.01), at + thick * (t0 - 0.01), u))
    tris = []
    for (a1, b1, u1), (a2, b2, u2) in zip(ring, ring[1:]):
        n = chord
        q = [(a1, uv(COVE, u1 * 3, 0.0)), (a2, uv(COVE, u2 * 3, 0.0)), (b2, uv(COVE, u2 * 3, 1.0)), (b1, uv(COVE, u1 * 3, 1.0))]
        tris += [(q[0], q[1], q[2], n), (q[0], q[2], q[3], n), (q[0], q[2], q[1], -n), (q[0], q[3], q[2], -n)]
    return tris


def bay(points):
    c, span, chord, thick = axes(points)
    rel = points - c
    s, ch, th = rel @ span, rel @ chord, rel @ thick
    base = c + thick * (th.min() - 0.03)
    up = thick if thick[1] > 0 else -thick
    p = [base + span * s.min() + chord * ch.min(), base + span * s.max() + chord * ch.min(), base + span * s.max() + chord * ch.max(), base + span * s.min() + chord * ch.max()]
    q = [(p[0], uv(BAY, 0, 0)), (p[1], uv(BAY, 1, 0)), (p[2], uv(BAY, 1, 1)), (p[3], uv(BAY, 0, 1))]
    return [(q[0], q[1], q[2], up), (q[0], q[2], q[3], up)]


def hull(pts):
    pts = sorted(set(map(tuple, np.round(pts, 4))))
    if len(pts) < 3:
        return pts

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lower, upper = [], []
    for p in pts:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(pts):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return lower[:-1] + upper[:-1]


def cap(poly2d, to3d, normal):
    poly = np.array(poly2d)
    lo, hi = poly.min(axis=0), poly.max(axis=0)
    span = np.maximum(hi - lo, 1e-6)
    c = poly.mean(axis=0)
    tris = []
    for a, b in zip(poly, np.roll(poly, -1, axis=0)):
        verts = [(to3d(p), uv(CAP, *((p - lo) / span))) for p in (c, a, b)]
        tris.append((verts[0], verts[1], verts[2], normal))
        tris.append((verts[0], verts[2], verts[1], -np.array(normal)))
    return tris


def cut_planes(regions):
    return regions["wing_cuts"] + [regions["wingtip_x"]]


def wing_section(x, z, regions):
    if z > regions["wing_max_z"]:
        return 0
    return int(np.searchsorted(cut_planes(regions), abs(x), side="left"))


def collect(obj_dir, names, regions):
    rules = ss.surface_rules()
    planes = cut_planes(regions)
    surfaces, fin, tips = {}, [], {(s, k): [] for s in (-1, 1) for k in range(1, len(planes) + 1)}
    fin_r = regions["fin"]
    for n in names:
        lines = open(os.path.join(obj_dir, n), encoding="utf-8", errors="replace").read().split("\n")
        vt, idx = ss.parse(lines)
        for _, first, count, drefs in ss.walk(lines):
            if count < 3:
                continue
            pts = vt[idx[first:first + count]]
            keys = [d for d in drefs if d in rules]
            if keys:
                surfaces.setdefault(keys[0], []).append(pts)
                continue
            if ss.TRIM in drefs:
                continue
            fy = np.interp(pts[:, 2], fin_r["cut_z"], fin_r["cut_y"])
            m = (np.abs(pts[:, 0]) < fin_r["half_x"]) & (pts[:, 2] > fin_r["cut_z"][0]) & (pts[:, 1] > fy) & (pts[:, 1] < fy + 0.35)
            if m.any():
                fin.append(pts[m])
            if any(WING_LEAVE.search(d) for d in drefs):
                continue
            for s, k in tips:
                x = planes[k - 1]
                m = (s * pts[:, 0] > x) & (s * pts[:, 0] < x + 0.4) & (pts[:, 1] < regions["wing_max_y"]) & (pts[:, 2] < regions["wing_max_z"])
                if m.any():
                    tips[(s, k)].append(pts[m])
    return {k: np.concatenate(v) for k, v in surfaces.items()}, (np.concatenate(fin) if fin else None), {key: (np.concatenate(v) if v else None) for key, v in tips.items()}


NATIVE = "sim/operation/failures/"
WING_LEAVE = re.compile(r"RAT|gear|rel_engsep|ENG\d|engine|cowl")


def pieces(surfaces, fin, tips, regions):
    out = []
    for dref, pts in sorted(surfaces.items()):
        sep = ss.surface_rules()[dref][0]
        side = "L" if "/l_" in dref else "R"
        hides = []
        if "spoiler" in dref:
            tris = bay(pts)
        else:
            tris = cove(pts)
            if "elevator" in dref:
                hides.append(f"{NATIVE}rel_hstb{side}")
            if "rudder" in dref:
                hides.append(f"{NATIVE}rel_vstb1")
            if "aileron2" in dref or "aileron3" in dref:
                hides.append(f"{NATIVE}rel_wing4{side}")
        c = pts.mean(axis=0)
        k = wing_section(c[0], c[2], regions)
        if 1 <= k <= 3:
            hides.append(f"{NATIVE}rel_wing{k}{side}")
        out.append((sep, hides, tris))
    if fin is not None and len(fin) > 3:
        poly = hull(fin[:, [0, 2]])
        fz, fy = regions["fin"]["cut_z"], regions["fin"]["cut_y"]
        to3d = lambda p: np.array([p[0], float(np.interp(p[1], fz, fy)) + 0.02, p[1]])
        out.append((f"{NATIVE}rel_vstb1", [], cap(poly, to3d, np.array([0.0, 1.0, 0.0]))))
    planes = cut_planes(regions)
    for (s, k), pts in sorted(tips.items()):
        if pts is None or len(pts) < 3:
            continue
        side = "L" if s < 0 else "R"
        poly = hull(pts[:, [1, 2]])
        x = s * (planes[k - 1] - 0.02)
        to3d = lambda p, x=x: np.array([x, p[0], p[1]])
        inboard = [f"{NATIVE}rel_wing{k - 1}{side}"] if k > 1 else []
        out.append((f"{NATIVE}rel_wing{k}{side}", inboard, cap(poly, to3d, np.array([float(s), 0.0, 0.0]))))
    return out


def write(path, parts):
    vts, idx, cmds = [], [], []
    for dref, hides, tris in parts:
        start = len(idx)
        for t in tris:
            (p0, u0), (p1, u1), (p2, u2), n = t
            n = np.array(n, float)
            n = n / (np.linalg.norm(n) or 1.0)
            for p, (u, v) in ((p0, u0), (p1, u1), (p2, u2)):
                idx.append(len(vts))
                vts.append((p[0], p[1], p[2], n[0], n[1], n[2], u, v))
        lo = "-1 5.5" if dref.startswith(NATIVE) else "-1 0.5"
        cmds += ["ANIM_begin", f"ANIM_hide {lo} {dref}"] + [f"ANIM_hide 5.5 6.5 {h}" for h in hides] + [f"TRIS {start} {len(idx) - start}", "ANIM_end"]
    lines = ["I", "800", "OBJ", "", f"TEXTURE {NAME}.png", f"POINT_COUNTS {len(vts)} 0 0 {len(idx)}", ""]
    lines += [f"VT {x:.4f} {y:.4f} {z:.4f} {a:.4f} {b:.4f} {c:.4f} {u:.5f} {v:.5f}" for x, y, z, a, b, c, u, v in vts]
    lines += [f"IDX {i}" for i in idx]
    lines += ["", "ATTR_no_blend 0.5"] + cmds
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    return len(vts) // 3


if __name__ == "__main__":
    regions_path, obj_dir, *names = sys.argv[1:]
    regions = json.loads(open(regions_path, encoding="utf-8").read())
    surfaces, fin, tips = collect(obj_dir, names, regions)
    parts = pieces(surfaces, fin, tips, regions)
    texture(os.path.join(obj_dir, f"{NAME}.png"))
    n = write(os.path.join(obj_dir, f"{NAME}.obj"), parts)
    print(len(parts), "scar pieces,", n, "triangles")
