"""usage: place_interior_gp.py <objects dir> <part.obj> <out stem>

place_interior.py for the GP7200 on the engine axes of gp7200_spec.json (gp7200_geom.frames): an engine-local Blender export (X starboard,
Y forward = -s from the spinner tip, Z up) is placed on all four engines as one OBJ8 gated like the rest of the GP engine (fbw/engine/gp7200 at 1,
fbw/anim/engine_bay_<n> up with the doors off, hidden when the engine separates), with the part's baked UVs and <out stem>_ALB/_NML/_MAT, and
every engine gets a falling copy a380_engdetach_<e>_<out stem>.obj relative to its centre, listed under "gp" in a380_engdetach.json."""

import json
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "optimize"))
from gp7200_geom import frames  # noqa: E402
from place_interior import read_wavefront, write_obj  # noqa: E402


def main(objects, part, stem):
    objects = Path(objects)
    v, vt, vn, faces = read_wavefront(part)
    FN = frames()
    texture_lines = [f"TEXTURE {stem}_ALB.png", f"TEXTURE_MAP normal {stem}_NML.png", f"TEXTURE_MAP material_gloss {stem}_MAT.png"]
    vts, idx, batches = [], [], []
    for e in (1, 2, 3, 4):
        f = FN[e]
        start, base_vt = len(idx), len(vts)
        cache = {}
        for fc in faces:
            for (vi, ti, ni) in fc[:3]:
                key = (vi, ti, ni)
                if key not in cache:
                    x, y, z = v[vi]
                    pos = f.o + f.side * x + f.up * z - f.d * y
                    nx, ny, nz = vn[ni] if ni >= 0 else (0, 0, 1)
                    nrm = f.side * nx + f.up * nz - f.d * ny
                    nrm /= np.linalg.norm(nrm) or 1.0
                    u, w = vt[ti] if ti >= 0 else (0.0, 0.0)
                    cache[key] = len(vts)
                    vts.append((*pos, *nrm, u, w))
                idx.append(cache[key])
        tri = np.array(idx[start:]).reshape(-1, 3)
        tri = tri[:, [0, 2, 1]]
        pos = np.round(np.array([vts[i][:3] for i in range(base_vt, len(vts))]), 5)
        nrm = np.round(np.array([vts[i][3:6] for i in range(base_vt, len(vts))]), 4)
        loc = tri - base_vt
        g = np.cross(pos[loc[:, 1]] - pos[loc[:, 0]], pos[loc[:, 2]] - pos[loc[:, 0]])
        flip = np.einsum("ij,ij->i", g, nrm[loc].sum(axis=1)) >= 0
        tri[flip] = tri[flip][:, [0, 2, 1]]
        q = np.round(pos * 2000).astype(np.int64)
        vk = q[:, 0] * 73856093 ^ q[:, 1] * 19349663 ^ q[:, 2] * 83492791
        key = np.sort(vk[loc], axis=1)
        _, first = np.unique(key, axis=0, return_index=True)
        tri = tri[np.sort(first)]
        idx[start:] = tri.ravel().tolist()
        batches.append((e - 1, start, len(idx) - start, base_vt, len(vts)))
    body = ["ATTR_LOD 0 3000", "ANIM_begin", "ANIM_hide -0.5 0.5 fbw/engine/gp7200"]
    for e, start, count, _, _ in batches:
        body += ["ANIM_begin", f"ANIM_hide -99999 99999 fbw/anim/engine_bay_{e + 1}", f"ANIM_show 0.5 1.5 fbw/anim/engine_bay_{e + 1}", f"ANIM_hide 5.5 6.5 sim/operation/failures/rel_engsep{e}", f"TRIS {start} {count}", "ANIM_end"]
    body += ["ANIM_end"]
    write_obj(objects / f"{stem}.obj", texture_lines, vts, idx, body)
    print(f"{stem}.obj", len(vts), "vertices", len(idx) // 3, "triangles,", len(idx) // 12, "per engine")
    manifest_path = objects / "a380_engdetach.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    block = manifest["gp"]
    for e, start, count, v0, v1 in batches:
        centre = np.array(block["engines"][str(e)]["centre"])
        local = [(x - centre[0], y - centre[1], z - centre[2], a, b, c, u, w) for x, y, z, a, b, c, u, w in vts[v0:v1]]
        piece = [i - v0 for i in idx[start:start + count]]
        name = f"a380_engdetach_{e}_{stem}.obj"
        write_obj(objects / name, texture_lines, local, piece, ["ATTR_LOD 0 4000", "ANIM_begin", f"ANIM_hide -99999 99999 fbw/fail/sep/cowl_{e + 1}", f"ANIM_show 0.5 1.5 fbw/fail/sep/cowl_{e + 1}", f"TRIS 0 {len(piece)}", "ANIM_end"])
        names = block["engines"][str(e)]["objects"]
        if name not in names:
            names.append(name)
    manifest_path.write_text(json.dumps(manifest, indent=1), encoding="utf-8")
    return [f"{stem}.obj"] + [f"a380_engdetach_{e}_{stem}.obj" for e in range(4)]


if __name__ == "__main__":
    main(*sys.argv[1:4])
