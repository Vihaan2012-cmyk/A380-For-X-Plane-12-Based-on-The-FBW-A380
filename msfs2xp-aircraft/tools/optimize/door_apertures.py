import re
import sys
from pathlib import Path

DOOR = re.compile(r"fbw/anim/ANIM_DOOR_([MU]\d[LR])$|fbw/door/([MU]\d[LR])/open_ratio$")
SHRINK = 0.05
SKIN = 0.03
CELL = 0.05


def parse(lines):
    vt, idx = [], []
    for l in lines:
        if l.startswith("VT"):
            p = l.split()
            vt.append((float(p[1]), float(p[2]), float(p[3])))
        elif l.startswith(("IDX10", "IDX ")):
            idx.extend(int(v) for v in l.split()[1:])
    return vt, idx


def batches(lines):
    stack = []
    for n, l in enumerate(lines):
        s = l.strip()
        if s == "ANIM_begin":
            stack.append([])
        elif s == "ANIM_end":
            if stack:
                stack.pop()
        elif s.startswith("ANIM_") and stack:
            m = DOOR.search(s.split()[-1])
            if m:
                stack[-1].append((m.group(1) or m.group(2), s.split()[-1]))
        elif s.startswith("TRIS "):
            doors = [d for g in stack for d in g]
            yield n, int(s.split()[1]), int(s.split()[2]), doors


def centroid(vt, idx, k):
    a, b, c = (vt[i] for i in idx[k:k + 3])
    return ((a[0] + b[0] + c[0]) / 3, (a[1] + b[1] + c[1]) / 3, (a[2] + b[2] + c[2]) / 3)


def leaf_bounds(objects):
    bounds = {}
    for lines, vt, idx in objects:
        for _, start, count, doors in batches(lines):
            if not doors:
                continue
            name, dref = doors[0]
            b = bounds.setdefault(name, {"dref": dref, "y": [99, -99], "z": [99, -99], "x": 99, "side": 0, "n": 0, "surf": {}})
            for k in range(start, start + count, 3):
                x, y, z = centroid(vt, idx, k)
                b["y"] = [min(b["y"][0], y), max(b["y"][1], y)]
                b["z"] = [min(b["z"][0], z), max(b["z"][1], z)]
                b["x"] = min(b["x"], abs(x))
                b["side"] += 1 if x > 0 else -1
                b["n"] += 1
                for px, py, pz in [vt[i] for i in idx[k:k + 3]] + [(x, y, z)]:
                    key = (round(py / CELL), round(pz / CELL))
                    b["surf"][key] = max(b["surf"].get(key, 0.0), abs(px))
    return {k: v for k, v in bounds.items() if v["n"] > 50 and k.startswith("U")}


def door_of(c, bounds):
    x, y, z = c
    for name, b in bounds.items():
        side = 1 if b["side"] > 0 else -1
        if x * side <= 0:
            continue
        if not (b["y"][0] + SHRINK < y < b["y"][1] - SHRINK and b["z"][0] + SHRINK < z < b["z"][1] - SHRINK):
            continue
        ky, kz = round(y / CELL), round(z / CELL)
        near = [b["surf"][(ky + i, kz + j)] for i in (-1, 0, 1) for j in (-1, 0, 1) if (ky + i, kz + j) in b["surf"]]
        if near and max(near) - SKIN <= abs(x) <= max(near) + 0.15:
            return name
    return None


def patch(lines, vt, idx, bounds):
    out, changed = [], 0
    plan = {n: (start, count) for n, start, count, doors in batches(lines) if not doors}
    for n, l in enumerate(lines):
        if n not in plan:
            out.append(l)
            continue
        start, count = plan[n]
        groups = {}
        for k in range(start, start + count, 3):
            groups.setdefault(door_of(centroid(vt, idx, k), bounds), []).append(idx[k:k + 3])
        if list(groups) == [None]:
            out.append(l)
            continue
        indent = l[:len(l) - len(l.lstrip())]
        pos = start
        for name in sorted(groups, key=lambda g: (g is not None, g or "")):
            flat = [i for t in groups[name] for i in t]
            idx[pos:pos + len(flat)] = flat
            if name is None:
                out.append(f"{indent}TRIS {pos} {len(flat)}")
            else:
                out += [f"{indent}ANIM_begin", f"{indent}ANIM_hide 0.02 9999 {bounds[name]['dref']}", f"{indent}TRIS {pos} {len(flat)}", f"{indent}ANIM_end"]
                changed += len(flat) // 3
            pos += len(flat)
    if not changed:
        return None, 0
    first = next(i for i, l in enumerate(out) if l.startswith(("IDX10", "IDX ")))
    out = [l for l in out if not l.startswith(("IDX10", "IDX "))]
    rows = []
    for k in range(0, len(idx) - len(idx) % 10, 10):
        rows.append("IDX10 " + " ".join(map(str, idx[k:k + 10])))
    for k in range(len(idx) - len(idx) % 10, len(idx)):
        rows.append(f"IDX {idx[k]}")
    out[first:first] = rows
    return out, changed


def main(acf, objects_dir, backup):
    t = Path(acf).read_bytes().decode("latin-1")
    files = sorted(set(re.findall(r"P _obja/\d+/_v10_att_file_stl (\S+)", t)))
    d = Path(objects_dir)
    loaded = {}
    for f in files:
        if not f.startswith("a380_exterior"):
            continue
        lines = (d / f).read_text(encoding="latin-1").split("\n")
        vt, idx = parse(lines)
        if vt:
            loaded[f] = (lines, vt, idx)
    bounds = leaf_bounds(loaded.values())
    for name, b in sorted(bounds.items()):
        print(name, b["dref"], "y", [round(v, 2) for v in b["y"]], "z", [round(v, 2) for v in b["z"]], "x", round(b["x"], 2))
    for f, (lines, vt, idx) in loaded.items():
        out, n = patch(lines, vt, idx, bounds)
        if out is None:
            continue
        bk = Path(backup) / f
        if not bk.exists():
            bk.write_bytes((d / f).read_bytes())
        (d / f).write_text("\n".join(out).rstrip("\n") + "\n", encoding="latin-1")
        print(f, "hides", n, "triangles while a door is open")


if __name__ == "__main__":
    main(*sys.argv[1:])
