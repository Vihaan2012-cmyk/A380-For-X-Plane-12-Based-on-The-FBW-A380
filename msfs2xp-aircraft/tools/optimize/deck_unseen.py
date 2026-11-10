"""usage: deck_unseen.py <aircraft dir> <backup dir> [--apply]

Hides the flight deck's detail (knobs, switch panels, pedestal, overhead, lettering, cables, screws) while
the eye is outside it and more than 20 m away, as FlightFactor's ckptOut does: the plugin's
fbw/anim/deck_unseen. Walls, ceiling, seats, glareshield, main panel faces, window frames, glass and
screens stay, so the flight deck still reads through the windscreen. Objects with the rain flag (0x2000)
cannot take an acf hide dataref, so their geometry is wrapped in an ANIM_hide block per LOD instead.
Originals go to the backup dir once and are never overwritten."""

import re
import shutil
import sys
from pathlib import Path

DATAREF = "fbw/anim/deck_unseen"
RAIN_FLAG = 0x2000
COCKPIT_OBJECT = 0x800
HIDE = re.compile(r"KNOB|PEDESTAL|KEYBOARD|DECAL|PUSH|SCREWS|CABLES|SIDEBOARDS|AFT_MISC|SIDEWALL_MISC|COCKPIT_MISC|STICKER|LEATHER|RUDDER|FCU|OVHD|PUSHBUTTON|LIGHT_OVHD", re.I)
KEEP = re.compile(r"GLARESHIELD|MIP0\d|WINDSHIELD|WINDOW|CEILING|SEAT|BACKWALL|FLOOR|SIDEWALL0|FRONTSHIELD|ISOLATIONS|DISPLAY|SUNSHADE|GLASS|REGISTRATION|ICE|solid_", re.I)
FAR_DATAREF = "fbw/anim/deck_far"
FAR = re.compile(r"SEATS0|SEAT_TEXTILE|SEATSBELT|ISOLATIONS|FLOOR|SUNSHADE", re.I)
MARK = "# deck_unseen"


def albedo(path):
    for line in open(path, encoding="latin-1", errors="replace"):
        w = line.split()
        if w[:1] == ["TEXTURE"] and len(w) > 1:
            return w[1]
        if w[:1] in (["VT"], ["POINT_COUNTS"]):
            break
    return ""


def wrap(path):
    text = path.read_bytes().decode("latin-1")
    if MARK in text:
        return False
    lines = text.split("\n")
    last_data = max(i for i, l in enumerate(lines) if l.lstrip().startswith(("VT", "IDX")))
    body = lines[last_data + 1:]
    while body and not body[-1].strip():
        body.pop()
    gate = ["ANIM_begin", f"ANIM_hide 0.5 1.5 {DATAREF}"]
    out, open_block = [], False
    for line in body:
        if line.split()[:1] == ["ATTR_LOD"]:
            if open_block:
                out.append("ANIM_end")
            out.append(line)
            out += gate
            open_block = True
            continue
        if not open_block and line.strip() and not line.lstrip().startswith("#"):
            out += gate
            open_block = True
        out.append(line)
    if open_block:
        out.append("ANIM_end")
    head = lines[:last_data + 1]
    head.insert(next(i for i, l in enumerate(head) if l.strip() == "OBJ") + 1, MARK)
    path.write_bytes(("\n".join(head + out) + "\n").encode("latin-1"))
    return True


def set_hide(text, index, dataref):
    key = f"P _obja/{index}/_obj_hide_dataref "
    line = re.search(rf"^{re.escape(key)}(.*)$", text, re.M)
    if line:
        if line.group(1).strip():
            return text, False
        return text[:line.start()] + key + dataref + text[line.end():], True
    anchor = re.search(rf"^P _obja/{index}/_obj_flags .*$", text, re.M)
    return text[:anchor.end()] + "\n" + key + dataref + text[anchor.end():], True


def main(aircraft_dir, backup_dir, apply):
    aircraft, backup = Path(aircraft_dir), Path(backup_dir)
    objects = aircraft / "objects"
    acfs = sorted(p for p in aircraft.glob("*.acf"))
    wrapped, total = set(), 0
    for acf in acfs:
        text = acf.read_bytes().decode("latin-1")
        files = {k: v.strip() for k, v in re.findall(r"^P _obja/(\d+)/_v10_att_file_stl (.+)$", text, re.M)}
        flags = {k: int(v) for k, v in re.findall(r"^P _obja/(\d+)/_obj_flags (\d+)", text, re.M)}
        hides = {k: v.strip() for k, v in re.findall(r"^P _obja/(\d+)/_obj_hide_dataref (.*)$", text, re.M)}
        changed = 0
        for k, name in sorted(files.items(), key=lambda kv: int(kv[0])):
            if not name.startswith("a380_cockpit_") or "cabin" in name or hides.get(k):
                continue
            f = flags.get(k, 0)
            if not f & 1 or f & COCKPIT_OBJECT:
                continue
            p = objects / name
            if not p.is_file():
                continue
            tex = albedo(p)
            if KEEP.search(tex):
                if FAR.search(tex) and not f & RAIN_FLAG:
                    print(f"  far   {name:34} {tex}") if acf == acfs[0] else None
                    text, c = set_hide(text, k, FAR_DATAREF)
                    changed += c
                continue
            if not HIDE.search(tex):
                continue
            if f & RAIN_FLAG:
                if name not in wrapped:
                    wrapped.add(name)
                    print(f"  wrap  {name:34} {tex}")
                    if apply:
                        if not (backup / name).exists():
                            shutil.copy2(p, backup / name)
                        wrap(p)
                continue
            print(f"  hide  {name:34} {tex}") if acf == acfs[0] else None
            text, c = set_hide(text, k, DATAREF)
            changed += c
        total += changed
        if apply and changed:
            if not (backup / acf.name).exists():
                shutil.copy2(acf, backup / acf.name)
            acf.write_bytes(text.encode("latin-1"))
        print(f"{acf.name}: {changed} hide datarefs {'set' if apply else 'to set'}")
    print(f"{len(wrapped)} rain-flag objects {'wrapped' if apply else 'to wrap'}")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if apply := "--apply" in sys.argv:
        Path(args[1]).mkdir(parents=True, exist_ok=True)
    main(args[0], args[1], apply)
