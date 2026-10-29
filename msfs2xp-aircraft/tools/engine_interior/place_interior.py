"""usage: place_interior.py <objects dir> <part.obj> <out stem> <trent|gp> [--palette] [--cowl-off]

Places an engine-local part (Blender export: X starboard, Y forward from the
spinner tip, Z up) on all four engines' own axes as one X-Plane OBJ8, each
engine's triangles gated like the rest of that engine (hidden when it
separates) and the whole object shown only with its engine type fitted.
With --palette each material is a flat colour tile (shape previews); without
it the part's own baked UVs and <out stem>_ALB/_NML/_MAT textures are used.
With --cowl-off each engine's copy is drawn only while that engine's fan
cowl has separated (it sits inside the closed cowl otherwise), and every
engine also gets a falling copy, a380_engdetach_<e>_<out stem>.obj relative
to that engine's centre, listed in a380_engdetach.json so a separated engine
carries it."""

import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "optimize"))
import cowl_separation as cs  # noqa: E402
from gp7200_variant import LIFT  # noqa: E402

PALETTE = {
    "stainless": (158, 158, 153), "titanium": (115, 110, 102), "casting": (92, 94, 89), "case": (77, 79, 77),
    "firesleeve": (158, 51, 26), "clamp": (179, 77, 31), "yellow": (204, 158, 20), "blue": (26, 66, 133),
    "harness": (20, 20, 20), "connector": (140, 120, 77), "rubber": (10, 10, 10), "label": (217, 217, 209),
}


def read_wavefront(path):
    v, vt, vn, faces = [], [], [], []
    mat = None
    for line in Path(path).read_text(encoding="utf-8", errors="replace").splitlines():
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
            corners = []
            for c in p[1:]:
                a = (c.split("/") + ["", ""])[:3]
                corners.append((int(a[0]) - 1, int(a[1]) - 1 if a[1] else -1, int(a[2]) - 1 if a[2] else -1))
            for k in range(1, len(corners) - 1):
                faces.append((corners[0], corners[k], corners[k + 1], mat))
    return np.array(v), np.array(vt) if vt else np.zeros((1, 2)), np.array(vn), faces


def engine_frames(objects, gp):
    axes, tip, _ = cs.main(Path(objects), preview=True)
    frames = {}
    for e, (p, d) in axes.items():
        if gp:
            p = p + np.array([0.0, LIFT[e + 1], 0.0])
        up = np.array([0.0, 1.0, 0.0]) - d[1] * d
        up /= np.linalg.norm(up)
        side = np.cross(d, up)
        frames[e] = (p + d * tip[e], side, d, up)
    return frames


def palette_uv(names):
    tiles = sorted(set(names))
    n = int(np.ceil(np.sqrt(len(tiles))))
    img = Image.new("RGB", (n * 8, n * 8))
    uv = {}
    for k, name in enumerate(tiles):
        x, y = (k % n) * 8, (k // n) * 8
        img.paste(PALETTE.get(name, (255, 0, 255)), (x, y, x + 8, y + 8))
        uv[name] = ((x + 4) / (n * 8), 1 - (y + 4) / (n * 8))
    return img, uv


def main(objects, part, stem, kind, palette=False, cowl_off=False):
    objects = Path(objects)
    v, vt, vn, faces = read_wavefront(part)
    gp = kind == "gp"
    frames = engine_frames(objects, gp)
    if palette:
        img, uv_of = palette_uv([f[3] for f in faces])
        img.save(objects / f"{stem}_PAL.png")
        texture_lines = [f"TEXTURE {stem}_PAL.png"]
    else:
        texture_lines = [f"TEXTURE {stem}_ALB.png", f"TEXTURE_MAP normal {stem}_NML.png", f"TEXTURE_MAP material_gloss {stem}_MAT.png"]
    vts, idx, batches = [], [], []
    for e in sorted(frames):
        origin, side, fwd, up = frames[e]
        start = len(idx)
        base_vt = len(vts)
        cache = {}
        for f in faces:
            for (vi, ti, ni) in f[:3]:
                key = (vi, ti, ni, f[3] if palette else None)
                if key not in cache:
                    x, y, z = v[vi]
                    pos = origin + side * x + fwd * y + up * z
                    nx, ny, nz = vn[ni] if ni >= 0 else (0, 0, 1)
                    nrm = side * nx + fwd * ny + up * nz
                    nrm /= np.linalg.norm(nrm) or 1.0
                    if palette:
                        u, w = uv_of[f[3]]
                    else:
                        u, w = vt[ti] if ti >= 0 else (0.0, 0.0)
                    cache[key] = len(vts)
                    vts.append((*pos, *nrm, u, w))
                idx.append(cache[key])
        # X-Plane draws front faces clockwise: flip the winding Blender exports.
        tri = np.array(idx[start:]).reshape(-1, 3)
        idx[start:] = tri[:, [0, 2, 1]].ravel().tolist()
        batches.append((e, start, len(idx) - start, base_vt, len(vts)))
    gate = "ANIM_hide 0.5 1.5 fbw/engine/gp7200" if not gp else "ANIM_hide -0.5 0.5 fbw/engine/gp7200"
    body = ["ATTR_LOD 0 3000", "ANIM_begin", gate]
    for e, start, count, _, _ in batches:
        body += ["ANIM_begin", f"ANIM_hide 5.5 6.5 sim/operation/failures/rel_engsep{e}"]
        if cowl_off:
            body += [f"ANIM_show 0.5 1.5 fbw/anim/engine_bay_{e + 1}"]
        body += [f"TRIS {start} {count}", "ANIM_end"]
    body += ["ANIM_end"]
    write_obj(objects / f"{stem}.obj", texture_lines, vts, idx, body)
    print(f"{stem}.obj", len(vts), "vertices", len(idx) // 3, "triangles")
    if cowl_off:
        manifest_path = objects / "a380_engdetach.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        block = manifest["gp"] if gp else manifest
        for e, start, count, v0, v1 in batches:
            centre = np.array(block["engines"][str(e)]["centre"])
            local = [(x - centre[0], y - centre[1], z - centre[2], a, b, c, u, w) for x, y, z, a, b, c, u, w in vts[v0:v1]]
            piece = [i - v0 for i in idx[start:start + count]]
            name = f"a380_engdetach_{e}_{stem}.obj"
            write_obj(objects / name, texture_lines, local, piece, ["ATTR_LOD 0 4000", "ANIM_begin", f"ANIM_show 0.5 1.5 fbw/fail/sep/cowl_{e + 1}", f"TRIS 0 {len(piece)}", "ANIM_end"])
            names = block["engines"][str(e)]["objects"]
            if name not in names:
                names.append(name)
        manifest_path.write_text(json.dumps(manifest, indent=1), encoding="utf-8")
        print("falling copies for", len(batches), "engines listed in a380_engdetach.json")


def write_obj(path, texture_lines, vts, idx, body):
    lines = ["I", "800", "OBJ", ""] + texture_lines + ["GLOBAL_specular 1.0", f"POINT_COUNTS {len(vts)} 0 0 {len(idx)}", ""]
    lines += [f"VT {x:.5f} {y:.5f} {z:.5f} {a:.4f} {b:.4f} {c:.4f} {u:.5f} {w:.5f}" for x, y, z, a, b, c, u, w in vts]
    lines += ["IDX10 " + " ".join(map(str, idx[k:k + 10])) for k in range(0, len(idx) - len(idx) % 10, 10)]
    lines += [f"IDX {i}" for i in idx[len(idx) - len(idx) % 10:]]
    lines += [""] + body + [""]
    Path(path).write_text("\n".join(lines), encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], "--palette" in sys.argv, "--cowl-off" in sys.argv)
