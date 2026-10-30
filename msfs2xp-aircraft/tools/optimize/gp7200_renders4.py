"""usage: gp7200_renders4.py <installed objects dir> <stage objects dir> <out dir>

Round-4 before / after sheets of the smoothly seated pylon: side views of engines 2 and 1, the front three-quarter view of engine 2, and a top-down
view of the pylon on engines 1 and 2."""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gp7200_renders as rr  # noqa: E402
import gp7200_renders2 as r2  # noqa: E402
import gp7200_renders3 as r3  # noqa: E402
from gp7200_geom import frames  # noqa: E402

FN = frames()


def views(e):
    f = FN[e]
    P = lambda a, b, s: [float(v) for v in f.point(np.array([a, b, s]))]
    v = r3.views(e)
    v["pylon_top"] = dict(eye=P(-0.5, 9.0, 1.0), at=P(0, 1.8, 2.6), fov=24)
    return v


def one(objects, tag, engine, name):
    r2.views = views
    return r2.one(objects, tag, r3.LIV, engine, name, {f"fbw/anim/engine_bay_{engine}": 0})


def main(install, stage, out):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    jobs = [("pylon", 2, "side"), ("pylon", 1, "side"), ("pylon_front", 2, "front34"), ("pylon_top", 1, "side"), ("pylon_top", 2, "side")]
    for name, e, photo in jobs:
        b = one(install, "before4", e, name)
        a = one(stage, "after4", e, name)
        r2.pair(b, a, rr.find_photo(rr.PHOTO[photo]), f"{r3.LIV.split()[0]} e{e} {name}", out / f"r4_e{e}_{name}.png")
        print("pair", name, e, flush=True)


if __name__ == "__main__":
    main(*sys.argv[1:4])
