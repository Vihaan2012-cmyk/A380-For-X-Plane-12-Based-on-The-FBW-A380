"""usage: lights_wing_flex.py <objects dir> [--check]

Puts the wing's lights (nav, strobe) in lights.obj on the same WING_FLEX joint chain as the wing skin
outboard of them, so they stay on the wingtip when the wing bends. Rerunnable: lights already on a
chain are left alone. --check only reports, and compares the chain with the skin's at the wingtip."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import wing_bend as wb  # noqa: E402

LIGHT = ("LIGHT_PARAM", "LIGHT_NAMED")


def light_xyz(line):
    t = line.split()
    if t and t[0] in LIGHT:
        return float(t[2]), float(t[3]), float(t[4])
    return None


def items_of(lines):
    """Top-level items: a lone line, or a whole ANIM_begin..ANIM_end block."""
    out, i = [], 0
    while i < len(lines):
        if lines[i].strip() == "ANIM_begin":
            depth, j = 1, i + 1
            while depth:
                s = lines[j].strip()
                depth += (s == "ANIM_begin") - (s == "ANIM_end")
                j += 1
            out.append(lines[i:j])
            i = j
        else:
            out.append([lines[i]])
            i += 1
    return out


def wrap(lines):
    out, done, kept = [], 0, 0
    for item in items_of(lines):
        pts = [p for p in map(light_xyz, item) if p]
        wing = [p for p in pts if abs(p[0]) > wb.STATIONS[0] and p[2] <= wb.MAX_Z]
        if not wing:
            out += item
            continue
        if any(wb.MARK in l for l in item):
            kept += len(wing)
            out += item
            continue
        side = 1 if wing[0][0] > 0 else -1
        q = min(sum(1 for s in wb.STATIONS if s <= abs(p[0])) for p in wing)
        out += ["ANIM_begin"] + wb.chain(side, wb.full(q)) + item + ["ANIM_end"]
        done += len(wing)
    return out, done, kept


def skin_chain(objects, point):
    """The flex chain around the wing-skin triangle nearest a point, as text."""
    best = None
    for f in sorted(objects.glob("a380_exterior_02[6-9].obj")):
        vt, idx, stack = [], [], []
        for line in f.read_text(errors="replace").splitlines():
            t = line.split()
            if not t:
                continue
            if t[0] == "VT":
                vt.append((float(t[1]), float(t[2]), float(t[3])))
            elif t[0] == "IDX10":
                idx += [int(x) for x in t[1:11]]
            elif t[0] == "IDX":
                idx.append(int(t[1]))
            elif t[0] == "ANIM_begin":
                stack.append([])
            elif t[0] == "ANIM_end":
                stack.pop()
            elif t[0].startswith("ANIM_") and stack:
                stack[-1].append(" ".join(t))
            elif t[0] == "TRIS":
                for j in range(int(t[1]), int(t[1]) + int(t[2])):
                    v = vt[idx[j]]
                    d = sum((a - b) ** 2 for a, b in zip(v, point))
                    if best is None or d < best[0]:
                        best = (d, [l for b in stack for l in b if wb.MARK in l or l.startswith(("ANIM_rotate_key", "ANIM_rotate_end", "ANIM_trans "))], f.name)
    return best


def main(objects_dir, *flags):
    objects = Path(objects_dir)
    path = objects / "lights.obj"
    lines = path.read_text().splitlines()
    out, done, kept = wrap(lines)
    print(f"lights.obj: {done} wing lights put on the flex chain, {kept} already on it")
    if "--check" in flags:
        for side in (-1, 1):
            tip = (side * 38.78, 2.27, 11.14)
            d, skin, name = skin_chain(objects, tip)
            mine = [" ".join(l.split()) for l in wb.chain(side, wb.full(4))]
            flex = [l for l in skin if wb.MARK in l]
            print(f"  {'left' if side < 0 else 'right'} tip: nearest skin {d ** 0.5:.2f} m away in {name}, its flex joints {'match' if flex == [l for l in mine if wb.MARK in l] else 'DIFFER'} ({len(flex)} rotations)")
        return
    if done:
        path.write_text("\n".join(out) + "\n")


if __name__ == "__main__":
    main(*sys.argv[1:])
