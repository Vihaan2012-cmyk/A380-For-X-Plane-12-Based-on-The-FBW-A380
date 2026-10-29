"""usage: text_mips.py <aircraft objects dir> <backup dir> <work dir>[,<work dir>...] [--only NAME,...]

Rebuilds the mip chain of the lettered cockpit textures so thin lettering keeps
its brightness at every level. Level 0 is kept byte for byte; each smaller
level is a Lanczos reduction of the one above, except where the lettering mask
covers at least a quarter of a 2x2 block, which takes the mean colour of the
lettering texels in it, for the first four levels. Albedos take their mask from the bake composites
(<name>_baked.png against <name>_src.png); _TXTLIT and _EMMIS_TXT glow
textures from their own bright texels.
"""
import shutil
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

BLOCK = {b"DXT1": 8, b"DXT5": 16}
TEXT_LEVELS = 4


def level_bytes(w, h, four):
    return max(1, (w + 3) // 4) * max(1, (h + 3) // 4) * BLOCK[four]


def encode(img, four):
    import io
    w, h = img.size
    if w < 4 or h < 4:
        pad = Image.new("RGBA", (max(4, w), max(4, h)))
        pad.paste(img, (0, 0))
        img = pad
    buf = io.BytesIO()
    img.save(buf, "DDS", pixel_format=four.decode())
    return buf.getvalue()[128:]


def albedo_mask(name, works, size):
    stem = name[:-4]
    for w in works:
        baked, src = Path(w) / f"{stem}_baked.png", Path(w) / f"{stem}_src.png"
        if baked.is_file() and src.is_file():
            b = Image.open(baked).convert("RGB")
            if b.size[0] * size[1] != b.size[1] * size[0]:
                return None
            o = Image.open(src).convert("RGB")
            if o.size != b.size:
                o = o.resize(b.size, Image.BOX)
            ink = (np.abs(np.asarray(b).astype(np.int16) - np.asarray(o).astype(np.int16)).max(axis=2) > 10).astype(np.uint8) * 255
            return np.asarray(Image.fromarray(ink).resize(size, Image.BOX)) > 127
    return None


def glow_mask(rgba):
    return rgba[..., :3].max(axis=2) > 40


def reduce(prev, mask):
    h, w = prev.shape[:2]
    nw, nh = max(1, w // 2), max(1, h // 2)
    lan = np.asarray(Image.fromarray(prev).resize((nw, nh), Image.Resampling.LANCZOS)).astype(np.float32)
    if mask is None or w < 2 or h < 2:
        return lan.clip(0, 255).astype(np.uint8), None
    p = prev[: nh * 2, : nw * 2].astype(np.float32).reshape(nh, 2, nw, 2, -1)
    m = mask[: nh * 2, : nw * 2].reshape(nh, 2, nw, 2)
    count = m.sum(axis=(1, 3))
    ink_sum = (p * m[..., None]).sum(axis=(1, 3))
    ink_mean = ink_sum / np.maximum(count, 1)[..., None]
    use = count >= 1
    out = np.where(use[..., None], ink_mean, lan)
    return out.clip(0, 255).astype(np.uint8), count >= 1


def rebuild(path, mask_fn):
    raw = path.read_bytes()
    header = raw[:128]
    four = header[84:88]
    if four not in BLOCK:
        return f"skipped ({four!r})"
    h, w = int.from_bytes(header[12:16], "little"), int.from_bytes(header[16:20], "little")
    mips = int.from_bytes(header[28:32], "little") or 1
    l0 = level_bytes(w, h, four)
    img = np.asarray(Image.open(path).convert("RGBA"))
    mask = mask_fn(img)
    if mask is None:
        return "no lettering mask, left unchanged"
    payload = [raw[128:128 + l0]]
    level, lm = img, mask
    lw, lh = w, h
    for k in range(1, mips):
        level, lm = reduce(level, lm if k <= TEXT_LEVELS else None)
        lw, lh = max(1, lw // 2), max(1, lh // 2)
        data = encode(Image.fromarray(level, "RGBA"), four)[: level_bytes(lw, lh, four)]
        payload.append(data)
    out = header + b"".join(payload)
    if len(out) != len(raw):
        return f"size mismatch {len(out)} vs {len(raw)}, left unchanged"
    path.write_bytes(out)
    return f"rebuilt {mips - 1} levels, {int(mask.sum())} lettering texels"


def main(objects, backup, works, only=None):
    objects, backup = Path(objects), Path(backup)
    backup.mkdir(parents=True, exist_ok=True)
    names = set()
    for w in works:
        for b in Path(w).glob("*_baked.png"):
            names.add(b.name[: -len("_baked.png")] + ".dds")
    for p in objects.glob("*.dds"):
        u = p.name.upper()
        if u.endswith("_TXTLIT.DDS") or u.endswith("_EMMIS_TXT.DDS") or u.endswith("_EMIS_TXT.DDS"):
            names.add(p.name)
    for name in sorted(names):
        if only and name not in only:
            continue
        path = objects / name
        if not path.is_file():
            continue
        if not (backup / name).exists():
            shutil.copy2(path, backup / name)
        src = backup / name
        shutil.copy2(src, path)
        u = name.upper()
        if "_TXTLIT" in u or "_TXT." in u:
            fn = glow_mask
        else:
            fn = lambda img, n=name: albedo_mask(n, works, (img.shape[1], img.shape[0]))
        print(f"{name}: {rebuild(path, fn)}", flush=True)


if __name__ == "__main__":
    a = sys.argv[1:]
    only = set(a[a.index("--only") + 1].split(",")) if "--only" in a else None
    pos = [x for i, x in enumerate(a) if not x.startswith("--") and (i == 0 or a[i - 1] != "--only")]
    main(pos[0], pos[1], pos[2].split(","), only)
