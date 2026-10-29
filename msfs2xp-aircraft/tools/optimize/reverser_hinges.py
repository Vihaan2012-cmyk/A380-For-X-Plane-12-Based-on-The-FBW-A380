"""usage: reverser_hinges.py <objects dir> [--angle DEG]

Opens the thrust reverser C-duct halves behind the fan cowl doors, as the
ground crew does after the fan cowls: fbw/anim/cowl_open_<n> now runs the
fan cowl doors over 0..0.5 and the reverser halves over 0.5..1. Each
engine's nacelle triangles in a380_exterior_022/023 (+_gp) between the fan
cowl seam and the trailing edge, outside the core cowl (and the matching
aft fan frame ring in 020/021), are split into a
left and a right half turning about their hinge beside the pylon. Run after
cowl_hinges.py; objects already done are left alone."""

import re
import sys
from pathlib import Path

import numpy as np

import cowl_clip as cc
import cowl_hinges as ch
import cowl_separation as cs
from gp7200_variant import LIFT

ENGSEP = re.compile(r"\s*ANIM_hide 5\.5 6\.5 sim/operation/failures/rel_engsep(\d)\s*$")
DOOR = re.compile(r"^(\s*)ANIM_rotate (\S+ \S+ \S+) 0 (\S+) 0 1 (fbw/anim/cowl_open_\d)\s*$")
OPEN_DEG = 45.0
CORE_COWL_MAX_R = 1.2
AFT_OF_TIP = 6.5


def keyed(pad, axis, angle, ref, start):
    return [f"{pad}ANIM_rotate_begin {axis} {ref}", f"{pad}ANIM_rotate_key {start - 0.5:.1f} 0", f"{pad}ANIM_rotate_key {start:.1f} {angle}", f"{pad}ANIM_rotate_end"]


def halves(vrows, tris, fr):
    p, d, up, side, tip = fr
    left, right, rest = [], [], []
    for tri in tris:
        q = np.array([vrows[i][:3] for i in tri]) - p
        c = q.mean(axis=0)
        along = c @ d - tip
        r = np.linalg.norm(c - (c @ d) * d)
        if not (-AFT_OF_TIP < along < -cs.SEAM_BEHIND_TIP + 0.05 and r >= CORE_COWL_MAX_R):
            rest.append(tri)
            continue
        x = q @ side
        pieces = cc.split(vrows, tri, x) if cc.clip(tri, x) else [(tri, x.mean() > 0)]
        for piece, starboard in pieces:
            (right if starboard else left).append(piece)
    return left, right, rest


def half_lines(pad, n, sign, fr, start, count, angle):
    p, d, up, side, tip = fr
    hinge = p + d * (tip - 5.0) + up * ch.HINGE_UP_M + side * (sign * ch.HINGE_SIDE_M)
    h = " ".join(f"{v:.4f}" for v in hinge)
    nh = " ".join(f"{-v:.4f}" for v in hinge)
    ax = " ".join(f"{v:.6f}" for v in d)
    return [f"{pad}ANIM_begin", f"{pad}ANIM_trans {h} {h}"] + keyed(pad, ax, f"{-sign * angle:.1f}", f"fbw/anim/cowl_open_{n}", 1.0) + [f"{pad}ANIM_trans {nh} {nh}", f"{pad}TRIS {start} {count}", f"{pad}ANIM_end"]


def process(path, frames, angle):
    lines, vt_lines, idx = cc.parse(path)
    if any(l.strip().startswith("ANIM_rotate_begin") and "fbw/anim/cowl_open_" in l for l in lines):
        return "already done"
    vrows = [np.array([float(x) for x in l.split()[1:9]]) for l in vt_lines]
    base = len(vrows)
    stack, plan = [], {}
    for i, l in enumerate(lines):
        t = l.strip()
        if t == "ANIM_begin":
            stack.append(None)
        elif t == "ANIM_end":
            if stack:
                stack.pop()
        elif ENGSEP.match(l) and stack:
            stack[-1] = ("engine", int(ENGSEP.match(l).group(1)))
        elif "fbw/fail/sep/cowl_" in t or "fbw/anim/cowl_open_" in t:
            if stack:
                stack[-1] = ("cowl", None)
        elif t.startswith("TRIS ") and stack:
            if any(s and s[0] == "cowl" for s in stack):
                continue
            engine = next((s[1] for s in reversed(stack) if s and s[0] == "engine"), None)
            if engine is None or engine not in frames:
                continue
            a, c = map(int, t.split()[1:3])
            tris = [tuple(idx[a + 3 * k:a + 3 * k + 3]) for k in range(c // 3)]
            left, right, rest = halves(vrows, tris, frames[engine])
            if left or right:
                plan[i] = (engine, left, right, rest)
    if not plan:
        return process_doors_only(path)
    new_idx, out = [], []
    for i, line in enumerate(lines):
        s = line.strip()
        if s.startswith(("IDX10", "IDX ")):
            continue
        if s.startswith("POINT_COUNTS"):
            out.append("@POINT_COUNTS@")
            continue
        pad = line[:len(line) - len(line.lstrip())]
        m = DOOR.match(line)
        if m:
            out += keyed(m.group(1), m.group(2), m.group(3), m.group(4), 0.5)
            continue
        if i in plan:
            e, left, right, rest = plan[i]
            if rest:
                out.append(f"{pad}TRIS {len(new_idx)} {len(rest) * 3}")
                new_idx += [v for t in rest for v in t]
            for group, sign in ((left, -1), (right, 1)):
                if group:
                    out += half_lines(pad, e + 1, sign, frames[e], len(new_idx), len(group) * 3, angle)
                    new_idx += [v for t in group for v in t]
            continue
        if s.startswith("TRIS "):
            a, k = map(int, s.split()[1:3])
            out.append(f"{pad}TRIS {len(new_idx)} {k}")
            new_idx += idx[a:a + k]
            continue
        out.append(line)
    added = [f"VT {' '.join(f'{x:.5f}' for x in v)}" for v in vrows[base:]]
    last_vt = max(i for i, l in enumerate(out) if l.startswith("VT"))
    rows = [f"IDX10 {' '.join(map(str, new_idx[k:k + 10]))}" for k in range(0, len(new_idx) - len(new_idx) % 10, 10)]
    rows += [f"IDX {v}" for v in new_idx[len(new_idx) - len(new_idx) % 10:]]
    out = out[:last_vt + 1] + added + rows + out[last_vt + 1:]
    out = [f"POINT_COUNTS {len(vrows)} 0 0 {len(new_idx)}" if l == "@POINT_COUNTS@" else l for l in out]
    path.write_text("\n".join(out), encoding="utf-8", newline="\n")
    sizes = sorted((e + 1, len(l), len(r)) for e, l, r, _ in plan.values())
    return f"reverser halves {sizes} (engine, left, right triangles)"


def main(objects_dir, angle=OPEN_DEG):
    objects = Path(objects_dir)
    axes, tip, _ = cs.main(objects, preview=True)
    gp_axes = {e: (p + np.array([0.0, LIFT[e + 1], 0.0]), d) for e, (p, d) in axes.items()}
    for stem in ("020", "021", "022", "023"):
        for suffix, chosen in (("", axes), ("_gp", gp_axes)):
            path = objects / f"a380_exterior_{stem}{suffix}.obj"
            if path.is_file():
                frames = {e: ch.frame(chosen[e], tip[e]) for e in chosen}
                print(path.name, process(path, frames, angle))


def process_doors_only(path):
    text = path.read_text(encoding="utf-8")
    lines = text.split("\n")
    out, changed = [], 0
    for line in lines:
        m = DOOR.match(line)
        if m:
            out += keyed(m.group(1), m.group(2), m.group(3), m.group(4), 0.5)
            changed += 1
        else:
            out.append(line)
    if changed:
        path.write_text("\n".join(out), encoding="utf-8", newline="\n")
    return f"{changed} door rotations keyed to 0..0.5"


if __name__ == "__main__":
    args = sys.argv[1:]
    angle = float(args[args.index("--angle") + 1]) if "--angle" in args else OPEN_DEG
    main(args[0], angle)
