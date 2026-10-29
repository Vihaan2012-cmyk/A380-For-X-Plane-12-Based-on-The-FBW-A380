import json
import re
import sys
from pathlib import Path

import numpy as np

from gp7200_variant import LIFT

ENGSEP = re.compile(r"ANIM_hide 5\.5 6\.5 sim/operation/failures/rel_engsep(\d)")
FAN_AXIS = re.compile(r"ANIM_rotate_begin (\S+) (\S+) (\S+) fbw/anim/N1_(\d)_anim")
SKIP_TEXTURE = re.compile(r"_FAN_|_PYLON_|ENGINE_BLUR")
INLET_AHEAD_OF_TIP = 0.6
SEAM_BEHIND_TIP = 3.05
INLET_MIN_R = 1.3
COWL_MIN_R = 1.7
DATAREF = "fbw/fail/sep/cowl_{}"


def read(path):
    lines = path.read_text(encoding="utf-8", errors="replace").split("\n")
    vt = np.array([[float(v) for v in l.split()[1:4]] for l in lines if l.startswith("VT")]).reshape(-1, 3)
    idx = np.array([int(v) for l in lines if l.startswith(("IDX10", "IDX ")) for v in l.split()[1:]], dtype=np.int64)
    return lines, vt, idx


def fan_axes(objects):
    lines = (objects / "a380_exterior_030.obj").read_text(encoding="utf-8", errors="replace").split("\n")
    _, vt, idx = read(objects / "a380_exterior_030.obj")
    axes = {}
    for i, l in enumerate(lines):
        m = FAN_AXIS.match(l)
        if not m:
            continue
        d = np.array([float(m.group(k)) for k in (1, 2, 3)])
        d /= np.linalg.norm(d)
        p = np.array([float(v) for v in lines[i - 1].split()[1:4]])
        axes[int(m.group(4)) - 1] = (p, d)
    return axes


def tips(objects, axes, centres):
    out = {}
    for e, (p, d) in axes.items():
        fan = objects / f"a380_engdetach_{e}_a380_exterior_030.obj"
        _, vt, _ = read(fan)
        along = (vt + centres[e] - p) @ d
        out[e] = along.max()
    return out


def cowl_mask(points, axis, tip):
    p, d = axis
    v = points - p
    along = v @ d
    r = np.linalg.norm(v - np.outer(along, d), axis=1)
    inlet = (along > tip - INLET_AHEAD_OF_TIP) & (r > INLET_MIN_R)
    cowl = (along > tip - SEAM_BEHIND_TIP) & (r > COWL_MIN_R)
    return inlet | cowl


def engine_stack(lines):
    stack, at = [], {}
    for i, l in enumerate(lines):
        t = l.strip()
        if t == "ANIM_begin":
            stack.append(None)
        elif t == "ANIM_end":
            if stack:
                stack.pop()
        else:
            m = ENGSEP.match(t)
            if m and stack:
                stack[-1] = int(m.group(1))
            elif t.startswith("TRIS "):
                e = next((s for s in reversed(stack) if s is not None), None)
                if e is not None:
                    at[i] = e
    return at


def wrap(lines, vt, idx, tris_engine, mask_of, offset=np.zeros(3)):
    out, reordered, wrapped, pieces = [], False, 0, {}
    for i, line in enumerate(lines):
        if i not in tris_engine:
            out.append(line)
            continue
        e = tris_engine[i]
        first, count = map(int, line.split()[1:3])
        tri = idx[first:first + count].reshape(-1, 3)
        sel = mask_of(e, vt[tri].mean(axis=1) + offset)
        if not sel.any():
            out.append(line)
            continue
        order = np.concatenate([np.flatnonzero(~sel), np.flatnonzero(sel)])
        if (order != np.arange(len(order))).any():
            idx[first:first + count] = tri[order].ravel()
            reordered = True
        keep = int((~sel).sum()) * 3
        if keep:
            out.append(f"TRIS {first} {keep}")
        out += ["ANIM_begin", f"ANIM_hide 0.5 1.5 {DATAREF.format(e + 1)}", f"TRIS {first + keep} {count - keep}", "ANIM_end"]
        pieces.setdefault(e, []).append((first + keep, count - keep))
        wrapped += 1
    if reordered:
        body = [l for l in out if not l.startswith(("IDX10", "IDX "))]
        at = next(i for i, l in enumerate(body) if l.startswith("VT")) + len(vt)
        rows = [f"IDX10 {' '.join(map(str, idx[k:k + 10]))}" for k in range(0, len(idx) - len(idx) % 10, 10)]
        rows += [f"IDX {v}" for v in idx[len(idx) - len(idx) % 10:]]
        out = body[:at] + rows + body[at:]
    return out, wrapped, pieces


def whole_object(lines):
    return {i: None for i, l in enumerate(lines) if l.startswith("TRIS ")}


def main(objects_dir, preview=None):
    objects = Path(objects_dir)
    manifest_path = objects / "a380_engdetach.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    centres = {int(k): np.array(v["centre"]) for k, v in manifest["engines"].items()}
    axes = fan_axes(objects)
    tip = tips(objects, axes, centres)

    def mask_of(e, points):
        return cowl_mask(points, axes[e], tip[e])

    if preview:
        return axes, tip, mask_of
    cowls = {}
    for path in sorted(objects.glob("a380_exterior_*.obj")):
        lines, vt, idx = read(path)
        if any(DATAREF.format(1)[:-1] in l for l in lines):
            print(path.name, "already split")
            continue
        texture = " ".join(l for l in lines[:60] if l.startswith("TEXTURE"))
        if SKIP_TEXTURE.search(texture):
            continue
        at = engine_stack(lines)
        if not at:
            continue
        out, wrapped, _ = wrap(lines, vt, idx, at, mask_of)
        if wrapped:
            path.write_text("\n".join(out), encoding="utf-8", newline="\n")
            print(path.name, wrapped, "cowl batches")
    cowls = split_pieces(objects, manifest["engines"], centres, mask_of)
    if cowls:
        manifest["cowls"] = cowls
    if "gp" in manifest:
        gp_axes = {e: (p + np.array([0.0, LIFT[e + 1], 0.0]), d) for e, (p, d) in axes.items()}

        def gp_mask_of(e, points):
            return cowl_mask(points, gp_axes[e], tip[e])

        gp_cowls = split_pieces(objects, manifest["gp"]["engines"], centres, gp_mask_of)
        if gp_cowls:
            manifest["gp"]["cowls"] = gp_cowls
    manifest_path.write_text(json.dumps(manifest, indent=1), encoding="utf-8")


def split_pieces(objects, engines, centres, mask_of):
    cowls = {}
    for e, m in sorted(engines.items()):
        e = int(e)
        parts = []
        for name in m["objects"]:
            path = objects / name
            lines, vt, idx = read(path)
            if any(DATAREF.format(e + 1) in l for l in lines):
                continue
            texture = " ".join(l for l in lines[:60] if l.startswith("TEXTURE"))
            if SKIP_TEXTURE.search(texture):
                continue
            at = {i: e for i in whole_object(lines)}
            out, wrapped, pieces = wrap(lines, vt, idx, at, mask_of, centres[e])
            if not wrapped:
                continue
            path.write_text("\n".join(out), encoding="utf-8", newline="\n")
            _, vt, idx = read(path)
            parts.append((name, lines, vt, idx, pieces[e]))
        if not parts:
            continue
        pts = np.concatenate([vt[idx[s:s + c]] for _, _, vt, idx, ranges in parts for s, c in ranges])
        centre = pts.mean(axis=0)
        names = []
        for name, lines, vt, idx, ranges in parts:
            cowl_name = name.replace("a380_engdetach_", "a380_cowldetach_")
            write_piece(objects / cowl_name, lines, vt, idx, ranges, centre)
            names.append(cowl_name)
        lo, hi = pts.min(axis=0), pts.max(axis=0)
        cowls[str(e)] = {"centre": [round(float(c), 3) for c in centre + centres[e]],
                         "half_extent": [round(float(h - l) / 2, 3) for l, h in zip(lo, hi)],
                         "bottom": round(float(lo[1] - centre[1]), 3),
                         "objects": names}
        print("engine", e + 1, "cowl", len(pts) // 3, "tris in", len(names), "objects")
    return cowls


def write_piece(out, lines, vt, idx, ranges, centre):
    full = [l.split()[1:9] for l in lines if l.startswith("VT")]
    header = [l for l in lines[:60] if l.startswith(("TEXTURE", "GLOBAL_", "NORMAL_METALNESS", "BLEND_GLASS"))]
    attrs = [l for l in lines if l.startswith(("ATTR_cull", "ATTR_no_cull", "ATTR_no_blend", "ATTR_blend", "ATTR_shadow_blend"))][:2]
    used = sorted({int(i) for s, c in ranges for i in idx[s:s + c]})
    remap = {old: new for new, old in enumerate(used)}
    tris = [remap[int(i)] for s, c in ranges for i in idx[s:s + c]]
    rows = []
    for old in used:
        v = [float(x) for x in full[old]]
        rows.append("VT " + " ".join(f"{x:.5f}" for x in [v[0] - centre[0], v[1] - centre[1], v[2] - centre[2]] + v[3:]))
    body = ["I", "800", "OBJ", ""] + header + [f"POINT_COUNTS {len(used)} 0 0 {len(tris)}", ""] + rows
    body += ["IDX10 " + " ".join(map(str, tris[k:k + 10])) for k in range(0, len(tris) - len(tris) % 10, 10)]
    body += [f"IDX {t}" for t in tris[len(tris) - len(tris) % 10:]]
    body += ["", "ATTR_LOD 0 4000"] + attrs + [f"TRIS 0 {len(tris)}", ""]
    out.write_text("\n".join(body), encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main(*sys.argv[1:])
