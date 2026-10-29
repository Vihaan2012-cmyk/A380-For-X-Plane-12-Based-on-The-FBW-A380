"""usage: text_repack.py <text-layer objects dir> <aircraft objects dir> <work dir> <pre-bake LIT dds> [--measure]

Gives the blended text layer (text_layer.py -> text_blend.py -> decal_offset.py) atlases of its own.
FlyByWire's lettering atlas packs words a few texels apart and its decal quads overlap their
neighbours, so the mips X-Plane draws the cockpit with smeared neighbouring words into each label.
Every connected decal piece gets its own slot in a new sheet holding only the texels under its own
triangles (grown EDGE texels for the anti-aliased rim), padded PAD texels on every side, and its UVs
move to that slot.
Pieces that mostly cover the same texels (the same scale on several knobs) share a slot. One layout
per source sheet, used by its <name>_TEXT.png (work dir) and, for objects with TEXTURE_LIT, the
pre-bake LIT sheet. Alpha is FlyByWire's own (the pre-bake sheet next to the LIT one), so strokes
keep FlyByWire's weight; colour comes from the _TEXT sheet, carried outward from the ink. Mip alpha
is a Lanczos reduction of the full-size alpha (the 24 texel pad holds its reach for the near levels), and mip colour is weighted by coverage; every clear
texel at every level takes its nearest ink's colour, so filtering never pulls a letter's edge towards black. Writes <name>_PACK.dds, A380_COCKPIT_DECAL_EMIS_LITA_<name>_PACK.dds and the
objects."""

import io
import os
import re
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

DECALS = [f"a380_cockpit_{n:03d}.obj" for n in range(20, 26)]
PAD = 24
MIP_LEVELS = int(os.environ.get("TEXT_MIP_LEVELS", "99"))
EDGE = 1
GLOW = 3
SAME = 0.5
WIDTH = 4096


def parse(path):
    lines = Path(path).read_text(encoding="latin-1").split("\n")
    vt = [i for i, l in enumerate(lines) if l.startswith("VT")]
    v = np.array([[float(x) for x in lines[i].split()[1:9]] for i in vt])
    idx = np.array([int(x) for l in lines if l.startswith(("IDX10", "IDX ")) for x in l.split()[1:]], dtype=np.int64)
    return lines, vt, v, idx[: len(idx) // 3 * 3].reshape(-1, 3)


def islands(tris, n):
    parent = np.arange(n)

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    for a, b, c in tris:
        ra = find(a)
        parent[find(b)] = ra
        parent[find(c)] = ra
    groups = {}
    for t, tri in enumerate(tris):
        groups.setdefault(find(tri[0]), []).append(t)
    return list(groups.values())


def coverage(v, tris, members, size):
    w, h = size
    corners = v[tris[members]][..., 6:8]
    px = np.stack([corners[..., 0] * w, (1 - corners[..., 1]) * h], axis=-1)
    x0, y0 = np.floor(px.reshape(-1, 2).min(axis=0)).astype(int) - EDGE
    x1, y1 = np.ceil(px.reshape(-1, 2).max(axis=0)).astype(int) + EDGE
    mask = Image.new("L", (x1 - x0, y1 - y0), 0)
    draw = ImageDraw.Draw(mask)
    for tri in px:
        draw.polygon([(float(x - x0), float(y - y0)) for x, y in tri], fill=255, outline=255)
    return (int(x0), int(y0)), np.asarray(mask) > 0


def window(arr, origin, shape, pad=0):
    h, w = arr.shape[:2]
    ys = np.arange(origin[1] - pad, origin[1] + shape[0] + pad) % h
    xs = np.arange(origin[0] - pad, origin[0] + shape[1] + pad) % w
    return arr[np.ix_(ys, xs)]


def merge(pieces):
    boxes = np.array([(o[0], o[1], o[0] + c.shape[1], o[1] + c.shape[0]) for o, c, _ in pieces])
    parent = list(range(len(pieces)))

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    counts = [c.sum() for _, c, _ in pieces]
    for i in range(len(pieces)):
        hit = np.nonzero((boxes[i + 1:, 0] < boxes[i, 2]) & (boxes[i + 1:, 2] > boxes[i, 0]) & (boxes[i + 1:, 1] < boxes[i, 3]) & (boxes[i + 1:, 3] > boxes[i, 1]))[0]
        for j in hit + i + 1:
            x0, y0 = max(boxes[i, 0], boxes[j, 0]), max(boxes[i, 1], boxes[j, 1])
            x1, y1 = min(boxes[i, 2], boxes[j, 2]), min(boxes[i, 3], boxes[j, 3])
            ci = pieces[i][1][y0 - boxes[i, 1]:y1 - boxes[i, 1], x0 - boxes[i, 0]:x1 - boxes[i, 0]]
            cj = pieces[j][1][y0 - boxes[j, 1]:y1 - boxes[j, 1], x0 - boxes[j, 0]:x1 - boxes[j, 0]]
            if (ci & cj).sum() >= SAME * min(counts[i], counts[j]):
                parent[find(j)] = find(i)
    groups = {}
    for i in range(len(pieces)):
        groups.setdefault(find(i), []).append(i)
    slots, where = [], [0] * len(pieces)
    for members in groups.values():
        x0, y0 = boxes[members, 0].min(), boxes[members, 1].min()
        x1, y1 = boxes[members, 2].max(), boxes[members, 3].max()
        ink = np.zeros((y1 - y0, x1 - x0), bool)
        for i in members:
            (ox, oy), c, m = pieces[i]
            ink[oy - y0:oy - y0 + c.shape[0], ox - x0:ox - x0 + c.shape[1]] |= m
            where[i] = len(slots)
        slots.append(((int(x0), int(y0)), ink))
    return slots, where


def pack(sizes):
    order = sorted(range(len(sizes)), key=lambda i: (-sizes[i][1], -sizes[i][0]))
    at, x, y, shelf = [None] * len(sizes), 0, 0, 0
    for i in order:
        w, h = sizes[i][0] + 2 * PAD, sizes[i][1] + 2 * PAD
        if x + w > WIDTH:
            x, y, shelf = 0, y + shelf, 0
        at[i] = (x + PAD, y + PAD)
        x += w
        shelf = max(shelf, h)
    height = y + shelf
    pot = 256
    while pot < height:
        pot *= 2
    return at, pot


def compose(src, slots, at, height, lit):
    out = np.zeros((height, WIDTH, 4), np.float32)
    for ((origin, ink), (x, y)) in zip(slots, at):
        block = window(src, origin, ink.shape, PAD).astype(np.float32)
        m = np.pad(ink, PAD)
        if lit:
            block *= ndimage.binary_dilation(m, iterations=GLOW)[..., None]
        else:
            block[..., 3] *= m
        out[y - PAD:y - PAD + block.shape[0], x - PAD:x - PAD + block.shape[1]] = block
    return out


def reduce(img, lit):
    h, w = img.shape[:2]
    nh, nw = max(1, h // 2), max(1, w // 2)
    p = img[: nh * 2, : nw * 2].reshape(nh, 2, nw, 2, 4)
    if lit:
        return p.mean(axis=(1, 3))
    a = p[..., 3]
    wsum = a.sum(axis=(1, 3))
    rgb = (p[..., :3] * a[..., None]).sum(axis=(1, 3)) / np.maximum(wsum, 1e-6)[..., None]
    rgb = np.where((wsum > 0)[..., None], rgb, p[..., :3].mean(axis=(1, 3)))
    alpha = a.mean(axis=(1, 3))
    return np.concatenate([rgb, alpha[..., None]], axis=-1)


def encode_level(img):
    h, w = img.shape[:2]
    im = Image.fromarray((img + 0.5).clip(0, 255).astype(np.uint8), "RGBA")
    if w < 4 or h < 4:
        pad = Image.new("RGBA", (max(4, w), max(4, h)))
        pad.paste(im, (0, 0))
        im = pad
    buf = io.BytesIO()
    im.save(buf, "DDS", pixel_format="DXT5")
    return buf.getvalue()


def carry_colour(img):
    ink = img[..., 3] > 0
    if not ink.any() or ink.all():
        return img
    _, (iy, ix) = ndimage.distance_transform_edt(~ink, return_distances=True, return_indices=True)
    img[..., :3] = img[iy, ix, :3]
    return img


def lanczos_alpha(alpha, shape):
    im = Image.fromarray(alpha.astype(np.float32), "F")
    return np.asarray(im.resize((shape[1], shape[0]), Image.LANCZOS)).clip(0, 255)


def write_dds(img, path, lit):
    levels = [img if lit else carry_colour(img)]
    alpha0 = None if lit else img[..., 3].copy()
    while min(levels[-1].shape[:2]) > 1 and len(levels) < MIP_LEVELS:
        nxt = reduce(levels[-1], lit)
        if not lit:
            nxt[..., 3] = lanczos_alpha(alpha0, nxt.shape[:2])
        levels.append(nxt if lit else carry_colour(nxt))
    first = encode_level(levels[0])
    header = bytearray(first[:128])
    header[8:12] = (int.from_bytes(header[8:12], "little") | 0x20000).to_bytes(4, "little")
    header[28:32] = len(levels).to_bytes(4, "little")
    header[108:112] = (int.from_bytes(header[108:112], "little") | 0x400008).to_bytes(4, "little")
    body = [first[128:]] + [encode_level(l)[128:] for l in levels[1:]]
    Path(path).write_bytes(bytes(header) + b"".join(body))


def main(textdir, outdir, work, lit_dds, measure=False):
    prebake = Path(lit_dds).parent
    textdir, outdir, work = Path(textdir), Path(outdir), Path(work)
    by_sheet = {}
    for n in DECALS:
        lines, vt, v, tris = parse(textdir / n)
        tex = next(m.group(1) for l in lines if (m := re.match(r"TEXTURE (\S+)_TEXT\.dds$", l)))
        lit = any(l.startswith("TEXTURE_LIT") for l in lines)
        by_sheet.setdefault(tex, []).append((n, lines, vt, v, tris, lit))
    lit_src = None
    for tex, objs in by_sheet.items():
        src = np.array(Image.open(work / f"{tex}_TEXT.png").convert("RGBA"))
        src[..., 3] = np.asarray(Image.open(prebake / f"{tex}.dds").convert("RGBA"))[..., 3]
        size = (src.shape[1], src.shape[0])
        seen, cuts, uses = {}, [], []
        for n, lines, vt, v, tris, lit in objs:
            for members in islands(tris, len(v)):
                origin, cover = coverage(v, tris, members, size)
                key = (origin, cover.shape, np.packbits(cover).tobytes())
                if key not in seen:
                    seen[key] = len(cuts)
                    cuts.append((origin, cover))
                uses.append((n, members, seen[key]))
        slots, where = merge([(o, c, ndimage.binary_dilation(c, iterations=EDGE)) for o, c in cuts])
        at, height = pack([(ink.shape[1], ink.shape[0]) for _, ink in slots])
        print(f"{tex}: {len(uses)} pieces, {len(cuts)} unique, {len(slots)} slots, sheet {WIDTH}x{height}")
        if measure:
            continue
        write_dds(compose(src, slots, at, height, False), outdir / f"{tex}_PACK.dds", False)
        if any(o[5] for o in objs):
            if lit_src is None:
                lit_src = np.asarray(Image.open(lit_dds).convert("RGBA"))
            if lit_src.shape[:2] != src.shape[:2]:
                raise SystemExit(f"LIT sheet {lit_src.shape[:2]} does not match {tex} {src.shape[:2]}")
            write_dds(compose(lit_src, slots, at, height, True), outdir / f"A380_COCKPIT_DECAL_EMIS_LITA_{tex}_PACK.dds", True)
        for n, lines, vt, v, tris, lit in objs:
            v = v.copy()
            for name, members, p in uses:
                if name != n:
                    continue
                slot = where[p]
                (ox, oy), _ = slots[slot]
                nx, ny = at[slot]
                ids = np.unique(tris[members])
                px = v[ids, 6] * size[0] - ox + nx
                py = (1 - v[ids, 7]) * size[1] - oy + ny
                v[ids, 6] = px / WIDTH
                v[ids, 7] = 1 - py / height
            out = list(lines)
            for row, i in enumerate(vt):
                keep = lines[i].split()[:7]
                out[i] = " ".join(keep) + f" {v[row, 6]:.7f} {v[row, 7]:.7f}"
            text = "\n".join(out)
            text = re.sub(rf"^TEXTURE {re.escape(tex)}_TEXT\.dds$", f"TEXTURE {tex}_PACK.dds", text, flags=re.M)
            text = re.sub(r"^TEXTURE_LIT A380_COCKPIT_DECAL_EMIS_LITA\.dds$", f"TEXTURE_LIT A380_COCKPIT_DECAL_EMIS_LITA_{tex}_PACK.dds", text, flags=re.M)
            (outdir / n).write_text(text, encoding="latin-1", newline="\n")
            print(f"  {n}: {len(vt)} vertices moved")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    main(*args[:4], measure="--measure" in sys.argv)
