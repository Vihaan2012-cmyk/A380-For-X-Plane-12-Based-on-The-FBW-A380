"""Give each gear leg a collapse pose of its own: an outermost rotation about the leg's hinge, keyed by fbw/anim/gear_fold_N, so a failed leg breaks away instead of playing the retraction. The bogie turns back about its beam pivot and stays flat on the ground."""
import math
import sys
from pathlib import Path

import numpy as np

REF = "fbw/anim/gear_fold_{}"

# hinges: the pivot text of every leg part that travels with the strut; parts left out (bay braces, doors) stay where they were, snapped.
# swing: the (sideways, aft) heading the wheels travel.
LEGS = {
    "c_gear": dict(
        extended=1.0,
        fold=0,
        pivot=(0.002489, -1.03969, -30.209529),
        swing=(0.12, 1.0),
        deg=92.0,
        hinges=["0.002489 -1.03969 -30.209529"],
    ),
    "l_b_gear": dict(
        extended=0.5,
        fold=1,
        pivot=(-2.628831, -1.53819, 0.692972),
        swing=(-1.0, 0.55),
        deg=72.0,
        hinges=[
            "-2.630664 -1.034108 3.389247",
            "-2.628831 -1.53819 0.692972",
            "-2.628153 -1.12628 2.067733",
            "-2.629011 -0.388135 2.152142",
            "-2.630664 -1.03427 3.38915",
            "-2.470244 0.039659 1.230543",
            "-2.46992 -0.520989 2.999567",
        ],
    ),
    "r_b_gear": dict(
        extended=0.5,
        fold=2,
        pivot=(2.631169, -1.53819, 0.692972),
        swing=(1.0, 0.55),
        deg=72.0,
        hinges=[
            "2.629336 -1.043398 3.391988",
            "2.631169 -1.53819 0.692972",
            "2.631838 -1.136511 2.070757",
            "2.630981 -0.399014 2.160652",
            "2.629336 -1.043559 3.39189",
            "2.789757 0.03666 1.241582",
            "2.790072 -0.531925 3.008067",
        ],
    ),
    "l_w_gear": dict(
        extended=0.5,
        fold=3,
        pivot=(-6.125643, -0.107318, -1.455327),
        swing=(-1.0, 0.45),
        deg=68.0,
        hinges=[
            "-6.125643 -0.107318 -1.455327",
            "-3.498055 -0.7782 -1.830861",
            "-3.560309 -0.765438 -2.422883",
            "-2.707773 -1.044637 -2.911923",
            "-2.80793 -1.014776 -2.459947",
        ],
    ),
    "r_w_gear": dict(
        extended=0.5,
        fold=4,
        pivot=(6.125643, -0.107318, -1.455327),
        swing=(1.0, 0.45),
        deg=68.0,
        hinges=[
            "6.125643 -0.107318 -1.455327",
            "3.498038 -0.778131 -1.830868",
            "3.560293 -0.765369 -2.422889",
            "2.707457 -1.044623 -2.911978",
            "2.805535 -1.015325 -2.460323",
        ],
    ),
}


def axis_of(swing):
    sx, sz = swing
    n = math.hypot(sx, sz)
    return (-sz / n, 0.0, sx / n)


def fold_block(leg):
    d = LEGS[leg]
    ax = axis_of(d["swing"])
    fwd = " ".join(f"{v:.6f}" for v in d["pivot"])
    back = " ".join(f"{-v:.6f}" for v in d["pivot"])
    return [
        f"ANIM_trans {fwd} {fwd}",
        f"ANIM_rotate_begin {ax[0]:.6f} {ax[1]:.6f} {ax[2]:.6f} {REF.format(d['fold'])}",
        "ANIM_rotate_key 0 0",
        f"ANIM_rotate_key 1 {d['deg']:.3f}",
        "ANIM_rotate_end",
        f"ANIM_trans {back} {back}",
    ]


def strip_folds(lines):
    out = []
    i = 0
    while i < len(lines):
        if lines[i].startswith("ANIM_trans ") and i + 1 < len(lines) and "fbw/anim/gear_fold_" in lines[i + 1]:
            i += 6
            continue
        out.append(lines[i])
        i += 1
    return out


def rotation(axis, deg):
    a = np.array(axis, dtype=float)
    a /= np.linalg.norm(a)
    t = math.radians(deg)
    k = np.array([[0, -a[2], a[1]], [a[2], 0, -a[0]], [-a[1], a[0], 0]])
    return np.eye(3) + math.sin(t) * k + (1 - math.cos(t)) * k @ k


def rotation_block(lines, i, value):
    """The rotation a keyed ANIM_rotate_begin at line i gives at value."""
    axis = [float(v) for v in lines[i].split()[1:4]]
    keys = []
    j = i + 1
    while not lines[j].startswith("ANIM_rotate_end"):
        keys.append([float(v) for v in lines[j].split()[1:3]])
        j += 1
    xs, ys = zip(*keys)
    return rotation(axis, float(np.interp(value, xs, ys)))


def bogie_block(lines, tilt_at, start, leg):
    """Undo the fold's rotation at the beam pivot, in the frame the leg's retraction leaves there."""
    d = LEGS[leg]
    ret = np.eye(3)
    for k in range(start, tilt_at - 1):
        if lines[k].startswith("ANIM_rotate_begin ") and lines[k].split()[-1] == f"fbw/anim/{leg}":
            ret = ret @ rotation_block(lines, k, d["extended"])
    ax = ret.T @ np.array(axis_of(d["swing"]))
    pivot = lines[tilt_at - 1].split()[1:4]
    fwd = " ".join(pivot)
    back = " ".join(f"{-float(v):.6f}" for v in pivot)
    return [
        f"ANIM_trans {fwd} {fwd}",
        f"ANIM_rotate_begin {ax[0]:.6f} {ax[1]:.6f} {ax[2]:.6f} {REF.format(d['fold'])}",
        "ANIM_rotate_key 0 0",
        f"ANIM_rotate_key 1 {-d['deg']:.3f}",
        "ANIM_rotate_end",
        f"ANIM_trans {back} {back}",
    ]


def chain_start(lines, i, ref):
    start = i - 1
    if lines[start - 1].strip() == "ANIM_trans_end":
        start -= 1
        while not lines[start].startswith("ANIM_trans_begin "):
            start -= 1
        assert lines[start].strip().endswith(ref), f"chain starts on {lines[start]}"
    return start


def apply(path):
    text = Path(path).read_text(encoding="utf-8", errors="replace")
    lines = strip_folds(text.split("\n"))
    inserts = {}
    for i, line in enumerate(lines):
        if not line.startswith("ANIM_rotate_begin "):
            continue
        ref = line.split()[-1]
        leg = ref.replace("fbw/anim/", "")
        if leg not in LEGS:
            continue
        previous = lines[i - 1].split()
        if previous[:1] != ["ANIM_trans"] or " ".join(previous[1:4]) not in LEGS[leg]["hinges"]:
            continue
        inserts.setdefault(chain_start(lines, i, ref), leg)
    blocks = {start: fold_block(leg) for start, leg in inserts.items()}
    for start, leg in inserts.items():
        k = start
        while k < len(lines) and not lines[k].startswith(("TRIS ", "ANIM_end", "LIGHT")):
            if lines[k].startswith("ANIM_rotate_begin ") and lines[k].split()[-1] == f"fbw/anim/{leg}_tilt":
                blocks[k - 1] = bogie_block(lines, k, start, leg)
                break
            k += 1
    for at in sorted(blocks, reverse=True):
        lines[at:at] = blocks[at]
    new = "\n".join(lines)
    if new != text:
        Path(path).write_text(new, encoding="utf-8", newline="")
    return len(inserts)


def main(objects_dir):
    total = 0
    for p in sorted(Path(objects_dir).glob("*.obj")):
        head = p.read_bytes()
        if not any(f"fbw/anim/{leg}".encode() in head for leg in LEGS):
            continue
        n = apply(p)
        if n:
            print(f"{p.name}: {n} leg chains")
        total += n
    print(f"gear fold: {total} chains in {objects_dir}")


if __name__ == "__main__":
    for d in sys.argv[1:]:
        main(d)
