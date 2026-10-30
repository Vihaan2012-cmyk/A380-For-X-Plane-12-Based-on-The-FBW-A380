"""usage: gp7200_renders2.py <installed objects dir> <stage objects dir> <out dir> [--only core|seams|dress]

Before (installed) / after (staged) renders of the GP7200 polish: cowl seams, lip, barrel, plug and core nozzle on five liveries (Emirates A6-EDY,
Air France F-HPJD, Etihad A6-API, Qatar A7-APC, Korean HL7621) at close range, and the core (intake and exhaust views, doors open, separated)
and the fan-case dressing with the doors open, each pair beside the closest reference photo from refs/gp7200/commons."""

import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gp7200_closeups as cu  # noqa: E402
import gp7200_renders as rr  # noqa: E402
from gp7200_geom import frames  # noqa: E402

FN = frames()
LIVERIES = ["Emirates A6-EDY", "Air France F-HPJD", "Etihad A6-API", "Qatar Airways A7-APC", "Korean Air HL7621"]
SEAM_VIEWS = ["side", "split_side", "lip_bottom"]
EXTRA_VIEWS = ["split_top", "split_low", "lip_front", "barrel", "plug", "nozzle", "cowl_aft"]


def views(e):
    f = FN[e]
    P = lambda a, b, s: [float(v) for v in f.point(np.array([a, b, s]))]
    v = cu.views(e)
    side = -1.0 if e in (1, 2) else 1.0
    v["side"] = dict(eye=P(side * 7.0, 0.3, 1.3), at=P(0, 0.0, 1.3), fov=26)
    v["intake"] = dict(eye=P(0.4, 0.2, -6.5), at=P(0, 0.0, 0.6), fov=34)
    v["exhaust"] = dict(eye=P(0.3, 0.3, 10.0), at=P(0, 0.0, 5.6), fov=28)
    v["core_open"] = dict(eye=P(side * 5.2, 2.2, -5.0), at=P(0, 0.0, 2.6), fov=36)
    v["dress_open"] = dict(eye=P(side * 6.5, 3.0, -5.5), at=P(0, 0.3, 1.3), fov=38)
    v["dress_close"] = dict(eye=P(side * 3.4, 1.6, -3.4), at=P(0, 0.0, 1.5), fov=40)
    v["separated"] = rr.views(e)["separated"]
    return v


def one(objects, tag, livery, engine, name, refs, extra=None):
    rr.LIVERY = str(cu.AIRCRAFT / "liveries" / livery / "objects") if livery else str(cu.AIRCRAFT / "objects")
    base = cu.OBJ_L if engine < 3 else cu.OBJ_R
    objs = [str(Path(objects) / n) for n in base if (Path(objects) / n).is_file()]
    if extra:
        objs = extra(objs)
    key = f"{tag}_{(livery or 'base').split()[0]}_e{engine}_{name}"
    outd = Path("E:/fbw-build/gp7200-model/work/pvout")
    outd.mkdir(parents=True, exist_ok=True)
    return rr.render_one(key, objs, views(engine)[name], refs, outd)


def pair(before, after, photo, label, out):
    a, b = Image.open(before).convert("RGB"), Image.open(after).convert("RGB")
    h = a.height
    parts = [a, b]
    names = ["before (installed)", "after (staged)"]
    if photo is not None:
        ph = Image.open(photo).convert("RGB")
        ph = ph.resize((int(ph.width * h / ph.height), h))
        parts.append(ph)
        names.append(f"reference: {photo.name}")
    sheet = Image.new("RGB", (sum(p.width for p in parts), h + 22), "white")
    d = ImageDraw.Draw(sheet)
    x = 0
    for p, n in zip(parts, names):
        sheet.paste(p, (x, 22))
        d.text((x + 4, 4), f"{label}: {n}", fill="black")
        x += p.width
    sheet.save(out)


def seams(install, stage, out, engine=2):
    for lv in LIVERIES:
        for name in SEAM_VIEWS + (EXTRA_VIEWS if lv == LIVERIES[0] else []):
            refs = {f"fbw/anim/engine_bay_{engine}": 0}
            b = one(install, "before", lv, engine, name, refs)
            a = one(stage, "after", lv, engine, name, refs)
            key = "front34" if name in ("lip_bottom", "lip_front", "barrel") else "exhaust" if name in ("plug", "nozzle", "cowl_aft") else "side"
            photo = rr.find_photo(rr.PHOTO[key])
            pair(b, a, photo, f"{lv.split()[0]} e{engine} {name}", out / f"seam_{lv.split()[0]}_e{engine}_{name}.png")
            print("pair", lv, name)


def core(install, stage, out, engine=2):
    shots = [("intake", {}, "intake"), ("exhaust", {}, "exhaust"), ("core_open", {f"fbw/anim/cowl_open_{engine}": 1, f"fbw/anim/engine_bay_{engine}": 1}, "doors_open"),
             ("separated", {f"sim/operation/failures/rel_engsep{engine - 1}": 6, f"fbw/anim/engine_bay_{engine}": 0}, "separated"),
             ("dress_open", {f"fbw/anim/cowl_open_{engine}": 1, f"fbw/anim/engine_bay_{engine}": 1}, "doors_open"),
             ("dress_close", {f"fbw/anim/cowl_open_{engine}": 1, f"fbw/anim/engine_bay_{engine}": 1}, "separated")]
    for name, refs, photo_key in shots:
        res = {}
        for tag, objects in (("before", install), ("after", stage)):
            extra = None
            view = name
            if name == "separated":
                def extra(objs, objects=objects, tag=tag):
                    work = Path("E:/fbw-build/gp7200-model/work/sep2") / tag
                    work.mkdir(parents=True, exist_ok=True)
                    keep = [o for o in objs if "engcore" not in o and "engdress" not in o]
                    return keep + rr.separated_objects(Path(objects), engine, work)
                view = "separated"
            if name in ("separated",):
                v = views(engine)
            res[tag] = one(objects, tag, "Emirates A6-EDY", engine, view, refs, extra)
        pair(res["before"], res["after"], rr.find_photo(rr.PHOTO[photo_key]), f"core e{engine} {name}", out / f"core_e{engine}_{name}.png")
        print("pair core", name)


def main(install, stage, out, only=None):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    if only in (None, "seams"):
        seams(Path(install), Path(stage), out)
    if only in (None, "core", "dress"):
        core(Path(install), Path(stage), out)


if __name__ == "__main__":
    a = sys.argv[1:]
    main(a[0], a[1], a[2], a[a.index("--only") + 1] if "--only" in a else None)
