"""usage: sdf_install.py <pre-bake objects dir> <aircraft objects dir> <work dir> NAME.dds...

Builds one signed distance field from the first atlas's alpha (sdf_atlas._tile_field, in parallel over
tiles) at the atlas's own resolution, and writes every listed atlas as <name>_SDF.dds with that field
as its alpha and its own colour carried outward from the ink. The variants are tinted copies of the
same lettering sheet, so one field serves them all."""

import os
import subprocess
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

from bake import TEXPIPE
from sdf_atlas import MARGIN, TILE, _tile_field

SCALE = 1
SPREAD = 4.0


def _job(args):
    alpha, y0, x0, ya, xa, y1, x1 = args
    t = _tile_field(alpha, SCALE, SPREAD)
    return y0, x0, t[(y0 - ya) * SCALE:(y1 - ya) * SCALE, (x0 - xa) * SCALE:(x1 - xa) * SCALE]


def field_of(alpha):
    h, w = alpha.shape
    jobs = []
    for y0 in range(0, h, TILE):
        for x0 in range(0, w, TILE):
            ya, yb = max(0, y0 - MARGIN), min(h, y0 + TILE + MARGIN)
            xa, xb = max(0, x0 - MARGIN), min(w, x0 + TILE + MARGIN)
            jobs.append((alpha[ya:yb, xa:xb].copy(), y0, x0, ya, xa, min(h, y0 + TILE), min(w, x0 + TILE)))
    field = np.zeros((h * SCALE, w * SCALE), np.float32)
    with ProcessPoolExecutor(max_workers=max(1, (os.cpu_count() or 4) - 2)) as pool:
        for y0, x0, t in pool.map(_job, jobs, chunksize=4):
            field[y0 * SCALE:y0 * SCALE + t.shape[0], x0 * SCALE:x0 * SCALE + t.shape[1]] = t
    return field


def colour_of(rgba, size):
    rgb = rgba[..., :3]
    seen = rgba[..., 3] > 32
    if seen.any():
        _, (iy, ix) = ndimage.distance_transform_edt(~seen, return_indices=True)
        rgb = rgb[iy, ix]
    img = Image.fromarray(rgb.astype(np.uint8))
    return np.asarray(img.resize(size, Image.BICUBIC) if img.size != size else img).astype(np.float32)


def main(prebake, objdir, work, names):
    prebake, objdir, work = Path(prebake), Path(objdir), Path(work)
    first = np.asarray(Image.open(prebake / names[0]).convert("RGBA"))
    field = field_of(first[..., 3].copy())
    size = (field.shape[1], field.shape[0])
    for n in names:
        rgba = np.asarray(Image.open(prebake / n).convert("RGBA")).astype(np.float32)
        out = np.dstack([colour_of(rgba, size), field * 255.0])
        png = work / n.replace(".dds", "_SDF.png")
        Image.fromarray((out + 0.5).clip(0, 255).astype(np.uint8), "RGBA").save(png)
        dst = objdir / n.replace(".dds", "_SDF.dds")
        subprocess.run([TEXPIPE, "dds", str(png), str(dst)], check=True, capture_output=True)
        print(f"{dst.name}: {size[0]}x{size[1]} distance field")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:])
