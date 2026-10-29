"""usage: collision_data.py <objects dir>

Collision shapes for everything the plugin drops: each falling engine, cowl
piece and separated part gets a cloud of spheres fitted to its own surface
("spheres", and "spheres_crushed" for the crushed engines) in
a380_engdetach.json / a380_partdetach.json, and the airframe itself becomes
a380_airframe_map.json: per 0.25 m cell of the aircraft's x/z plane, the solid
y interval of its static geometry, kept in layers named by what hides that
geometry (an engine, a cowl, a surface, a native separation, the gear), so
the plugin can drop a layer once that part is gone or retracted."""

import json
import math
import re
import sys
from pathlib import Path

import numpy as np

SKIP = ("_gp", "_hail", "engdetach", "partdetach", "cowldetach", "sep_scars")
GATE = re.compile(r"\s*ANIM_(hide|show)\s+\S+\s+\S+\s+(\S+)")
MOVE = re.compile(r"\s*ANIM_(rotate|trans|rotate_begin|trans_begin)\b.*\s(\S+)\s*$")
GEAR = re.compile(r"fbw/anim/[lrc]_?[wb]?_gear$|fbw/anim/c_gear$")
LAYER_GATES = re.compile(r"sim/operation/failures/rel_(engsep\d|hstb[LR]|vstb1|wing[1-4][LR])$|fbw/fail/sep/")
IGNORED_GATES = ("fbw/anim/eye_in_flight_deck", "fbw/engine/gp7200")
CELL = 0.25
SAMPLE = 0.2
VOXEL = 0.4
MAX_SPHERES = 400
THIN_MARGIN = 0.03
MIN_RADIUS = 0.07


def read(path):
    lines = path.read_text(encoding="utf-8", errors="replace").split("\n")
    vt = np.array([[float(v) for v in l.split()[1:4]] for l in lines if l.startswith("VT")]).reshape(-1, 3)
    idx = np.array([int(v) for l in lines if l.startswith(("IDX10", "IDX ")) for v in l.split()[1:]], dtype=np.int64)
    return lines, vt, idx


def batches(lines):
    stack = []
    for l in lines:
        t = l.strip()
        if t == "ANIM_begin":
            stack.append([])
        elif t == "ANIM_end":
            if stack:
                stack.pop()
        elif stack and (GATE.match(l) or MOVE.match(l)):
            stack[-1].append(l)
        elif t.startswith("TRIS "):
            a, c = map(int, t.split()[1:3])
            yield a, c, [x for s in stack for x in s]


FT = 0.3048


def gear_legs(acf_path):
    vals = {}
    for line in acf_path.read_text(encoding="utf-8", errors="replace").splitlines():
        m = re.match(r"P _gear/(\d+)/(_gear_x|_gear_y|_gear_z|_leg_len|_tire_radius|_gear_type) (\S+)", line)
        if m:
            vals.setdefault(int(m.group(1)), {})[m.group(2)] = float(m.group(3))
    legs = []
    for k, v in sorted(vals.items()):
        if v.get("_gear_type", 0) in (0, 1) or "_leg_len" not in v:
            continue
        x, z = v["_gear_x"] * FT, v["_gear_z"] * FT
        top = v["_gear_y"] * FT
        bottom = (v["_gear_y"] - v["_leg_len"] - v["_tire_radius"]) * FT
        half_x, half_z = (0.8, 0.9) if abs(z) > 20 else (1.0, 2.4) if abs(x) < 4 else (1.0, 1.6)
        legs.append((x, z, bottom, top + 0.5, half_x, half_z))
    return legs


def layer_of(anims):
    keys, gear = set(), False
    for a in anims:
        g = GATE.match(a)
        if g:
            ref = g.group(2)
            if LAYER_GATES.search(ref):
                keys.add(ref)
            elif ref not in IGNORED_GATES:
                return None
            continue
        m = MOVE.match(a)
        if m and GEAR.search(m.group(2)):
            gear = True
    if gear:
        return None
    return "|".join(sorted(keys)) or "base"


def samples(tris):
    out = [tris.reshape(-1, 3)]
    edge = np.max(np.linalg.norm(tris - np.roll(tris, 1, axis=1), axis=2), axis=1)
    n = np.ceil(edge / SAMPLE).astype(int)
    for k in np.unique(n[n > 1]):
        t = tris[n == k]
        u, v = np.meshgrid(np.arange(k + 1), np.arange(k + 1))
        keep = (u + v) <= k
        w1, w2 = u[keep] / k, v[keep] / k
        w0 = 1 - w1 - w2
        pts = t[:, None, 0] * w0[None, :, None] + t[:, None, 1] * w1[None, :, None] + t[:, None, 2] * w2[None, :, None]
        out.append(pts.reshape(-1, 3))
    return np.concatenate(out)


def spheres(paths, offsets=None):
    tris = []
    for k, p in enumerate(paths):
        if not p.is_file():
            continue
        _, vt, idx = read(p)
        n3 = len(idx) // 3 * 3
        if n3:
            shift = np.array(offsets[k]) if offsets else np.zeros(3)
            tris.append(vt[idx[:n3]].reshape(-1, 3, 3) + shift)
    return spheres_of(np.concatenate(tris)) if tris else []


def spheres_of(tris, cap=MAX_SPHERES):
    pts = samples(tris)
    size = VOXEL
    while True:
        keys = np.floor(pts / size).astype(np.int64)
        _, inv = np.unique(keys, axis=0, return_inverse=True)
        inv = inv.ravel()
        if inv.max() + 1 <= cap:
            break
        size *= 1.2
    count = np.bincount(inv)
    centre = np.stack([np.bincount(inv, pts[:, k]) for k in range(3)], axis=1) / count[:, None]
    rel = pts - centre[inv]
    d = np.linalg.norm(rel, axis=1)
    r = np.zeros(len(count))
    np.maximum.at(r, inv, d)
    cov = np.zeros((len(count), 3, 3))
    np.add.at(cov, inv, rel[:, :, None] * rel[:, None, :])
    minor = np.linalg.eigh(cov)[1][:, :, 0]
    thick = np.zeros(len(count))
    np.maximum.at(thick, inv, np.abs(np.einsum("ij,ij->i", rel, minor[inv])))
    r = np.minimum(r + 0.02, np.maximum(thick + THIN_MARGIN, MIN_RADIUS))
    return [[round(float(c[0]), 3), round(float(c[1]), 3), round(float(c[2]), 3), round(float(rr), 3)] for c, rr in zip(centre, r)]


def airframe_map(objects):
    layers = {}
    lo_x, lo_z = -45.0, -45.0
    nx = nz = int(90 / CELL)
    for path in sorted(objects.glob("a380_exterior_*.obj")):
        if any(k in path.stem for k in SKIP):
            continue
        lines, vt, idx = read(path)
        if not len(vt):
            continue
        groups = {}
        for a, c, anims in batches(lines):
            key = layer_of(anims)
            if key is None:
                continue
            groups.setdefault(key, []).append(idx[a:a + c])
        for key, parts in groups.items():
            ii = np.concatenate(parts)
            n3 = len(ii) // 3 * 3
            pts = samples(vt[ii[:n3]].reshape(-1, 3, 3))
            cx = np.floor((pts[:, 0] - lo_x) / CELL).astype(np.int64)
            cz = np.floor((pts[:, 2] - lo_z) / CELL).astype(np.int64)
            ok = (cx >= 0) & (cx < nx) & (cz >= 0) & (cz < nz)
            flat = cx[ok] * nz + cz[ok]
            lo, hi = layers.setdefault(key, (np.full(nx * nz, np.inf), np.full(nx * nz, -np.inf)))
            np.minimum.at(lo, flat, pts[ok, 1])
            np.maximum.at(hi, flat, pts[ok, 1])
        print(path.name, "->", sorted(groups))
    legs = gear_legs(objects.parent / "A380.acf")
    lo, hi = layers.setdefault("gear", (np.full(nx * nz, np.inf), np.full(nx * nz, -np.inf)))
    for x, z, bottom, top, half_x, half_z in legs:
        for i in range(int(np.floor((x - half_x - lo_x) / CELL)), int(np.floor((x + half_x - lo_x) / CELL)) + 1):
            for j in range(int(np.floor((z - half_z - lo_z) / CELL)), int(np.floor((z + half_z - lo_z) / CELL)) + 1):
                f = i * nz + j
                lo[f] = min(lo[f], bottom)
                hi[f] = max(hi[f], top)
    print("gear legs:", [(round(x, 2), round(z, 2), round(b, 2), round(t, 2)) for x, z, b, t, _, _ in legs])
    out = {"cell": CELL, "x0": lo_x, "z0": lo_z, "nx": nx, "nz": nz, "ground": round(min(b for _, _, b, _, _, _ in legs), 3), "layers": {}}
    for key, (lo, hi) in sorted(layers.items()):
        filled = np.flatnonzero(np.isfinite(lo))
        out["layers"][key] = [[int(f // nz), int(f % nz), round(float(lo[f]), 2), round(float(hi[f]), 2)] for f in filled]
    return out


def engine_obstacles(objects):
    """The attached engines as sphere clouds (a column of the map would fill
    the space between the core and the pylon that the cowl occupies)."""
    stems = sorted({p.stem[:-3] if p.stem.endswith("_gp") else p.stem for p in objects.glob("a380_exterior_*.obj") if not any(k in p.stem for k in ("_hail", "engdetach", "partdetach", "cowldetach", "sep_scars"))})
    out = []
    for variant in ("trent", "gp"):
        groups = {}
        for stem in stems:
            path = objects / f"{stem}_gp.obj" if variant == "gp" and (objects / f"{stem}_gp.obj").is_file() else objects / f"{stem}.obj"
            text = path.read_text(encoding="utf-8", errors="replace")
            if "rel_engsep" not in text:
                continue
            lines, vt, idx = read(path)
            for a, c, anims in batches(lines):
                key = layer_of(anims)
                if key is None or "rel_engsep" not in key:
                    continue
                n3 = c // 3 * 3
                groups.setdefault(key, []).append(vt[idx[a:a + n3]].reshape(-1, 3, 3))
        for key, tris in sorted(groups.items()):
            sph = spheres_of(np.concatenate(tris), cap=600)
            out.append({"gates": key.split("|") + [f"variant:{variant}"], "spheres": sph})
            print("obstacle", variant, key, len(sph), "spheres")
    return out


def main(objects_dir):
    objects = Path(objects_dir)
    eng_path = objects / "a380_engdetach.json"
    eng = json.loads(eng_path.read_text(encoding="utf-8"))
    for block in (eng, eng.get("gp", {})):
        for e, entry in block.get("engines", {}).items():
            entry["spheres"] = spheres([objects / n for n in entry["objects"]])
            if entry.get("crushed"):
                entry["spheres_crushed"] = spheres([objects / n for n in entry["crushed"]])
            print("engine", e, len(entry["spheres"]), "spheres")
        for e, entry in block.get("cowls", {}).items():
            for piece in entry.get("pieces", []):
                piece["spheres"] = spheres([objects / n for n in piece["objects"]])
                print("cowl", e, piece["kind"], len(piece["spheres"]), "spheres")
    eng_path.write_text(json.dumps(eng, indent=1), encoding="utf-8")
    part_path = objects / "a380_partdetach.json"
    parts = json.loads(part_path.read_text(encoding="utf-8"))
    for name, entry in parts["parts"].items():
        placed = [(n, [0.0, 0.0, 0.0]) for n in list(entry["objects"]) + [n for subs in entry.get("sub", {}).values() for n in subs]]
        for c in entry.get("carry", []):
            other = parts["parts"][c]
            offset = [a - b for a, b in zip(other["centre"], entry["centre"])]
            placed += [(n, offset) for n in list(other["objects"]) + [n for subs in other.get("sub", {}).values() for n in subs]]
        entry["spheres"] = spheres([objects / n for n, _ in placed], [o for _, o in placed])
        print("part", name, len(entry["spheres"]), "spheres")
    part_path.write_text(json.dumps(parts, indent=1), encoding="utf-8")
    amap = airframe_map(objects)
    (objects / "a380_airframe_map.json").write_text(json.dumps(amap, separators=(",", ":")), encoding="utf-8")
    print("airframe map:", {k: len(v) for k, v in amap["layers"].items()})


if __name__ == "__main__":
    main(sys.argv[1])
