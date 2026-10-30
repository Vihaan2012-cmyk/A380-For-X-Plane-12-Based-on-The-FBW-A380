"""usage: gp7200_closeups.py <objects dir> <out dir> <tag> [--livery NAME ...] [--views a,b,...] [--engine N]

Close-up renders of the GP7200 skin through the E:/fbw-gse/preview viewer with a livery's engine textures: the cowl split, the inlet lip,
the barrel, the plug and the core nozzle, plus the doors-open, separated-core and dressing views. Output <out dir>/<tag>_<livery>_<view>.png."""

import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gp7200_renders as rr  # noqa: E402
from gp7200_geom import frames  # noqa: E402

AIRCRAFT = Path("D:/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12")
FN = frames()
OBJ_L = ["a380_exterior_022_gp.obj", "a380_exterior_020_gp.obj", "a380_exterior_030_gp.obj", "a380_exterior_031_gp.obj", "a380_exterior_001_gp.obj", "a380_engcore_gp.obj", "a380_engdress_gp.obj"]
OBJ_R = ["a380_exterior_023_gp.obj", "a380_exterior_021_gp.obj", "a380_exterior_030_gp.obj", "a380_exterior_031_gp.obj", "a380_exterior_001_gp.obj", "a380_engcore_gp.obj", "a380_engdress_gp.obj"]


def views(e):
    f = FN[e]
    P = lambda a, b, s: [float(v) for v in f.point(np.array([a, b, s]))]
    side = -1.0 if e in (1, 2) else 1.0
    return {
        "split_side": dict(eye=P(side * 4.6, 0.2, 2.4), at=P(side * 1.8, 0.0, 2.4), fov=22),
        "split_top": dict(eye=P(side * 2.4, 3.6, 3.4), at=P(0, 1.8, 2.44), fov=26),
        "split_low": dict(eye=P(side * 3.2, -3.2, 1.5), at=P(side * 0.8, -1.9, 2.44), fov=28),
        "lip_bottom": dict(eye=P(side * 1.6, -3.0, -4.0), at=P(0, -1.75, -0.9), fov=22),
        "lip_front": dict(eye=P(side * 1.2, 0.4, -5.5), at=P(side * 0.2, -0.6, -0.9), fov=34),
        "barrel": dict(eye=P(-side * 0.2, 0.1, -3.2), at=P(side * 0.4, 0.3, -0.3), fov=48),
        "plug": dict(eye=P(0.5, 0.4, 11.0), at=P(0, 0, 6.3), fov=16),
        "nozzle": dict(eye=P(side * 2.4, 0.6, 9.0), at=P(0, 0, 5.4), fov=24),
        "cowl_aft": dict(eye=P(side * 5.0, -0.4, 6.4), at=P(side * 1.4, 0.0, 4.8), fov=26),
        "side": dict(eye=P(side * 30, 0, 2.5), at=P(0, 0, 2.5), fov=18),
        "front34": dict(eye=P(side * 8.5, 2.2, -11), at=P(0, -0.2, 1.2), fov=32),
        "intake": dict(eye=P(0, 0, -15), at=P(0, 0, 0.4), fov=24),
        "exhaust": dict(eye=P(0, 0, 14), at=P(0, 0, 5.5), fov=30),
    }


def render(objects, out, tag, livery, engine, names, only=None, size=(1100, 700)):
    objects, out = Path(objects), Path(out)
    out.mkdir(parents=True, exist_ok=True)
    rr.LIVERY = str(AIRCRAFT / "liveries" / livery / "objects") if livery else str(AIRCRAFT / "objects")
    base = OBJ_L if engine < 3 else OBJ_R
    objs = [str(objects / n) for n in base if (objects / n).is_file()]
    vs = views(engine)
    for name, v in vs.items():
        if only and name not in only:
            continue
        refs = {f"fbw/anim/engine_bay_{engine}": 0}
        key = f"{tag}_{livery.split()[0] if livery else 'base'}_e{engine}_{name}"
        png = rr.render_one(key, objs, v, refs, out, size)
        names.append(str(png))
        print("rendered", png.name)


def main(objects, out, tag, liveries, only, engine):
    names = []
    for lv in liveries or [None]:
        render(objects, out, tag, lv, engine, names, only)
    return names


if __name__ == "__main__":
    a = sys.argv[1:]
    lv = [a[i + 1] for i, x in enumerate(a) if x == "--livery"]
    only = a[a.index("--views") + 1].split(",") if "--views" in a else None
    eng = int(a[a.index("--engine") + 1]) if "--engine" in a else 2
    main(a[0], a[1], a[2], lv, only, eng)
