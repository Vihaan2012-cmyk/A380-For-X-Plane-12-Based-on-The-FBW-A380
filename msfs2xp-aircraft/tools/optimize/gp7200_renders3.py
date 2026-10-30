"""usage: gp7200_renders3.py <installed objects dir> <stage objects dir> <out dir>

Round-3 before / after / reference sheets: exhaust plug, core nozzle, lip bottom, keel (split_low), the pylon-to-nacelle junction on engines 1 and 2,
and the Emirates logo at close range, each next to the closest reference photo."""

import shutil
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gp7200_closeups as cu  # noqa: E402
import gp7200_renders as rr  # noqa: E402
import gp7200_renders2 as r2  # noqa: E402
from gp7200_geom import frames  # noqa: E402

FN = frames()
BASE_VIEWS = r2.views
LIV = "Emirates A6-EDY"


def views(e):
    f = FN[e]
    P = lambda a, b, s: [float(v) for v in f.point(np.array([a, b, s]))]
    v = BASE_VIEWS(e)
    v["pylon"] = dict(eye=P(-9.0, 2.3, 2.6), at=P(0, 2.0, 2.6), fov=22)
    v["pylon_front"] = dict(eye=P(-5.5, 3.2, -1.5), at=P(0, 1.9, 1.2), fov=30)
    v["logo"] = dict(eye=P(-4.6, 0.1, 1.2), at=P(-1.8, 0.0, 1.2), fov=17)
    return v


def one(objects, tag, engine, name, refs=None):
    r2.views = views
    return r2.one(objects, tag, LIV, engine, name, refs or {f"fbw/anim/engine_bay_{engine}": 0})


def main(install, stage, out):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    jobs = [("plug", 2, "exhaust"), ("nozzle", 2, "exhaust"), ("lip_bottom", 2, "front34"), ("split_low", 2, "side"), ("pylon", 2, "side"), ("pylon", 1, "side"),
            ("pylon_front", 2, "front34"), ("logo", 2, "side")]
    for name, e, photo in jobs:
        b = one(install, "before", e, name)
        a = one(stage, "after", e, name)
        r2.pair(b, a, rr.find_photo(rr.PHOTO[photo]), f"{LIV.split()[0]} e{e} {name}", out / f"r3_e{e}_{name}.png")
        print("pair", name, e)
    ev = Path("E:/fbw-build/gp7200-model/work/dbg/dbg_synth.png")
    if ev.is_file():
        shutil.copy2(ev, out / "evidence_synthetic_cone_in_viewer.png")


if __name__ == "__main__":
    main(*sys.argv[1:4])
