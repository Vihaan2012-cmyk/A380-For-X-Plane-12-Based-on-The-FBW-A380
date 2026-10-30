"""usage: gp7200_corebake.py [work dir]

Bakes the GP7200 core's textures in Blender: exports one object per part of gp7200_core.build_core (one copy of every replicated row, with two
neighbouring copies as shading context), lets tools/engine_interior/bake_gp_core.py unwrap, pack and bake them to 2048 sheets, and
turns the raw maps into A380_GPCORE_ALB/NML/MAT.png (finish_textures.py: ALB = colour x (0.6 + 0.4 ao), NML red/green with blue cleared,
MAT red metal / green gloss). gp7200_core.main reads the packed UVs back through apply()."""

import subprocess
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "engine_interior"))

import gp7200_core as core  # noqa: E402
from gp7200_geom import Mesh, box  # noqa: E402

BLENDER = "C:/Program Files/Blender Foundation/Blender 5.2/blender.exe"
WORK = Path("E:/fbw-build/gp7200-model/corebake")


def tag_arrays(mesh):
    parts = np.array([t.split("|") for t in mesh.tag], dtype=object)
    return parts[:, 0], parts[:, 1].astype(int), parts[:, 2].astype(int)


def write_object(f, name, mesh, tri_ids, base):
    T = mesh.T[tri_ids]
    used, inv = np.unique(T.ravel(), return_inverse=True)
    f.write(f"o {name}\n")
    for p in mesh.P[used]:
        f.write(f"v {p[0]:.5f} {p[1]:.5f} {p[2]:.5f}\n")
    for n in mesh.N[used]:
        f.write(f"vn {n[0]:.4f} {n[1]:.4f} {n[2]:.4f}\n")
    inv = inv.reshape(-1, 3) + 1 + base
    for a, b, c in inv:
        f.write(f"f {a}//{a} {b}//{b} {c}//{c}\n")
    return base + len(used)


def export(groups, path):
    n_parts = 0
    n_ctx = 0
    base = 0
    with open(path, "w") as f:
        for mesh in groups:
            mat, pid, loc = tag_arrays(mesh)
            for p in np.unique(pid):
                idx = np.nonzero(pid == p)[0]
                nl = int(loc[idx].max()) + 1
                ncopy = len(idx) // nl
                assert len(idx) == ncopy * nl and (loc[idx[:nl]] == np.arange(nl)).all(), (p, len(idx), nl)
                m = mat[idx[0]]
                base = write_object(f, f"{m}__{p}", mesh, idx[:nl], base)
                n_parts += 1
                if ncopy > 2:
                    for k in (1, ncopy - 1):
                        base = write_object(f, f"ctx__{m}__{p}__{k}", mesh, idx[k * nl:(k + 1) * nl], base)
                        n_ctx += 1
        lod = box(np.array([0.0, 0.0, 0.0]), (0.04, 0.04, 0.04))
        lod = core.mark(lod, "black")
        lod.N = lod.N
        mat, pid, loc = tag_arrays(lod)
        base = write_object(f, f"black__{pid[0]}", lod, np.arange(len(lod.T)), base)
        n_parts += 1
    return n_parts, n_ctx, int(pid[0])


def load_uv(work=WORK):
    z = np.load(Path(work) / "uv.npz")
    return {k: z[k] for k in z.files}


def apply(mesh, uvs):
    mat, pid, loc = tag_arrays(mesh)
    cu = np.zeros((len(mesh.T), 3, 2))
    for p in np.unique(pid):
        sel = pid == p
        cu[sel] = uvs[str(p)][loc[sel]]
    keys = np.concatenate([mesh.T.reshape(-1, 1), np.round(cu.reshape(-1, 2) * 1e5).astype(np.int64)], axis=1)
    uniq, inv = np.unique(keys, axis=0, return_inverse=True)
    out = Mesh()
    out.add(mesh.P[uniq[:, 0]], mesh.N[uniq[:, 0]], uniq[:, 1:] / 1e5, inv.reshape(-1, 3), "")
    out.tag = list(mesh.tag)
    return out


def bake(work=WORK, size=2048):
    work = Path(work)
    work.mkdir(parents=True, exist_ok=True)
    core.PID[0] = 0
    always, rare, rot_a, rot_r = core.build_core()
    n_parts, n_ctx, lod_pid = export([always, rare, rot_a, rot_r], work / "parts.obj")
    (work / "lod_pid.txt").write_text(str(lod_pid))
    print(n_parts, "parts,", n_ctx, "context copies")
    subprocess.run([BLENDER, "-b", "--factory-startup", "--python", str(HERE.parent / "engine_interior" / "bake_gp_core.py"), "--", str(work / "parts.obj"), str(work), str(size)], check=True)
    import finish_textures as ft
    (work / "tex").mkdir(exist_ok=True)
    ft.main(work, "GPCORE", work / "tex", "A380_GPCORE")


if __name__ == "__main__":
    bake(*sys.argv[1:2])
