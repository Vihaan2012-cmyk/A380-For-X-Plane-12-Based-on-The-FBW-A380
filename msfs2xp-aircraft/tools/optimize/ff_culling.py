"""usage: ff_culling.py <aircraft dir> <backup dir> [--dry-run]

FlightFactor-style whole-object culling for the A380:

- every cabin object lying wholly aft of the flight deck (z >= CABIN_AFT_OF) gets the acf hide dataref
  fbw/anim/cabin_hidden (1 while the eye is in the flight deck with its door shut) and, as a fallback that
  needs no acf support, the same gate inside the object;
- every exterior object that is either already hidden from the flight deck in all of its geometry, or
  lies wholly away from it (OUT_OF_SIGHT), gets the acf hide dataref fbw/anim/eye_in_flight_deck.

Both .acf files are edited as bytes (they are LF); objects and acfs are copied to the backup dir first.
Re-running only adds what is missing."""

import re
import shutil
import sys
from pathlib import Path

CABIN_AFT_OF = -29.8
CABIN_PREFIXES = ("a380_cockpit_", "a380_cabin_")
OUT_OF_SIGHT = {
    "a380_engdress_trent.obj", "a380_engdress_gp.obj", "a380_engine_covers.obj", "a380_engine_covers_gp.obj",
    "a380_sep_scars.obj", "a380_evac_slides.obj", "a380_door_arms.obj", "a380_walkaround.obj",
}
EYE = "fbw/anim/eye_in_flight_deck"
CABIN = "fbw/anim/cabin_hidden"
CABIN_GATE = f"ANIM_hide 0.5 1.5 {CABIN}"
ACFS = ("A380.acf",)


def z_range(path):
    zs = [float(line.split()[3]) for line in open(path, encoding="latin-1", errors="replace") if line.startswith("VT")]
    return (min(zs), max(zs)) if zs else (None, None)


def fully_eye_gated(path):
    stack, total, gated = [], 0, 0
    for line in open(path, encoding="latin-1", errors="replace"):
        w = line.split()
        if not w:
            continue
        if w[0] == "ANIM_begin":
            stack.append(False)
        elif w[0] == "ANIM_end" and stack:
            stack.pop()
        elif w[0] == "ANIM_hide" and stack and w[-1] == EYE:
            stack[-1] = True
        elif w[0] == "TRIS":
            n = int(w[2])
            total += n
            if any(stack):
                gated += n
    return total > 0 and gated == total


def gate_inside(path):
    """Wraps every top-level piece of geometry in an ANIM block hidden by the cabin dataref."""
    text = path.read_text(encoding="latin-1")
    if CABIN in text:
        return False
    lines = text.split("\n")
    first = next(i for i, l in enumerate(lines) if l.split()[:1] in (["ANIM_begin"], ["TRIS"], ["ATTR_cull"], ["ATTR_no_blend"], ["ATTR_poly_os"], ["ATTR_light_level"], ["ATTR_blend"]))
    last = max(i for i, l in enumerate(lines) if l.split()[:1] in (["ANIM_end"], ["TRIS"], ["ATTR_light_level_reset"]))
    lines = lines[:first] + ["ANIM_begin", CABIN_GATE] + lines[first:last + 1] + ["ANIM_end"] + lines[last + 1:]
    path.write_text("\n".join(lines), encoding="latin-1", newline="\n")
    return True


def set_hide(acf_bytes, index, dataref):
    text = acf_bytes.decode("latin-1")
    key = f"P _obja/{index}/_obj_hide_dataref "
    line = re.search(rf"^{re.escape(key)}(.*)$", text, re.M)
    if line:
        if line.group(1).strip() == dataref:
            return acf_bytes, False
        text = text[:line.start()] + key + dataref + text[line.end():]
    else:
        anchor = re.search(rf"^P _obja/{index}/_obj_flags .*$", text, re.M)
        text = text[:anchor.end()] + "\n" + key + dataref + text[anchor.end():]
    return text.encode("latin-1"), True


def main(aircraft_dir, backup_dir, dry=False):
    aircraft, backup = Path(aircraft_dir), Path(backup_dir)
    objects = aircraft / "objects"
    acf_text = (aircraft / ACFS[0]).read_text(encoding="latin-1")
    names = sorted({n.strip() for n in re.findall(r"^P _obja/\d+/_v10_att_file_stl (.+)$", acf_text, re.M)})
    plan = {}
    for n in names:
        p = objects / n
        if not p.is_file():
            continue
        if n.startswith(CABIN_PREFIXES):
            zmin, _ = z_range(p)
            if zmin is not None and zmin >= CABIN_AFT_OF:
                plan[n] = CABIN
        elif n in OUT_OF_SIGHT or fully_eye_gated(p):
            plan[n] = EYE
    print(f"{sum(v == CABIN for v in plan.values())} cabin objects, {sum(v == EYE for v in plan.values())} exterior objects")
    for n, d in sorted(plan.items()):
        print(f"  {d:28} {n}")
    if dry:
        return
    backup.mkdir(parents=True, exist_ok=True)
    for acf in ACFS:
        if not (backup / acf).exists():
            shutil.copy2(aircraft / acf, backup / acf)
    gated = 0
    for n, d in plan.items():
        if d == CABIN:
            if not (backup / n).exists():
                shutil.copy2(objects / n, backup / n)
            gated += gate_inside(objects / n)
    for acf in ACFS:
        path = aircraft / acf
        data = path.read_bytes()
        text = data.decode("latin-1")
        index = {v.strip(): k for k, v in re.findall(r"^P _obja/(\d+)/_v10_att_file_stl (.+)$", text, re.M)}
        changed = 0
        for n, d in plan.items():
            if n in index:
                data, c = set_hide(data, index[n], d)
                changed += c
        path.write_bytes(data)
        print(f"{acf}: {changed} hide datarefs set")
    print(f"{gated} cabin objects gated inside")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    main(args[0], args[1], "--dry-run" in sys.argv)
