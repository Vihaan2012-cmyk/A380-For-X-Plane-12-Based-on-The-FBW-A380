"""usage: decal_mips.py <backup dir> <decal dds>...

Rebuilds a blended decal's colour so its edges never darken: the converter's DDS writer leaves the
clear texels of every mip below level 0 black, and filtering a letter's edge against them draws a
dark outline (the registration plates). Level 0 alpha is kept; every clear texel takes the colour of
the nearest inked one, and the mips are rebuilt with coverage-weighted colour (text_repack.write_dds).
Each file is copied to the backup dir before it is replaced."""

import shutil
import sys
from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

from text_repack import write_dds


def carry_colour(rgba):
    out = rgba.astype(np.float32)
    ink = rgba[..., 3] > 0
    if not ink.any():
        return out
    _, (iy, ix) = ndimage.distance_transform_edt(~ink, return_indices=True)
    out[..., :3] = out[iy, ix, :3]
    return out


def main(backup, paths):
    backup = Path(backup)
    for p in map(Path, paths):
        keep = backup / p.parent.parent.name / p.name
        keep.parent.mkdir(parents=True, exist_ok=True)
        if not keep.exists():
            shutil.copy2(p, keep)
        write_dds(carry_colour(np.asarray(Image.open(keep).convert("RGBA"))), p, False)
        print(f"{p.parent.parent.name}/{p.name}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2:])
