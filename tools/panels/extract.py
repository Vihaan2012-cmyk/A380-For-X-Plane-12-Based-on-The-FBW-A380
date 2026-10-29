"""Panel descriptions for pop-out panels from the installed cockpit OBJs.

python extract.py [--aircraft DIR] [--out E:/fbw-build/panels] [--px-per-mm 4]
"""
import argparse
import collections
import json
import os
import re
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from obj8 import Obj8  # noqa: E402

DEFAULT_AIRCRAFT = "D:/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12"
EYE = np.array([0.0, 3.158, -32.07])
MARGIN_MM = 12.0
VISUAL = ["a380_cockpit_000.obj", "a380_cockpit_003.obj"] + [f"a380_cockpit_{i:03d}.obj" for i in range(11, 96)] + [
    "a380_cockpit_039_lamps.obj", "a380_cockpit_042_lamps.obj"]

TITLES = {
    "glareshield_capt": "Captain's glareshield",
    "efis_cp_capt": "Captain's EFIS control panel",
    "fcu": "FCU",
    "efis_cp_fo": "First officer's EFIS control panel",
    "glareshield_fo": "First officer's glareshield",
    "ovhd_left": "Overhead left",
    "ovhd_center_fwd": "Overhead centre forward",
    "ovhd_center_mid": "Overhead centre",
    "ovhd_center_aft": "Overhead centre aft",
    "ovhd_right": "Overhead right",
    "ovhd_aft": "Aft overhead",
    "lateral_capt": "Captain's lateral",
    "lateral_fo": "First officer's lateral",
    "instr_center": "Centre instrument panel",
    "rmp1": "RMP 1", "rmp2": "RMP 2", "rmp3": "RMP 3",
    "ecam_cp": "ECAM control panel",
    "kccu_capt": "Captain's KCCU", "kccu_fo": "First officer's KCCU",
    "keyboard_capt": "Captain's table keyboard", "keyboard_fo": "First officer's table keyboard",
    "acp3": "Overhead audio control (RMP 3 side)",
    "eng_master": "Engine masters",
    "surv": "SURV panel",
    "flaps_speedbrake": "Flaps, speedbrake and thrust levers",
    "lighting_capt": "Captain's lighting and handling",
    "lighting_fo": "First officer's lighting and handling",
}

COMBINED = {"glareshield": ["efis_cp_capt", "fcu", "efis_cp_fo"]}
TITLES["glareshield"] = "Glareshield (FCU and EFIS control panels)"
OUTLIER_COMBINED = {"glareshield": 0.6}

ORDER = ["glareshield_capt", "efis_cp_capt", "fcu", "efis_cp_fo", "glareshield_fo",
         "ovhd_left", "ovhd_center_fwd", "ovhd_center_mid", "ovhd_center_aft", "ovhd_right", "ovhd_aft",
         "instr_center", "rmp1", "rmp2", "rmp3", "acp3", "ecam_cp", "kccu_capt", "kccu_fo", "eng_master", "surv",
         "flaps_speedbrake", "lateral_capt", "lateral_fo", "lighting_capt", "lighting_fo"]

ARGS_BEFORE_TIP = {
    "command": 2, "push": 4, "toggle": 4, "command_knob": 3, "command_switch_up_down": 3,
    "command_switch_left_right": 3, "axis_knob": 6, "axis_switch_up_down": 6, "axis_switch_left_right": 6,
    "drag_axis": 7, "device": 2, "command_axis": 4, "drag_xy": 10,
}


def parse_manip(line):
    p = line.split()
    kind = p[0][len("ATTR_manip_"):]
    a = p[1:]
    n = ARGS_BEFORE_TIP.get(kind, len(a))
    tip = " ".join(a[n:])
    return kind, a[:n], tip


def binding_for(kind, a, wheel):
    """Panel kind, binding (popout.rs Binding) and the exact manipulator for the plugin."""
    manip = {"type": kind, "cursor": a[0] if a else None}
    if kind == "command":
        manip["command"] = a[1]
        return "button", {"type": "command", "name": a[1]}, manip
    if kind == "push":
        v_down, v_up, dref = float(a[1]), float(a[2]), a[3]
        manip.update(dataref=dref, down=v_down, up=v_up)
        return "button", {"type": "dataref", "name": dref, "values": [v_up, v_down]}, manip
    if kind == "toggle":
        v_on, v_off, dref = float(a[1]), float(a[2]), a[3]
        manip.update(dataref=dref, on=v_on, off=v_off)
        return "switch", {"type": "dataref", "name": dref, "values": [v_off, v_on]}, manip
    if kind == "command_knob":
        manip.update(positive=a[1], negative=a[2])
        return "knob", {"type": "knob", "dec": a[2], "inc": a[1]}, manip
    if kind in ("command_switch_up_down", "command_switch_left_right"):
        manip.update(positive=a[1], negative=a[2])
        return "switch", {"type": "knob", "dec": a[2], "inc": a[1]}, manip
    if kind in ("axis_knob", "axis_switch_up_down", "axis_switch_left_right"):
        lo, hi, click, hold, dref = float(a[1]), float(a[2]), float(a[3]), float(a[4]), a[5]
        manip.update(dataref=dref, min=lo, max=hi, click=click, hold=hold)
        step = abs(click) or abs(hold) or 1.0
        k = "rotary" if kind == "axis_knob" else "switch"
        return k, {"type": "axis", "name": dref, "min": min(lo, hi), "max": max(lo, hi), "step": step}, manip
    if kind == "drag_axis":
        dx, dy, dz, v1, v2, dref = float(a[1]), float(a[2]), float(a[3]), float(a[4]), float(a[5]), a[6]
        manip.update(dataref=dref, dx=dx, dy=dy, dz=dz, v1=v1, v2=v2)
        return "rotary", {"type": "axis", "name": dref, "min": min(v1, v2), "max": max(v1, v2),
                          "step": round(abs(v2 - v1) / 10.0, 6) or 0.1}, manip
    manip["args"] = a[1:]
    return "button", {"type": "command", "name": a[1] if len(a) > 1 else ""}, manip


def assign(tip, dref, c):
    x, y, z = c
    t = tip.upper()
    d = (dref or "").upper()
    if re.search(r"\bRMP ?1\b|RMP1", t): return "rmp1"
    if re.search(r"\bRMP ?2\b|RMP2", t): return "rmp2"
    if re.search(r"\bRMP ?3\b|RMP3", t): return "acp3" if y > 3.3 else "rmp3"
    if "KBD CPT" in t: return "keyboard_capt"
    if "KBD FO" in t: return "keyboard_fo"
    if "KCCUL" in t or "KCCU_L" in d or "KCCU L" in t: return "kccu_capt"
    if "KCCUR" in t or "KCCU_R" in d or "KCCU R" in t: return "kccu_fo"
    if "SURV" in t: return "surv"
    if t.startswith("PUSH ECAM") or "ECAM CP" in t: return "ecam_cp"
    if re.match(r"(KNOB|PUSH) EFIS (CS|FO) (ND|MFD|OIT|PFD|RECONF)\b", t):
        return "lateral_capt" if x < 0 else "lateral_fo"
    if "EFIS CS" in t: return "efis_cp_capt"
    if "EFIS FO" in t: return "efis_cp_fo"
    if "FCU" in t: return "fcu"
    if re.match(r"(PUSH|KNOB) GLARESHIELD", t) and y > 2.7:
        return "glareshield_capt" if x < 0 else "glareshield_fo"
    if t.startswith("SWITCH ENGINES") or "ENG MODE" in t or "ENGINE MASTER" in t or "ENG MASTER" in t: return "eng_master"
    if y > 3.25 and abs(x) < 0.33 and -32.6 < z < -31.6 and not t.startswith(("SUNSHADE", "HANDLING")):
        if z >= -31.865: return "ovhd_aft"
        if x < -0.17: return "ovhd_left"
        if x > 0.17: return "ovhd_right"
        if z < -32.36: return "ovhd_center_fwd"
        if z < -32.05: return "ovhd_center_mid"
        return "ovhd_center_aft"
    if "ACD" in t or "LAPTOP" in t or "OIS" in t or "OIT SIDE" in t or "CONSOLE FLOOR" in t:
        return "lateral_capt" if x < 0 else "lateral_fo"
    if "HANDLING" in t or "LIGHTING" in t:
        return "lighting_capt" if x < 0 else "lighting_fo"
    if re.search(r"FLAPS|SPEEDBRAKE|SPOILER|THROTTLE|THRUST|PARKING|PARK BRK|PARK_BRAKE", t + " " + d): return "flaps_speedbrake"
    if re.match(r"(PUSH|KNOB) ISIS|PUSH CHRONO|AUTOBKR|GEAR|BRAKE FAN|BRK|GRVTY|CLOCK|CHRONO|SWITCH AUTOBKR", t) and y > 2.3 and z < -32.3:
        return "instr_center"
    if "ATHR" in t: return "eng_master"
    return None


OUTLIER_M = {"kccu_capt": 0.2, "kccu_fo": 0.2, "rmp1": 0.15, "rmp2": 0.15, "rmp3": 0.15, "acp3": 0.15, "keyboard_capt": 0.3, "keyboard_fo": 0.3, "surv": 0.1,
             "ecam_cp": 0.15, "eng_master": 0.15, "fcu": 0.2, "efis_cp_capt": 0.2, "efis_cp_fo": 0.2,
             "glareshield_capt": 0.35, "glareshield_fo": 0.35}


def drop_outliers(pid, items):
    lim = OUTLIER_COMBINED.get(pid) or OUTLIER_M.get(pid, 0.6)
    C = np.array([e["c"] for *_, e in items])
    med = np.median(C, axis=0)
    keep = np.linalg.norm(C - med, axis=1) <= lim
    return [it for it, k in zip(items, keep) if k], [it[2] for it, k in zip(items, keep) if not k]


def frame(points, tris):
    ctr = points.mean(0)
    lo, hi = points.min(0) - 0.01, points.max(0) + 0.01
    tc, tn = tris
    sel = np.all((tc >= lo) & (tc <= hi), axis=1)
    acc = tn[sel]
    acc = acc[(acc @ (EYE - ctr)) > 0] if len(acc) else acc
    if len(acc) and np.linalg.norm(acc.sum(0)) > 1e-9:
        n = acc.sum(0) / np.linalg.norm(acc.sum(0))
    elif len(points) >= 3:
        n = np.linalg.svd(points - ctr)[2][2]
    else:
        n = np.array([0.0, 0.0, 1.0])
    to_eye = EYE - ctr
    if np.dot(n, to_eye) < 0:
        n = -n
    right = np.array([1.0, 0.0, 0.0]) - n * n[0]
    if np.linalg.norm(right) < 1e-3:
        right = np.array([0.0, 0.0, -1.0]) - n * -n[2]
    right /= np.linalg.norm(right)
    up = np.cross(n, right)
    return ctr, n, right, up


def project(P, fr):
    ctr, n, r, u = fr
    d = P - ctr
    return np.stack([d @ r, d @ u, d @ n], axis=1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--aircraft", default=DEFAULT_AIRCRAFT)
    ap.add_argument("--out", default="E:/fbw-build/panels")
    ap.add_argument("--px-per-mm", type=float, default=4.0)
    args = ap.parse_args()
    objs = os.path.join(args.aircraft, "objects")
    ppm = args.px_per_mm
    os.makedirs(args.out, exist_ok=True)

    click = Obj8(os.path.join(objs, "a380_cockpit_click.obj"))
    merged = collections.OrderedDict()
    for d in click.draws:
        m = d.state.get("manip")
        if not m or d.lod > 0:
            continue
        P = click.positions(d)
        c = P.mean(0)
        key = None
        for k, e in merged.items():
            if e["line"] == m and np.linalg.norm(e["c"] - c) < 0.03:
                key = k
                break
        if key is None:
            merged[len(merged)] = {"line": m, "P": P, "c": c, "wheel": d.state.get("wheel"), "device": d.state.get("device")}
        else:
            e = merged[key]
            e["P"] = np.vstack([e["P"], P])
            e["c"] = e["P"].mean(0)
            e["wheel"] = e["wheel"] or d.state.get("wheel")

    groups = collections.defaultdict(list)
    unassigned = collections.Counter()
    for e in merged.values():
        kind, a, tip = parse_manip(e["line"])
        if kind == "device":
            continue
        dref = a[-1] if kind in ("push", "toggle", "axis_knob", "axis_switch_up_down", "drag_axis") else (a[1] if len(a) > 1 else "")
        pid = assign(tip, dref, e["c"])
        if pid is None:
            unassigned[(tip.split() or ["?"])[0]] += 1
            continue
        groups[pid].append((kind, a, tip, e))

    lamps = []
    screens = []
    tri_c, tri_n = [], []
    for name in VISUAL:
        path = os.path.join(objs, name)
        if not os.path.exists(path):
            continue
        o = Obj8(path)
        for d in o.draws:
            if d.lod > 0 or d.hidden or d.count == 0:
                continue
            P, N, _ = o.triangles(d)
            T = P[: len(P) // 3 * 3].reshape(-1, 3, 3)
            area = np.linalg.norm(np.cross(T[:, 1] - T[:, 0], T[:, 2] - T[:, 0]), axis=1) / 2
            tri_c.append(T.mean(1))
            tri_n.append(N[: len(P) // 3 * 3].reshape(-1, 3, 3).mean(1) * area[:, None])
            if "device" in d.state and name == "a380_cockpit_003.obj":
                screens.append((d.state["device"], P))
            elif "light" in d.state:
                lamps.append((d.state["light"][2], P, name))
    tris = (np.vstack(tri_c), np.vstack(tri_n))

    for pid, parts in COMBINED.items():
        groups[pid] = [it for p in parts for it in groups.get(p, [])]
    summary = []
    for pid in list(COMBINED) + ORDER:
        items = groups.get(pid)
        if not items:
            continue
        items, dropped = drop_outliers(pid, items)
        if dropped:
            print(f"  {pid}: dropped outliers {dropped}")
        allP = np.vstack([e["P"] for *_, e in items])
        fr = frame(allP, tris)
        rects3 = [project(e["P"], fr) for *_, e in items]
        lo = np.min([r[:, :2].min(0) for r in rects3], axis=0) - MARGIN_MM / 1000
        hi = np.max([r[:, :2].max(0) for r in rects3], axis=0) + MARGIN_MM / 1000
        depth = np.concatenate([r[:, 2] for r in rects3])
        dmin, dmax = depth.min() - 0.06, depth.max() + 0.03

        def to_px(q):
            x = (q[:, 0] - lo[0]) * 1000 * ppm
            y = (hi[1] - q[:, 1]) * 1000 * ppm
            return [round(float(x.min()), 1), round(float(y.min()), 1), round(float(x.max()), 1), round(float(y.max()), 1)]

        size = (int(round((hi[0] - lo[0]) * 1000 * ppm)), int(round((hi[1] - lo[1]) * 1000 * ppm)))
        controls = []
        ids = collections.Counter()
        for (kind, a, tip, e), q in zip(items, rects3):
            pkind, binding, manip = binding_for(kind, a, e["wheel"])
            if e["wheel"]:
                manip["wheel"] = e["wheel"]
            base = re.sub(r"[^a-z0-9]+", "_", (tip or manip.get("command") or manip.get("dataref") or kind).lower()).strip("_")
            ids[base] += 1
            cid = base if ids[base] == 1 else f"{base}_{ids[base]}"
            if "GUARD" in tip.upper() and pkind == "switch":
                pkind = "switch"
            controls.append({"id": cid, "rect": to_px(q), "kind": pkind, "binding": binding,
                             "tooltip": tip, "manip": manip})

        panel_lamps = []
        for dref, P, src in lamps:
            q = project(P, fr)
            c = q.mean(0)
            if not (lo[0] <= c[0] <= hi[0] and lo[1] <= c[1] <= hi[1] and dmin <= c[2] <= dmax):
                continue
            panel_lamps.append({"dataref": dref, "rect": to_px(q), "source": src})
        for c in controls:
            r = c["rect"]
            best = None
            for L in panel_lamps:
                lr = L["rect"]
                cx, cy = (lr[0] + lr[2]) / 2, (lr[1] + lr[3]) / 2
                if r[0] - 2 <= cx <= r[2] + 2 and r[1] - 2 <= cy <= r[3] + 2:
                    score = ("SEQ" in L["dataref"], (lr[2] - lr[0]) * (lr[3] - lr[1]))
                    if best is None or score > best[0]:
                        best = (score, L)
            if best:
                c["lamp"] = best[1]["dataref"]
                c["lamps"] = sorted({L["dataref"] for L in panel_lamps
                                     if r[0] - 2 <= (L["rect"][0] + L["rect"][2]) / 2 <= r[2] + 2
                                     and r[1] - 2 <= (L["rect"][1] + L["rect"][3]) / 2 <= r[3] + 2})
        panel_screens = []
        for dev, P in screens:
            q = project(P, fr)
            c = q.mean(0)
            if lo[0] <= c[0] <= hi[0] and lo[1] <= c[1] <= hi[1] and dmin <= c[2] <= dmax:
                panel_screens.append([dev, to_px(q)])

        ctr, n, r, u = fr
        origin = ctr + r * lo[0] + u * hi[1]
        panel = {
            "id": pid, "title": TITLES.get(pid, pid), "size": list(size), "art": f"art/{pid}_off.png",
            "art_lit": f"art/{pid}_on.png", "px_per_mm": ppm,
            "controls": controls, "screens": panel_screens, "lamps": panel_lamps,
            "plane": {"origin": [round(float(v), 6) for v in origin], "right": [round(float(v), 6) for v in r],
                      "up": [round(float(v), 6) for v in u], "normal": [round(float(v), 6) for v in n],
                      "width_m": round(float(hi[0] - lo[0]), 5), "height_m": round(float(hi[1] - lo[1]), 5),
                      "depth_front_m": round(float(dmax), 5), "depth_back_m": round(float(dmin), 5)},
        }
        with open(os.path.join(args.out, f"{pid}.json"), "w", encoding="utf-8", newline="\n") as f:
            json.dump(panel, f, indent=1)
        summary.append((pid, len(controls), len(panel_lamps), len(panel_screens), size))
        print(f"{pid:18s} controls={len(controls):3d} lamps={len(panel_lamps):3d} screens={len(panel_screens)} "
              f"{size[0] / ppm:5.0f} x {size[1] / ppm:4.0f} mm")
    print("unassigned", sum(unassigned.values()), unassigned.most_common(30))


if __name__ == "__main__":
    main()
