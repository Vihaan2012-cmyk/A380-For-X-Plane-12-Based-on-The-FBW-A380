"""usage: cowl_hinges.py <objects dir> [--angle DEG]

Lets the fan cowl doors open, as on the real aircraft for maintenance: each
engine's fan cowl (the fbw/fail/sep/cowl_<n> block cowl_clip.py leaves) is
split into the fixed inlet and the left and right doors (triangles crossing
either boundary are clipped there), and each door turns about its own hinge
line beside the pylon, driven by fbw/anim/cowl_open_<n> (0 shut, 1 fully
open). Run after cowl_clip.py; it rewrites a380_exterior_020..023 and their
_gp copies in place and leaves objects already hinged alone."""

import re
import sys
from pathlib import Path

import numpy as np

import cowl_clip as cc
import cowl_separation as cs
from gp7200_variant import LIFT

BLOCK = re.compile(r"\s*ANIM_hide 0\.5 1\.5 fbw/fail/sep/cowl_(\d)\s*$")
DOOR_FRONT = cs.INLET_AHEAD_OF_TIP
HINGE_SIDE_M = 0.24
HINGE_UP_M = 1.93
OPEN_DEG = 55.0


def frame(axis, tip):
    p, d = axis
    up = np.array([0.0, 1.0, 0.0]) - d[1] * d
    up /= np.linalg.norm(up)
    side = np.cross(d, up)
    return p, d, up, side, tip


def split_block(vrows, tris, fr):
    p, d, up, side, tip = fr
    inlet, left, right = [], [], []
    for tri in tris:
        pts = np.array([vrows[i][:3] for i in tri])
        f = (pts - p) @ d - (tip - DOOR_FRONT)
        pieces = cc.split(vrows, tri, f) if cc.clip(tri, f) else [(tri, f.mean() > 0)]
        for piece, ahead in pieces:
            if ahead:
                inlet.append(piece)
                continue
            q = np.array([vrows[i][:3] for i in piece])
            x = (q - p) @ side
            halves = cc.split(vrows, piece, x) if cc.clip(piece, x) else [(piece, x.mean() > 0)]
            for half, starboard in halves:
                (right if starboard else left).append(half)
    return inlet, left, right


def door_lines(pad, n, sign, fr, start, count, angle):
    """A door turning about its hinge line (parallel to the engine axis);
    the starboard door (sign +1) opens with a negative turn about the
    forward axis, which lifts its lower edge out and up."""
    p, d, up, side, tip = fr
    hinge = p + d * (tip - 2.0) + up * HINGE_UP_M + side * (sign * HINGE_SIDE_M)
    h = " ".join(f"{v:.4f}" for v in hinge)
    nh = " ".join(f"{-v:.4f}" for v in hinge)
    ax = " ".join(f"{v:.6f}" for v in d)
    return [f"{pad}ANIM_begin", f"{pad}ANIM_trans {h} {h}", f"{pad}ANIM_rotate {ax} 0 {-sign * angle:.1f} 0 1 fbw/anim/cowl_open_{n}", f"{pad}ANIM_trans {nh} {nh}", f"{pad}TRIS {start} {count}", f"{pad}ANIM_end"]


def process(path, frames, angle):
    lines, vt_lines, idx = cc.parse(path)
    if any("fbw/anim/cowl_open_" in l for l in lines):
        return "already hinged"
    vrows = [np.array([float(x) for x in l.split()[1:9]]) for l in vt_lines]
    base = len(vrows)
    plan = {}
    for i in range(len(lines) - 3):
        m = BLOCK.match(lines[i + 1])
        if lines[i].strip() == "ANIM_begin" and m and lines[i + 2].strip().startswith("TRIS ") and lines[i + 3].strip() == "ANIM_end":
            e = int(m.group(1)) - 1
            a, c = map(int, lines[i + 2].split()[1:3])
            tris = [tuple(idx[a + 3 * k:a + 3 * k + 3]) for k in range(c // 3)]
            plan[i + 2] = (e, split_block(vrows, tris, frames[e]))
    if not plan:
        return "no cowl blocks"
    new_idx, out = [], []
    for i, line in enumerate(lines):
        s = line.strip()
        if s.startswith(("IDX10", "IDX ")):
            continue
        if s.startswith("POINT_COUNTS"):
            out.append("@POINT_COUNTS@")
            continue
        pad = line[:len(line) - len(line.lstrip())]
        if i in plan:
            e, (inlet, left, right) = plan[i]
            if inlet:
                out.append(f"{pad}TRIS {len(new_idx)} {len(inlet) * 3}")
                new_idx += [v for t in inlet for v in t]
            for group, sign in ((left, -1), (right, 1)):
                if group:
                    out += door_lines(pad, e + 1, sign, frames[e], len(new_idx), len(group) * 3, angle)
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
    sizes = [(e + 1, len(g[0]), len(g[1]), len(g[2])) for e, g in plan.values()]
    return f"hinged {sizes} (engine, inlet, left door, right door triangles)"


def main(objects_dir, angle=OPEN_DEG):
    objects = Path(objects_dir)
    axes, tip, _ = cs.main(objects, preview=True)
    gp_axes = {e: (p + np.array([0.0, LIFT[e + 1], 0.0]), d) for e, (p, d) in axes.items()}
    for stem in ("020", "021", "022", "023"):
        for suffix, chosen in (("", axes), ("_gp", gp_axes)):
            path = objects / f"a380_exterior_{stem}{suffix}.obj"
            if path.is_file():
                frames = {e: frame(chosen[e], tip[e]) for e in chosen}
                print(path.name, process(path, frames, angle))


if __name__ == "__main__":
    args = sys.argv[1:]
    angle = float(args[args.index("--angle") + 1]) if "--angle" in args else OPEN_DEG
    main(args[0], angle)
