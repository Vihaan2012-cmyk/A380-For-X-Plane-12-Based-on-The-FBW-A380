"""usage: exterior_lods.py <aircraft dir> <backup dir> [--apply] [--min-tris N]

Gives the exterior objects distance levels: up close every object stays exactly as it is (objdecimate level
with error 0), further out each batch is simplified by meshoptimizer to an error below half a pixel at the
level's near distance (2560 px across 60 degrees), small parts held to a share of their own size. Every
command and animation is kept per level. Originals go to the backup dir once and are always the source,
so a rerun never simplifies twice. Objects already carrying several levels, the hail and damage variants,
parts that only show when something breaks, and the cockpit and cabin are left alone."""

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

OBJDECIMATE = r"E:/fbw-build/target/release/objdecimate.exe"
LEVELS = ["0:90:0", "90:250:0.012:keep:0.02:0.02", "250:800:0.04:prune:0.02", "800:30000:0.12:prune:0.02"]
SKIP = re.compile(r"(_hail|walkaround|engdetach|cowldetach|partdetach|detach|debris|sep_scars|separation|brake_fire|evac_slides|door_arms|cowl_rods|crushed|burnt|damage|fan_blur|blur|_lamps|lights|spill|a380_hud)", re.I)
MARK = "# exterior_lods"
MIN_TRIS = 15000


def stats(path):
    tris, lods, cmds = 0, 0, 0
    for line in open(path, encoding="latin-1", errors="replace"):
        t = line.lstrip()
        if t.startswith("TRIS"):
            tris += int(t.split()[2]) // 3
            cmds += 1
        elif t.startswith("ATTR_LOD"):
            lods += 1
    return tris, lods, cmds


def per_level(path):
    out, cur = [], None
    for line in open(path, encoding="latin-1", errors="replace"):
        t = line.lstrip()
        if t.startswith("ATTR_LOD"):
            cur = [t.split()[1], t.split()[2], 0, 0]
            out.append(cur)
        elif t.startswith("TRIS") and cur is not None:
            cur[2] += int(t.split()[2]) // 3
            cur[3] += 1
    return out


def main(aircraft_dir, backup_dir, apply, min_tris):
    aircraft, backup = Path(aircraft_dir), Path(backup_dir)
    objects = aircraft / "objects"
    acf = (aircraft / "A380.acf").read_bytes().decode("latin-1")
    names = sorted({v.strip() for v in re.findall(r"^P _obja/\d+/_v10_att_file_stl (.+)$", acf, re.M)})
    todo = []
    for n in names:
        if n.startswith("a380_cockpit_") or "cabin" in n or SKIP.search(n):
            continue
        src = backup / n if (backup / n).exists() else objects / n
        if not src.is_file():
            continue
        if MARK.encode() in (objects / n).read_bytes()[:400] and not (backup / n).exists():
            continue
        tris, lods, cmds = stats(src)
        if tris < min_tris or lods > 1:
            continue
        todo.append((n, src, tris, cmds))
    print(f"{len(todo)} objects, {sum(t for _, _, t, _ in todo):,} triangles")
    if not apply:
        for n, _, t, c in todo:
            print(f"  {n:36} {t:>8} tris {c:>4} batches")
        return
    backup.mkdir(parents=True, exist_ok=True)
    total_before = total_far = 0
    with tempfile.TemporaryDirectory() as tmp:
        for n, src, tris, cmds in todo:
            if not (backup / n).exists():
                shutil.copy2(objects / n, backup / n)
                src = backup / n
            out = Path(tmp) / n
            r = subprocess.run([OBJDECIMATE, str(src), str(out), *LEVELS], capture_output=True, text=True)
            if r.returncode != 0 or not out.is_file():
                print(f"  {n}: objdecimate failed: {r.stderr.strip()[-300:]}")
                continue
            levels = per_level(out)
            if len(levels) != len(LEVELS) or any(l[3] != cmds for l in levels) or levels[0][2] != tris:
                print(f"  {n}: levels do not match the original ({levels}), left unchanged")
                continue
            text = [l for l in out.read_bytes().decode("latin-1").split("\n") if not re.match(r"^\s*TRIS \d+ 0\s*$", l)]
            text.insert(next(i for i, l in enumerate(text) if l.strip() == "OBJ") + 1, MARK)
            (objects / n).write_bytes("\n".join(text).encode("latin-1"))
            total_before += tris
            total_far += levels[1][2]
            print(f"  {n:36} " + "  ".join(f"{a}-{b} m {t:>7}" for a, b, t, _ in levels))
    print(f"90-250 m: {total_before:,} -> {total_far:,} triangles")


if __name__ == "__main__":
    a = sys.argv[1:]
    mt = int(a[a.index("--min-tris") + 1]) if "--min-tris" in a else MIN_TRIS
    pos = [x for i, x in enumerate(a) if not x.startswith("--") and (i == 0 or a[i - 1] != "--min-tris")]
    main(pos[0], pos[1], "--apply" in a, mt)
