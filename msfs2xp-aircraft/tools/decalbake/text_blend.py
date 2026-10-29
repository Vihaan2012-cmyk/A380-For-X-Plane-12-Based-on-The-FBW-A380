"""usage: text_blend.py <aircraft objects dir> <work dir>

Turns the cut-out lettering layer (ATTR_no_blend on *_SDF.dds) into FF's text overlay: a blended
sheet whose alpha is the smoothed outline anti-aliased over RAMP texels, with FF's flat
TEXTURE_NORMAL + NORMAL_METALNESS gloss (blue metalness, alpha gloss) and no polygon offset, as FF's
text_xp12.obj has it; ATTR_poly_os 1 left every label invisible in the sim. Alpha-tested strokes break
into dots once the mips average them below the cutoff; blended ones fade to a tint instead. Reads
work/<name>_SDF.png (sdf_install.py) and writes <name>_TEXT.dds and A380_COCKPIT_TEXT_NML.png."""

import re
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image

from bake import TEXPIPE
from sdf_install import SPREAD

RAMP = 1.0
GLOSS = 90
NML = "A380_COCKPIT_TEXT_NML.png"
HEAD = re.compile(r"^(TEXTURE_MAP|TEXTURE_NORMAL|NORMAL_METALNESS|GLOBAL_specular)\b.*\n", re.M)
DECALS = [f"a380_cockpit_{n:03d}.obj" for n in range(20, 26)]


def text_sheet(sdf_png, out_png):
    a = np.asarray(Image.open(sdf_png).convert("RGBA")).astype(np.float32)
    d = (a[..., 3] / 255.0 - 0.5) * 2 * SPREAD
    a[..., 3] = np.clip(0.5 + d / RAMP, 0, 1) * 255.0
    Image.fromarray((a + 0.5).clip(0, 255).astype(np.uint8), "RGBA").save(out_png)


def blend_object(text):
    def tex(m):
        return f"TEXTURE {m.group(1)}_TEXT.dds"

    text = re.sub(r"^TEXTURE (\S+)_SDF\.dds$", tex, text, flags=re.M)
    head, sep, body = text.partition("POINT_COUNTS")
    head = HEAD.sub("", head).rstrip("\n") + f"\nTEXTURE_NORMAL {NML}\nNORMAL_METALNESS\nGLOBAL_specular 1.0\n"
    text = head + sep + body
    text = re.sub(r"^(\s*)ATTR_poly_os 1$", r"\1ATTR_poly_os 0", text, flags=re.M)
    return re.sub(r"^(\s*)ATTR_no_blend 0\.5$", r"\1ATTR_blend", text, flags=re.M)


def main(objdir, work):
    objdir, work = Path(objdir), Path(work)
    Image.new("RGBA", (8, 8), (128, 128, 0, GLOSS)).save(objdir / NML)
    (objdir / "A380_COCKPIT_TEXT_MAT.png").unlink(missing_ok=True)
    for sdf in sorted(work.glob("*_SDF.png")):
        name = sdf.name.replace("_SDF.png", "_TEXT")
        png = work / f"{name}.png"
        text_sheet(sdf, png)
        subprocess.run([TEXPIPE, "dds", str(png), str(objdir / f"{name}.dds")], check=True, capture_output=True)
        print(f"{name}.dds")
    for n in DECALS:
        p = objdir / n
        before = p.read_text(encoding="utf-8")
        after = blend_object(before)
        p.write_text(after, encoding="utf-8", newline="\n")
        print(f"{n}: {after.count('ATTR_blend')} blended, {len(re.findall(r'_TEXT.dds', after))} text sheet")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
