"""usage: place_hud.py <hud_blender.py out dir> <aircraft objects dir>

Turns hud_blender.py's captain unit into a380_hud_unit.obj for both seats: the projector fairing, its lens
and the hinge housing stay put; the arm, grip and rim with the glass swing about the hinge on
fbw/hud/<l|r>/deploy (stowed at STOW_DEG, up and aft against the ceiling); everything hides with
fbw/hud/<l|r>/installed. The first officer's unit is the mirror image. Textures: a380_hud_unit_ALB.png
(baked colour darkened by its ambient occlusion, the glass patch tinted and see-through), _MAT.png (gloss
from the baked roughness) and a flat _NML.png."""

import sys
from pathlib import Path

import numpy as np
from PIL import Image

HINGE = np.array([-0.648, 3.262, -32.434])
STOW_DEG = -95.0
STATIC = {"fairing", "lens", "housing", "bezel", "screw", "label", "labeltext"}
MOVING = {"arm", "rim", "grip", "screw_arm", "bracket"}
GLASS_RGBA = (178, 214, 200, 18)
STEM = "a380_hud_unit"


def read(path):
    v, vt, vn, faces = [], [], [], []
    mat = None
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        p = line.split()
        if not p:
            continue
        if p[0] == "v":
            v.append([float(x) for x in p[1:4]])
        elif p[0] == "vt":
            vt.append([float(x) for x in p[1:3]])
        elif p[0] == "vn":
            vn.append([float(x) for x in p[1:4]])
        elif p[0] == "usemtl":
            mat = p[1]
        elif p[0] == "f":
            corner = []
            for c in p[1:4]:
                a = c.split("/")
                corner.append((int(a[0]) - 1, int(a[1]) - 1 if a[1] else -1, int(a[2]) - 1))
            faces.append((mat, corner))
    return np.array(v), np.array(vt), np.array(vn), faces


def textures(bake, objects):
    col = np.asarray(Image.open(Path(bake) / "col.png").convert("RGB")).astype(np.float32) / 255
    ao = np.asarray(Image.open(Path(bake) / "ao.png").convert("L")).astype(np.float32) / 255
    rough = np.asarray(Image.open(Path(bake) / "rough.png").convert("L")).astype(np.float32) / 255
    alb = col * (0.55 + 0.45 * ao)[..., None]
    rgba = np.dstack([np.clip(alb * 255 + 0.5, 0, 255), np.full(ao.shape, 255.0)]).astype(np.uint8)
    h, w = ao.shape
    rgba[: int(0.075 * h), int(0.935 * w):] = GLASS_RGBA
    Image.fromarray(rgba, "RGBA").save(Path(objects) / f"{STEM}_ALB.png")
    gloss = np.clip((1 - rough) * 255 + 0.5, 0, 255).astype(np.uint8)
    Image.fromarray(np.dstack([np.zeros_like(gloss), gloss, np.zeros_like(gloss)]), "RGB").save(Path(objects) / f"{STEM}_MAT.png")
    Image.open(Path(bake) / "nml.png").convert("RGB").save(Path(objects) / f"{STEM}_NML.png")


def pivot_lines(hinge, dataref):
    p = " ".join(f"{c:.6f}" for c in hinge)
    q = " ".join(f"{-c:.6f}" for c in hinge)
    return [f"ANIM_trans {p} {p}", f"ANIM_rotate_begin 1 0 0 {dataref}", f"ANIM_rotate_key 0 {STOW_DEG:.1f}",
            "ANIM_rotate_key 1 0.0", "ANIM_rotate_end", f"ANIM_trans {q} {q}"]


def main(bake, objects):
    v, vt, vn, faces = read(Path(bake) / "hud_unit.obj")
    textures(bake, objects)
    verts, index, groups = [], {}, {}
    for side, s in (("l", 1.0), ("r", -1.0)):
        for mat, corner in faces:
            ids = []
            for vi, ti, ni in corner:
                key = (side, vi, ti, ni)
                if key not in index:
                    p, n = v[vi].copy(), vn[ni].copy()
                    p[0] *= s
                    n[0] *= s
                    uv = vt[ti] if ti >= 0 else (0.0, 0.0)
                    index[key] = len(verts)
                    verts.append((*p, *n, *uv))
                ids.append(index[key])
            a, b, c = ids
            tri = (a, c, b) if s > 0 else (a, b, c)
            kind = "static" if mat in STATIC else "moving" if mat in MOVING else "glass"
            groups.setdefault((side, kind), []).extend(tri)
    idx, spans = [], {}
    for key, tris in groups.items():
        spans[key] = f"TRIS {len(idx)} {len(tris)}"
        idx += tris
    lines = ["I", "800", "OBJ", "", "GLOBAL_cockpit_lit", f"TEXTURE {STEM}_ALB.png", f"TEXTURE_MAP normal {STEM}_NML.png",
             f"TEXTURE_MAP material_gloss {STEM}_MAT.png", "GLOBAL_specular 1.0", f"POINT_COUNTS {len(verts)} 0 0 {len(idx)}", ""]
    lines += ["VT " + " ".join(f"{x:.6f}" for x in row) for row in verts]
    full = len(idx) - len(idx) % 10
    lines += ["IDX10 " + " ".join(map(str, idx[k:k + 10])) for k in range(0, full, 10)] + [f"IDX {i}" for i in idx[full:]]
    lines += ["", "ATTR_no_blend 0.5"]
    for side, s in (("l", 1.0), ("r", -1.0)):
        hinge = HINGE * np.array([s, 1, 1])
        lines += ["ANIM_begin", f"ANIM_hide -0.5 0.5 fbw/hud/{side}/installed", spans[(side, "static")],
                  "ANIM_begin", *pivot_lines(hinge, f"fbw/hud/{side}/deploy"), spans[(side, "moving")],
                  "ATTR_blend", "ATTR_no_shadow", spans[(side, "glass")], "ATTR_shadow", "ATTR_no_blend 0.5", "ANIM_end", "ANIM_end"]
    (Path(objects) / f"{STEM}.obj").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"{STEM}.obj", len(verts), "vertices", len(idx) // 3, "triangles")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
