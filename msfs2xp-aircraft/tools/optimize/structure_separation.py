import json
import os
import sys

import numpy as np

SEP = "fbw/fail/sep/"
NATIVE = "sim/operation/failures/"
TRIM = "fbw/anim/trimtab_elevator_key"


def surface_rules():
    rules = {}
    for side in "lr":
        for k in (1, 2, 3):
            rules[f"fbw/anim/{side}_aileron{k}_percent_key"] = [f"{SEP}{side}_aileron{k}"]
        for part in ("inner", "outer"):
            rules[f"fbw/anim/{side}_{part}_elevator_deflection"] = [f"{SEP}{side}_elevator_{part}"]
        for k in range(1, 9):
            rules[f"fbw/anim/{side}_spoiler{k}_key"] = [f"{SEP}{side}_spoiler{k}"]
    for which in ("upper", "lower"):
        rules[f"fbw/anim/{which}_rudder_percent_key"] = [f"{SEP}rudder_{which}", f"{NATIVE}rel_vstb1"]
    return rules


ANIM_DREF = ("ANIM_rotate_begin", "ANIM_trans_begin", "ANIM_rotate", "ANIM_trans", "ANIM_hide", "ANIM_show")


def dref_of(parts):
    return parts[-1] if parts[0] in ("ANIM_rotate_begin", "ANIM_trans_begin", "ANIM_hide", "ANIM_show") else (parts[-1] if len(parts) >= 10 else None)


def parse(lines):
    vt = np.array([[float(v) for v in l.split()[1:4]] for l in lines if l.startswith("VT")]).reshape(-1, 3)
    idx = np.array([int(v) for l in lines if l.startswith(("IDX10", "IDX ")) for v in l.split()[1:]], dtype=np.int64)
    return vt, idx


def walk(lines):
    stack, out = [[]], []
    for i, line in enumerate(lines):
        p = line.split()
        if not p:
            continue
        if p[0] == "ANIM_begin":
            stack.append([])
        elif p[0] == "ANIM_end":
            stack.pop()
        elif p[0] in ANIM_DREF:
            d = dref_of(p)
            if d:
                stack[-1].append(d)
        elif p[0] == "TRIS":
            out.append((i, int(p[1]), int(p[2]), [d for frame in stack for d in frame]))
    return out


def probe(obj_dir, names):
    rules = surface_rules()
    boxes = {}
    for n in names:
        lines = open(os.path.join(obj_dir, n), encoding="utf-8", errors="replace").read().split("\n")
        vt, idx = parse(lines)
        for _, first, count, drefs in walk(lines):
            c = vt[idx[first:first + count]]
            for d in drefs:
                if d in rules or d == TRIM:
                    lo, hi = c.min(axis=0), c.max(axis=0)
                    b = boxes.setdefault(d, [lo, hi, set()])
                    b[0], b[1] = np.minimum(b[0], lo), np.maximum(b[1], hi)
                    b[2].add(n)
    for d, (lo, hi, files) in sorted(boxes.items()):
        print(f"{d:50s} x {lo[0]:7.2f}..{hi[0]:7.2f} y {lo[1]:6.2f}..{hi[1]:6.2f} z {lo[2]:7.2f}..{hi[2]:7.2f}  {sorted(files)}")


def keys_for(centres, drefs, regions, rules):
    base = []
    for d in drefs:
        base += rules.get(d, [])
    in_trim = TRIM in drefs
    keys = []
    for c in centres:
        k = list(base)
        if in_trim and abs(c[0]) > regions["stab_centre_x"]:
            k.append(f"{NATIVE}rel_hstbL" if c[0] < 0 else f"{NATIVE}rel_hstbR")
            k.append(f"{SEP}hstab_l" if c[0] < 0 else f"{SEP}hstab_r")
        if any(d.endswith("elevator_deflection") for d in drefs):
            k.append(f"{NATIVE}rel_hstbL" if c[0] < 0 else f"{NATIVE}rel_hstbR")
        fin = regions["fin"]
        if not in_trim and abs(c[0]) < fin["half_x"] and c[2] > fin["cut_z"][0] and c[1] > np.interp(c[2], fin["cut_z"], fin["cut_y"]):
            k.append(f"{NATIVE}rel_vstb1")
        if not in_trim and abs(c[0]) > regions["wingtip_x"] and c[1] < regions["wing_max_y"]:
            k.append(f"{NATIVE}rel_wing4L" if c[0] < 0 else f"{NATIVE}rel_wing4R")
        for d in drefs:
            if d.endswith(("aileron2_percent_key", "aileron3_percent_key")):
                k.append(f"{NATIVE}rel_wing4L" if c[0] < 0 else f"{NATIVE}rel_wing4R")
        keys.append(tuple(sorted(set(k))))
    return keys


def hide_lines(key):
    out = []
    for d in key:
        out.append(f"ANIM_hide 5.5 6.5 {d}" if d.startswith(NATIVE) else f"ANIM_hide 0.5 1.5 {d}")
    return out


def apply(obj_dir, names, regions):
    rules = surface_rules()
    for name in names:
        path = os.path.join(obj_dir, name)
        lines = open(path, encoding="utf-8", errors="replace").read().split("\n")
        if any(SEP in l or "rel_vstb1" in l or "rel_hstb" in l or "rel_wing4" in l for l in lines):
            print(name, "already split")
            continue
        vt, idx = parse(lines)
        tris = {i: (first, count, drefs) for i, first, count, drefs in walk(lines)}
        out, reordered, wrapped = [], False, 0
        for i, line in enumerate(lines):
            if i not in tris:
                out.append(line)
                continue
            first, count, drefs = tris[i]
            if count < 3:
                out.append(line)
                continue
            tri = idx[first:first + count].reshape(-1, 3)
            keys = keys_for(vt[tri].mean(axis=1), drefs, regions, rules)
            groups = list(dict.fromkeys(keys))
            if groups == [()]:
                out.append(line)
                continue
            order = np.concatenate([np.flatnonzero([k == g for k in keys]) for g in groups])
            if (order != np.arange(len(order))).any():
                idx[first:first + count] = tri[order].ravel()
                reordered = True
            at = first
            for g in groups:
                n = sum(1 for k in keys if k == g) * 3
                if g:
                    out.extend(["ANIM_begin", *hide_lines(g), f"TRIS {at} {n}", "ANIM_end"])
                    wrapped += 1
                else:
                    out.append(f"TRIS {at} {n}")
                at += n
        if not wrapped:
            continue
        if reordered:
            body = [l for l in out if not l.startswith(("IDX10", "IDX "))]
            at = next(i for i, l in enumerate(body) if l.startswith("VT")) + len(vt)
            rows = [f'IDX10 {" ".join(map(str, idx[i:i + 10]))}' for i in range(0, len(idx) - len(idx) % 10, 10)]
            rows += [f"IDX {v}" for v in idx[len(idx) - len(idx) % 10:]]
            out = body[:at] + rows + body[at:]
        with open(path, "w", encoding="utf-8", newline="\n") as f:
            f.write("\n".join(out))
        print(name, "split,", wrapped, "batches" + (", indices reordered" if reordered else ""))


if __name__ == "__main__":
    mode, obj_dir, *names = sys.argv[1:]
    if mode == "probe":
        probe(obj_dir, names)
    else:
        apply(obj_dir, names, json.loads(open(mode, encoding="utf-8").read()))
