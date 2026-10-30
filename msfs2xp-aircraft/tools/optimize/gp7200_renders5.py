"""usage: gp7200_renders5.py <installed objects dir> <stage objects dir> <out dir> [job names...]

Round-5 before / after / reference sheets for engines 1 and 2: the full views (front34, rear34, side, intake, exhaust), the raised boxes on the aft
cowl, the louvre, the fan hub and spinner, and the pylon-to-nacelle gap from the side, above and low behind; plus one non-Emirates livery."""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gp7200_renders as rr  # noqa: E402
import gp7200_renders2 as r2  # noqa: E402
from gp7200_geom import frames  # noqa: E402

FN = frames()
LIV = "Emirates A6-EDY"
LIV2 = "Etihad A6-API"
BASE = r2.views
PHOTOS = {"side": "side", "front34": "front34", "rear34": "rear34", "intake": "intake", "exhaust": "exhaust"}


def views(e):
    f = FN[e]
    P = lambda a, b, s: [float(v) for v in f.point(np.array([a, b, s]))]
    side = -1.0 if e in (1, 2) else 1.0
    v = BASE(e)
    v["rear34"] = rr.views(e)["rear34"]
    v["box_all"] = dict(eye=P(side * 7.5, 0.0, 3.5), at=P(side * 1.7, 0.0, 3.5), fov=18)
    v["box_up"] = dict(eye=P(side * 4.2, 3.6, 3.5), at=P(side * 1.26, 1.1, 3.5), fov=10)
    v["box_mid"] = dict(eye=P(side * 5.6, 0.0, 3.5), at=P(side * 1.69, 0.0, 3.5), fov=10)
    v["box_low"] = dict(eye=P(side * 3.4, -4.1, 3.5), at=P(side * 1.09, -1.33, 3.5), fov=10)
    v["louvre"] = dict(eye=P(side * 3.0, -4.6, -0.7), at=P(side * 1.0, -1.5, -0.7), fov=12)
    v["hub"] = dict(eye=P(0.15, 0.05, -5.0), at=P(0, 0, 0.3), fov=7)
    v["hub_wide"] = dict(eye=P(0.4, 0.2, -6.5), at=P(0, 0, 0.5), fov=14)
    v["gap_side"] = dict(eye=P(side * 9, 1.2, 3.6), at=P(0, 1.9, 3.6), fov=12)
    v["gap_top"] = dict(eye=P(side * 0.5, 8, 2.5), at=P(0, 1.8, 3.8), fov=16)
    v["gap_lowrear"] = dict(eye=P(side * 4, -0.5, 9), at=P(0, 1.7, 4), fov=20)
    v["gap_rear"] = dict(eye=P(side * 2.5, 3.2, 16), at=P(0, 2.1, 3.5), fov=16)
    return v


def photo_for(name):
    key = {"front34": "front34", "rear34": "rear34", "side": "side", "intake": "intake", "exhaust": "exhaust", "hub": "front34", "hub_wide": "front34",
           "louvre": "side"}.get(name, "side")
    return rr.find_photo(rr.PHOTO[key])


JOBS = [(n, e, LIV) for e in (2, 1) for n in ("front34", "rear34", "side", "intake", "exhaust")]
JOBS += [(n, 2, LIV) for n in ("box_all", "box_up", "box_mid", "box_low")]
JOBS += [("box_all", 3, LIV)]
JOBS += [(n, e, LIV) for e in (2, 1) for n in ("louvre", "hub", "hub_wide", "gap_side", "gap_top", "gap_lowrear", "gap_rear")]
JOBS += [(n, 2, LIV2) for n in ("front34", "rear34", "hub", "box_all")]


def main(install, stage, out, only=None):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    for name, e, liv in JOBS:
        tag = f"{name}_e{e}" + ("" if liv == LIV else "_" + liv.split()[0].lower())
        if only and not any(o in tag for o in only):
            continue
        r2.views = views
        refs = {f"fbw/anim/engine_bay_{e}": 0}
        b = r2.one(install, "before5", liv, e, name, refs)
        a = r2.one(stage, "after5", liv, e, name, refs)
        r2.pair(b, a, photo_for(name), f"{liv.split()[0]} e{e} {name}", out / f"r5_{tag}.png")
        print("pair", tag, flush=True)


if __name__ == "__main__":
    a = sys.argv[1:]
    main(a[0], a[1], a[2], a[3:] or None)
