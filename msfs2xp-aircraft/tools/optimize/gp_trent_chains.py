"""usage: gp_trent_chains.py <objects dir> [base ...]

Gives every GP7200 engine object the animation of its Trent twin: each
ANIM_ line of a380_exterior_<base>_gp.obj is replaced by the matching line of
a380_exterior_<base>.obj, except the engine type gate. The shapes stay the
GP7200's; the pivots and wing bend weights become the Trent's, so all parts
of an engine bend and wobble together. A GP core with its own weight moved
against the nacelle and opened a hairline under engines 1 and 3.

Run it last, after gp7200_variant, wing_bend and gp7200_exhaust."""

import sys
from pathlib import Path

BASES = ("001", "020", "021", "022", "023", "030", "031")
GATE = "fbw/engine/gp7200"


def lines_of(path):
    text = path.read_text(encoding="utf-8", errors="replace")
    nl = "\r\n" if "\r\n" in text else "\n"
    return text.split(nl), nl


def merge(trent, gp):
    anims = [l for l in trent if l.strip().startswith("ANIM_")]
    out, k = [], 0
    for l in gp:
        t = l.strip()
        if not t.startswith("ANIM_"):
            out.append(l)
            continue
        if k >= len(anims):
            raise ValueError("the GP object has more ANIM lines than its Trent twin")
        twin = anims[k]
        k += 1
        if twin.split()[0] != t.split()[0]:
            raise ValueError(f"ANIM line {k} differs in kind: {twin!r} / {t!r}")
        out.append(l if GATE in t else twin)
    if k != len(anims):
        raise ValueError("the Trent twin has more ANIM lines than the GP object")
    return out


def main(objects_dir, *bases):
    objects = Path(objects_dir)
    for base in bases or BASES:
        trent_path = objects / f"a380_exterior_{base}.obj"
        gp_path = objects / f"a380_exterior_{base}_gp.obj"
        if not (trent_path.is_file() and gp_path.is_file()):
            print(base, "skipped: missing a twin")
            continue
        trent, _ = lines_of(trent_path)
        gp, nl = lines_of(gp_path)
        merged = merge(trent, gp)
        changed = sum(1 for a, b in zip(gp, merged) if a != b)
        gp_path.write_text(nl.join(merged), encoding="utf-8", newline="")
        print(base, changed, "ANIM lines taken from the Trent twin")


if __name__ == "__main__":
    main(*sys.argv[1:])
