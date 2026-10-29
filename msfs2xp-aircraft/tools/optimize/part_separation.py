"""usage: part_separation.py <objects dir>

Falling pieces for every separable surface: each triangle batch gated by a
separation dataref becomes part of one part's piece objects, written
relative to that part's centre, plus a32-style composites (stabiliser half,
fin, outer wing) that carry their sub-surfaces as separate objects so the
plugin drops only the ones that have not fallen already. Writes
a380_partdetach_*.obj and a380_partdetach.json."""

import json
import re
import sys
from pathlib import Path

import numpy as np

from engine_detach import FAMILIES, HEADER_KEEP, read, write_detached

SEP = "fbw/fail/sep/"
NATIVE = {"sim/operation/failures/rel_hstbL": "hstab_l", "sim/operation/failures/rel_hstbR": "hstab_r",
          "sim/operation/failures/rel_vstb1": "fin", "sim/operation/failures/rel_wing4L": "wingtip_l",
          "sim/operation/failures/rel_wing4R": "wingtip_r"}
NATIVE.update({f"sim/operation/failures/rel_wing{k}{s}": f"wing{k}_{s.lower()}" for k in (1, 2, 3) for s in "LR"})
COMPOSITES = {name: name for name in NATIVE.values()}
WING_SECTIONS = ["wing1", "wing2", "wing3", "wingtip"]
CARRY = {f"{w}_{s}": [f"{o}_{s}" for o in WING_SECTIONS[k + 1:]] for k, w in enumerate(WING_SECTIONS[:-1]) for s in "lr"}
HIDE = re.compile(r"ANIM_hide\s+\S+\s+\S+\s+(\S+)")
SKIP = re.compile(r"^a380_(engdetach|cowldetach|partdetach|sep_scars)|_(gp|hail)\.obj$")


def part_keys(drefs):
    surfaces, composite = [], None
    for d in drefs:
        if d.startswith(SEP):
            name = d[len(SEP):]
            if name in COMPOSITES:
                composite = name
            elif not name.startswith("cowl_"):
                surfaces.append(name)
        elif d in NATIVE:
            composite = composite or NATIVE[d]
    return (surfaces[0] if surfaces else None), composite


def batches(lines):
    stack, state, out = [], {}, []
    for line in lines:
        t = line.strip()
        head = t.split(" ", 1)[0]
        if head == "ATTR_reset":
            state = {}
        elif head in FAMILIES:
            state[FAMILIES[head]] = t
        if t == "ANIM_begin":
            stack.append([])
        elif t == "ANIM_end":
            if stack:
                stack.pop()
        elif t.startswith("ANIM_hide") and stack:
            m = HIDE.match(t)
            if m:
                stack[-1].append(m.group(1))
        elif t.startswith("TRIS "):
            s, c = map(int, t.split()[1:3])
            drefs = [d for frame in stack for d in frame]
            if drefs:
                out.append((s, c, tuple(sorted(state.values())), drefs))
    return out


def main(objects_dir):
    d = Path(objects_dir)
    parts = {}
    for path in sorted(d.glob("a380_exterior_*.obj")):
        if SKIP.search(path.name):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        if SEP not in text and not any(k in text for k in NATIVE):
            continue
        lines, vt, idx = read(path)
        header = [l for l in lines[:60] if l.startswith(HEADER_KEEP)]
        for s, c, attrs, drefs in batches(lines):
            surface, composite = part_keys(drefs)
            if surface:
                parts.setdefault(surface, {}).setdefault(path, (header, vt, idx, []))[3].append((s, c, attrs))
            if composite:
                slot = f"{composite}/{surface}" if surface else composite
                parts.setdefault(slot, {}).setdefault(path, (header, vt, idx, []))[3].append((s, c, attrs))
    manifest = {}
    centres = {}
    for name, by_file in sorted(parts.items()):
        owner = name.split("/")[0]
        if owner not in centres:
            own = parts.get(owner, {})
            pts = [[float(x) for x in vt[i][:3]] for _, (_, vt, idx, ranges) in own.items() for s, c, _ in ranges for i in idx[s:s + c]]
            if not pts:
                pts = [[float(x) for x in vt[i][:3]] for key, files in parts.items() if key.split("/")[0] == owner for _, (_, vt, idx, ranges) in files.items() for s, c, _ in ranges for i in idx[s:s + c]]
            centres[owner] = [sum(p[a] for p in pts) / len(pts) for a in range(3)]
        centre = centres[owner]
        pts = np.array([[float(x) for x in vt[i][:3]] for _, (_, vt, idx, ranges) in by_file.items() for s, c, _ in ranges for i in idx[s:s + c]])
        names = []
        for path, (header, vt, idx, ranges) in by_file.items():
            out = f"a380_partdetach_{name.replace('/', '__')}_{path.stem}.obj"
            write_detached(d / out, header, vt, idx, ranges, centre)
            names.append(out)
        entry = manifest.setdefault(owner, {"centre": [round(c, 3) for c in centre], "objects": [], "sub": {}, "lo": [1e9] * 3, "hi": [-1e9] * 3})
        if "/" in name:
            entry["sub"][name.split("/")[1]] = names
        else:
            entry["objects"] += names
        entry["lo"] = [min(a, b) for a, b in zip(entry["lo"], pts.min(axis=0))]
        entry["hi"] = [max(a, b) for a, b in zip(entry["hi"], pts.max(axis=0))]
    for name, e in manifest.items():
        carry = [c for c in CARRY.get(name, []) if c in manifest]
        if carry:
            e["carry"] = carry
        lo, hi = e.pop("lo"), e.pop("hi")
        e["half_extent"] = [round((h - l) / 2, 3) for l, h in zip(lo, hi)]
        e["bottom"] = round(lo[1] - e["centre"][1], 3)
        print(f"{name:20} {len(e['objects'])} objects, sub-parts {sorted(e['sub'])}, half extent {e['half_extent']}")
    (d / "a380_partdetach.json").write_text(json.dumps({"parts": manifest}, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main(sys.argv[1])
