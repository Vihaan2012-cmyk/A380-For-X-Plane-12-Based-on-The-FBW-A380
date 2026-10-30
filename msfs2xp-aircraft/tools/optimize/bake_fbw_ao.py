"""usage: bake_fbw_ao.py <aircraft objects dir> <FBW MSFS texture dir> <backup dir> [--only NAME,...] [--out DIR] [--text-work DIR,...] [--copies JSON] [--local X] [--global X] [--floor X]

Puts FlyByWire's own ambient occlusion (the red channel of each material's MSFS COMP texture) into the
X-Plane albedo it belongs to, the way MSFS shows it: MSFS applies that occlusion to ambient light only, so
a straight multiply (texture/mod.rs bake_occlusion) darkens busy panels by a third or more. Here only the
occlusion's local detail is taken - each texel against the occlusion around it - so contact shadows,
recesses and seams darken while the panel keeps its brightness, plus a small share of the absolute
occlusion for depth. Done in linear light; the DDS keeps its format (DXT1/DXT5), size and mip count.
Originals go to the backup dir and are always the source, so re-running never bakes twice. Lettering
baked into a panel by tools/decalbake (<work>/<name>_baked.png against <name>_src.png) is kept out of the
occlusion, since FBW's map describes the panel under it, not the text."""

import io
import json
import os
import re
import shutil
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

LOCAL = float(os.environ.get("BAKE_AO_LOCAL", 1.0))
GLOBAL = float(os.environ.get("BAKE_AO_GLOBAL", 0.40))
FLOOR = float(os.environ.get("BAKE_AO_FLOOR", 0.22))
BLUR_FRACTION = 0.03
SKIP = re.compile(r"DECAL|EMIS|TXTLIT|_LIT|DISPLAY|GLASS|BLUR|SCREEN|KEYBOARD", re.I)


def to_lin(c):
    c = c / 255.0
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def to_srgb(c):
    c = np.clip(c, 0, 1)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * c ** (1 / 2.4) - 0.055) * 255.0


def occlusion_factor(ao, size):
    a = Image.fromarray((ao * 255).astype(np.uint8)).resize(size, Image.BILINEAR)
    ao = np.asarray(a).astype(np.float32) / 255.0
    radius = max(2, int(max(size) * BLUR_FRACTION))
    around = np.asarray(a.filter(ImageFilter.GaussianBlur(radius))).astype(np.float32) / 255.0
    local = np.minimum(1.0, (ao + 0.02) / (around + 0.02))
    f = (1 - LOCAL * (1 - local)) * (1 - GLOBAL * (1 - ao))
    return np.maximum(f, FLOOR)


def dds_payload(img, fmt):
    w, h = img.size
    if w < 4 or h < 4:
        pad = Image.new("RGBA", (max(4, w), max(4, h)))
        pad.paste(img, (0, 0))
        img = pad
    buf = io.BytesIO()
    img.save(buf, "DDS", pixel_format=fmt)
    return buf.getvalue()[128:]


def comp_for(name, fbw):
    stem = re.split(r"_ALBEDO", name, flags=re.I)[0]
    for cand in (f"{stem}_COMP.PNG.DDS", f"{stem}_COMP.png.DDS", f"{stem}_COMP.DDS"):
        p = fbw / cand
        if p.is_file():
            return p
    low = stem.lower() + "_comp"
    for p in fbw.iterdir():
        if p.name.lower().startswith(low) and p.name.lower().endswith(".dds"):
            return p
    return None


def text_mask(name, works, size, copies=None):
    stem = name[:-4]
    for w in works:
        for s in (stem, stem.replace("_CLEAN", "")):
            baked, src = Path(w) / f"{s}_baked.png", Path(w) / f"{s}_src.png"
            if baked.is_file() and src.is_file():
                b = np.asarray(Image.open(baked).convert("RGB")).astype(np.int16)
                o = Image.open(src).convert("RGB")
                if copies and name in copies and o.size[1] * copies[name]["ext"] == b.shape[0] * o.size[0] // b.shape[1]:
                    o = Image.fromarray(moved(np.asarray(o), copies[name]))
                if o.size != (b.shape[1], b.shape[0]):
                    o = o.resize((b.shape[1], b.shape[0]), Image.BOX)
                ink = (np.abs(b - np.asarray(o).astype(np.int16)).max(axis=2) > 10).astype(np.uint8) * 255
                m = Image.fromarray(ink).resize(size, Image.BOX).filter(ImageFilter.MaxFilter(5))
                return np.asarray(m) > 0
    return None


def moved(ao, inf, grid=4096):
    s = ao.shape[1] / grid
    out = np.zeros((ao.shape[0] * inf["ext"],) + ao.shape[1:], ao.dtype)
    out[:ao.shape[0]] = ao
    src = out.copy()
    for oy, ox, h, w, ny, nx in inf["rects"]:
        a = [int(round(v * s)) for v in (oy, ox, h, w, ny, nx)]
        out[a[4]:a[4] + a[2], a[5]:a[5] + a[3]] = src[a[0]:a[0] + a[2], a[1]:a[1] + a[3]]
    return out


def bake(src, comp, out, works=(), copies=None):
    raw = src.read_bytes()
    header = raw[:128]
    four = header[84:88]
    if four not in (b"DXT1", b"DXT5"):
        return f"skipped ({four!r})"
    mips = int.from_bytes(header[28:32], "little") or 1
    img = Image.open(src).convert("RGBA")
    rgba = np.asarray(img).astype(np.float32)
    ao = np.asarray(Image.open(comp).convert("RGBA")).astype(np.float32)[..., 0] / 255.0
    if copies and out.name in copies:
        ao = moved(ao, copies[out.name])
    f = occlusion_factor(ao, img.size)
    mask = text_mask(out.name, works, img.size, copies)
    if mask is not None:
        f[mask] = 1.0
    rgba[..., :3] = to_srgb(to_lin(rgba[..., :3]) * f[..., None])
    level = Image.fromarray((rgba + 0.5).clip(0, 255).astype(np.uint8), "RGBA")
    payload = []
    fmt = four.decode()
    for _ in range(mips):
        payload.append(dds_payload(level, fmt))
        w, h = level.size
        level = level.resize((max(1, w // 2), max(1, h // 2)), Image.Resampling.LANCZOS)
    data = header + b"".join(payload)
    if len(data) != len(raw):
        return f"size mismatch {len(data)} vs {len(raw)}, left unchanged"
    out.write_bytes(data)
    return f"baked, mean factor {f.mean():.2f}" + (f", {int(mask.sum())} lettering texels kept" if mask is not None else "")


def main(objects_dir, fbw_dir, backup_dir, only=None, out_dir=None, works=(), copies=None):
    objects, fbw, backup = Path(objects_dir), Path(fbw_dir), Path(backup_dir)
    copies = json.load(open(copies)) if copies else None
    out = Path(out_dir) if out_dir else objects
    backup.mkdir(parents=True, exist_ok=True)
    out.mkdir(parents=True, exist_ok=True)
    names = sorted(p.name for p in objects.glob("*.dds") if re.search(r"_ALBEDO", p.name, re.I) and not SKIP.search(p.name))
    if only:
        names = [n for n in names if any(o.lower() in n.lower() for o in only)]
    for n in names:
        comp = comp_for(n, fbw)
        if comp is None:
            continue
        original = backup / n
        if not original.exists():
            if out == objects:
                shutil.copy2(objects / n, original)
            else:
                original = objects / n
        print(f"  {n:58} <- {comp.name:46} {bake(original, comp, out / n, works, copies)}")


if __name__ == "__main__":
    a = sys.argv[1:]
    only = a[a.index("--only") + 1].split(",") if "--only" in a else None
    out = a[a.index("--out") + 1] if "--out" in a else None
    works = a[a.index("--text-work") + 1].split(",") if "--text-work" in a else ()
    copies = a[a.index("--copies") + 1] if "--copies" in a else None
    for flag, name in (("--local", "LOCAL"), ("--global", "GLOBAL"), ("--floor", "FLOOR")):
        if flag in a:
            globals()[name] = float(a[a.index(flag) + 1])
    pos = [x for i, x in enumerate(a) if not x.startswith("--") and (i == 0 or a[i - 1] not in ("--only", "--out", "--text-work", "--copies", "--local", "--global", "--floor"))]
    main(pos[0], pos[1], pos[2], only, out, works, copies)
