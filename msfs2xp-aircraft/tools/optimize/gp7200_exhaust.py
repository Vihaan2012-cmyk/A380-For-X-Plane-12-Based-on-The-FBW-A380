"""usage: gp7200_exhaust.py <aircraft objects dir> <backup dir>

Reshapes the GP7200 core exhaust of every *_gp object (exterior, pylon, engine detach copies) to Airbus AC
A380 fig 2-12-0-991-004: core cowl to 0.35 m behind the reverser exit, primary nozzle to 0.93 m, plug base
r 0.46, plug tip 2.11 m. Run after gp7200_variant.py, engine_detach.py, cowl_separation.py and engine_damage.py."""

import json
import shutil
import sys
from pathlib import Path

import numpy as np

from gp7200_variant import GP_AFT, engine_of, vertices

MARKER = "# gp7200_exhaust"
SRC = [-100.0, -0.30, 0.00, 0.38, 0.75, GP_AFT, 2.60, 100.0]
DST = [-100.0, -0.30, 0.00, 0.35, 0.93, GP_AFT, 2.60, 100.0]
SCALE = [1.00, 1.00, 0.94, 0.95, 0.78, 0.78, 1.00, 1.00]
R_FULL, R_NONE = 1.18, 1.35
EXTERIOR = ["a380_exterior_020_gp.obj", "a380_exterior_021_gp.obj", "a380_exterior_022_gp.obj", "a380_exterior_023_gp.obj",
            "a380_exterior_031_gp.obj"]
FAN = "a380_exterior_030_gp.obj"
CORES = EXTERIOR[:4]


def smoothstep(x, a, b):
    t = np.clip((x - a) / (b - a), 0, 1)
    return t * t * (3 - 2 * t)


def read(path):
    return path.read_text(encoding="utf-8", errors="replace").split("\n")


def axes(obj_dir):
    _, fan = vertices(read(obj_dir / FAN))
    core = np.concatenate([vertices(read(obj_dir / n))[1] for n in CORES])
    out = {}
    for e in (1, 2, 3, 4):
        f = fan[engine_of(fan[:, 0]) == e]
        blades = f[np.hypot(f[:, 0] - f[:, 0].mean(), f[:, 1] - f[:, 1].mean()) > 1.0]
        front = np.array([(blades[:, 0].max() + blades[:, 0].min()) / 2, (blades[:, 1].max() + blades[:, 1].min()) / 2, blades[:, 2].mean()])
        c = core[engine_of(core[:, 0]) == e]
        near = c[np.hypot(c[:, 0] - front[0], c[:, 1] - front[1]) < 0.35]
        tip = near[near[:, 2].argmax(), :3]
        out[e] = dict(front=front, tip=tip, exit=tip[2] - GP_AFT)
        print(f"engine {e}: fan centre {front.round(3)} plug tip {tip.round(3)} reverser exit z {tip[2] - GP_AFT:.3f}")
    return out


def centre(a, z):
    t = (z - a["front"][2]) / (a["tip"][2] - a["front"][2])
    return a["front"][:2] + (a["tip"][:2] - a["front"][:2]) * t[:, None]


def place(p, a):
    d = p[:, 2] - a["exit"]
    c0 = centre(a, p[:, 2])
    rel = p[:, :2] - c0
    w = 1 - smoothstep(np.hypot(rel[:, 0], rel[:, 1]), R_FULL, R_NONE)
    z = p[:, 2] + w * (np.interp(d, SRC, DST) - d)
    k = 1 + w * (np.interp(d, SRC, SCALE) - 1)
    return np.column_stack([centre(a, z) + rel * k[:, None], z])


def morph(vt, a):
    p = vt[:, :3]
    q = place(p, a)
    moved = np.abs(q - p).max(axis=1) > 1e-7
    if not moved.any():
        return vt, moved
    h = 1e-4
    jac = np.stack([(place(p[moved] + h * np.eye(3)[i], a) - place(p[moved] - h * np.eye(3)[i], a)) / (2 * h) for i in range(3)], axis=2)
    n = np.einsum("nji,nj->ni", np.linalg.inv(jac), vt[moved, 3:6])
    n /= np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-12)
    out = vt.copy()
    out[:, :3] = q
    out[moved, 3:6] = n
    return out, moved


def apply(path, backup, a_of, offset=None):
    lines = read(path)
    if MARKER in lines:
        print(f"{path.name}: already reshaped")
        return
    rows, vt = vertices(lines)
    if offset is not None:
        vt[:, :3] += offset
    out = vt.copy()
    total = 0
    for e, a in a_of.items():
        sel = np.ones(len(vt), bool) if offset is not None else engine_of(vt[:, 0]) == e
        if not sel.any():
            continue
        part, moved = morph(vt[sel], a)
        out[sel] = part
        total += moved.sum()
    if total == 0:
        print(f"{path.name}: nothing in the exhaust")
        return
    if offset is not None:
        out[:, :3] -= offset
    backup.mkdir(parents=True, exist_ok=True)
    if not (backup / path.name).exists():
        shutil.copy2(path, backup / path.name)
    for i, row, old in zip(rows, out, vt):
        if np.abs(row - old).max() > 1e-9:
            lines[i] = "VT " + " ".join(f"{x:.6g}" for x in row)
    first = next(i for i, l in enumerate(lines) if l.startswith("POINT_COUNTS"))
    lines.insert(first, MARKER)
    path.write_text("\n".join(lines), encoding="utf-8", newline="\n")
    print(f"{path.name}: {total} vertices reshaped")


def main(obj_dir, backup):
    obj_dir, backup = Path(obj_dir), Path(backup)
    a = axes(obj_dir)
    for name in EXTERIOR:
        apply(obj_dir / name, backup, a)
    layout = json.loads((obj_dir / "a380_engdetach.json").read_text(encoding="utf-8"))
    for k, eng in layout["gp"]["engines"].items():
        e = int(k) + 1
        for group in ("objects", "crushed", "burnt"):
            for name in eng.get(group, []):
                if (obj_dir / name).exists():
                    apply(obj_dir / name, backup, {e: a[e]}, np.array(eng["centre"]))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
