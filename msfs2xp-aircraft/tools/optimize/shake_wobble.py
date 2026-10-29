import json
import re
import shutil
import sys
from pathlib import Path

HIDE = re.compile(r"(\s*)ANIM_hide 5\.5 6\.5 sim/operation/failures/rel_engsep(\d)\s*$")
SHAKE = ["ANIM_trans -1 0 0 1 0 0 -1 1 fbw/anim/buffet_x", "ANIM_trans 0 -1 0 0 1 0 -1 1 fbw/anim/buffet_y", "ANIM_trans 0 0 -1 0 0 1 -1 1 fbw/anim/buffet_z"]


def internal_objects(acf):
    raw = acf.read_text(encoding="utf-8", errors="replace")
    objs = {}
    for m in re.finditer(r"P _obja/(\d+)/(_v10_att_file_stl|_v10_is_internal) (\S+)", raw):
        objs.setdefault(m.group(1), {})[m.group(2)] = m.group(3)
    return sorted({o["_v10_att_file_stl"] for o in objs.values() if o.get("_v10_is_internal") == "1" and "_v10_att_file_stl" in o})


def add_shake(lines):
    if any("fbw/anim/buffet_x" in l for l in lines):
        return None
    out, inside = [], False
    first_command = next((i for i, l in enumerate(lines) if l.startswith(("TRIS", "ANIM_", "ATTR_", "LIGHT", "EMITTER")) and not l.startswith("ATTR_layer_group")), None)
    if first_command is None:
        return None
    for i, l in enumerate(lines):
        if l.startswith("ATTR_LOD"):
            if inside:
                out.append("ANIM_end")
            out.append(l)
            out.append("ANIM_begin")
            out.extend(SHAKE)
            inside = True
            continue
        if i == first_command and not inside and not l.startswith("ATTR_LOD"):
            out.append("ANIM_begin")
            out.extend(SHAKE)
            inside = True
        out.append(l)
    while out and not out[-1].strip():
        out.pop()
    if inside:
        out.append("ANIM_end")
    return out


def add_wobble(lines, pivots):
    if any("fbw/anim/engine_wobble" in l for l in lines):
        return None
    out, changed = [], False
    for l in lines:
        out.append(l)
        m = HIDE.match(l)
        if not m:
            continue
        pad, e = m.group(1), int(m.group(2))
        if e not in pivots:
            continue
        x, y, z = pivots[e]
        n = e + 1
        out += [f"{pad}ANIM_trans {x:.3f} {y:.3f} {z:.3f} {x:.3f} {y:.3f} {z:.3f}",
                f"{pad}ANIM_rotate_begin 1 0 0 fbw/anim/engine_wobble/{n}/pitch",
                f"{pad}ANIM_rotate_key -5 -5", f"{pad}ANIM_rotate_key 5 5", f"{pad}ANIM_rotate_end",
                f"{pad}ANIM_rotate_begin 0 1 0 fbw/anim/engine_wobble/{n}/yaw",
                f"{pad}ANIM_rotate_key -5 -5", f"{pad}ANIM_rotate_key 5 5", f"{pad}ANIM_rotate_end",
                f"{pad}ANIM_trans {-x:.3f} {-y:.3f} {-z:.3f} {-x:.3f} {-y:.3f} {-z:.3f}"]
        changed = True
    return out if changed else None


def main(aircraft_dir, backup_dir):
    a = Path(aircraft_dir)
    objects = a / "objects"
    backup = Path(backup_dir)
    backup.mkdir(parents=True, exist_ok=True)
    layout = json.loads((objects / "a380_engdetach.json").read_text())
    pivots = {}
    for k, e in layout["engines"].items():
        c, h = e["centre"], e["half_extent"]
        pivots[int(k)] = (c[0], c[1] + h[1] * 0.8, c[2])
    shaken = wobbled = 0
    internal = set(internal_objects(a / "A380.acf"))
    targets = internal | {p.name for p in objects.glob("a380_exterior_*.obj")}
    for name in sorted(targets):
        path = objects / name
        if not path.exists():
            continue
        raw = path.read_bytes()
        nl = "\r\n" if b"\r\n" in raw else "\n"
        lines = raw.decode("utf-8", errors="replace").split(nl)
        result = None
        if name in internal:
            result = add_shake(lines)
            shaken += result is not None
        else:
            result = add_wobble(lines, pivots)
            wobbled += result is not None
        if result is None:
            continue
        if not (backup / name).exists():
            shutil.copy2(path, backup / name)
        path.write_bytes((nl.join(result) + nl).encode("utf-8"))
    print("cockpit objects shaken:", shaken, "engine objects wobbled:", wobbled)


if __name__ == "__main__":
    main(*sys.argv[1:])
