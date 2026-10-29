"""usage: upgrade_4k.py <objects dir> <work dir> <backup dir> NAME...

Re-encodes baked-lettering panels at the full 4096 of their composite (<work>/<NAME>_baked.png) instead
of the 2048 resize.py left them at, so their labels get twice the texels. Each composite is first checked
against the installed texture (shrunk to its size, mean difference under MAX_DIFF levels) so only the
composite the installed panel actually came from is used; the old DDS goes to the backup dir."""

import os
import shutil
import struct
import subprocess
import sys

import numpy as np
from PIL import Image

from bake import TEXPIPE

MAX_DIFF = 6.0


def dds_size(path):
    h, w = struct.unpack("<II", open(path, "rb").read(20)[12:20])
    return w, h


def main(objdir, work, backup, names):
    os.makedirs(backup, exist_ok=True)
    for name in names:
        dds = os.path.join(objdir, name + ".dds")
        png = os.path.join(work, name + "_baked.png")
        if not (os.path.exists(dds) and os.path.exists(png)):
            print(f"{name}: missing")
            continue
        comp = Image.open(png).convert("RGB")
        w, h = dds_size(dds)
        if comp.size[0] <= w:
            print(f"{name}: already {w}")
            continue
        installed = np.asarray(Image.open(dds).convert("RGB")).astype(np.float32)
        shrunk = np.asarray(comp.resize((w, h), Image.BOX)).astype(np.float32)
        diff = float(np.abs(installed - shrunk).mean())
        if diff > MAX_DIFF:
            print(f"{name}: composite differs from the installed panel by {diff:.1f} levels, left alone")
            continue
        tmp = os.path.join(work, name + "_4096.png")
        Image.open(png).save(tmp)
        out = os.path.join(work, name + "_4096.dds")
        subprocess.run([TEXPIPE, "dds", tmp, out], check=True, capture_output=True)
        if not os.path.exists(os.path.join(backup, name + ".dds")):
            shutil.copy2(dds, os.path.join(backup, name + ".dds"))
        shutil.copy2(out, dds)
        print(f"{name}: {w} -> {dds_size(dds)[0]} (composite matched within {diff:.1f} levels)")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:])
