"""Learns how X-Plane 12 acf files map back to X-Plane 11 ones from aircraft that ship both versions.

Writes acf_map.json: part slots (XP12 _body/_wing index -> XP11 _part index), renamed keys and keys whose
values differ between the versions. Run: python -I acf_learn.py <xp11 root> [out.json]
"""
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

PAIRS = [
    ("ToLissA320_V1p2p1/a320.acf", "ToLissA320_V1p2p1/a320_XP11.acf"),
    ("ToLissA319_V1p8p1/a319.acf", "ToLissA319_V1p8p1/a319_XP11.acf"),
    ("ToLissA321_V1p8p0/a321.acf", "ToLissA321_V1p8p0/a321_XP11.acf"),
    ("ToLissA339_V1p1p0/A330-900.acf", "ToLissA339_V1p1p0/A330-900_XP11.acf"),
    ("FlightFactor777_200ER_300ER_F_EE/777-200ER_xp12.acf", "FlightFactor777_200ER_300ER_F_EE/777-200ER.acf"),
    ("747 Felis 1.2.11.1/B742_PW_Felis_XP12.acf", "747 Felis 1.2.11.1/B742_PW_Felis.acf"),
]


def read_acf(path):
    props = {}
    for line in Path(path).read_text(encoding="latin-1").splitlines():
        if line.startswith("P "):
            parts = line[2:].split(" ", 1)
            props[parts[0]] = parts[1] if len(parts) > 1 else ""
    return props


def num(v):
    try:
        return float(v)
    except ValueError:
        return None


def entities(props, array):
    out = defaultdict(dict)
    pat = re.compile(r"^" + array + r"/(\d+)/(.+)$")
    for k, v in props.items():
        m = pat.match(k)
        if m:
            out[int(m.group(1))][m.group(2)] = v
    return out


def geo_print(fields):
    vals = sorted((k, round(num(v) or 0.0, 3)) for k, v in fields.items() if k.startswith("_geo_xyz/"))
    nonzero = [x for x in vals if x[1] != 0.0]
    return tuple(nonzero[:60]) if nonzero else None


def pos(fields):
    return tuple(round(num(fields.get(k, "0")) or 0.0, 3) for k in ("_part_x", "_part_y", "_part_z"))


def wing_print(fields):
    return tuple(round(num(fields.get(k, "0")) or 0.0, 3) for k in ("_Croot", "_Ctip", "_semilen_SEG", "_dihed_design", "_sweep_design"))


def match_slots(p12, p11):
    bodies, wings12 = entities(p12, "_body"), entities(p12, "_wing")
    parts, wings11 = entities(p11, "_part"), entities(p11, "_wing")
    slots = {}
    for j, f in bodies.items():
        g = geo_print(f)
        if g is None:
            continue
        hits = [k for k, pf in parts.items() if geo_print(pf) == g and pos(pf) == pos(f)]
        if len(hits) == 1:
            slots[f"_body/{j}"] = hits[0]
    for i, f in wings12.items():
        w = wing_print(f)
        if w[0] == 0.0 and w[2] == 0.0:
            continue
        hits = [k for k, wf in wings11.items() if wing_print(wf) == w and pos(parts.get(k, {})) == pos(f)]
        if len(hits) == 1:
            slots[f"_wing/{i}"] = hits[0]
    return slots


def normalise(key, slots12=None):
    return re.sub(r"/(\d+)(?=/|$)", "/#", key)


def main():
    root = Path(sys.argv[1]) / "Aircraft" / "Laminar Research"
    out = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(__file__).with_name("acf_map.json")
    slot_votes = defaultdict(Counter)
    att_votes = defaultdict(Counter)
    same = defaultdict(lambda: [0, 0])
    rename_votes = defaultdict(Counter)
    only11_seen = Counter()
    pairs = 0
    for a12, a11 in PAIRS:
        f12, f11 = root / a12, root / a11
        if not (f12.exists() and f11.exists()):
            continue
        pairs += 1
        p12, p11 = read_acf(f12), read_acf(f11)
        slots = match_slots(p12, p11)
        for e, k in slots.items():
            slot_votes[e][k] += 1
        for arr in ("_obja", "_lite"):
            e12, e11 = entities(p12, arr), entities(p11, arr)
            for n, f in e12.items():
                g = e11.get(n)
                if not g or f.get("_v10_att_file_stl") != g.get("_v10_att_file_stl"):
                    continue
                part = g.get("_v10_att_part")
                for kind in ("_v10_att_body", "_v10_att_wing", "_v10_att_gear"):
                    idx = f.get(kind, "-1")
                    if idx not in ("-1", None):
                        att_votes[f"{kind}={idx}"][part] += 1
                if all(f.get(k, "-1") == "-1" for k in ("_v10_att_body", "_v10_att_wing", "_v10_att_gear")):
                    att_votes["none"][part] += 1
        k12 = {normalise(k): k for k in p12}
        k11 = {normalise(k): k for k in p11}
        for nk, rk in k11.items():
            if nk in k12:
                s = same[nk]
                s[1] += 1
                if p12[k12[nk]] == p11[rk]:
                    s[0] += 1
            else:
                only11_seen[nk] += 1
                v11 = p11[rk]
                if num(v11) in (None, 0.0):
                    continue
                for nk12, rk12 in k12.items():
                    if nk12 not in k11 and p12[rk12] == v11:
                        rename_votes[nk][nk12] += 1
    slots = {e: c.most_common(1)[0][0] for e, c in slot_votes.items() if c.most_common(1)[0][1] == sum(c.values())}
    conflicts = {e: dict(c) for e, c in slot_votes.items() if e not in slots}
    renames = {}
    for nk, c in rename_votes.items():
        best, n = c.most_common(1)[0]
        if n >= max(2, only11_seen[nk] * 0.6) and (len(c) == 1 or c.most_common(2)[1][1] < n):
            renames[nk] = best
    differ = sorted(k for k, (eq, tot) in same.items() if tot >= 2 and eq == 0)
    report = {
        "pairs": pairs,
        "slots": dict(sorted(slots.items(), key=lambda x: (x[0].split("/")[0], int(x[0].split("/")[1])))),
        "slot_conflicts": conflicts,
        "attachments": {k: dict(v) for k, v in sorted(att_votes.items())},
        "renames": dict(sorted(renames.items())),
        "always_differ": differ,
        "only11": sorted(only11_seen),
    }
    out.write_text(json.dumps(report, indent=1), encoding="utf-8")
    print(f"{pairs} pairs; {len(slots)} slots ({len(conflicts)} conflicts); {len(renames)} renames; {len(differ)} keys always differ; {len(only11_seen)} XP11-only keys -> {out}")


if __name__ == "__main__":
    main()
