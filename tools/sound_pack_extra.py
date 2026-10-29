"""usage: sound_pack_extra.py <sound set dir> [...]

Adds the x_A380 pack's events the converted .snd never played -- fuel and
hydraulic pumps, gear doors opening and closing, the right and upper pack
air, exterior packs and the exterior touchdown thumps -- to every A380*.snd
in each set directory whose GUIDs.txt has the event. Run again after the
pack is rebuilt; an attachment already present is left alone."""

import re
import sys
from pathlib import Path

EXTRA = [
    ("/Dynamics/FuelP", "0 0.6 -18", "fbw/snd/fuel_pumps == 1", None),
    ("/Dynamics/FuelPExt", "0 -1.2 0", "fbw/snd/fuel_pumps == 1", None),
    ("/Dynamics/Hydraulics", "0 -1.4 -2", "fbw/snd/hyd_pressurised == 1", None),
    ("/Dynamics/HydraulicsX", "0 -1.8 -2", "fbw/snd/hyd_pressurised == 1", None),
    ("/Dynamics/GearDO", "0 -1.4 2", "DELTA=fbw/snd/gear_doors_opening", None),
    ("/Dynamics/GearDC", "0 -1.4 2", "DELTA=fbw/snd/gear_doors_closing", None),
    ("/Cockpit/PacksR", "0 3.85 -31.3", "fbw/snd/packs_on == 1", None),
    ("/Cockpit/PacksU", "0 6.2 -18", "fbw/snd/packs_on == 1", None),
    ("/Dynamics/PacksEXT", "0 -2.2 -4", "fbw/snd/packs_on == 1", None),
    ("/Dynamics/TouchFX", "0 -4.2 -31", "sim/flightmodel2/gear/on_ground[0] == 1", None),
    ("/Dynamics/TouchLX", "-3 -4.2 2", "sim/flightmodel2/gear/on_ground[1] == 1", None),
    ("/Dynamics/TouchRX", "3 -4.2 2", "sim/flightmodel2/gear/on_ground[2] == 1", None),
    ("/Dynamics/GRollT", "0 -1.4 -25", "sim/flightmodel2/gear/on_ground[0] == 1", None),
    ("/Cockpit/Sel3", "0 2.5 -32.4", "DELTA=fbw/snd/click_sel3", None),
]


def events(guids):
    return set(re.findall(r"event:(/\S+)", guids))


def present(snd):
    return set(re.findall(r"^\s*EVENT_NAME\s+(\S+)", snd, re.M))


def block(name, xyz, start, end):
    lines = ["BEGIN_SOUND_ATTACHMENT", f"\tEVENT_NAME {name}", f"\tVEH_XYZ {xyz}", f"\tEVENT_START_COND {start}"]
    if end:
        lines.append(f"\tEVENT_END_COND {end}")
    lines.append("END_SOUND_ATTACHMENT")
    return lines


def patch(snd_path, known):
    raw = snd_path.read_text(encoding="utf-8")
    nl = "\r\n" if "\r\n" in raw else "\n"
    have = present(raw)
    add = [e for e in EXTRA if e[0] in known and e[0] not in have]
    if not add:
        return []
    lines = ["", "# Systems and exterior sounds the pack has but the conversion did not play ###########", ""]
    for e in add:
        lines += block(*e) + [""]
    snd_path.write_text(raw.rstrip("\r\n") + nl + nl.join(lines) + nl, encoding="utf-8", newline="")
    return [e[0] for e in add]


def main(dirs):
    for d in map(Path, dirs):
        guids = d / "GUIDs.txt"
        if not guids.exists():
            print(f"{d}: no GUIDs.txt")
            continue
        known = events(guids.read_text(encoding="utf-8", errors="replace"))
        for snd in sorted(d.glob("A380*.snd")):
            added = patch(snd, known)
            print(f"{snd}: added {len(added)} {' '.join(added)}")


if __name__ == "__main__":
    main(sys.argv[1:])
