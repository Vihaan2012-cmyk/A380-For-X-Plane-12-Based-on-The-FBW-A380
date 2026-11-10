"""usage: gate_object.py <obj> <backup dir> <dataref> <show from> [show to]

Draws an object only while <dataref> is in [from, to]: every LOD's geometry goes inside an ANIM block that
first hides the whole range and then shows the window (a lone ANIM_show never hides). Idempotent; the
original goes to the backup dir once."""
import shutil
import sys
from pathlib import Path


def main(obj, backup, dataref, lo, hi):
    obj, backup = Path(obj), Path(backup)
    mark = f"# gated {dataref}"
    text = obj.read_bytes().decode("latin-1")
    if mark in text:
        print(f"{obj.name}: already gated")
        return
    backup.mkdir(parents=True, exist_ok=True)
    if not (backup / obj.name).exists():
        shutil.copy2(obj, backup / obj.name)
    lines = text.split("\n")
    last = max(i for i, l in enumerate(lines) if l.lstrip().startswith(("VT", "IDX")))
    head, body = lines[: last + 1], lines[last + 1:]
    while body and not body[-1].strip():
        body.pop()
    gate = ["ANIM_begin", f"ANIM_hide -99999 99999 {dataref}", f"ANIM_show {lo} {hi} {dataref}"]
    out, open_block = [], False
    for line in body:
        if line.split()[:1] == ["ATTR_LOD"]:
            if open_block:
                out.append("ANIM_end")
            out += [line] + gate
            open_block = True
            continue
        if not open_block and line.strip() and not line.lstrip().startswith("#"):
            out += gate
            open_block = True
        out.append(line)
    if open_block:
        out.append("ANIM_end")
    head.insert(next(i for i, l in enumerate(head) if l.strip() == "OBJ") + 1, mark)
    obj.write_bytes(("\n".join(head + out) + "\n").encode("latin-1"))
    print(f"{obj.name}: drawn only while {dataref} is {lo}..{hi}")


if __name__ == "__main__":
    a = sys.argv[1:]
    main(a[0], a[1], a[2], a[3], a[4] if len(a) > 4 else "99999")
