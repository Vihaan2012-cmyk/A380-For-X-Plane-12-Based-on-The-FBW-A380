"""usage: fix_lone_show.py <objects dir> <name.obj>...

An ANIM_show with no ANIM_hide of the same dataref before it in its block never hides anything: X-Plane
starts every block visible. Each such show is moved to the top of its block behind an ANIM_hide of the
dataref's whole range, so the part is drawn only inside the show's range, and the block's other hides
(engine separation and so on) still apply after it. Already-fixed blocks are left as they are."""

import sys
from pathlib import Path

WHOLE = "-99999 99999"


def fix(lines):
    out, changed = [], 0
    starts = []
    for line in lines:
        out.append(line)
        t = line.strip()
        if t == "ANIM_begin":
            starts.append({"at": len(out), "hidden": set(), "inserted": 0})
            continue
        if t == "ANIM_end":
            if starts:
                starts.pop()
            continue
        if not starts:
            continue
        b = starts[-1]
        p = t.split()
        if p[:1] == ["ANIM_hide"] and len(p) >= 4:
            b["hidden"].add(p[3])
        elif p[:1] == ["ANIM_show"] and len(p) >= 4 and p[3] not in b["hidden"]:
            out.pop()
            indent = line[: len(line) - len(line.lstrip())]
            at = b["at"] + b["inserted"]
            out[at:at] = [f"{indent}ANIM_hide {WHOLE} {p[3]}", line]
            b["inserted"] += 2
            for later in starts:
                if later is not b and later["at"] > b["at"]:
                    later["at"] += 2
            b["hidden"].add(p[3])
            changed += 1
    return out, changed


def main(objects, *names):
    for n in names:
        p = Path(objects) / n
        lines = p.read_text(encoding="latin-1").split("\n")
        out, changed = fix(lines)
        if changed:
            p.write_text("\n".join(out), encoding="latin-1", newline="\n")
        print(n, changed, "lone shows fixed")


if __name__ == "__main__":
    main(*sys.argv[1:])
