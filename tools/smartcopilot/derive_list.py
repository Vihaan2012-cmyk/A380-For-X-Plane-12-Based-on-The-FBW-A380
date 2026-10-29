"""Derive the SmartCopilot sync list (sync_list.json) for the A380.

Inputs: the cockpit OBJs (their manipulators) and the native displays' read
sets. Output: one JSON object that gen_cfg.py turns into smartcopilot.cfg.

    python tools/smartcopilot/derive_list.py --objects "<aircraft>/objects" \
        --read-sets src/native_displays/read_sets.txt \
        --out tools/smartcopilot/sync_list.json
"""

import argparse
import json
import re
import sys
from pathlib import Path

DATAREF_MANIPS = {
    "toggle": 3,
    "push": 3,
    "drag_axis": 6,
    "axis_knob": 5,
    "axis_switch_up_down": 5,
    "axis_switch_left_right": 5,
}
COMMAND_MANIPS = {
    "command": [1],
    "command_knob": [1, 2],
    "command_switch_up_down": [1, 2],
    "command_switch_left_right": [1, 2],
}
DISCRETE_UNITS = {"bool", "boolean", "enum"}
FUNCTIONAL_SUFFIXES = ("_pp", "_hold")

SEND_BACK = [
    "sim/joystick/yoke_pitch_ratio",
    "sim/joystick/yoke_roll_ratio",
    "sim/joystick/yoke_heading_ratio",
    "sim/cockpit2/controls/left_brake_ratio",
    "sim/cockpit2/controls/right_brake_ratio",
    "fbw/cockpit/yoke_lever_stick1_l_r",
    "fbw/cockpit/yoke_lever_stick_fore1_aft",
] + [f"sim/cockpit2/engine/actuators/throttle_ratio[{i}]" for i in range(4)]

OVERRIDE = {
    "sim/operation/override/override_joystick": 10,
    "sim/operation/override/override_throttles": 10,
}

SCP_CHANNELS_TRIGGERS = [
    "fbw/scp/in_seq",
    "fbw/scp/in_event",
    "fbw/scp/fms_seq",
    "fbw/scp/fms_blob",
]

SLOW = [
    "sim/time/zulu_time_sec",
    "sim/time/local_date_days",
    "sim/flightmodel/weight/m_fixed",
    "sim/flightmodel/weight/m_fuel_total",
]

SETUP = {
    "SLOW_INTERVAL": "5.0",
    "USE_AUTOPILOT_SEQUENCER": "0",
    "ENHANCED_SLAVE_SMOOTHNESS": "1",
}

INFO = {"ID": "A380_XP12", "VERSION": "0.1", "AUTHOR": "A380 for X-Plane 12"}


def sanitise(name):
    return re.sub(r"[^A-Za-z0-9_/]", "_", name)


def var_dataref(name):
    if name.startswith(("L:", "A:")):
        name = name[2:]
    return "fbw/" + sanitise(name)


def scan_objects(objects):
    datarefs, commands, functional = set(), set(), set()
    for obj in sorted(Path(objects).glob("*.obj")):
        with open(obj, encoding="utf-8", errors="replace") as f:
            for line in f:
                if not line.startswith("ATTR_manip_"):
                    continue
                parts = line.split()
                kind = parts[0][len("ATTR_manip_"):]
                if kind in DATAREF_MANIPS:
                    i = DATAREF_MANIPS[kind] + 1
                    if len(parts) > i:
                        d = parts[i]
                        (functional if d.endswith(FUNCTIONAL_SUFFIXES) else datarefs).add(d)
                elif kind in COMMAND_MANIPS:
                    for i in COMMAND_MANIPS[kind]:
                        if len(parts) > i + 1:
                            commands.add(parts[i + 1])
    return datarefs, commands, functional


def scan_read_sets(path):
    discrete, continuous, strings = set(), set(), set()
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        cols = line.split("\t")
        if len(cols) < 3:
            continue
        if cols[1] == "var":
            unit = cols[3].strip().lower() if len(cols) > 3 else ""
            (discrete if unit in DISCRETE_UNITS else continuous).add(var_dataref(cols[2]))
        elif cols[1] == "str":
            strings.add(cols[2])
    return discrete, continuous - discrete, strings


def build(objects, read_sets):
    manip_refs, commands, functional = scan_objects(objects)
    discrete, continuous, strings = scan_read_sets(read_sets)
    send_back = set(SEND_BACK)
    triggers = (manip_refs | discrete | set(SCP_CHANNELS_TRIGGERS)) - send_back
    continued = continuous - triggers - send_back
    entries = []

    def add(section, names, value=0, source=""):
        for n in sorted(names):
            entries.append({"section": section, "name": n, "value": value, "source": source})

    add("TRIGGERS", manip_refs - send_back, source="manipulator")
    add("TRIGGERS", (discrete - manip_refs) - send_back, source="display read set (discrete)")
    add("TRIGGERS", SCP_CHANNELS_TRIGGERS, source="plugin scp channel (to add)")
    add("CONTINUED", continued, source="display read set (continuous)")
    add("SEND_BACK", SEND_BACK, source="flight controls")
    add("COMMANDS", commands, source="manipulator")
    for n, v in OVERRIDE.items():
        entries.append({"section": "OVERRIDE", "name": n, "value": v, "source": "control hand-over"})
    add("SLOW", SLOW, source="time and weights")
    return {
        "info": INFO,
        "setup": SETUP,
        "entries": entries,
        "not_syncable": {
            "functional_inputs": sorted(functional),
            "string_vars": sorted(strings),
        },
        "counts": {
            "TRIGGERS": len(triggers),
            "CONTINUED": len(continued),
            "SEND_BACK": len(SEND_BACK),
            "COMMANDS": len(commands),
            "functional_inputs": len(functional),
            "string_vars": len(strings),
        },
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--objects", required=True)
    ap.add_argument("--read-sets", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    data = build(a.objects, a.read_sets)
    Path(a.out).write_text(json.dumps(data, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(data["counts"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
