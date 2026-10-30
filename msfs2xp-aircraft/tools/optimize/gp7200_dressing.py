"""usage: gp7200_dressing.py <dressing work dir> <installed objects dir> <stage objects dir>

Puts the GP fan-case dressing into the stage: the baked textures (finish_textures.py output in <work>/tex) with their DXT1 a380_engdress_gp_ALB.dds,
a380_engdress_gp.obj and the four falling copies placed on the GP engine axes by tools/engine_interior/place_interior_gp.py. The work dir is made by
  blender -b --factory-startup -P fan_case_dressing_gp.py -- <work>/fan_case_gp.obj <work>/fan_case_gp.blend
  blender -b <work>/fan_case_gp.blend -P bake_textures.py -- <work>/bake a380_engdress_gp 2048
  python finish_textures.py <work>/bake a380_engdress_gp <work>/tex a380_engdress_gp"""

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "engine_interior"))
import place_interior_gp as placer  # noqa: E402

DDSTOOL = "E:/fbw-build/target/release/ddstool.exe"
STEM = "a380_engdress_gp"


def dds_of(png, dst):
    img = Image.open(png).convert("RGBA")
    with tempfile.TemporaryDirectory() as tmp:
        raw = Path(tmp) / "raw.rgba"
        raw.write_bytes(np.asarray(img, np.uint8).tobytes())
        subprocess.run([DDSTOOL, "encode", str(raw), str(img.width), str(img.height), str(dst)], check=True, stdout=subprocess.DEVNULL)


def main(work, installed, stage):
    work, stage = Path(work), Path(stage)
    for kind in ("ALB", "NML", "MAT"):
        shutil.copy2(work / "tex" / f"{STEM}_{kind}.png", stage / f"{STEM}_{kind}.png")
    dds_of(stage / f"{STEM}_ALB.png", stage / f"{STEM}_ALB.dds")
    return placer.main(stage, work / "bake" / f"{STEM}.obj", STEM)


if __name__ == "__main__":
    print(main(*sys.argv[1:4]))
