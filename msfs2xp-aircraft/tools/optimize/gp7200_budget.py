"""usage: gp7200_budget.py <objects dir> [<objects dir> ...]

Triangles, TRIS batches (draw calls) and texture files with their sizes for the GP7200 engine objects, so every detail group can be reported before / after.
Texture memory is reported two ways: as GPU memory if the file is loaded as uncompressed RGBA8 with mips (4 bytes per texel x 4/3) and as DXT5-compressed
with mips (1 byte per texel x 4/3); X-Plane compresses by user setting, so the truth is between the two."""

import json
import sys
from pathlib import Path

from PIL import Image

GP_OBJECTS = ["a380_exterior_022_gp.obj", "a380_exterior_023_gp.obj", "a380_exterior_020_gp.obj", "a380_exterior_021_gp.obj", "a380_exterior_030_gp.obj",
              "a380_exterior_031_gp.obj", "a380_exterior_001_gp.obj", "a380_engcore_gp.obj", "a380_engdress_gp.obj"]


def obj_stats(path):
    tris = batches = 0
    tex = []
    for l in Path(path).read_text(encoding="utf-8", errors="replace").split("\n"):
        t = l.split()
        if not t:
            continue
        if t[0] == "TRIS":
            batches += 1
            tris += int(t[2]) // 3
        elif t[0] == "TEXTURE":
            tex.append(t[1])
        elif t[0] == "TEXTURE_MAP":
            tex.append(t[2])
    return tris, batches, tex


def tex_size(folder, name):
    p = Path(folder) / name
    if not p.is_file():
        return None
    try:
        w, h = Image.open(p).size
    except Exception:
        return None
    return w, h


def main(folders):
    out = {}
    for folder in folders:
        folder = Path(folder)
        rows, textures = {}, {}
        for n in GP_OBJECTS:
            if not (folder / n).is_file():
                continue
            t, b, tx = obj_stats(folder / n)
            rows[n] = dict(triangles=t, batches=b)
            for name in tx:
                textures[name] = tex_size(folder, name)
        raw = comp = 0.0
        for name, sz in textures.items():
            if sz:
                raw += sz[0] * sz[1] * 4 * 4 / 3
                comp += sz[0] * sz[1] * 4 / 3
        out[str(folder)] = dict(objects=rows, total_triangles=sum(r["triangles"] for r in rows.values()), total_batches=sum(r["batches"] for r in rows.values()),
                                textures={k: v for k, v in sorted(textures.items())}, tex_mb_uncompressed=round(raw / 2 ** 20, 1), tex_mb_compressed=round(comp / 2 ** 20, 1))
    return out


if __name__ == "__main__":
    print(json.dumps(main(sys.argv[1:]), indent=1))
