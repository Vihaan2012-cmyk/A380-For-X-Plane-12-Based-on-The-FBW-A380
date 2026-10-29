"""usage: flap_separation.py <objects dir>

Gives each flap panel its own separation gate: the flap-animated triangles of
the panel objects are split at the panel boundaries measured on the meshes
(inboard 4.3-11.55 m, mid to 19.52 m, outboard beyond) and wrapped in
ANIM_hide on fbw/fail/sep/<side>_flap_<panel>."""

import sys
from pathlib import Path

import numpy as np

OBJECTS = {"a380_exterior_027.obj": "l", "a380_exterior_029.obj": "r"}
BOUNDS = ((11.55, "inner"), (19.52, "mid"), (1e9, "outer"))
GATE = "fbw/fail/sep/{}_flap_{}"


def panel(x):
    return next(name for edge, name in BOUNDS if abs(x) < edge)


def split(lines, side):
    vt = np.array([[float(v) for v in l.split()[1:4]] for l in lines if l.startswith("VT")])
    idx = np.array([int(v) for l in lines if l.startswith(("IDX10", "IDX ")) for v in l.split()[1:]], dtype=np.int64)
    out, stack, reordered, wrapped = [], [], False, 0
    for line in lines:
        t = line.strip()
        p = t.split()
        if t == "ANIM_begin":
            stack.append(False)
        elif t == "ANIM_end":
            if stack:
                stack.pop()
        elif p and p[0].startswith("ANIM_") and stack and p[-1].endswith(f"{side}_flap_percent_key"):
            stack[-1] = True
        if not (t.startswith("TRIS ") and any(stack)):
            out.append(line)
            continue
        first, count = map(int, p[1:3])
        tri = idx[first:first + count].reshape(-1, 3)
        names = [panel(x) for x in vt[tri].mean(axis=1)[:, 0]]
        groups = list(dict.fromkeys(names))
        order = np.concatenate([np.flatnonzero([n == g for n in names]) for g in groups])
        if (order != np.arange(len(order))).any():
            idx[first:first + count] = tri[order].ravel()
            reordered = True
        pad = line[:len(line) - len(line.lstrip())]
        at = first
        for g in groups:
            n = names.count(g) * 3
            out += [f"{pad}ANIM_begin", f"{pad}ANIM_hide 0.5 1.5 {GATE.format(side, g)}", f"{pad}TRIS {at} {n}", f"{pad}ANIM_end"]
            at += n
            wrapped += 1
    if reordered:
        body = [l for l in out if not l.startswith(("IDX10", "IDX "))]
        at = next(i for i, l in enumerate(body) if l.startswith("VT")) + len(vt)
        rows = [f"IDX10 {' '.join(map(str, idx[k:k + 10]))}" for k in range(0, len(idx) - len(idx) % 10, 10)]
        rows += [f"IDX {v}" for v in idx[len(idx) - len(idx) % 10:]]
        out = body[:at] + rows + body[at:]
    return out, wrapped


def main(obj_dir):
    for name, side in OBJECTS.items():
        path = Path(obj_dir) / name
        raw = path.read_bytes().decode("utf-8")
        nl = "\r\n" if "\r\n" in raw else "\n"
        lines = raw.split(nl)
        if any(f"{side}_flap_inner" in l for l in lines):
            print(name, "already split")
            continue
        out, wrapped = split(lines, side)
        path.write_bytes(nl.join(out).encode("utf-8"))
        print(name, wrapped, "flap batches gated")


if __name__ == "__main__":
    main(sys.argv[1])
