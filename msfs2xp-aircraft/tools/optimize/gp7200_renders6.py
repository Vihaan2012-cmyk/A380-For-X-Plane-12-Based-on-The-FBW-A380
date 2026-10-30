"""usage: gp7200_renders6.py <installed objects dir> <stage objects dir> <out dir> [preview|cycles|all]

Round-6 sheets. "preview" = the three.js preview (albedo only: NO normal or material maps), before | after | reference. "cycles" = Blender Cycles with the real
albedo, normal and material maps of the objects, before (installed) | after (staged), labelled CYCLES. The Cycles images show what the maps do; how X-Plane shades
them (its normal-map convention, mip filtering, lighting) can only be confirmed in X-Plane."""

import json
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gp7200_renders5 as r5  # noqa: E402
import gp7200_renders2 as r2  # noqa: E402
from gp7200_geom import frames  # noqa: E402

PREVIEW_ONLY = ["front34_e2", "rear34_e2", "side_e2", "intake_e2", "exhaust_e2", "front34_e1", "rear34_e1", "side_e1", "intake_e1", "exhaust_e1", "hub_e2", "hub_e1", "gap_rear_e2", "gap_rear_e1", "hub_e2_etihad", "front34_e2_etihad", "rear34_e2_etihad"]
FN = frames()
BLENDER = "C:/Program Files/Blender Foundation/Blender 5.2/blender.exe"
CYCLES = Path(__file__).resolve().parent / "gp7200_cycles.py"
AC = "D:/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12/"
LIVERIES = {"Emirates": "Emirates A6-EDY", "Etihad": "Etihad A6-API"}
CY_OBJECTS = {1: ("a380_exterior_022_gp.obj", "a380_exterior_020_gp.obj", "a380_exterior_030_gp.obj", "a380_engcore_gp.obj", "a380_exterior_031_gp.obj"),
              3: ("a380_exterior_023_gp.obj", "a380_exterior_021_gp.obj", "a380_exterior_030_gp.obj", "a380_engcore_gp.obj", "a380_exterior_031_gp.obj")}


def cy_views(e):
    f = FN[e]
    P = lambda a, b, s: [float(v) for v in f.point(np.array([a, b, s]))]
    side = -1.0 if e in (1, 2) else 1.0
    pa = np.radians(239.0)
    pd = np.radians(191.0 if side < 0 else 169.0)
    pt = lambda ang, r, s: (r * np.sin(ang), r * np.cos(ang), s)
    v_at = pt(pa if side < 0 else -pa, 1.65, 2.45)
    d_at = pt(pd, 1.65, 0.62)
    return {
        "side": dict(eye=P(side * 14, 0.2, 2.5), at=P(0, 0, 2.5), fov=26),
        "door": dict(eye=P(side * 4.2, 0.1, 2.0), at=P(side * 1.7, 0.0, 2.0), fov=24),
        "vent": dict(eye=P(*(np.array(v_at) * 2.6 + [0, 0, 0])), at=P(*v_at), fov=12),
        "dots": dict(eye=P(*(np.array(d_at) * 2.0)), at=P(*d_at), fov=16),
        "lip_joint": dict(eye=P(side * 3.0, 0.8, -4.5), at=P(side * 1.5, 0.3, -0.9), fov=20),
        "lower_panel": dict(eye=P(side * 3.0, -2.6, -2.0), at=P(side * 1.3, -1.2, -0.5), fov=22),
        "intake": dict(eye=P(0.3, 0.5, -5.0), at=P(0, 0, 0.2), fov=32),
        "intake_side": dict(eye=P(side * -0.9, 0.3, -3.0), at=P(side * -0.7, -0.1, 0.3), fov=38),
        "fan_front": dict(eye=P(-0.9, -0.5, -4.5), at=P(0, 0, 0.3), fov=22),
        "fan_hub": dict(eye=P(-0.3, -0.2, -2.5), at=P(0, 0, 0.2), fov=14),
        "fan_oblique": dict(eye=P(-2.2, -1.0, -3.5), at=P(0, 0, 0.3), fov=24),
        "exhaust": dict(eye=P(0.3, 0.3, 10.0), at=P(0, 0, 5.6), fov=28),
        "exh_side": dict(eye=P(side * 3.2, 0.8, 9.0), at=P(0, 0, 5.3), fov=24),
        "pylon_rear": dict(eye=P(side * 2.5, 3.2, 16), at=P(0, 2.1, 3.5), fov=16),
        "pylon_rear2": dict(eye=P(1.7, 1.9, 13.4), at=P(0, 1.9, 3.0), fov=18),
        "pylon_side": dict(eye=P(side * 9, 1.2, 3.6), at=P(0, 1.9, 3.6), fov=12),
    }


def cycles_sheet(install, stage, out, engine, livery, names, samples=40):
    out.mkdir(parents=True, exist_ok=True)
    work = Path("E:/fbw-build/gp7200-model/work/cy6")
    work.mkdir(parents=True, exist_ok=True)
    views = cy_views(engine)
    ld = AC + f"liveries/{livery}/objects"
    done = {}
    for tag, root in (("before", install), ("after", stage)):
        objs = [dict(path=str(Path(root) / n), engine=engine, albedo_dir=ld) for n in CY_OBJECTS[1 if engine < 3 else 3] if (Path(root) / n).is_file()]
        spec = dict(objects=objs, views=[dict(views[n], name=f"{tag}_{n}") for n in names], out_dir=str(work), size=[1000, 640], samples=samples, bg_strength=0.9)
        sp = work / f"spec_{tag}_e{engine}.json"
        sp.write_text(json.dumps(spec))
        subprocess.run([BLENDER, "-b", "--factory-startup", "-P", str(CYCLES), "--", str(sp)], check=True, stdout=subprocess.DEVNULL)
    for n in names:
        a, b = Image.open(work / f"before_{n}.png").convert("RGB"), Image.open(work / f"after_{n}.png").convert("RGB")
        sheet = Image.new("RGB", (a.width * 2, a.height + 22), "white")
        d = ImageDraw.Draw(sheet)
        sheet.paste(a, (0, 22))
        sheet.paste(b, (a.width, 22))
        d.text((4, 4), f"CYCLES (real normal + material maps) {livery} e{engine} {n}: before (installed)", fill="black")
        d.text((a.width + 4, 4), "after (staged)", fill="black")
        name = f"r6_cycles_e{engine}_{n}" + ("" if livery == LIVERIES["Emirates"] else "_" + livery.split()[0].lower()) + ".png"
        sheet.save(out / name)
        done[n] = name
        print("cycles", name, flush=True)
    return done


def main(install, stage, out, mode="all"):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    if mode in ("preview", "all"):
        r5.main(install, stage, out, PREVIEW_ONLY)
        for p in out.glob("r5_*.png"):
            p.rename(out / p.name.replace("r5_", "r6_prev_", 1))
    if mode in ("cycles", "all"):
        for e in (2, 1):
            cycles_sheet(install, stage, out, e, LIVERIES["Emirates"], ["side", "door", "vent", "dots", "lip_joint", "lower_panel", "intake_side", "fan_front", "fan_hub",
                                                                         "fan_oblique", "exhaust", "exh_side", "pylon_rear", "pylon_rear2", "pylon_side"])
        cycles_sheet(install, stage, out, 2, LIVERIES["Etihad"], ["side", "door", "fan_front", "fan_hub", "pylon_rear"])


if __name__ == "__main__":
    a = sys.argv[1:]
    main(a[0], a[1], a[2], a[3] if len(a) > 3 else "all")
