"""usage: sdf_atlas.py <in.dds> <out.png> [--scale 2] [--spread 4]

Turns a decal atlas's alpha into a signed distance field for X-Plane's alpha test (ATTR_no_blend 0.5):
the alpha is reconstructed at SUPER x the source, smoothed by SMOOTH source texels so the outline follows
the glyph's curve instead of its pixels, thresholded at 0.5, and the distance to that edge is stored at
`scale` x the source, 0.5 on the edge, `spread` output texels to reach 0 or 1. Bilinear filtering then
keeps the thresholded edge a clean line at any magnification, and the averaged mips keep thin strokes
instead of fading them. Worked in overlapping tiles so the 8x reconstruction stays small. Colour is
carried outward from the ink so an edge texel never blends toward the transparent background."""

import sys

import numpy as np
from PIL import Image
from scipy import ndimage

SUPER = 8
SMOOTH = 0.45
TILE = 256
MARGIN = 12


def _tile_field(alpha, scale, spread):
    h, w = alpha.shape
    hi = np.asarray(Image.fromarray(alpha).resize((w * SUPER, h * SUPER), Image.BICUBIC)).astype(np.float32)
    hi = ndimage.gaussian_filter(hi, SMOOTH * SUPER)
    ink = hi >= 127.5
    reach = (spread / scale + 1) * SUPER
    if not ink.any():
        return np.zeros((h * scale, w * scale), np.float32)
    if ink.all():
        return np.ones((h * scale, w * scale), np.float32)
    outside = ndimage.distance_transform_edt(~ink)
    inside = ndimage.distance_transform_edt(ink)
    signed = (np.minimum(inside, reach) - np.minimum(outside, reach)) / SUPER * scale
    f = SUPER // scale
    signed = signed.reshape(h * scale, f, w * scale, f).mean(axis=(1, 3))
    return np.clip(0.5 + signed / (2 * spread), 0, 1).astype(np.float32)


def sdf_atlas(src, scale=2, spread=4.0):
    im = Image.open(src).convert("RGBA")
    w, h = im.size
    a = np.asarray(im).astype(np.float32)
    alpha = a[..., 3].astype(np.uint8)
    field = np.zeros((h * scale, w * scale), np.float32)
    for y0 in range(0, h, TILE):
        for x0 in range(0, w, TILE):
            ya, yb = max(0, y0 - MARGIN), min(h, y0 + TILE + MARGIN)
            xa, xb = max(0, x0 - MARGIN), min(w, x0 + TILE + MARGIN)
            t = _tile_field(alpha[ya:yb, xa:xb], scale, spread)
            y1, x1 = min(h, y0 + TILE), min(w, x0 + TILE)
            field[y0 * scale:y1 * scale, x0 * scale:x1 * scale] = t[(y0 - ya) * scale:(y1 - ya) * scale, (x0 - xa) * scale:(x1 - xa) * scale]
    rgb_src = a[..., :3]
    seen = a[..., 3] > 32
    if seen.any():
        _, (iy, ix) = ndimage.distance_transform_edt(~seen, return_indices=True)
        rgb_src = rgb_src[iy, ix]
    rgb = np.asarray(Image.fromarray(rgb_src.astype(np.uint8)).resize((w * scale, h * scale), Image.BICUBIC)).astype(np.float32)
    out = np.dstack([rgb, field * 255.0])
    return Image.fromarray((out + 0.5).clip(0, 255).astype(np.uint8), "RGBA")


if __name__ == "__main__":
    a = sys.argv[1:]
    scale = int(a[a.index("--scale") + 1]) if "--scale" in a else 2
    spread = float(a[a.index("--spread") + 1]) if "--spread" in a else 4.0
    sdf_atlas(a[0], scale, spread).save(a[1])
