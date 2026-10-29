"""usage: cowl_clip.py <objects dir>

Cuts the inlet and fan cowl exactly at their planes instead of along whole
triangles: every kept/hidden pair of fbw/fail/sep/cowl_<n> batches is merged,
triangles crossing the seam (fan tip - SEAM_BEHIND_TIP on the outer barrel) or
the inlet plane (fan tip - INLET_AHEAD_OF_TIP inside the duct) are split
there, and the falling cowl pieces are rebuilt from the new cut."""

import json
import re
import sys
from pathlib import Path

import numpy as np

import cowl_separation as cs
from gp7200_variant import LIFT

HIDE = re.compile(r"\s*ANIM_hide 0\.5 1\.5 fbw/fail/sep/cowl_(\d)\s*$")
EPS = 1e-4


def parse(path):
    lines = path.read_text(encoding="utf-8", errors="replace").split("\n")
    vt = [l for l in lines if l.startswith("VT")]
    idx = [int(v) for l in lines if l.startswith(("IDX10", "IDX ")) for v in l.split()[1:]]
    return lines, vt, idx


def pairs(lines):
    out = {}
    for i in range(len(lines) - 3):
        m = HIDE.match(lines[i + 1])
        if lines[i].strip() == "ANIM_begin" and m and lines[i + 2].strip().startswith("TRIS ") and lines[i + 3].strip() == "ANIM_end":
            b, c = map(int, lines[i + 2].split()[1:3])
            kept, j = None, i - 1
            while j > 0 and lines[j].strip().startswith("ATTR_"):
                j -= 1
            if j >= 0 and lines[j].strip().startswith("TRIS "):
                a, k = map(int, lines[j].split()[1:3])
                if a + k == b:
                    kept = j
            out[i + 2] = (int(m.group(1)) - 1, kept)
    return out


def clip(tri, f):
    pos = [k for k in range(3) if f[k] > EPS]
    neg = [k for k in range(3) if f[k] < -EPS]
    if not pos or not neg:
        return None
    return pos, neg


def split(vrows, tri, f):
    a, b, c = tri
    fa, fb, fc = f
    out = []
    order = [(a, fa), (b, fb), (c, fc)]
    poly_pos, poly_neg = [], []
    for k in range(3):
        (i, fi), (j, fj) = order[k], order[(k + 1) % 3]
        if fi >= -EPS:
            poly_pos.append(i)
        if fi <= EPS:
            poly_neg.append(i)
        if (fi > EPS and fj < -EPS) or (fi < -EPS and fj > EPS):
            t = fi / (fi - fj)
            v = vrows[i] + t * (vrows[j] - vrows[i])
            n = v[3:6]
            v[3:6] = n / (np.linalg.norm(n) or 1.0)
            vrows.append(v)
            poly_pos.append(len(vrows) - 1)
            poly_neg.append(len(vrows) - 1)
    for poly, side in ((poly_pos, True), (poly_neg, False)):
        for k in range(1, len(poly) - 1):
            out.append(((poly[0], poly[k], poly[k + 1]), side))
    return out


ENGSEP = re.compile(r"\s*ANIM_hide 5\.5 6\.5 sim/operation/failures/rel_engsep(\d)")


def units(lines, engine_of_file):
    """Every engine batch: its kept TRIS line and, when it has one, the TRIS
    line of its cowl block right after it."""
    found = pairs(lines)
    paired_kept = {k for _, k in found.values() if k is not None}
    out = [(e, kept, hid) for hid, (e, kept) in found.items()]
    stack = []
    for i, l in enumerate(lines):
        t = l.strip()
        if t == "ANIM_begin":
            stack.append(None)
        elif t == "ANIM_end":
            if stack:
                stack.pop()
        elif stack and ENGSEP.match(l):
            stack[-1] = int(ENGSEP.match(l).group(1))
        elif stack and HIDE.match(l):
            stack[-1] = "cowl"
        elif t.startswith("TRIS ") and i not in paired_kept and "cowl" not in stack:
            e = next((x for x in reversed(stack) if isinstance(x, int)), engine_of_file)
            if e is not None:
                out.append((e, i, None))
    return out


def classify(vrows, tri, offset, p, d, tip):
    pts = np.array([vrows[i][:3] for i in tri]) + offset
    v = pts - p
    along = v @ d
    r_max = max(np.linalg.norm(v[k] - along[k] * d) for k in range(3))
    if r_max > cs.COWL_MIN_R:
        return along - (tip - cs.SEAM_BEHIND_TIP)
    if r_max > cs.INLET_MIN_R:
        return along - (tip - cs.INLET_AHEAD_OF_TIP)
    return None


def process(path, geometry, engine_of_file=None):
    lines, vt_lines, idx = parse(path)
    vrows = [np.array([float(x) for x in l.split()[1:9]]) for l in vt_lines]
    base = len(vrows)
    plan = {}
    moved = 0
    for e, kept_line, hid_line in units(lines, engine_of_file):
        if e not in geometry:
            continue
        p, d, tip, offset = geometry[e]
        tris, was = [], []
        for line, hidden in ((kept_line, False), (hid_line, True)):
            if line is None:
                continue
            a, c = map(int, lines[line].split()[1:3])
            for q in range(c // 3):
                tris.append(tuple(idx[a + 3 * q:a + 3 * q + 3]))
                was.append(hidden)
        keep, hide = [], []
        for tri, hidden in zip(tris, was):
            f = classify(vrows, tri, offset, p, d, tip)
            if f is None:
                ahead = False
            elif clip(tri, f):
                for piece, side in split(vrows, tri, f):
                    (hide if side else keep).append(piece)
                moved += 1
                continue
            else:
                ahead = bool(f.mean() > 0)
            (hide if ahead else keep).append(tri)
            moved += ahead != hidden
        if hid_line is None and not hide:
            continue
        plan[kept_line if kept_line is not None else hid_line] = (e, keep, hide, kept_line, hid_line)
    if not moved:
        return 0
    drop = {v[4] for v in plan.values() if v[4] is not None and v[3] is not None}
    block = set()
    for v in plan.values():
        if v[4] is not None and v[3] is not None:
            block.update({v[4] - 2, v[4] - 1, v[4] + 1})
    new_idx, out = [], []
    pad = lambda line: line[:len(line) - len(line.lstrip())]
    for i, line in enumerate(lines):
        s = line.strip()
        if s.startswith(("IDX10", "IDX ")):
            continue
        if s.startswith("POINT_COUNTS"):
            out.append("@POINT_COUNTS@")
            continue
        if i in drop or i in block:
            continue
        if i in plan:
            e, keep, hide, kept_line, hid_line = plan[i]
            if kept_line is None:
                at = len(new_idx)
                new_idx += [v for t in hide for v in t]
                out.append(pad(line) + f"TRIS {at} {len(hide) * 3}")
                continue
            if keep:
                at = len(new_idx)
                new_idx += [v for t in keep for v in t]
                out.append(pad(line) + f"TRIS {at} {len(keep) * 3}")
            if hide:
                at = len(new_idx)
                new_idx += [v for t in hide for v in t]
                out += [pad(line) + "ANIM_begin", pad(line) + f"ANIM_hide 0.5 1.5 fbw/fail/sep/cowl_{e + 1}", pad(line) + f"TRIS {at} {len(hide) * 3}", pad(line) + "ANIM_end"]
            continue
        if s.startswith("TRIS "):
            a, k = map(int, s.split()[1:3])
            at = len(new_idx)
            new_idx += idx[a:a + k]
            out.append(pad(line) + f"TRIS {at} {k}")
            continue
        out.append(line)
    added = [f"VT {' '.join(f'{x:.5f}' for x in v)}" for v in vrows[base:]]
    last_vt = max(i for i, l in enumerate(out) if l.startswith("VT"))
    rows = [f"IDX10 {' '.join(map(str, new_idx[k:k + 10]))}" for k in range(0, len(new_idx) - len(new_idx) % 10, 10)]
    rows += [f"IDX {v}" for v in new_idx[len(new_idx) - len(new_idx) % 10:]]
    out = out[:last_vt + 1] + added + rows + out[last_vt + 1:]
    out = [f"POINT_COUNTS {len(vrows)} 0 0 {len(new_idx)}" if l == "@POINT_COUNTS@" else l for l in out]
    path.write_text("\n".join(out), encoding="utf-8", newline="\n")
    return moved


def main(objects_dir):
    objects = Path(objects_dir)
    manifest = json.loads((objects / "a380_engdetach.json").read_text(encoding="utf-8"))
    centres = {int(k): np.array(v["centre"]) for k, v in manifest["engines"].items()}
    axes, tip, _ = cs.main(objects, preview=True)
    gp_axes = {e: (p + np.array([0.0, LIFT[e + 1], 0.0]), d) for e, (p, d) in axes.items()}
    for path in sorted(objects.glob("a380_e*.obj")):
        if not path.name.startswith(("a380_exterior_", "a380_engdetach_")) or any(k in path.stem for k in ("_hail", "_burnt", "_crushed", "_fire")):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        if "rel_engsep" not in text and not path.name.startswith("a380_engdetach_"):
            continue
        texture = " ".join(l for l in text.split("\n")[:60] if l.startswith("TEXTURE"))
        if cs.SKIP_TEXTURE.search(texture):
            continue
        chosen = gp_axes if "_gp" in path.stem else axes
        detached = path.name.startswith("a380_engdetach_")
        engine = int(path.name.split("_")[2]) if detached else None
        geometry = {e: (*chosen[e], tip[e], centres[e] if detached else np.zeros(3)) for e in chosen}
        n = process(path, geometry, engine)
        if n:
            print(path.name, n, "triangles moved or cut")
    for key, chosen in (("engines", axes), ("gp", gp_axes)):
        cowls = manifest["cowls"] if key == "engines" else manifest.get("gp", {}).get("cowls", {})
        for e, entry in cowls.items():
            entry["pieces"] = pieces(objects, int(e), entry, centres[int(e)], chosen[int(e)], tip[int(e)])
            print(key, "engine", int(e) + 1, [(q["kind"], q["triangles"]) for q in entry["pieces"]])
    (objects / "a380_engdetach.json").write_text(json.dumps(manifest, indent=1), encoding="utf-8")


KINDS = ("inlet", "door_in", "door_out")


def pieces(objects, e, entry, centre, axis, tip):
    p, d = axis
    groups = {k: [] for k in KINDS}
    sources = []
    for cowl_name in entry["objects"]:
        src = objects / cowl_name.replace("a380_cowldetach_", "a380_engdetach_")
        lines, vt_lines, idx = parse(src)
        vrows = [np.array([float(x) for x in l.split()[1:9]]) for l in vt_lines]
        k = len(sources)
        sources.append((cowl_name, lines, vrows))
        outward = np.sign(centre[0]) or 1.0
        for hid in pairs(lines):
            a, c = map(int, lines[hid].split()[1:3])
            for q in range(c // 3):
                tri = tuple(idx[a + 3 * q:a + 3 * q + 3])
                pts = np.array([vrows[i][:3] for i in tri]) + centre
                along = (pts - p) @ d
                f = along - (tip - cs.INLET_AHEAD_OF_TIP)
                parts = split(vrows, tri, f) if clip(tri, f) else [(tri, f.mean() > 0)]
                for piece, ahead in parts:
                    if ahead:
                        kind = "inlet"
                    else:
                        c3 = np.array([vrows[i][:3] for i in piece]).mean(0) + centre
                        lateral = (c3 - p) - ((c3 - p) @ d) * d
                        kind = "door_out" if lateral[0] * outward > 0 else "door_in"
                    groups[kind].append((k, piece))
    out = []
    for kind in KINDS:
        tris = groups[kind]
        if not tris:
            continue
        pts = np.array([sources[k][2][i][:3] for k, t in tris for i in t])
        local = pts.mean(0)
        names = []
        for k, (cowl_name, lines, vrows) in enumerate(sources):
            mine = [t for kk, t in tris if kk == k]
            if not mine:
                continue
            name = cowl_name.replace(".obj", f"_{kind}.obj")
            write_group(objects / name, lines, vrows, mine, local)
            names.append(name)
        lo, hi = pts.min(0), pts.max(0)
        out.append({"kind": kind, "centre": [round(float(v), 3) for v in local + centre],
                    "half_extent": [round(float(h - l) / 2, 3) for l, h in zip(lo, hi)],
                    "bottom": round(float(lo[1] - local[1]), 3), "objects": names, "triangles": len(tris)})
    return out


def write_group(path, lines, vrows, tris, local):
    header = [l for l in lines[:60] if l.startswith(("TEXTURE", "GLOBAL_", "NORMAL_METALNESS", "BLEND_GLASS"))]
    attrs = [l for l in lines if l.startswith(("ATTR_cull", "ATTR_no_cull", "ATTR_no_blend", "ATTR_blend", "ATTR_shadow_blend"))][:2]
    used = sorted({i for t in tris for i in t})
    remap = {old: new for new, old in enumerate(used)}
    idx = [remap[i] for t in tris for i in t]
    rows = []
    for old in used:
        v = vrows[old].copy()
        v[:3] = v[:3] - local
        rows.append("VT " + " ".join(f"{x:.5f}" for x in v))
    body = ["I", "800", "OBJ", ""] + header + [f"POINT_COUNTS {len(used)} 0 0 {len(idx)}", ""] + rows
    body += ["IDX10 " + " ".join(map(str, idx[k:k + 10])) for k in range(0, len(idx) - len(idx) % 10, 10)]
    body += [f"IDX {t}" for t in idx[len(idx) - len(idx) % 10:]]
    body += ["", "ATTR_LOD 0 4000"] + attrs + [f"TRIS 0 {len(idx)}", ""]
    path.write_text("\n".join(body), encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main(sys.argv[1])
