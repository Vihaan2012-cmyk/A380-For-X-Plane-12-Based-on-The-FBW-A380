"""Fuel leak effects: mist, drip and plume particles in a380_engdetach_fire.pss, and one emitter group per leak site in a380_brake_fire.obj, shown by fbw/damage/fuel_leak_N (plugin fuel_leak_fx.rs)."""
import re
import sys
from pathlib import Path

PSS = "a380_engdetach_fire.pss"
OBJ = "a380_brake_fire.obj"

# fuel_leak_N order: the 11 network tanks, then the 4 pylon stubs.
SITES = [
    (-33.0, 2.15, 9.0),
    (-23.5, 1.5, 4.5),
    (-20.0, 1.5, 2.6),
    (-9.5, 0.1, -3.0),
    (-12.8, 0.62, -1.0),
    (12.8, 0.62, -1.0),
    (9.5, 0.1, -3.0),
    (20.0, 1.5, 2.6),
    (23.5, 1.5, 4.5),
    (33.0, 2.15, 9.0),
    (-6.0, 3.8, 31.5),
    (-25.5, 1.42, 2.0),
    (-14.9, 0.68, -5.5),
    (14.9, 0.68, -5.5),
    (25.5, 1.42, 2.0),
]


def table(name, rows, interp="CUBIC_AVG"):
    out = [name, f"INTERP_MODE {interp}"]
    for row in rows:
        out.append("\t" + "\t".join(f"{v:.6f}" for v in row))
    out.append("END_KEYFRAME_TABLE")
    return out


def particle(name, max_particles, size, alpha, tint, gravity, turbulence, drag, shade=0.8):
    return [
        "PARTICLE",
        f" NAME {name}",
        f" MAX_PARTICLES {max_particles}",
        " BILLBOARD_MODE BILLBOARD",
        " BLEND_MODE BLEND",
        " TEX_CELLS_X 8",
        " TEX_CELLS_Y 8",
        " ANIM_CELL_START 54",
        " ANIM_CELL_COUNT 1",
        " ANIM_CELL_REPEAT 1",
        " ANIM_CELL_RANDOM 0",
        *table("ANIM_CELL_KF", [(0, 0), (1, 1)], "LINEAR"),
        *table("SIZE_CURVE", size),
        *table("ALPHA_CURVE", alpha),
        *table("LENGTH_CURVE", [(0, 0.1), (1, 0.1)]),
        f" DIFFUSE {shade:.6f}",
        f" AMBIENT {shade:.6f}",
        *table("EMISSIVE", [(0, 0), (1, 0)]),
        *table("TINT", tint),
        *table("GRAVITY_CURVE", [(0, gravity), (1, gravity)]),
        *table("TURBULENCE", [(0, turbulence), (1, turbulence)]),
        *table("DRAG_CURVE", [(0, drag), (1, drag)]),
        *table("SPIN_CURVE", [(0, 0), (1, 0)]),
        "END_PARTICLE",
    ]


def emitter(name, kind, rate, speed, spread, size, alpha, life):
    both = lambda lo, hi: [(0, lo, hi), (1, lo, hi)]
    return [
        "EMITTER",
        f" NAME {name}",
        " EMIT_MODE STREAM",
        "SUB_EMITTER",
        f" PARTICLE_TYPE {kind}",
        *table("EMIT_RATE", both(*rate)),
        *table("INITIAL_SPEED", both(*speed)),
        *table("ROTATION_SPEED", both(1, 1)),
        *table("INITIAL_HEADING", both(-180, 180)),
        *table("INITIAL_PITCH", both(-90, 90)),
        *table("DX", both(-spread, spread)),
        *table("DY", both(0, 0)),
        *table("DZ", both(-spread, spread)),
        *table("DLON", both(0, 0)),
        *table("DLAT", both(0, 0.5)),
        *table("INITIAL_ROTATION", both(-90, 90)),
        *table("INITIAL_SIZE", both(*size)),
        *table("INITIAL_ALPHA", both(*alpha)),
        *table("TIME_TO_LIVE", both(*life)),
        "END_SUB_EMITTER",
        "END_EMITTER",
    ]


def patch_pss(path):
    text = Path(path).read_bytes().decode("utf-8")
    text = re.sub(r"PARTICLE\r?\n NAME Fuel .*?END_PARTICLE\r?\n", "", text, flags=re.S)
    text = re.sub(r"EMITTER\r?\n NAME Fuel_.*?END_EMITTER\r?\n", "", text, flags=re.S)
    first = len(re.findall(r"^PARTICLE\r?$", text, flags=re.M))
    pale = [(0, 0.85, 0.87, 0.88), (1, 0.92, 0.93, 0.94)]
    particles = [
        particle("Fuel Mist", 1000, [(0, 1), (1, 6)], [(0, 0), (0.15, 1), (1, 0)], pale, 0.12, 0.3, 0.8),
        particle("Fuel Drip", 1000, [(0, 1), (1, 1.6)], [(0, 1), (0.8, 0.8), (1, 0)], pale, 0.9, 0.0, 0.15, 1.0),
        particle("Fuel Plume", 2000, [(0, 1), (1, 7)], [(0, 0), (0.1, 1), (1, 0)], pale, 0.02, 0.2, 0.8, 1.0),
    ]
    emitters = [
        emitter("Fuel_Mist", first, (15, 25), (0, 0.4), 0.25, (0.2, 0.5), (0.06, 0.12), (2.5, 4)),
        emitter("Fuel_Drip", first + 1, (55, 75), (0, 0.3), 0.12, (0.05, 0.11), (0.55, 0.8), (1.0, 1.5)),
        emitter("Fuel_Plume", first + 2, (130, 160), (0, 0.5), 0.15, (0.35, 0.7), (0.18, 0.28), (2.5, 3.5)),
    ]
    add_particles = "\r\n".join(l for p in particles for l in p) + "\r\n"
    add_emitters = "\r\n".join(l for e in emitters for l in e) + "\r\n"
    last = [m.end() for m in re.finditer(r"END_PARTICLE\r?\n", text)][-1]
    text = text[:last] + add_particles + text[last:]
    effect = re.search(r"^EFFECT\r?$", text, flags=re.M).start()
    text = text[:effect] + add_emitters + text[effect:]
    Path(path).write_bytes(text.encode("utf-8"))


def group(n, lo, hi, emitters):
    ref = f"fbw/damage/fuel_leak_{n}"
    return ["ANIM_begin", f"ANIM_hide -1000000 1000000 {ref}", f"ANIM_show {lo} {hi} {ref}", *emitters, "ANIM_end", ""]


def line(name, x, y, z):
    return f"EMITTER {name} {x:.3f} {y:.3f} {z:.3f} 0 0 0"


def site_groups(n, site):
    x, y, z = site
    out = []
    out += group(n, 0.5, 2.5, [line("Fuel_Mist", x, y, z), line("Fuel_Drip", x, y, z)])
    out += group(n, 1.5, 2.5, [line("Fuel_Mist", x + 0.6, y, z + 0.3), line("Fuel_Drip", x - 0.4, y, z + 0.3), line("Fuel_Drip", x + 0.3, y, z - 0.4)])
    out += group(n, 2.5, 4.5, [line("Fuel_Plume", x, y, z)])
    out += group(n, 3.5, 4.5, [line("Fuel_Plume", x + 0.5, y, z + 0.4), line("Fuel_Plume", x - 0.5, y, z - 0.4)])
    return out


def patch_obj(path):
    text = Path(path).read_text(encoding="utf-8").replace("\r\n", "\n")
    text = re.sub(r"ANIM_begin\nANIM_hide -1000000 1000000 fbw/damage/fuel_leak_\d+\n.*?ANIM_end\n\n?", "", text, flags=re.S)
    lines = text.rstrip("\n").split("\n") + [""]
    for n, site in enumerate(SITES):
        lines += site_groups(n, site)
    Path(path).write_text("\n".join(lines), encoding="utf-8", newline="\n")


def main(objects_dir):
    d = Path(objects_dir)
    patch_pss(d / PSS)
    patch_obj(d / OBJ)
    print(f"fuel leak: {len(SITES)} sites in {d / OBJ}")


if __name__ == "__main__":
    main(sys.argv[1])
