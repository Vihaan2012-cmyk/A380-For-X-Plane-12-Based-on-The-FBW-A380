"""usage: make_checklist_conditions.py <checklists.json5>

Writes checklist_conditions.js next to this script: FlyByWire's checklist
item conditions (the flyPad's autofill) keyed by checklist name and item."""

import json
import re
import sys
from pathlib import Path


def json5_to_json(text):
    out, i, in_str, quote = [], 0, False, ""
    while i < len(text):
        ch = text[i]
        if in_str:
            out.append(ch)
            if ch == "\\":
                out.append(text[i + 1])
                i += 2
                continue
            if ch == quote:
                in_str = False
            i += 1
            continue
        if ch in "\"'":
            in_str, quote = True, ch
            out.append(ch)
            i += 1
            continue
        if text.startswith("//", i):
            i = text.find("\n", i)
            i = len(text) if i < 0 else i
            continue
        out.append(ch)
        i += 1
    s = "".join(out)
    s = re.sub(r"'([^'\\]*)'", lambda m: json.dumps(m.group(1)), s)
    s = re.sub(r"([{,]\s*)([A-Za-z_][A-Za-z0-9_]*)\s*:", r'\1"\2":', s)
    s = re.sub(r",(\s*[}\]])", r"\1", s)
    return json.loads(s)


EFB_TITLES = {"SECURING AIRCRAFT": "SECURING THE AIRCRAFT"}
EFB_ITEMS = {"AUTO BRAKE": "AUTO BRK", "EMER EXIT LT": "EMER EXIT LIGHT"}

WALKAROUND_FITTED = ["PITOT_COVER_1", "PITOT_COVER_2", "PITOT_COVER_3", "STATIC_COVERS", "AOA_COVER_1", "AOA_COVER_2", "AOA_COVER_3"] + [
    f"ENG_{kind}_COVER_{n}" for kind in ("INLET", "EXHAUST") for n in range(1, 5)
] + ["GEAR_PIN_NOSE", "GEAR_PIN_LWING", "GEAR_PIN_RWING", "GEAR_PIN_LBODY", "GEAR_PIN_RBODY"]


def cond(var, result, comp="EQ"):
    return {"v": var, "r": result, "c": comp}


EXTRA = {
    "COCKPIT PREPARATION": {"GEAR PINS & COVERS": [cond(f"L:A32NX_WALKAROUND_{w}", 0) for w in WALKAROUND_FITTED]},
    "TAXI": {"FLAPS": [cond("L:A32NX_FLAPS_HANDLE_INDEX", 1, "GE"), cond("L:A32NX_FLAPS_HANDLE_INDEX", 3, "LE")]},
    "LINE-UP": {"PACK 1 & 2": [cond("L:A32NX_OVHD_COND_PACK_1_PB_IS_ON", 1), cond("L:A32NX_OVHD_COND_PACK_2_PB_IS_ON", 1)]},
    "LANDING": {"FLAPS": [cond("L:A32NX_FLAPS_HANDLE_INDEX", 3, "GE")]},
    "PARKING": {
        "PARKING BRAKE OR CHOCKS": [cond("L:A32NX_PARK_BRAKE_LEVER_POS", 1)],
        "ENGINES": [cond(f"L:A32NX_ENGINE_STATE:{n}", 0) for n in range(1, 5)],
    },
}


def main(src):
    data = json5_to_json(Path(src).read_text(encoding="utf-8"))
    table = {}
    for cl in data["checklists"]:
        items = {}
        for it in cl.get("items", []):
            cond = it.get("condition")
            if not cond:
                continue
            items[EFB_ITEMS.get(it["item"], it["item"])] = [{"v": c["varName"], "r": c["result"], "c": c.get("comp", "EQ")} for c in cond]
        if items:
            table[EFB_TITLES.get(cl["name"], cl["name"])] = items
    for checklist, items in EXTRA.items():
        for item, conds in items.items():
            table.setdefault(checklist, {}).setdefault(item, conds)
    out = Path(__file__).with_name("checklist_conditions.js")
    out.write_text("const CL_CONDITIONS = " + json.dumps(table, indent=1) + ";\n", encoding="utf-8")
    print(f"{sum(len(v) for v in table.values())} conditions in {len(table)} checklists -> {out}")


if __name__ == "__main__":
    main(sys.argv[1])
