"""usage: finish_textures.py <bake dir> <stem> <objects dir> [<out stem>]

Turns bake_textures.py's raw maps into the three X-Plane textures the
placed object names: <stem>_ALB.png (colour darkened by its ambient
occlusion), <stem>_NML.png (tangent normal in red/green, blue cleared like
the aircraft's other normal maps) and <stem>_MAT.png (red metalness, green
gloss)."""

import sys
from pathlib import Path

import numpy as np
from PIL import Image


def load(path):
    return np.asarray(Image.open(path).convert("RGB")).astype(np.float32) / 255.0


def main(bake_dir, stem, objects_dir, out_stem=None):
    b, out = Path(bake_dir), Path(objects_dir)
    name = out_stem or stem
    col = load(b / f"{stem}_col_raw.png")
    ao = load(b / f"{stem}_ao_raw.png")[..., :1]
    nrm = load(b / f"{stem}_nrm_raw.png")
    rough = load(b / f"{stem}_rough_raw.png")[..., 0]
    metal = load(b / f"{stem}_metal_raw.png")[..., 0]
    albedo = np.clip(col * (0.6 + 0.4 * ao), 0, 1)
    Image.fromarray((albedo * 255 + 0.5).astype(np.uint8)).save(out / f"{name}_ALB.png")
    nml = np.zeros_like(nrm)
    nml[..., :2] = nrm[..., :2]
    empty = (col.sum(axis=2) < 0.004)
    nml[empty, 0] = 0.5
    nml[empty, 1] = 0.5
    Image.fromarray((nml * 255 + 0.5).astype(np.uint8)).save(out / f"{name}_NML.png")
    mat = np.zeros_like(nrm)
    mat[..., 0] = metal
    mat[..., 1] = 1.0 - rough
    Image.fromarray((mat * 255 + 0.5).astype(np.uint8)).save(out / f"{name}_MAT.png")
    print(stem, "textures", albedo.shape[1], "px")


if __name__ == "__main__":
    main(*sys.argv[1:5])
