"""usage: gp7200_texel.py <objects dir> <side LH|RH> r,g,b [tolerance]

Finds an atlas texel of the nacelle ENG_{LH,RH} maps that is (nearly) uniform over a 21 px square, has a flat tangent normal, and has the wanted albedo colour in the base albedo
and in every livery albedo (2K and 4K, up to a few exceptions that are listed). Used to point flat geometry (panel, slats, rub strip) at an existing texel, so no texture changes."""

import glob
import json
import os
import sys

import numpy as np
from PIL import Image
from scipy.ndimage import minimum_filter, uniform_filter


def load(p, size=2048):
    im = Image.open(p).convert("RGB")
    if im.size[0] != size:
        im = im.resize((size, size), Image.BILINEAR)
    return np.asarray(im).astype(np.float32)


def find(ac, side, target, tol=22, allow=3, erosion=21):
    n = load(ac + f"objects/A380_EXTERIOR_ENG_{side}_NORM_NML.png")
    files = sorted(glob.glob(ac + f"liveries/*/objects/A380_EXTERIOR_ENG_{side}_ALBEDO.dds")) + [ac + f"objects/A380_EXTERIOR_ENG_{side}_ALBEDO.dds"]
    target = np.array(target, np.float32)
    votes = np.zeros((2048, 2048), np.int16)
    score = np.zeros((2048, 2048), np.float32)
    per = []
    for p in files:
        a = load(p)
        mean = uniform_filter(a, (5, 5, 1))
        sd = np.sqrt(np.maximum(uniform_filter(a * a, (5, 5, 1)) - mean ** 2, 0))
        ok = (np.abs(mean - target).max(2) < tol) & (sd.max(2) < 16)
        votes += ok
        score += np.abs(mean - target).max(2)
        per.append(ok)
    flat = (np.abs(uniform_filter(n[..., 0], 5) - 127.5) < 6) & (np.abs(uniform_filter(n[..., 1], 5) - 127.5) < 6) & (uniform_filter(np.abs(n[..., 0] - 127.5) + np.abs(n[..., 1] - 127.5), 5) < 12)
    c = minimum_filter((flat & (votes >= len(files) - allow)).astype(np.uint8), size=erosion) > 0
    if not c.any():
        return None
    sc = np.where(c, score, 1e12)
    y, x = np.unravel_index(sc.argmin(), sc.shape)
    fails = [os.path.normpath(f).split(os.sep)[-3] if "liveries" in f else "base" for f, o in zip(files, per) if not o[y, x]]
    return dict(uv=((x + 0.5) / 2048, 1 - (y + 0.5) / 2048), base_albedo=load(files[-1])[y, x].tolist(), fails=fails, files=len(files))


if __name__ == "__main__":
    a = sys.argv[1:]
    print(json.dumps(find(a[0], a[1], [float(v) for v in a[2].split(",")], float(a[3]) if len(a) > 3 else 22), indent=1))
