import re
import sys
from pathlib import Path

import numpy as np

DATAREF = "fbw/engine/gp7200"
INBOARD_MIN, OUTBOARD_MIN = 12.5, 20.25
GP_WIDTH = 3.80
TRENT_AFT, GP_AFT = 2.19, 2.11
LIFT = {1: -0.05, 2: 0.13, 3: 0.13, 4: -0.05}
GP_SPINNER = 0.45
SKIN_FROM, SKIN_FULL = 1.60, 1.80
SPINNER_MAX_R = 0.46
CORES = ["a380_exterior_020.obj", "a380_exterior_021.obj", "a380_exterior_022.obj", "a380_exterior_023.obj"]
FAN = "a380_exterior_030.obj"
PYLON = "a380_exterior_031.obj"
BLUR = [f"a380_exterior_0{k}.obj" for k in range(34, 42)]
SOURCES = ["a380_exterior_001.obj", *CORES, FAN, PYLON, "a380_engine_covers.obj", *BLUR]
ENGINE_PIVOT = re.compile(r"ANIM_rotate_begin .* fbw/anim/(N1_\d_anim|thrust_rev_\d|engine_wobble/\d/\w+|ENG\d_WOBBLE)$")
STATIC_TRANS = re.compile(r"ANIM_trans (\S+) (\S+) (\S+) (\S+) (\S+) (\S+)$")
PYLON_REACH = 1.2
PYLON_FULL, PYLON_NONE = 0.15, 0.6
WING_CLEAR, PYLON_BAND = 0.05, 0.1
WINGS = ["a380_exterior_026.obj", "a380_exterior_027.obj", "a380_exterior_028.obj", "a380_exterior_029.obj"]
NACELLES = ["a380_exterior_022.obj", "a380_exterior_023.obj"]


def engine_of(x):
    a = np.abs(x)
    e = np.where(x < 0, np.where(a >= OUTBOARD_MIN, 1, 2), np.where(a >= OUTBOARD_MIN, 4, 3))
    return np.where(a < INBOARD_MIN, 0, e)


def vertices(lines):
    rows = [i for i, l in enumerate(lines) if l.startswith("VT")]
    return rows, np.array([[float(v) for v in lines[i].split()[1:9]] for i in rows])


def engine_geometry(objects):
    _, fan = vertices(objects[FAN])
    core = np.concatenate([vertices(objects[n])[1] for n in CORES])
    skins = np.concatenate([vertices(objects[n])[1] for n in NACELLES])
    geo = {}
    for e in (1, 2, 3, 4):
        f = fan[engine_of(fan[:, 0]) == e]
        blades = f[np.hypot(f[:, 0] - f[:, 0].mean(), f[:, 1] - f[:, 1].mean()) > 1.0]
        cx = (blades[:, 0].max() + blades[:, 0].min()) / 2
        cy = (blades[:, 1].max() + blades[:, 1].min()) / 2
        r = np.hypot(f[:, 0] - cx, f[:, 1] - cy)
        spinner = f[r < SPINNER_MAX_R]
        tip = spinner[:, 2].min()
        root = blades[:, 2].min()
        c = core[engine_of(core[:, 0]) == e]
        on_axis = c[np.hypot(c[:, 0] - cx, c[:, 1] - cy) < 0.3]
        plug_tip = on_axis[:, 2].max()
        s = skins[engine_of(skins[:, 0]) == e]
        skin = s[np.abs(s[:, 1] - np.median(s[:, 1])) < 0.6]
        width = skin[:, 0].max() - skin[:, 0].min()
        top = s[np.abs(s[:, 0] - cx) < 0.3][:, 1].max() - cy
        geo[e] = dict(cx=cx, cy=cy, tip=tip, root=root, exit=plug_tip - TRENT_AFT, width=width, top=top, plug=plug_tip)
        print(f"engine {e}: width {width:.3f} m, centre ({cx:.3f}, {cy:.3f}) spinner {tip:.2f}..{root:.2f} nozzle exit {plug_tip - TRENT_AFT:.2f} plug tip {plug_tip:.2f}")
    return geo


def smoothstep(x, a, b):
    t = np.clip((x - a) / (b - a), 0, 1)
    return t * t * (3 - 2 * t)


def wing_floor(wing, g, z):
    w = wing[np.abs(wing[:, 0] - g["cx"]) < PYLON_REACH]
    floor = np.full(len(z), np.inf)
    for i, zi in enumerate(z):
        near = w[np.abs(w[:, 2] - zi) < 0.25]
        if len(near):
            floor[i] = near[:, 1].min() - g["cy"]
    return floor


def morph_pylon(vt, geo, wing):
    out = vt.copy()
    eng = engine_of(vt[:, 0])
    for e, g in geo.items():
        if LIFT[e] >= 0:
            continue
        p = out[eng == e]
        near = (np.abs(p[:, 0] - g["cx"]) < PYLON_REACH) & (p[:, 2] > g["tip"] - 0.5) & (p[:, 2] < g["plug"] + 0.5)
        hi = np.minimum(g["top"] + PYLON_NONE, wing_floor(wing, g, p[:, 2]) - WING_CLEAR)
        lo = np.minimum(g["top"] + PYLON_FULL, hi - PYLON_BAND)
        w = near * (1 - smoothstep(p[:, 1] - g["cy"], lo, hi))
        aft = p[:, 2] > g["exit"]
        p[aft, 2] += w[aft] * (g["exit"] + (p[aft, 2] - g["exit"]) * (GP_AFT / TRENT_AFT) - p[aft, 2])
        p[:, 1] += w * LIFT[e]
        out[eng == e] = p
    return out


def morph(vt, geo, fan):
    out = vt.copy()
    eng = engine_of(vt[:, 0])
    for e, g in geo.items():
        m = eng == e
        if not m.any():
            continue
        p = out[m]
        dx, dy = p[:, 0] - g["cx"], p[:, 1] - g["cy"]
        r = np.hypot(dx, dy)
        k = 1 + (GP_WIDTH / g["width"] - 1) * smoothstep(r, SKIN_FROM, SKIN_FULL)
        p[:, 0] = g["cx"] + dx * k
        aft = p[:, 2] > g["exit"]
        p[aft, 2] = g["exit"] + (p[aft, 2] - g["exit"]) * (GP_AFT / TRENT_AFT)
        if fan:
            s = (r < SPINNER_MAX_R) & (p[:, 2] < g["root"] - 0.05)
            if s.any():
                length = g["root"] - g["tip"]
                t = np.clip((p[s, 2] - g["tip"]) / length, 0, 1)
                hub = r[s].max()
                rr = hub * np.sqrt(np.clip(1 - (1 - t) ** 2, 0, 1))
                scale = np.where(r[s] > 1e-6, rr / np.maximum(r[s], 1e-6), 0)
                p[s, 0] = g["cx"] + dx[s] * scale
                p[s, 1] = g["cy"] + dy[s] * scale
                p[s, 2] = g["root"] - (1 - t) * GP_SPINNER
                u = (1 - t)
                radial = np.where(r[s, None] > 1e-6, np.stack([dx[s], dy[s]], axis=1) / np.maximum(r[s, None], 1e-6), 0)
                nr = np.sqrt(np.clip(1 - u ** 2, 0, 1)) / max(hub, 1e-6)
                nz = -u / GP_SPINNER
                n = np.stack([radial[:, 0] * nr, radial[:, 1] * nr, nz], axis=1)
                n /= np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-9)
                p[s, 3:6] = n
        p[:, 1] += LIFT[e]
        out[m] = p
    return out


def wrap(lines, lo, hi):
    if any(DATAREF in l for l in lines):
        return lines, False
    last_data = max(i for i, l in enumerate(lines) if l.startswith(("VT", "IDX")))
    first = next(i for i in range(last_data + 1, len(lines)) if lines[i].strip() and not lines[i].startswith("#"))
    end = len(lines)
    while end > 0 and not lines[end - 1].strip():
        end -= 1
    return lines[:first] + ["ANIM_begin", f"ANIM_hide {lo} {hi} {DATAREF}"] + lines[first:end] + ["ANIM_end"] + lines[end:], True


def unwrap(lines):
    marker = f"ANIM_hide 0.5 1.5 {DATAREF}"
    if marker not in lines:
        return lines
    i = lines.index(marker)
    out = lines[:i - 1] + lines[i + 1:]
    last = max(j for j, l in enumerate(out) if l.strip() == "ANIM_end")
    return out[:last] + out[last + 1:]


def write_vt(lines, rows, vt):
    out = list(lines)
    for i, row in zip(rows, vt):
        out[i] = "VT " + " ".join(f"{v:.6g}" for v in row)
    return out


def static_trans(line):
    m = STATIC_TRANS.match(line.strip())
    if not m:
        return None
    v = [float(g) for g in m.groups()]
    return v[:3] if v[:3] == v[3:] else None


def move_pivots(lines, place):
    out = list(lines)
    moved = 0
    for i in range(len(out) - 1):
        p = static_trans(out[i])
        if p is None or not ENGINE_PIVOT.match(out[i + 1].strip()):
            continue
        j = i + 1
        while out[j].strip().startswith("ANIM_rotate_"):
            j += 1
        assert static_trans(out[j]) == [-v for v in p], (i, out[i], j, out[j])
        q = place(np.array([p + [0.0] * 5]))[0, :3]
        if np.allclose(q, p):
            continue
        pad_i = out[i][:len(out[i]) - len(out[i].lstrip())]
        pad_j = out[j][:len(out[j]) - len(out[j].lstrip())]
        out[i] = pad_i + "ANIM_trans " + " ".join(f"{v:.6f}" for v in [*q, *q])
        out[j] = pad_j + "ANIM_trans " + " ".join(f"{-v:.6f}" for v in [*q, *q])
        moved += 1
    return out, moved


def main(obj_dir):
    obj_dir = Path(obj_dir)
    objects = {n: unwrap((obj_dir / n).read_text(encoding="utf-8", errors="replace").split("\n")) for n in SOURCES}
    geo = engine_geometry(objects)
    wing = np.concatenate([vertices((obj_dir / n).read_text(encoding="utf-8", errors="replace").split("\n"))[1] for n in WINGS])
    for name, lines in objects.items():
        rows, vt = vertices(lines)
        place = (lambda v: morph_pylon(v, geo, wing)) if name == PYLON else (lambda v: morph(v, geo, False))
        gp_vt = morph_pylon(vt, geo, wing) if name == PYLON else morph(vt, geo, name == FAN)
        gp_lines, pivots = move_pivots(write_vt(lines, rows, gp_vt), place)
        gp_lines, _ = wrap(gp_lines, -0.5, 0.5)
        gp_name = name.replace(".obj", "_gp.obj")
        (obj_dir / gp_name).write_text("\n".join(gp_lines), encoding="utf-8", newline="\n")
        trent, changed = wrap(lines, 0.5, 1.5)
        if changed:
            (obj_dir / name).write_text("\n".join(trent), encoding="utf-8", newline="\n")
        print(f"{name}: {len(rows)} vertices, {pivots} pivots -> {gp_name}{' (Trent copy now hides for GP)' if changed else ''}")


if __name__ == "__main__":
    main(sys.argv[1])
