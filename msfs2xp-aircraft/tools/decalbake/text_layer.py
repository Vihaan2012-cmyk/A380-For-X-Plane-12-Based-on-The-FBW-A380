"""usage: text_layer.py <aircraft objects dir> <pre-bake objects dir> <backup dir> <work dir>,<work dir>... [copies.json]

Undoes the 12 Oct lettering bake in favour of an FF-style text layer:

- the decal objects come back whole from the pre-bake copies (with the cockpit-shake wrapper added since),
  drawn as alpha-tested cut-outs (ATTR_no_blend 0.5) from signed-distance-field atlases (sdf_atlas.py)
  instead of blended, so every label edge stays sharp at any zoom and has no halo;
- each baked panel gets its pre-lettering composite (<work>/<name>_src.png) back at the installed
  resolution or higher, never lower, with the islands unshare.py moved (copies.json) moved again;
- the text-only glow textures (_TXTLIT) become black and the merged emissive ones (_TXT) go back to the
  emissive they were made from, since the decals carry their own glow again.

Everything replaced is copied to the backup dir first. Then run text_blend.py, decal_offset.py and
bake_fbw_ao.py --copies on the panels."""

import json
import os
import re
import shutil
import struct
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image

from bake import TEXPIPE
from sdf_atlas import sdf_atlas
from unshare import extend_and_copy

DECAL_OBJECTS = [f"a380_cockpit_0{n}.obj" for n in (20, 21, 22, 23, 24, 25)]
WRAP = ["ANIM_begin", "ANIM_trans -1 0 0 1 0 0 -1 1 fbw/anim/buffet_x", "ANIM_trans 0 -1 0 0 1 0 -1 1 fbw/anim/buffet_y",
        "ANIM_trans 0 0 -1 0 0 1 -1 1 fbw/anim/buffet_z"]
EXTRA_PANELS = ["A380_COCKPIT_MIP_KNOBS_02_4K_ALBEDO.dds"]


def dds_size(path):
    h, w = struct.unpack("<II", open(path, "rb").read(20)[12:20])
    return w, h


def encode(img, out, work):
    tmp = Path(work) / (Path(out).stem + "_textlayer.png")
    img.save(tmp)
    subprocess.run([TEXPIPE, "dds", str(tmp), str(out)], check=True, capture_output=True)
    tmp.unlink()


def keep(path, backup):
    target = Path(backup) / Path(path).name
    if Path(path).exists() and not target.exists():
        shutil.copy2(path, target)


def sdf_name(tex):
    return re.sub(r"\.dds$", "_SDF.dds", tex)


def restore_decal_object(src, dst):
    lines = Path(src).read_text(encoding="latin-1").split("\n")
    pc = next(i for i, l in enumerate(lines) if l.startswith("POINT_COUNTS"))
    body_start = next(i for i in range(pc + 1, len(lines)) if lines[i].split()[:1] and lines[i].split()[0] not in ("VT", "IDX", "IDX10"))
    insert = body_start + 1 if lines[body_start].startswith("ATTR_LOD") else body_start
    last = max(i for i, l in enumerate(lines) if l.strip())
    out = lines[:insert] + WRAP + lines[insert:last + 1] + ["ANIM_end", ""]
    textures = []
    for i, l in enumerate(out):
        w = l.split()
        if not w:
            continue
        if w[0] == "TEXTURE":
            textures.append(w[1])
            out[i] = f"TEXTURE {sdf_name(w[1])}"
        elif w[0] == "ATTR_blend":
            out[i] = "ATTR_no_blend 0.5"
        elif w[0] == "ATTR_poly_os":
            out[i] = "ATTR_poly_os 0"
    Path(dst).write_text("\n".join(out), encoding="latin-1", newline="\n")
    return textures


def main(objdir, prebake, backup, works, copies=None):
    objdir, prebake, backup = Path(objdir), Path(prebake), Path(backup)
    copies = json.load(open(copies)) if copies else {}
    backup.mkdir(parents=True, exist_ok=True)
    work0 = works[0]
    atlases = set()
    for n in DECAL_OBJECTS:
        keep(objdir / n, backup)
        textures = restore_decal_object(prebake / n, objdir / n)
        atlases.update(textures)
        tris = sum(int(l.split()[2]) // 3 for l in (objdir / n).read_text(encoding="latin-1").split("\n") if l.startswith("TRIS"))
        print(f"{n}: restored, {tris} decal triangles, cut-out")
    for tex in sorted(atlases):
        src = objdir / tex
        if not src.exists():
            src = prebake / tex
        img = sdf_atlas(src)
        encode(img, objdir / sdf_name(tex), work0)
        print(f"{sdf_name(tex)}: distance field {img.size[0]}x{img.size[1]} from {dds_size(src)}")
    results = json.load(open(Path(work0) / "bake_result.json"))["results"]
    panels = list(results) + EXTRA_PANELS
    for name in panels:
        stem = name[:-4]
        src = next((Path(w) / f"{stem}_src.png" for w in works if (Path(w) / f"{stem}_src.png").is_file()), None)
        dst = objdir / name
        if src is None or not dst.exists():
            print(f"{name}: no pre-lettering composite, left as is")
            continue
        keep(dst, backup)
        w, h = dds_size(dst)
        img = Image.open(src).convert("RGB")
        if name in copies:
            img = Image.fromarray(extend_and_copy(np.asarray(img), copies[name]))
        if img.size[0] < w:
            print(f"{name}: composite {img.size} is smaller than installed {w}x{h}, left as is")
            continue
        if img.size != (w, h):
            img = img.resize((w, h), Image.BOX)
        encode(img, dst, work0)
        print(f"{name}: pre-lettering panel back at {w}x{h}")
        lit = (results.get(name) or {}).get("lit") or (stem + "_TXTLIT.dds")
        if (objdir / lit).exists():
            keep(objdir / lit, backup)
            encode(Image.new("RGB", (64, 64)), objdir / lit, work0)
            print(f"  {lit}: text glow removed")
    for p in sorted(objdir.glob("*_TXT.dds")):
        original = objdir / p.name.replace("_TXT.dds", ".dds")
        if original.exists() and "COCKPIT" in p.name:
            keep(p, backup)
            shutil.copy2(original, p)
            print(f"{p.name}: back to {original.name}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4].split(","), sys.argv[5] if len(sys.argv) > 5 else None)
