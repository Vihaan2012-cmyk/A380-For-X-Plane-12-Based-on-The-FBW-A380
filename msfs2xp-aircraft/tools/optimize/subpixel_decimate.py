"""usage: subpixel_decimate.py <aircraft dir> <backup dir> [--error-mm MM] [--dry-run]

Cuts triangles nobody can see: every acf object is simplified with objdecimate (meshoptimizer, UV seams
and normals kept, every command and animation kept) to an error below what its closest viewing distance
can show (cockpit 0.2 mm, cabin and exterior 0.5 mm, or one limit for all with --error-mm). An object is only replaced when its commands are
unchanged, all indices are in range and it lost at least MIN_GAIN of its triangles; originals go to the
backup dir first and are the source of every later run, so a new limit re-cuts the original instead of
decimating twice. Each result carries a `# subpixel <mm>` header line; objects already at that limit are skipped."""

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

OBJDECIMATE = r"D:/A380/fbw-build/target/release/objdecimate.exe"
COCKPIT_ERROR_M = 0.0002
OTHER_ERROR_M = 0.0005
MIN_GAIN = 0.03
SKIP = re.compile(r"(_click|_lamps|_hail|exterior_044|walkaround|engdetach|cowldetach|partdetach|a380_hud|cowl_rods|evac_slides|sep_scars|brake_fire|door_arms)", re.I)
MARK = "# subpixel"


def parse(path):
    vt = idx = 0
    commands, lods = [], []
    for line in open(path, encoding="latin-1", errors="replace"):
        w = line.split()
        if not w:
            continue
        if w[0] == "VT":
            vt += 1
        elif w[0] in ("IDX10", "IDX"):
            idx += len(w) - 1
        elif w[0] == "TRIS":
            commands.append("TRIS")
        elif w[0] == "ATTR_LOD":
            lods.append((w[1], w[2]))
        elif w[0] in ("POINT_COUNTS", "I", "800", "OBJ") or w[0].startswith("#"):
            continue
        else:
            commands.append(" ".join(w))
    return vt, idx, commands, lods


def indices_ok(path, vt):
    for line in open(path, encoding="latin-1", errors="replace"):
        w = line.split()
        if w and w[0] in ("IDX10", "IDX"):
            if any(int(x) >= vt for x in w[1:]):
                return False
    return True


def main(aircraft_dir, backup_dir, dry=False, error_mm=None, rel=0.0):
    aircraft, backup = Path(aircraft_dir), Path(backup_dir)
    objects = aircraft / "objects"
    acf = (aircraft / "A380.acf").read_text(encoding="latin-1")
    internal = dict(re.findall(r"^P _obja/(\d+)/_v10_is_internal (\d)$", acf, re.M))
    names = {}
    for k, n in re.findall(r"^P _obja/(\d+)/_v10_att_file_stl (.+)$", acf, re.M):
        names[n.strip()] = internal.get(k) == "1"
    before_total = after_total = 0
    tmp = Path(tempfile.mkdtemp(prefix="subpixel-"))
    for n, inside in sorted(names.items()):
        src = objects / n
        if not src.is_file() or SKIP.search(n):
            continue
        cockpit = n.startswith("a380_cockpit_") and inside
        error = error_mm / 1000 if error_mm else COCKPIT_ERROR_M if cockpit else OTHER_ERROR_M
        mark = f"{MARK} {error * 1000:g}" + (f" rel{rel:g}" if rel else "")
        if mark in src.read_text(encoding="latin-1", errors="replace")[:400]:
            continue
        original = backup / n if (backup / n).is_file() else src
        vt, idx, commands, lods = parse(original)
        if idx < 3000:
            continue
        if len(lods) > 1:
            continue
        near, far = lods[0] if lods else ("0", "100000")
        out = tmp / n
        r = subprocess.run([OBJDECIMATE, str(original), str(out), f"{near}:{far}:{error}:keep:0.02:{rel}" if rel else f"{near}:{far}:{error}:keep"], capture_output=True, text=True)
        if r.returncode != 0 or not out.is_file():
            print(f"  {n}: objdecimate failed: {r.stderr.strip()[:120]}")
            continue
        vt2, idx2, commands2, lods2 = parse(out)
        if [c for c in commands2 if c != "TRIS"] != [c for c in commands if c != "TRIS"] or commands2.count("TRIS") != commands.count("TRIS"):
            print(f"  {n}: commands changed, kept original")
            continue
        if not indices_ok(out, vt2):
            print(f"  {n}: index out of range, kept original")
            continue
        gain = 1 - idx2 / idx
        if gain < MIN_GAIN:
            continue
        before_total += idx // 3
        after_total += idx2 // 3
        print(f"  {n:44} {idx // 3:8} -> {idx2 // 3:8} ({gain * 100:4.1f}% at {error * 1000:.1f} mm)")
        if dry:
            continue
        backup.mkdir(parents=True, exist_ok=True)
        if not (backup / n).exists():
            shutil.copy2(src, backup / n)
        lines = out.read_text(encoding="latin-1").split("\n")
        lines.insert(3, mark)
        src.write_text("\n".join(lines), encoding="latin-1", newline="\n")
    print(f"total {before_total} -> {after_total} triangles in the objects changed ({(before_total - after_total) / 1e6:.2f} M fewer)")


if __name__ == "__main__":
    argv = sys.argv[1:]
    mm = float(argv[argv.index("--error-mm") + 1]) if "--error-mm" in argv else None
    rel = float(argv[argv.index("--rel") + 1]) if "--rel" in argv else 0.0
    pos = [x for i, x in enumerate(argv) if not x.startswith("--") and (i == 0 or argv[i - 1] not in ("--error-mm", "--rel"))]
    main(pos[0], pos[1], "--dry-run" in argv, mm, rel)
