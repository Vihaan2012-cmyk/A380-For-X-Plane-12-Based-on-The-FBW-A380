"""X-Plane 12 OBJ -> X-Plane 11 OBJ.

Material maps become XP11's single normal map, cockpit devices go (XP11 has none), XP12's airplane lights
become XP11's named and parameterised ones, and the screen glass moves into the cockpit object, mapped
onto the 3D panel texture with ATTR_cockpit at the place the panel atlas gives each screen.
"""
import math
import re

GEOMETRY = ("VT", "IDX", "IDX10", "VLINE", "VLIGHT")
SPECULAR_WITHOUT_MATERIAL = 0.5


def split(text):
    """(header lines, VT lines, flat index list, command lines)."""
    lines = text.splitlines()
    last_geo = -1
    first_geo = None
    for i, line in enumerate(lines):
        tok = line.split(None, 1)[0] if line.strip() else ""
        if tok in GEOMETRY:
            last_geo = i
            if first_geo is None:
                first_geo = i
    if first_geo is None:
        head_end = next((i + 1 for i, l in enumerate(lines) if l.startswith("POINT_COUNTS")), len(lines))
        return lines[:head_end], [], [], lines[head_end:]
    header = lines[:first_geo]
    vts, idx = [], []
    for line in lines[first_geo:last_geo + 1]:
        tok = line.split()
        if not tok:
            continue
        if tok[0] == "VT":
            vts.append(line)
        elif tok[0] in ("IDX", "IDX10"):
            idx.extend(int(t) for t in tok[1:])
    return header, vts, idx, lines[last_geo + 1:]


def join(header, vts, idx, cmds):
    out = []
    for line in header:
        if line.startswith("POINT_COUNTS"):
            line = f"POINT_COUNTS {len(vts)} 0 0 {len(idx)}"
        out.append(line)
    out.extend(vts)
    for i in range(0, len(idx) - len(idx) % 10, 10):
        out.append("IDX10 " + " ".join(str(v) for v in idx[i:i + 10]))
    for v in idx[len(idx) - len(idx) % 10:]:
        out.append(f"IDX {v}")
    out.extend(cmds)
    return "\n".join(out) + "\n"


def _cd(token):
    return float(token[:-2]) if token.endswith("cd") else float(token)


def _aim(dx, dy, dz):
    """ANIM_rotate angles turning XP11's fixed light direction (0, 0, -1) to (dx, dy, dz)."""
    n = math.sqrt(dx * dx + dy * dy + dz * dz) or 1.0
    dx, dy, dz = dx / n, dy / n, dz / n
    yaw = math.degrees(math.atan2(-dx, -dz))
    pitch = math.degrees(math.asin(max(-1.0, min(1.0, dy))))
    return yaw, pitch


SPILL_SIZE = lambda cd: max(4.0, min(120.0, math.sqrt(cd) / 12.0))
BILLBOARD_SIZE = lambda cd: max(0.3, min(2.5, cd ** 0.25 / 20.0))


def light(line):
    """One XP12 LIGHT_PARAM airplane_*_bb / _pm line -> XP11 lines, or None to keep it."""
    tok = line.split()
    if len(tok) < 2 or tok[0] != "LIGHT_PARAM":
        return None
    m = re.match(r"^airplane_(landing|taxi|generic|nav|strobe|beacon)_(bb|pm)$", tok[1])
    if not m or len(tok) < 14:
        return None
    kind, style = m.groups()
    indent = line[: len(line) - len(line.lstrip())]
    x, y, z, r, g, b = (float(t) for t in tok[2:8])
    idx = tok[8]
    cd = _cd(tok[9])
    dx, dy, dz, w = (float(t) for t in tok[10:14])
    p = lambda *v: " ".join(f"{a:.6g}" if isinstance(a, float) else str(a) for a in v)
    if kind == "nav":
        if style == "bb":
            side = "left" if r > 0.8 and g < 0.3 else "right" if g > 0.5 and r < 0.3 else "tail"
            return [f"{indent}LIGHT_NAMED airplane_nav_{side} {p(x, y, z)}"]
        return [f"{indent}LIGHT_PARAM airplane_nav_sp {p(x, y, z, r, g, b, idx, SPILL_SIZE(cd), dx, dy, dz, w)}"]
    if kind == "strobe":
        if style == "bb":
            return [f"{indent}LIGHT_PARAM airplane_strobe_dir {p(x, y, z, r, g, b, idx, BILLBOARD_SIZE(cd), dx, dy, dz, w)}"]
        return [f"{indent}LIGHT_PARAM airplane_strobe_sp {p(x, y, z, r, g, b, idx, SPILL_SIZE(cd), dx, dy, dz, w)}"]
    if kind == "beacon":
        if style == "bb":
            return [f"{indent}LIGHT_PARAM airplane_beacon_strobe {p(x, y, z, BILLBOARD_SIZE(cd), idx)}"]
        return [f"{indent}LIGHT_PARAM airplane_beacon_strobe_sp {p(x, y, z, SPILL_SIZE(cd), idx)}"]
    yaw, pitch = _aim(dx, dy, dz)
    out = [f"{indent}ANIM_begin", f"{indent}ANIM_trans {p(x, y, z, x, y, z)}", f"{indent}ANIM_rotate 0 1 0 {p(yaw, yaw)}",
           f"{indent}ANIM_rotate 1 0 0 {p(pitch, pitch)}"]
    if style == "bb":
        size = BILLBOARD_SIZE(cd)
        out.append(f"{indent}LIGHT_PARAM airplane_{kind}_core {p(0.0, 0.0, 0.0, r, g, b, idx, size)}")
        out.append(f"{indent}LIGHT_PARAM airplane_{kind}_glow {p(0.0, 0.0, 0.0, r, g, b, idx, size * 2)}")
    else:
        out.append(f"{indent}LIGHT_PARAM airplane_{kind}_sp {p(0.0, 0.0, 0.0, r, g, b, idx, SPILL_SIZE(cd), w)}")
    out.append(f"{indent}ANIM_end")
    return out


def material_maps(text):
    """(normal, material_gloss) of an object's XP12 TEXTURE_MAP lines, None for each absent."""
    found = {"normal": None, "material_gloss": None}
    for line in text.splitlines():
        tok = line.split()
        if len(tok) >= 3 and tok[0] == "TEXTURE_MAP" and tok[1] in found and found[tok[1]] is None:
            found[tok[1]] = " ".join(tok[2:])
    return found["normal"], found["material_gloss"]


def generic(text, combined=None):
    """Any object: (XP11 text, changed). `combined(normal, material)` names the XP11 normal map packing
    both (normal in RG, metalness in B, gloss in A, read with NORMAL_METALNESS); without it the material
    map is dropped."""
    lines = text.splitlines()
    has_normal = any(l.split(None, 1)[0] == "TEXTURE_NORMAL" for l in lines if l.strip())
    has_metalness = any(l.strip() == "NORMAL_METALNESS" for l in lines)
    pair = material_maps(text)
    packed = combined(*pair) if combined and (pair[0] or pair[1]) and not has_normal else None
    dropped_material = False
    out = []
    changed = False
    for line in lines:
        tok = line.split()
        head = tok[0] if tok else ""
        if head == "TEXTURE_MAP" and len(tok) >= 3 and packed:
            changed = True
            if not has_normal:
                out.append(f"TEXTURE_NORMAL {packed}")
                if not has_metalness:
                    out.append("NORMAL_METALNESS")
                has_normal = True
            continue
        if head == "TEXTURE_MAP" and len(tok) >= 3:
            changed = True
            if tok[1] == "normal" and not has_normal:
                out.append(f"TEXTURE_NORMAL {' '.join(tok[2:])}")
                if not has_metalness:
                    out.append("NORMAL_METALNESS")
                    has_metalness = True
                has_normal = True
            elif tok[1] != "normal":
                dropped_material = True
            continue
        if head == "ATTR_cockpit_device":
            changed = True
            continue
        if head == "ATTR_manip_device":
            changed = True
            out.append(line[: len(line) - len(line.lstrip())] + "ATTR_manip_none")
            continue
        if head == "LIGHT_PARAM":
            conv = light(line)
            if conv is not None:
                changed = True
                out.extend(conv)
                continue
        out.append(line)
    if dropped_material:
        for i, line in enumerate(out):
            tok = line.split()
            if tok and tok[0] == "GLOBAL_specular" and len(tok) > 1:
                try:
                    out[i] = f"GLOBAL_specular {min(float(tok[1]), SPECULAR_WITHOUT_MATERIAL):g}"
                except ValueError:
                    pass
    return "\n".join(out) + ("\n" if text.endswith("\n") else ""), changed


def device_vertices(idx, cmds):
    """Vertex -> cockpit device, from each device's TRIS."""
    owner = {}
    device = None
    for line in cmds:
        tok = line.split()
        if not tok:
            continue
        if tok[0] == "ATTR_cockpit_device":
            device = tok[1]
        elif tok[0] == "ATTR_no_cockpit":
            device = None
        elif tok[0] == "TRIS" and device:
            off, cnt = int(tok[1]), int(tok[2])
            for v in idx[off:off + cnt]:
                owner.setdefault(v, device)
    return owner


def remap_vt(line, rect, panel):
    tok = line.split()
    u, v = float(tok[7]), float(tok[8])
    x, y, w, h = rect
    tok[7] = f"{(x + u * w) / panel[0]:.6f}"
    tok[8] = f"{(y + v * h) / panel[1]:.6f}"
    return " ".join(tok)


def screens_outside_only(text):
    """The XP12 screen object, kept for outside views (dark glass) and gone from the cockpit, where the
    cockpit object draws the screens from the panel texture."""
    header, vts, idx, cmds = split(text)
    body = []
    lod = []
    for line in cmds:
        tok = line.split()
        head = tok[0] if tok else ""
        if head == "ATTR_LOD":
            lod.append(line)
        elif head == "ATTR_cockpit_device":
            continue
        elif head == "ATTR_manip_device":
            body.append("ATTR_manip_none")
        else:
            body.append(line)
    gate = ["ANIM_begin", "ANIM_hide -99999 99999 sim/graphics/view/view_is_external", "ANIM_show 0.5 1.5 sim/graphics/view/view_is_external"]
    return join(header, vts, idx, lod + gate + body + ["ANIM_end"])


def cockpit_with_screens(click_text, screens_text, atlas, panel):
    """The XP12 cockpit (click) object plus the screen glass of the screen object, on the panel texture."""
    c_head, c_vts, c_idx, c_cmds = split(click_text)
    s_head, s_vts, s_idx, s_cmds = split(screens_text)
    kept = []
    skipping = False
    for line in c_cmds:
        tok = line.split()
        head = tok[0] if tok else ""
        if head in ("ATTR_manip_device", "ATTR_cockpit_device"):
            skipping = True
            continue
        if skipping and head == "TRIS":
            continue
        if head == "ATTR_manip_none":
            skipping = False
        kept.append(line)
    owner = device_vertices(s_idx, s_cmds)
    vbase, ibase = len(c_vts), len(c_idx)
    new_vts = []
    for j, line in enumerate(s_vts):
        dev = owner.get(j)
        r = atlas.get(dev) if dev else None
        new_vts.append(remap_vt(line, (r["x"], r["y"], r["w"], r["h"]), panel) if r else line)
    added = ["", "ATTR_draw_enable", "ATTR_no_blend 0.5"]
    for line in s_cmds:
        tok = line.split()
        head = tok[0] if tok else ""
        if head == "ATTR_LOD":
            continue
        if head == "ATTR_cockpit_device":
            added.append("ATTR_cockpit" if tok[1] in atlas else "ATTR_no_cockpit")
        elif head == "ATTR_manip_device":
            added.append("ATTR_manip_none")
        elif head == "TRIS":
            added.append(f"TRIS {int(tok[1]) + ibase} {tok[2]}")
        else:
            added.append(line)
    added.append("ATTR_no_cockpit")
    return join(c_head, c_vts + new_vts, c_idx + [v + vbase for v in s_idx], kept + added)


def screen_devices(screens_text):
    """Device ids the screen object draws, in order."""
    out = []
    for line in screens_text.splitlines():
        tok = line.split()
        if tok and tok[0] == "ATTR_cockpit_device" and tok[1] not in out:
            out.append(tok[1])
    return out
