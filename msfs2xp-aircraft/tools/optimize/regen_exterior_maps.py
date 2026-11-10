"""usage: regen_exterior_maps.py <aircraft objects dir> <FBW MSFS texture dir> <backup dir> [--max N] [--apply] NAME...

Rebuilds exterior normal (_NML.png) and metal/gloss (_MAT.png) maps from FlyByWire's MSFS sources at up to
--max (default 2048) instead of the size the converter's shrink-to-detail rule left them at (256 for the
wings, gear, pylons and fans): thin panel lines, seams and fastener rows are a small share of a map's
texels, so a whole-map RMS measure lets them go. Conversion matches msfs2xp's texture module: normal red,
255-green (MSFS is DirectX style), blue empty; material red = metalness x gloss / 255, green =
gloss = (1 - roughness)^2.2. Texels an MSFS COMP map leaves unused (all channels 0) are filled from their
neighbours by push-pull, so mips never average a mirror-smooth padding into the real surface.
Originals go to the backup dir once."""
import shutil
import sys
from pathlib import Path

import numpy as np
from PIL import Image


def source_for(name, fbw):
    stem = name[: -len("_NML.png")] if name.endswith("_NML.png") else name[: -len("_MAT.png")]
    for cand in (f"{stem}.PNG.DDS", f"{stem}.png.DDS", f"{stem}.DDS"):
        if (fbw / cand).is_file():
            return fbw / cand
    return None


def push_pull(values, valid):
    """Fills invalid texels with the average of valid ones at the coarsest level that has any."""
    levels = [(values * valid[..., None], valid.astype(np.float32))]
    while min(levels[-1][1].shape) > 1:
        v, w = levels[-1]
        h2, w2 = v.shape[0] // 2 * 2, v.shape[1] // 2 * 2
        v, w = v[:h2, :w2], w[:h2, :w2]
        v = v.reshape(h2 // 2, 2, w2 // 2, 2, -1).sum(axis=(1, 3))
        w = w.reshape(h2 // 2, 2, w2 // 2, 2).sum(axis=(1, 3))
        levels.append((v, w))
    filled = levels[-1][0] / np.maximum(levels[-1][1], 1e-6)[..., None]
    for v, w in reversed(levels[:-1]):
        up = np.repeat(np.repeat(filled, 2, axis=0), 2, axis=1)
        up = np.pad(up, ((0, v.shape[0] - up.shape[0]), (0, v.shape[1] - up.shape[1]), (0, 0)), mode="edge")
        own = v / np.maximum(w, 1e-6)[..., None]
        filled = np.where((w > 0)[..., None], own, up)
    return filled


def fit(img, size):
    w, h = img.size
    k = size / max(w, h)
    return img.resize((max(1, round(w * k)), max(1, round(h * k))), Image.Resampling.LANCZOS)


def uv_mask(objects, name, w, h):
    """The texels some object's triangles sample, rasterized from their UVs; None when no object uses the map."""
    from PIL import ImageDraw
    mask = Image.new("L", (w, h), 0)
    draw = ImageDraw.Draw(mask)
    hit = False
    for obj in objects.glob("*.obj"):
        with open(obj, "rb") as f:
            head = f.read(4000)
        if name.encode() not in head:
            continue
        uv, idx, tris = [], [], []
        for line in open(obj, encoding="latin-1", errors="replace"):
            t = line.lstrip()
            if t.startswith("VT"):
                q = t.split()
                uv.append((float(q[7]), float(q[8])))
            elif t.startswith("IDX"):
                idx.extend(int(x) for x in t.split()[1:])
            elif t.startswith("TRIS"):
                q = t.split()
                tris.append((int(q[1]), int(q[2])))
        uv = np.array(uv) if uv else np.zeros((0, 2))
        for off, cnt in tris:
            for i in range(off, off + cnt - 2, 3):
                pts = uv[idx[i:i + 3]]
                if len(pts) < 3:
                    continue
                base = np.floor(pts.min(axis=0))
                pts = pts - base
                draw.polygon([(u * w, (1 - v) * h) for u, v in pts], fill=255)
                hit = True
    return (np.asarray(mask) > 0) if hit else None


def grow(mask, px=2):
    m = mask.copy()
    for _ in range(px):
        g = m.copy()
        g[1:] |= m[:-1]; g[:-1] |= m[1:]; g[:, 1:] |= m[:, :-1]; g[:, :-1] |= m[:, 1:]
        m = g
    return m


UV_OBJECTS = None


def normal(src, size):
    n = np.asarray(Image.open(src).convert("RGB")).astype(np.float32)
    used = n[..., :2].max(axis=2) > 2
    if UV_OBJECTS is not None:
        m = uv_mask(UV_OBJECTS[0], UV_OBJECTS[1], n.shape[1], n.shape[0])
        if m is not None:
            used = grow(m)
    if used.any() and not used.all():
        n = push_pull(n, used)
    n = np.asarray(fit(Image.fromarray(np.clip(n + 0.5, 0, 255).astype(np.uint8), "RGB"), size)).astype(np.uint8)
    out = np.zeros_like(n)
    out[..., 0] = n[..., 0]
    out[..., 1] = 255 - n[..., 1]
    return Image.fromarray(out, "RGB")


def material(src, size):
    c = np.asarray(Image.open(src).convert("RGB")).astype(np.float32)
    used = c.max(axis=2) > 0
    if UV_OBJECTS is not None:
        m = uv_mask(UV_OBJECTS[0], UV_OBJECTS[1], c.shape[1], c.shape[0])
        if m is not None:
            used = grow(m)
    if used.any() and not used.all():
        c = push_pull(c, used)
    c = np.asarray(fit(Image.fromarray(np.clip(c + 0.5, 0, 255).astype(np.uint8), "RGB"), size)).astype(np.float32)
    gloss = ((1 - c[..., 1] / 255.0) ** 2.2) * 255.0
    metal = c[..., 2] * gloss / 255.0
    out = np.zeros(c.shape, np.uint8)
    out[..., 0] = np.clip(metal + 0.5, 0, 255)
    out[..., 1] = np.clip(gloss + 0.5, 0, 255)
    return Image.fromarray(out, "RGB")


def main(objects, fbw, backup, names, cap, apply):
    for n in names:
        src = source_for(n, fbw)
        inst = objects / n
        if src is None or not inst.is_file():
            print(f"  {n:52} no source, left")
            continue
        have = Image.open(inst).size[0]
        size = min(cap, max(Image.open(src).size))
        if size <= have:
            print(f"  {n:52} already {have}, left")
            continue
        global UV_OBJECTS
        UV_OBJECTS = (objects, n) if USE_UV else None
        img = normal(src, size) if n.endswith("_NML.png") else material(src, size)
        if apply:
            backup.mkdir(parents=True, exist_ok=True)
            if not (backup / n).exists():
                shutil.copy2(inst, backup / n)
            img.save(inst, optimize=False, compress_level=6)
        print(f"  {n:52} {have} -> {size}{'' if apply else ' (dry run)'}")


USE_UV = "--uv" in sys.argv

if __name__ == "__main__":
    a = sys.argv[1:]
    cap = int(a[a.index("--max") + 1]) if "--max" in a else 2048
    pos = [x for i, x in enumerate(a) if not x.startswith("--") and (i == 0 or a[i - 1] != "--max")]
    main(Path(pos[0]), Path(pos[1]), Path(pos[2]), pos[3:], cap, "--apply" in a)
