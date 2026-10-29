"""usage: remove_cduct_opening.py <objects dir> <backup dir>

Undoes reverser_hinges.py: the thrust reverser C-duct halves no longer open with
fbw/anim/cowl_open_<n> (the fan cowl doors still open over 0..0.5), and the C-duct
hold-open rods in a380_cowl_rods.obj are removed."""

import re
import shutil
import sys
from pathlib import Path

ROTATE = re.compile(r"^\s*ANIM_rotate_begin \S+ \S+ \S+ fbw/anim/cowl_open_\d\s*$")
TRIS = re.compile(r"^(\s*)TRIS (\d+) (\d+)\s*$")
ROD_GATE = re.compile(r"^\s*ANIM_hide -1 0\.985 fbw/anim/cowl_open_\d\s*$")
EXTERIOR = [f"a380_exterior_0{n}{s}.obj" for n in (20, 21, 22, 23) for s in ("", "_gp")]
RODS = "a380_cowl_rods.obj"


def is_cduct_block(lines, i):
    w = [l.strip() for l in lines[i:i + 9]]
    return (len(w) >= 9 and w[0] == "ANIM_begin" and w[1].startswith("ANIM_trans ") and ROTATE.match(lines[i + 2])
            and w[3] == "ANIM_rotate_key 0.5 0" and w[4].startswith("ANIM_rotate_key 1.0 ") and w[5] == "ANIM_rotate_end"
            and w[6].startswith("ANIM_trans "))


def collapse(lines):
    out, i, n = [], 0, 0
    while i < len(lines):
        if is_cduct_block(lines, i):
            j = i + 7
            body = []
            while lines[j].strip() != "ANIM_end":
                body.append(lines[j])
                j += 1
            if all(TRIS.match(b) for b in body):
                out += body
                i = j + 1
                n += 1
                continue
        out.append(lines[i])
        i += 1
    return out, n


def merge_tris(lines):
    out = []
    for l in lines:
        m = TRIS.match(l)
        p = TRIS.match(out[-1]) if out else None
        if m and p and m.group(1) == p.group(1) and int(p.group(2)) + int(p.group(3)) == int(m.group(2)):
            out[-1] = f"{p.group(1)}TRIS {p.group(2)} {int(p.group(3)) + int(m.group(3))}"
        else:
            out.append(l)
    return out


def drop_rods(lines):
    out, i, n = [], 0, 0
    while i < len(lines):
        if lines[i].strip() == "ANIM_begin" and i + 3 < len(lines) and ROD_GATE.match(lines[i + 1]) \
                and TRIS.match(lines[i + 2]) and lines[i + 3].strip() == "ANIM_end":
            i += 4
            n += 1
            continue
        out.append(lines[i])
        i += 1
    return out, n


def save(path, backup, lines):
    backup.mkdir(parents=True, exist_ok=True)
    if not (backup / path.name).exists():
        shutil.copy2(path, backup / path.name)
    path.write_text("\n".join(lines), encoding="utf-8", newline="\n")


def main(obj_dir, backup):
    obj_dir, backup = Path(obj_dir), Path(backup)
    for name in EXTERIOR:
        path = obj_dir / name
        if not path.exists():
            continue
        lines = path.read_text(encoding="utf-8", errors="replace").split("\n")
        out, n = collapse(lines)
        if n:
            save(path, backup, merge_tris(out))
        print(f"{name}: {n} C-duct halves fixed in place")
    path = obj_dir / RODS
    lines = path.read_text(encoding="utf-8", errors="replace").split("\n")
    out, n = drop_rods(lines)
    if n:
        save(path, backup, out)
    print(f"{RODS}: {n} C-duct rod groups removed")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
