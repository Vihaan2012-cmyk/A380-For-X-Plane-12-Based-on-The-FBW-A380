import json
import re
import sys
from pathlib import Path

HIDE = re.compile(r"ANIM_hide 5\.5 6\.5 sim/operation/failures/rel_engsep(\d)")
HEADER_KEEP = ("TEXTURE", "GLOBAL_", "NORMAL_METALNESS", "BLEND_GLASS")
CONSTANT_EMITTERS = ("Fire_Emitter", "Smoke_Back_Emitter")
FAMILIES = {
    "ATTR_blend": "blend", "ATTR_no_blend": "blend", "ATTR_shadow_blend": "blend",
    "ATTR_shadow": "shadow", "ATTR_no_shadow": "shadow",
    "ATTR_cull": "cull", "ATTR_no_cull": "cull",
    "ATTR_poly_os": "poly_os",
    "ATTR_shiny_rat": "shiny",
    "ATTR_light_level": "light", "ATTR_light_level_reset": "light",
    "ATTR_draw_disable": "draw", "ATTR_draw_enable": "draw",
}


def read(path):
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    vt, idx = [], []
    for l in lines:
        if l.startswith("VT"):
            vt.append(l.split()[1:9])
        elif l.startswith(("IDX10", "IDX ")):
            idx.extend(int(v) for v in l.split()[1:])
    return lines, vt, idx


def engine_tris(lines):
    found = {}
    stack = []
    state = {}
    for l in lines:
        t = l.strip()
        head = t.split(" ", 1)[0]
        if head == "ATTR_reset":
            state = {}
        elif head in FAMILIES:
            state[FAMILIES[head]] = t
        if t == "ANIM_begin":
            stack.append(None)
        elif t == "ANIM_end":
            if stack:
                stack.pop()
        else:
            m = HIDE.match(t)
            if m and stack:
                stack[-1] = int(m.group(1))
            elif t.startswith("TRIS "):
                engine = next((e for e in reversed(stack) if e is not None), None)
                if engine is not None:
                    s, c = map(int, t.split()[1:3])
                    found.setdefault(engine, []).append((s, c, tuple(sorted(state.values()))))
    return found


def write_detached(out, header, vt, idx, ranges, centre):
    used = sorted({i for s, c, _ in ranges for i in idx[s:s + c]})
    remap = {old: new for new, old in enumerate(used)}
    tris = [remap[i] for s, c, _ in ranges for i in idx[s:s + c]]
    rows = []
    for old in used:
        v = [float(x) for x in vt[old]]
        v[0] -= centre[0]
        v[1] -= centre[1]
        v[2] -= centre[2]
        rows.append("VT " + " ".join(f"{x:.5f}" for x in v))
    body = ["I", "800", "OBJ", ""] + header + [f"POINT_COUNTS {len(used)} 0 0 {len(tris)}", ""] + rows
    for k in range(0, len(tris) - len(tris) % 10, 10):
        body.append("IDX10 " + " ".join(map(str, tris[k:k + 10])))
    for k in range(len(tris) - len(tris) % 10, len(tris)):
        body.append(f"IDX {tris[k]}")
    body += ["", "ATTR_LOD 0 4000"]
    offset = 0
    for s, c, attrs in ranges:
        body += ["ATTR_reset"] + list(attrs) + [f"TRIS {offset} {c}"]
        offset += c
    body.append("")
    out.write_text("\n".join(body), encoding="utf-8")


def constant_rate_pss(src, dst, texture):
    out = [f" TEXTURE {texture}" if l.strip().startswith("TEXTURE ") else l
           for l in src.read_text(encoding="utf-8", errors="replace").splitlines()]
    emitter = None
    for i, l in enumerate(out):
        s = l.strip()
        if s.startswith("NAME "):
            emitter = s[5:]
            if i > 0 and out[i - 1].strip() == "EMITTER":
                emitter = emitter.replace(" ", "_")
                out[i] = f" NAME {emitter}"
        elif s == "EMIT_RATE" and emitter in CONSTANT_EMITTERS:
            full = out[i + 3].split()
            out[i + 2] = f"\t0.000000\t{full[1]}\t{full[2]}"
    dst.write_text("\n".join(out), encoding="utf-8")


def fire_object(out, pss_name):
    body = ["I", "800", "OBJ", "", f"PARTICLE_SYSTEM {pss_name}", "POINT_COUNTS 0 0 0 0", "",
            "EMITTER Fire_Emitter 0 0.2 0 0 0 0",
            "EMITTER Fire_Emitter 0.9 0.2 1.2 0 0 0",
            "EMITTER Fire_Emitter -0.8 0.2 -1.0 0 0 0",
            "EMITTER Smoke_Back_Emitter 0 1.0 0 0 0 0", ""]
    out.write_text("\n".join(body), encoding="utf-8")


def main(objects_dir, explosion_pss):
    d = Path(objects_dir)
    per_engine = {}
    for path in sorted(d.glob("*.obj")):
        if path.name.startswith("a380_engdetach_") or path.stem.endswith("_gp"):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        if "rel_engsep" not in text:
            continue
        lines, vt, idx = read(path)
        header = [l for l in lines[:60] if l.startswith(HEADER_KEEP)]
        for engine, ranges in engine_tris(lines).items():
            per_engine.setdefault(engine, []).append((path, header, vt, idx, ranges))
    manifest = {"engines": {}}
    for engine, parts in sorted(per_engine.items()):
        pts = [[float(x) for x in vt[i][:3]] for _, _, vt, idx, ranges in parts for s, c, _ in ranges for i in idx[s:s + c]]
        centre = [sum(p[a] for p in pts) / len(pts) for a in range(3)]
        lo = [min(p[a] for p in pts) for a in range(3)]
        hi = [max(p[a] for p in pts) for a in range(3)]
        names = []
        for path, header, vt, idx, ranges in parts:
            name = f"a380_engdetach_{engine}_{path.stem}.obj"
            write_detached(d / name, header, vt, idx, ranges, centre)
            names.append(name)
        manifest["engines"][str(engine)] = {"centre": [round(c, 3) for c in centre],
                                             "half_extent": [round((h - l) / 2, 3) for l, h in zip(lo, hi)],
                                             "bottom": round(lo[1] - centre[1], 3),
                                             "objects": names}
        gp_parts = []
        for path, header, _, _, _ in parts:
            gp_path = path.with_name(path.stem + "_gp.obj")
            if not gp_path.is_file():
                gp_path = path
            lines, vt, idx = read(gp_path)
            gp_parts.append((gp_path, header, vt, idx, engine_tris(lines)[engine]))
        gp_pts = [[float(x) for x in vt[i][:3]] for _, _, vt, idx, ranges in gp_parts for s, c, _ in ranges for i in idx[s:s + c]]
        lo = [min(p[a] for p in gp_pts) for a in range(3)]
        hi = [max(p[a] for p in gp_pts) for a in range(3)]
        gp_names = []
        for path, header, vt, idx, ranges in gp_parts:
            name = f"a380_engdetach_{engine}_{path.stem}.obj"
            if path.stem.endswith("_gp"):
                write_detached(d / name, header, vt, idx, ranges, centre)
            gp_names.append(name)
        manifest.setdefault("gp", {"engines": {}})["engines"][str(engine)] = {
            "centre": [round(c, 3) for c in centre],
            "half_extent": [round((h - l) / 2, 3) for l, h in zip(lo, hi)],
            "bottom": round(lo[1] - centre[1], 3),
            "objects": gp_names}
    constant_rate_pss(Path(explosion_pss), d / "a380_engdetach_fire.pss", "../../../Resources/effects/aircraft_explosion.png")
    fire_object(d / "a380_engdetach_fire.obj", "a380_engdetach_fire.pss")
    manifest["fire"] = "a380_engdetach_fire.obj"
    (d / "a380_engdetach.json").write_text(json.dumps(manifest, indent=1), encoding="utf-8")
    for e, m in manifest["engines"].items():
        print("engine", int(e) + 1, "centre", m["centre"], "half", m["half_extent"], len(m["objects"]), "objects")


if __name__ == "__main__":
    main(*sys.argv[1:])
