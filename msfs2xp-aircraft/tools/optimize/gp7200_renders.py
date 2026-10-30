"""usage: gp7200_renders.py <stage objects dir> <out dir> [engines]

Preview renders (E:/fbw-gse/preview) of the staged GP7200 engines 2 and 1: side, front and rear three-quarter, intake and exhaust looking
in, cowl doors open and the separated engine, each placed next to the closest reference photo in refs/gp7200/commons."""

import json
import os
import re
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gp7200_geom import frames  # noqa: E402

PV = "E:/fbw-gse/preview"
REFS = Path("refs/gp7200/commons")
LIVERY = "D:/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12/liveries/Emirates A6-EDY/objects"
FN = frames()
OBJECTS = ["a380_exterior_022_gp.obj", "a380_exterior_020_gp.obj", "a380_exterior_030_gp.obj", "a380_exterior_031_gp.obj", "a380_exterior_001_gp.obj", "a380_engcore_gp.obj"]
RIGHT = ["a380_exterior_023_gp.obj", "a380_exterior_021_gp.obj", "a380_exterior_030_gp.obj", "a380_exterior_031_gp.obj", "a380_exterior_001_gp.obj", "a380_engcore_gp.obj"]
PHOTO = {
    "side": "ILA_2010_-_gravitat-OFF_-_Airbus_380_079.jpg",
    "front34": "Airbus_A380_(F-WWDD)_at_Domodedovo_International_Airport_(248-27).jpg",
    "rear34": "Airbus_A380_(F-WWDD)_at_Domodedovo_International_Airport_(248-28).jpg",
    "intake": "A380_ENGINE.jpg",
    "exhaust": "Airbus_A380_03_(4825832887).jpg",
    "doors_open": "Airbus_Lagard\u00e8re_-_GP7200_engine_MSN108_(1).JPG",
    "separated": "GP7000_MTU-Museum_20231112_03w.jpg",
}


def find_photo(name):
    key = re.sub(r"[^A-Za-z0-9]", "", name)
    for p in REFS.iterdir():
        if re.sub(r"[^A-Za-z0-9]", "", p.name) == key:
            return p
    return None


def views(e):
    f = FN[e]
    P = lambda a, b, s: [float(v) for v in f.point(np.array([a, b, s]))]
    return {
        "side": dict(eye=P(-30, 0, 2.5), at=P(0, 0, 2.5), fov=18),
        "front34": dict(eye=P(-8.5, 2.2, -11), at=P(0, -0.2, 1.2), fov=32),
        "rear34": dict(eye=P(-8, 2.0, 15), at=P(0, 0, 3.0), fov=32),
        "intake": dict(eye=P(0, 0, -15), at=P(0, 0, 0.4), fov=24),
        "exhaust": dict(eye=P(0, 0, 14), at=P(0, 0, 5.5), fov=30),
        "doors_open": dict(eye=P(-8, 3.5, -10), at=P(0, 1.2, 1.5), fov=36),
        "separated": dict(eye=P(-9, 1.0, -11), at=P(0, -3.4, 2.5), fov=36),
    }


def place(src, dst, offset):
    lines = Path(src).read_text(encoding="utf-8", errors="replace").split("\n")
    out = []
    for l in lines:
        if l.startswith("VT "):
            p = l.split()
            v = [float(x) + o for x, o in zip(p[1:4], offset)]
            l = "VT " + " ".join(f"{x:.5f}" for x in v) + " " + " ".join(p[4:9])
        out.append(l)
    Path(dst).write_text("\n".join(out), encoding="utf-8", newline="\n")


def separated_objects(stage, e, work):
    manifest = json.loads((stage / "a380_engdetach.json").read_text(encoding="utf-8"))
    gp = manifest["gp"]
    ent = gp["engines"][str(e - 1)]
    drop = np.array([0.0, -3.4, 2.5])
    out = []
    for name in ent["objects"]:
        if "engdress" in name and not (stage / name).is_file():
            continue
        dst = work / name
        place(stage / name, dst, np.array(ent["centre"]) + drop)
        out.append(str(dst))
    cow = gp["cowls"].get(str(e - 1), {})
    for k, piece in enumerate(cow.get("pieces", [])):
        sh = np.array(piece["centre"]) + drop + np.array([(-1.2, 0.5, -1.0), (-2.2, 0.3, 0.0), (2.2, 0.3, 0.0)][k % 3])
        for name in piece["objects"]:
            dst = work / name
            place(stage / name, dst, sh)
            out.append(str(dst))
    return out


def render_one(name, objs, view, refs, out_dir, size=(1000, 640)):
    work = Path("E:/fbw-build/gp7200-model/work/pv") / name
    work.mkdir(parents=True, exist_ok=True)
    spec = dict(objects=objs, datarefs={"fbw/engine/gp7200": 1, "fbw/anim/eye_in_flight_deck": 0, **refs}, views=[view], size=list(size))
    (work / "spec.json").write_text(json.dumps(spec))
    env = dict(os.environ, TEX_OVERRIDE=LIVERY)
    subprocess.run([sys.executable, f"{PV}/obj_preview.py", str(work / "out"), str(work / "spec.json")], check=True, stdout=subprocess.DEVNULL, env=env)
    png = out_dir / f"{name}.png"
    subprocess.run(["node", f"{PV}/cdp_tabs.mjs", f"file:///{(work / 'out' / 'index.html').as_posix()}", f"{PV}/wait.js", str(png), str(size[0]), str(size[1])], check=True, timeout=240, stdout=subprocess.DEVNULL)
    return png


def compose(png, photo, label, out):
    im = Image.open(png).convert("RGB")
    h = im.height
    if photo is not None:
        ph = Image.open(photo).convert("RGB")
        ph = ph.resize((int(ph.width * h / ph.height), h))
        sheet = Image.new("RGB", (ph.width + im.width, h + 22), "white")
        sheet.paste(ph, (0, 22))
        sheet.paste(im, (ph.width, 22))
        d = ImageDraw.Draw(sheet)
        d.text((4, 4), f"reference: {photo.name}", fill="black")
        d.text((ph.width + 4, 4), label, fill="black")
    else:
        sheet = im
    sheet.save(out)


def main(stage, out_dir, engines="2,1"):
    stage, out_dir = Path(stage), Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    for e in [int(x) for x in engines.split(",")]:
        base = (OBJECTS if e < 3 else RIGHT)
        objs = [str(stage / n) for n in base]
        vs = views(e)
        refs = {f"fbw/anim/engine_bay_{e}": 0}
        for key, v in vs.items():
            r = dict(refs)
            use = objs
            if key == "doors_open":
                r.update({f"fbw/anim/cowl_open_{e}": 1, f"fbw/anim/engine_bay_{e}": 1})
            if key == "separated":
                r.update({f"sim/operation/failures/rel_engsep{e - 1}": 6})
                work = Path("E:/fbw-build/gp7200-model/work/sep") / f"e{e}"
                work.mkdir(parents=True, exist_ok=True)
                use = [str(stage / n) for n in base if "engcore" not in n] + separated_objects(stage, e, work)
            if key == "intake" or key == "exhaust":
                pass
            png = render_one(f"e{e}_{key}", use, v, r, out_dir)
            compose(png, find_photo(PHOTO[key]), f"A380 for X-Plane 12, GP7200 engine {e}, {key}", out_dir / f"e{e}_{key}_vs_photo.png")
            print("rendered", png.name)


if __name__ == "__main__":
    main(*sys.argv[1:4])
