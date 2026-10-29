"""usage: unshared_panels.py <aircraft objects dir> <copies.json> <ao originals dir> <backup dir> <work dir>,<work dir>...

Puts the pre-lettering composites (<work>/<name>_src.png) into the layout the cockpit objects use since
unshare.py: the islands it moved are copied to their new place (and the keyboard and pedestal knob
sheets doubled) before scaling to the installed size. text_layer.py had put the composites back in the
old layout, which left moved islands showing whatever lay under them (the black L/G GRVTY plate, the
scrambled keyboard tray). Panels with an occlusion original in <ao originals dir> get the corrected
composite there as their new original (run bake_fbw_ao.py --only on them next); the others go straight
into the objects dir. Everything replaced is copied to the backup dir first."""

import json
import shutil
import struct
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image

from bake import TEXPIPE
from unshare import extend_and_copy


def dds_size(path):
    h, w = struct.unpack("<II", open(path, "rb").read(20)[12:20])
    return w, h


def main(objdir, copies, ao, backup, works):
    objdir, ao, backup = Path(objdir), Path(ao), Path(backup)
    backup.mkdir(parents=True, exist_ok=True)
    info = json.load(open(copies))
    for name, inf in info.items():
        if inf["ext"] == 1 and not inf["rects"]:
            continue
        stem = name[:-4]
        src = next((Path(w) / f"{stem}_src.png" for w in works if (Path(w) / f"{stem}_src.png").is_file()), None)
        if src is None:
            print(f"{name}: no pre-lettering composite")
            continue
        img = extend_and_copy(np.asarray(Image.open(src).convert("RGB")), inf)
        dst = ao / name if (ao / name).is_file() else objdir / name
        w, h = dds_size(objdir / name)
        out = Image.fromarray(img)
        if out.size != (w, h):
            out = out.resize((w, h), Image.BOX)
        tag = "ao" if dst.parent == ao else "objects"
        if not (backup / tag / name).exists():
            (backup / tag).mkdir(exist_ok=True)
            shutil.copy2(dst, backup / tag / name)
        png = Path(works[0]) / f"{stem}_unshared_src.png"
        out.save(png)
        subprocess.run([TEXPIPE, "dds", str(png), str(dst)], check=True, capture_output=True)
        print(f"{name}: {len(inf['rects'])} islands moved, x{inf['ext']}, {w}x{h} -> {tag}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5].split(","))
