"""Builds the A380's .acf from a blank, without Laminar's A330 file as its base.

Every property of the current A380 .acf is rebuilt from, in order:
  1. ours: values the port set itself (anything that differs from the A330 it started from);
  2. structure: array sizes and counts;
  3. regenerated: the hidden wing mesh (`_geo_xyz`), rib chords and Plane Maker's lock flags,
     computed from our own planform or zeroed;
  4. trivial: 0, 1 and -1, which say nothing about the A330 (switches, unused slots);
  5. airfoils: X-Plane's own library airfoils, which every X-Plane install has, instead of
     copies of the A330's;
  6. decided: an A380 value with its source (acf_decisions.DECIDED), ahead of every rule;
  7. owned: an X-Plane system the plugin replaces (acf_decisions.OWNED), set to the value most of
     Laminar's aircraft share for that property;
  8. blank: Plane Maker's default, the value most of X-Plane's own Laminar aircraft share (at
     least `BLANK_SHARE` of them) for that property at any index, where it is also the current value.
Whatever is left is carried over unchanged and listed in the report as still to decide:
inferring those from other aircraft gave nonsense (a Boeing manufacturer, 24-blade propellers,
a 6 s gear cycle), so each needs an A380 value of its own.

python -I tools/acf_build.py --xplane "D:/X-Plane 12" --out E:/fbw-build/acf
"""
import argparse
import collections
import json
import math
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from acf_decisions import DECIDED, OWNED, RELAYS_ON_ANY_BUS

LAMINAR = [
    'Aero-Works Aerolite 103', 'BETA Technologies Alia-250', 'Beechcraft Baron 58', 'Beechcraft King Air C90B',
    'Boeing 737-800', 'Cessna 172 SP', 'Cessna Citation X', 'Cirrus SR22', 'Cirrus Vision SF50', 'Grumman F-14 Tomcat',
    'Lancair Evolution', 'McDonnell Douglas F-4 Phantom II', 'McDonnell Douglas MD-82', 'Piper PA-18 Super Cub',
    'Robinson R22 Beta II', 'Schleicher ASK 21', 'Sikorsky S-76', 'Stinson L-5 Sentinel', "Van's Aircraft RV-10",
]
BLANK_SHARE = 0.6
TRIVIAL = {'0', '1', '-1', '0.000000000', '-0.000000000', '1.000000000', '-1.000000000', ''}
FT = 3.28084
AIRFOILS = {
    'Airbus Root (high subsonic).afl': 'Boeing Root (high subsonic).afl',
    'Airbus Mid (high subsonic).afl': 'Boeing Mid (high subsonic).afl',
    'Airbus Tip (high subsonic).afl': 'Boeing Tip (high subsonic).afl',
}


def read(path: Path):
    head, props = [], {}
    with open(path, encoding='latin-1', newline='') as f:
        lines = f.read().split('\n')
    i = 0
    while i < len(lines) and not lines[i].startswith('PROPERTIES_BEGIN'):
        head.append(lines[i])
        i += 1
    tail_at = None
    for j in range(i + 1, len(lines)):
        line = lines[j]
        if line.startswith('P '):
            k, _, v = line[2:].partition(' ')
            props[k] = v
        elif line.startswith('PROPERTIES_END'):
            tail_at = j
            break
    tail = lines[tail_at:] if tail_at is not None else []
    return head, props, tail


def acfs(root: Path, names):
    out = {}
    for n in names:
        for p in sorted((root / 'Aircraft' / 'Laminar Research' / n).glob('*.acf')):
            if '_AI' in p.stem:
                continue
            out[n] = read(p)[1]
            break
    return out


def number(v: str):
    try:
        return float(v)
    except ValueError:
        return None


def fmt_like(template: str, x: float) -> str:
    if '.' not in template and number(template) is not None:
        return str(int(round(x)))
    return f'{x:.9f}'


def wing_mesh(props, w, key):
    """One `_geo_xyz/i,j,k` point of wing `w`'s hidden mesh, feet, from its planform: span stations
    along the quarter-chord line, chordwise points over the top (leading to trailing edge) and back
    under the bottom, a 12 % to 9 % thick section."""
    i, j, k = (int(x) for x in key.rsplit('/', 1)[1].split(','))
    g = lambda name, d=0.0: number(props.get(f'_wing/{w}/{name}', str(d))) or d
    s_dim, r_dim = int(g('_s_dim', 20)), int(g('_r_dim', 14))
    if i >= s_dim or j >= r_dim:
        return 0.0
    semilen, croot, ctip = g('_semilen_SEG'), g('_Croot'), g('_Ctip')
    sweep, dihed = math.radians(g('_sweep_design')), math.radians(g('_dihed_design'))
    x0, y0, z0 = g('_part_x'), g('_part_y'), g('_part_z')
    side = -1.0 if props.get(f'_wing/{w}/_is_left', '1') != '0' and w % 2 == 0 else 1.0
    t = i / max(1, s_dim - 1)
    along = t * semilen
    chord = croot + (ctip - croot) * t
    span = along * math.cos(sweep)
    qx = x0 + side * span * math.cos(dihed)
    qy = y0 + span * math.sin(dihed)
    qz = z0 + along * math.sin(sweep)
    half = r_dim // 2
    u = j / max(1, half - 1) if j < half else (r_dim - 1 - j) / max(1, half - 1)
    u = 0.5 - 0.5 * math.cos(math.pi * u)
    tc = 0.12 + (0.09 - 0.12) * t
    thick = 5 * tc * chord * (0.2969 * math.sqrt(u) - 0.126 * u - 0.3516 * u ** 2 + 0.2843 * u ** 3 - 0.1036 * u ** 4)
    z = qz - 0.25 * chord + u * chord
    y = qy + (thick if j < half else -thick)
    return [qx, y, z][k]


def pattern(key: str) -> str:
    return re.sub(r'/\d+(,\d+)*(?=/|$)', '/*', key)


def laminar_modes(laminar):
    """For each property pattern, the value most of Laminar's aircraft give it and its share."""
    per = collections.defaultdict(list)
    for p in laminar.values():
        for k, v in p.items():
            per[pattern(k)].append(v)
    out = {}
    for k, vs in per.items():
        mode, n = collections.Counter(vs).most_common(1)[0]
        out[k] = (mode, n / len(vs))
    return out


def build(xplane: Path, a380: Path):
    head, ours_all, tail = read(a380)
    a330 = read(xplane / 'Aircraft' / 'Laminar Research' / 'Airbus A330-300' / 'A330.acf')[1]
    laminar = acfs(xplane, LAMINAR)
    modes = laminar_modes(laminar)
    out, source = {}, {}
    for key, value in ours_all.items():
        if key in DECIDED:
            out[key], source[key] = DECIDED[key][0], 'decided'
            continue
        if a330.get(key) != value:
            out[key], source[key] = value, 'ours'
            continue
        leaf = key.rsplit('/', 1)[-1]
        if leaf.endswith('_count') or leaf == 'count' or leaf in ('_s_dim', '_r_dim', '_version', '_file_writer_version'):
            out[key], source[key] = value, 'structure'
            continue
        if '/_locked/' in key:
            out[key], source[key] = '0', 'regenerated'
            continue
        if '/_geo_xyz/' in key:
            mesh = wing_mesh(ours_all, int(key.split('/')[1]), key) if key.startswith('_wing/') else 0.0
            out[key], source[key] = f'{mesh:.9f}', 'regenerated'
            continue
        if '_for_RIB/' in key or key.startswith('_sbrk/') and '/_geo/' in key:
            out[key], source[key] = fmt_like(value, 0.0), 'regenerated'
            continue
        if value in TRIVIAL:
            out[key], source[key] = value, 'trivial'
            continue
        if leaf.startswith('_afl_file') and value in AIRFOILS:
            out[key], source[key] = AIRFOILS[value], 'airfoils'
            continue
        if key.startswith('_wing/') and leaf == '_els':
            out[key], source[key] = value, 'decided'
            continue
        if key.startswith(RELAYS_ON_ANY_BUS[0]):
            out[key], source[key] = RELAYS_ON_ANY_BUS[1], 'decided'
            continue
        vals = [p[key] for p in laminar.values() if key in p]
        if vals:
            mode, n = collections.Counter(vals).most_common(1)[0]
            if n >= BLANK_SHARE * len(laminar) and mode == value:
                out[key], source[key] = mode, 'blank'
                continue
        mode = modes.get(pattern(key))
        if any(key.startswith(prefix) for prefix, _ in OWNED) and mode:
            out[key], source[key] = mode[0], 'owned'
            continue
        if mode and mode[0] == value and mode[1] >= 0.5:
            out[key], source[key] = value, 'blank'
            continue
        out[key], source[key] = value, 'carried'
    for key in DECIDED.keys() - out.keys():
        out[key], source[key] = DECIDED[key][0], 'decided'
    return head, out, tail, source


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--xplane', type=Path, required=True)
    ap.add_argument('--a380', type=Path)
    ap.add_argument('--out', type=Path, required=True)
    a = ap.parse_args()
    a380 = a.a380 or a.xplane / 'Aircraft' / 'A380 for X-Plane 12' / 'A380.acf'
    head, props, tail, source = build(a.xplane, a380)
    a.out.mkdir(parents=True, exist_ok=True)
    body = head + ['PROPERTIES_BEGIN'] + [f'P {k} {v}' for k, v in props.items()] + tail
    (a.out / 'A380.acf').write_bytes('\n'.join(body).encode('latin-1'))
    counts = collections.Counter(source.values())
    groups = collections.defaultdict(collections.Counter)
    for k, s in source.items():
        groups[s][k.split('/')[0] if not k.startswith('_') else '/'.join(k.split('/')[:1])] += 1
    owned_why = lambda k: next(why for prefix, why in OWNED if k.startswith(prefix))
    report = {'counts': dict(counts), 'by_group': {s: dict(c) for s, c in groups.items()},
              'decided': {k: {'value': props[k], 'source': DECIDED[k][1] if k in DECIDED else RELAYS_ON_ANY_BUS[2] if k.startswith(RELAYS_ON_ANY_BUS[0]) else 'element count our per-element aileron and spoiler assignments use (src/flight_controls.rs)'}
                          for k, s in sorted(source.items()) if s == 'decided'},
              'owned': {k: {'value': props[k], 'why': owned_why(k)} for k, s in sorted(source.items()) if s == 'owned'},
              'carried': sorted(k for k, s in source.items() if s == 'carried')}
    (a.out / 'report.json').write_text(json.dumps(report, indent=1))
    print(json.dumps(report['counts']))


if __name__ == '__main__':
    main()
