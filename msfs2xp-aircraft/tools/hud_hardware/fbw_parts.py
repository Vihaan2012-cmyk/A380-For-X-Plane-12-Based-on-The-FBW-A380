"""FlyByWire's own control hardware for the HUD control panels: the glareshield LOUDSPEAKER knob, the
CONSOLE & FLOOR LT toggle and the AUTOLAND pushbutton cap, copied from the cockpit objects with their
textures and set onto each seat's panel (make_hud_hardware.py) in place of hand-built parts. Each part is
added to the FlyByWire object it came from (same texture), always starting from that object's original
copy: a small object of its own is dropped by X-Plane's screen-size culling beyond arm's length. Each part is
turned from its own panel's orientation onto the HUD strip, seated on the panel face, and keeps the
travel FlyByWire gave it: the knob's OFF-MAX sweep for OFF-BRT, the toggle's two end positions for
NORM / X WIND."""

from pathlib import Path

import numpy as np

KNOB = ("a380_cockpit_042_lamps.obj", 20031, 20031, np.array([-0.676165, 2.954047, -32.722256]),
        np.array([-0.229458, -0.382602, -0.894966]), [(0, 0.0), (0.5, 121.580213), (1, 246.333448)])
TOGGLE = ("a380_cockpit_043.obj", 82104, 894, np.array([-0.859165, 2.735422, -32.854556]),
          np.array([0.851394, -0.070814, -0.519724]), [(0, 0.0), (1, 47.626715)])
TOGGLE_BASE = ("a380_cockpit_043.obj", 0, 69996, 0.015)
BUTTON = ("a380_cockpit_068.obj", 25380, 216, None, np.array([-0.00001, -0.00044, -0.001156]), None)
KNOB_MANIP = "ATTR_manip_axis_knob rotate_medium 0 1 0.05 0.5"
HEADER_KEYS = ("GLOBAL_cockpit_lit", "TEXTURE ", "TEXTURE_MAP", "GLOBAL_specular")
SEAT = 0.0003


def unit(v):
    v = np.asarray(v, float)
    return v / np.linalg.norm(v)


def rotation(axis, deg):
    k = unit(axis)
    t = np.radians(deg)
    K = np.array([[0, -k[2], k[1]], [k[2], 0, -k[0]], [-k[1], k[0], 0]])
    return np.eye(3) + np.sin(t) * K + (1 - np.cos(t)) * K @ K


def align(src, dst):
    s, d = unit(src), unit(dst)
    c = np.cross(s, d)
    if np.linalg.norm(c) < 1e-9:
        return np.eye(3)
    return rotation(c, np.degrees(np.arctan2(np.linalg.norm(c), s @ d)))


def read_part(objects, spec):
    name, first, count = spec[:3]
    lines = (Path(objects) / name).read_text(encoding="latin-1").split("\n")
    vt = np.array([[float(x) for x in l.split()[1:9]] for l in lines if l.startswith("VT")])
    idx = np.array([int(x) for l in lines if l.startswith(("IDX10", "IDX ")) for x in l.split()[1:]], dtype=np.int64)
    tri = idx[first:first + count].reshape(-1, 3)
    used, inverse = np.unique(tri, return_inverse=True)
    header = []
    for l in lines:
        if l.startswith("POINT_COUNTS"):
            break
        if l.startswith(HEADER_KEYS):
            header.append(l)
    return vt[used].copy(), inverse.reshape(-1, 3), header


def place(v, rot, pivot, target, n, seat=None):
    out = v.copy()
    out[:, :3] = (v[:, :3] - pivot) @ rot.T
    out[:, 3:6] = v[:, 3:6] @ rot.T
    floor = (v if seat is None else seat)[:, :3]
    lift = -(((floor - pivot) @ rot.T) @ n).min() - SEAT
    out[:, :3] += target + n * lift
    return out, target + n * lift


def read_base(objects, spec, pivot):
    name, first, count, radius = spec
    lines = (Path(objects) / name).read_text(encoding="latin-1").split("\n")
    vt = np.array([[float(x) for x in l.split()[1:9]] for l in lines if l.startswith("VT")])
    idx = np.array([int(x) for l in lines if l.startswith(("IDX10", "IDX ")) for x in l.split()[1:]], dtype=np.int64)
    tri = idx[first:first + count].reshape(-1, 3)
    tri = tri[np.linalg.norm(vt[tri, :3] - pivot, axis=2).max(axis=1) < radius]
    used, inverse = np.unique(tri, return_inverse=True)
    return vt[used].copy(), inverse.reshape(-1, 3)


def knob(objects, n, b, target):
    v, tri, header = read_part(objects, KNOB)
    out_axis = -unit(KNOB[4])
    rot = align(out_axis, n)
    v2, pivot = place(v, rot, KNOB[3], target, n)
    return v2, tri, header, pivot, unit(rot @ KNOB[4]), KNOB[5]


def toggle(objects, n, a, b, target):
    v, tri, header = read_part(objects, TOGGLE)
    hinge = unit(TOGGLE[4])
    rel = v[:, :3] - TOGGLE[3]
    tip = rel[np.argmax(np.linalg.norm(rel, axis=1))]
    mid = rotation(hinge, TOGGLE[5][-1][1] / 2) @ tip
    out_s = unit(mid - (mid @ hinge) * hinge)
    src = np.stack([hinge, np.cross(out_s, hinge), out_s])
    dst = np.stack([a, np.cross(n, a), n])
    rot = dst.T @ src
    base, base_tri = read_base(objects, TOGGLE_BASE, TOGGLE[3])
    v2, pivot = place(v, rot, TOGGLE[3], target, n, seat=base)
    base2, _ = place(base, rot, TOGGLE[3], target, n, seat=base)
    axis = unit(rot @ hinge)
    rest_tip = (v2[:, :3] - pivot)[np.argmax(np.linalg.norm(v2[:, :3] - pivot, axis=1))]
    sign = 1.0 if rest_tip @ b > 0 else -1.0
    return v2, tri, header, pivot, axis * sign, TOGGLE[5], base2, base_tri


def button(objects, n, target):
    v, tri, header = read_part(objects, BUTTON)
    push = unit(BUTTON[4])
    rot = align(-push, n)
    centre = v[:, :3].mean(axis=0)
    v2, pivot = place(v, rot, centre, target, n)
    return v2, tri, header, pivot, -n * np.linalg.norm(BUTTON[4]), None


def inject(src, dst, parts, prelude=None):
    lines = Path(src).read_text(encoding="latin-1").split("\n")
    rows = [i for i, l in enumerate(lines) if l.startswith("VT")]
    idx = [int(x) for l in lines if l.startswith(("IDX10", "IDX ")) for x in l.split()[1:]]
    head = lines[: rows[0]]
    body = [l for l in lines[rows[-1] + 1:] if not l.startswith(("IDX10", "IDX "))]
    vt = [lines[i] for i in rows]
    tail = prelude or ["ATTR_cull", "ATTR_poly_os 0", "ATTR_shadow", "ATTR_light_level_reset", "ATTR_manip_none"]
    for side, v, tri, anim in parts:
        base = len(vt)
        vt += ["VT " + " ".join(f"{x:.6f}" for x in row) for row in v]
        start = len(idx)
        idx += (tri + base).ravel().tolist()
        tail += ["ANIM_begin", f"ANIM_hide -0.5 0.5 fbw/hud/{side}/installed", *anim(f"TRIS {start} {len(idx) - start}"), "ANIM_end"]
    for i, l in enumerate(head):
        if l.startswith("POINT_COUNTS"):
            p = l.split()
            head[i] = f"POINT_COUNTS {len(vt)} {p[2]} {p[3]} {len(idx)}"
    full = len(idx) - len(idx) % 10
    rows_out = ["IDX10 " + " ".join(map(str, idx[k:k + 10])) for k in range(0, full, 10)] + [f"IDX {i}" for i in idx[full:]]
    while body and not body[-1].strip():
        body.pop()
    Path(dst).write_text("\n".join(head + vt + rows_out + body + tail + [""]), encoding="latin-1", newline="\n")
    return len(parts)


CLICK_OBJ = "a380_cockpit_click.obj"


def box(centre, u, v, w, half):
    out = []
    for su in (-1, 1):
        for sv in (-1, 1):
            for sw in (-1, 1):
                p = centre + u * su * half[0] + v * sv * half[1] + w * sw * half[2]
                out.append([*p, *w, 0.0, 0.0])
    faces = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
    tri = []
    for a, b, c, d in faces:
        tri += [(a, b, c), (a, c, d), (a, c, b), (a, d, c)]
    return np.array(out), np.array(tri)


def click_spots(seats, stow_deg):
    spots = []
    x = np.array([1.0, 0.0, 0.0])
    for side, info in seats.items():
        n, a = info["normal"], info["a"]
        b = np.cross(n, a)
        c = info["controls"]
        hinge = info["hinge"]
        v, t = box(c["bright"] + n * 0.010, a, b, n, (0.012, 0.012, 0.012))
        spots.append((side, v, t, lambda tris, s=side: [f"{KNOB_MANIP} fbw/hud/{s}/brightness HUD brightness", tris, "ATTR_manip_none"]))
        v, t = box(c["xwind"] + n * 0.010, a, b, n, (0.007, 0.011, 0.012))
        spots.append((side, v, t, lambda tris, s=side: [f"ATTR_manip_command hand fbw/hud/{s}/xwind_toggle HUD XWIND switch: NORM / X WIND", tris, "ATTR_manip_none"]))
        v, t = box(c["declutter"] + n * 0.004, a, b, n, (0.009, 0.009, 0.006))
        spots.append((side, v, t, lambda tris, s=side: [f"ATTR_manip_push button 1 0 fbw/hud/{s}/declutter_push HUD DECLUTTER", tris, "ATTR_manip_none"]))
        g = info["glass"]
        y, z = np.array([0.0, 1.0, 0.0]), np.array([0.0, 0.0, 1.0])
        v, t = box(g, x, y, z, (0.14, 0.085, 0.008))
        spots.append((side, v, t, lambda tris, s=side, h=hinge: [
            *rotate_lines(h, x, [(0, stow_deg), (1, 0.0)], f"fbw/hud/{s}/deploy"),
            f"ATTR_manip_command hand fbw/hud/{s}/combiner_toggle HUD combiner: deploy or stow", tris, "ATTR_manip_none"]))
    return spots


def add_click_spots(objects, out_dir, seats, stow_deg, originals=None):
    src = Path(originals) / CLICK_OBJ if originals else Path(objects) / CLICK_OBJ
    if "fbw/hud/" in src.read_text(encoding="latin-1"):
        raise SystemExit(f"{src} already carries the HUD click spots; pass the originals dir")
    spots = click_spots(seats, stow_deg)
    inject(src, Path(out_dir) / CLICK_OBJ, spots, prelude=["ATTR_no_cockpit", "ATTR_draw_disable", "ATTR_manip_none"])
    return len(spots)


def rotate_lines(pivot, axis, keys, dataref):
    p = " ".join(f"{c:.6f}" for c in pivot)
    q = " ".join(f"{-c:.6f}" for c in pivot)
    return [f"ANIM_trans {p} {p}", f"ANIM_rotate_begin {axis[0]:.6f} {axis[1]:.6f} {axis[2]:.6f} {dataref}",
            *[f"ANIM_rotate_key {v} {d:.6f}" for v, d in keys], "ANIM_rotate_end", f"ANIM_trans {q} {q}"]


def build(objects, out_dir, seats, originals=None):
    out_dir = Path(out_dir)
    knobs, toggles, buttons = [], [], []
    headers = {}
    for side, info in seats.items():
        n, a = info["normal"], info["a"]
        b = np.cross(n, a)
        c = info["controls"]
        v, tri, headers["knob"], pivot, axis, keys = knob(objects, n, b, c["bright"])
        knobs.append((side, v, tri, lambda tris, s=side, p=pivot, ax=axis, k=keys: [
            *rotate_lines(p, ax, k, f"fbw/hud/{s}/brightness"),
            f"{KNOB_MANIP} fbw/hud/{s}/brightness HUD brightness",
            f"ATTR_light_level 0 1 fbw/cockpit/lt/KNOB_GLARESHIELD_{'CS' if s == 'l' else 'FO'}_SPEAKER", tris,
            "ATTR_light_level_reset", "ATTR_manip_none"]))
        v, tri, headers["toggle"], pivot, axis, keys, base, base_tri = toggle(objects, n, a, b, c["xwind"])
        toggles.append((side, base, base_tri, lambda tris: [tris]))
        toggles.append((side, v, tri, lambda tris, s=side, p=pivot, ax=axis, k=keys: [
            *rotate_lines(p, ax, k, f"fbw/hud/{s}/xwind"),
            f"ATTR_manip_command hand fbw/hud/{s}/xwind_toggle HUD XWIND switch: NORM / X WIND", tris, "ATTR_manip_none"]))
        v, tri, headers["button"], pivot, push, _ = button(objects, n, c["declutter"])
        buttons.append((side, v, tri, lambda tris, s=side, d=push: [
            f"ANIM_trans_begin fbw/hud/{s}/declutter_push", "ANIM_trans_key 0 0 0 0",
            f"ANIM_trans_key 1 {d[0]:.6f} {d[1]:.6f} {d[2]:.6f}", "ANIM_trans_end",
            f"ATTR_manip_push button 1 0 fbw/hud/{s}/declutter_push HUD DECLUTTER", tris, "ATTR_manip_none"]))
    out = {}
    for spec, parts in ((KNOB, knobs), (TOGGLE, toggles), (BUTTON, buttons)):
        name = spec[0]
        src = Path(originals) / name if originals else Path(objects) / name
        if "fbw/hud/" in src.read_text(encoding="latin-1"):
            raise SystemExit(f"{src} already carries the HUD parts; pass the originals dir")
        out[name] = inject(src, out_dir / name, parts)
    return out
