"""What X-Plane actually draws of an aircraft for one view: per object, the triangles, draw batches and
shadow-casting triangles left after the acf hide datarefs, ATTR_LOD at a distance, and ANIM_hide/ANIM_show
under a dataref state. python -I visible_tris.py <acf> <view: ext|deck|cabin> [distance m] [key=value ...] [--json out]"""
import json
import re
import sys
from pathlib import Path

VIEWS = {
    "ext": {"sim/graphics/view/view_is_external": 1, "fbw/anim/eye_in_flight_deck": 0, "fbw/anim/cabin_hidden": 0},
    "deck": {"sim/graphics/view/view_is_external": 0, "fbw/anim/eye_in_flight_deck": 1, "fbw/anim/cabin_hidden": 1},
    "cabin": {"sim/graphics/view/view_is_external": 0, "fbw/anim/eye_in_flight_deck": 0, "fbw/anim/cabin_hidden": 0},
}
DIST = {"ext": 45.0, "deck": 30.0, "cabin": 10.0}
CMD = re.compile(rb"^\s*(ATTR_LOD|ANIM_begin|ANIM_end|ANIM_hide|ANIM_show|TRIS|ATTR_no_shadow|ATTR_shadow|ATTR_reset)\b(.*)$", re.M)


def objects_of(acf):
    props = {}
    for line in acf.read_text(errors="replace").splitlines():
        m = re.match(r"P _obja/(\d+)/(\S+) ?(.*)", line)
        if m:
            props.setdefault(int(m[1]), {})[m[2]] = m[3]
    out = []
    for n, p in sorted(props.items()):
        rel = p.get("_v10_att_file_stl")
        if rel:
            out.append({"n": n, "obj": rel, "hide": p.get("_obj_hide_dataref", ""), "flags": int(p.get("_obj_flags", "0") or 0)})
    return out


def value(state, ref):
    ref = ref.decode() if isinstance(ref, bytes) else ref
    if ref in state:
        return state[ref]
    m = re.match(r"(.*)\[(\d+)\]$", ref)
    if m and m[1] in state:
        v = state[m[1]]
        return v[int(m[2])] if isinstance(v, list) else v
    return 0.0


def walk(path, state, dist):
    data = path.read_bytes()
    lods = []
    stack = [True]
    lod_on = True
    shadow = True
    tris = batches = shadow_tris = 0
    hidden_tris = 0
    for m in CMD.finditer(data):
        cmd, rest = m[1], m[2].split()
        if cmd == b"ATTR_LOD":
            a, b = float(rest[0]), float(rest[1])
            lods.append((a, b))
            lod_on = a <= dist < b
            stack = [True]
        elif cmd == b"ANIM_begin":
            stack.append(stack[-1])
        elif cmd == b"ANIM_end":
            if len(stack) > 1:
                stack.pop()
        elif cmd in (b"ANIM_hide", b"ANIM_show") and len(rest) >= 3:
            lo, hi = float(rest[0]), float(rest[1])
            v = value(state, rest[2])
            if lo <= v <= hi:
                stack[-1] = cmd == b"ANIM_show" and (len(stack) < 2 or stack[-2])
        elif cmd == b"ATTR_no_shadow":
            shadow = False
        elif cmd in (b"ATTR_shadow", b"ATTR_reset"):
            shadow = True
        elif cmd == b"TRIS":
            t = int(rest[1]) // 3
            if lod_on and stack[-1]:
                tris += t
                batches += 1
                if shadow:
                    shadow_tris += t
            elif lod_on:
                hidden_tris += t
    return {"tris": tris, "batches": batches, "shadow_tris": shadow_tris, "hidden_tris": hidden_tris, "lods": lods}


def main():
    a = [x for x in sys.argv[1:]]
    out = a[a.index("--json") + 1] if "--json" in a else None
    a = [x for i, x in enumerate(a) if x != "--json" and (i == 0 or a[i - 1] != "--json")]
    acf, view = Path(a[0]), a[1]
    dist = float(a[2]) if len(a) > 2 and "=" not in a[2] else DIST[view]
    state = dict(VIEWS[view])
    for kv in a[2:]:
        if "=" in kv:
            k, v = kv.split("=", 1)
            state[k] = float(v)
    rows = []
    for o in objects_of(acf):
        p = acf.parent / "objects" / o["obj"]
        if not p.is_file():
            continue
        if o["hide"] and value(state, o["hide"]) != 0:
            r = {"tris": 0, "batches": 0, "shadow_tris": 0, "hidden_tris": 0, "lods": [], "acf_hidden": True}
        else:
            r = walk(p, state, dist)
        r.update(o)
        rows.append(r)
    rows.sort(key=lambda r: -r["tris"])
    t = sum(r["tris"] for r in rows)
    b = sum(r["batches"] for r in rows)
    s = sum(r["shadow_tris"] for r in rows if r["flags"] & 0x18)
    print(f"view {view} at {dist:.0f} m: {t/1e6:.2f}M tris, {b} batches, shadow-casting objects' tris {s/1e6:.2f}M")
    for r in rows[:40]:
        if r["tris"] == 0:
            break
        print(f"  {r['obj'][:40]:40} {r['tris']:>8} tris {r['batches']:>4} b  shadow {r['shadow_tris']:>8}  flags {r['flags']:>5}  lods {r['lods'][:3]}")
    if out:
        Path(out).write_text(json.dumps(rows, indent=1))


if __name__ == "__main__":
    main()
