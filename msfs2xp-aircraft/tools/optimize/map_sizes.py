"""usage: map_sizes.py <aircraft dir> <FBW MSFS texture dir> <prefix> [role]

Every texture the acf's objects whose names start with <prefix> bind, by role, with its installed size
against FlyByWire's MSFS source size, smallest share first."""
import re
import struct
import sys
from pathlib import Path

ROLES = {"TEXTURE": "albedo", "TEXTURE_LIT": "lit", "TEXTURE_NORMAL": "normal"}


def size(p):
    h = p.read_bytes()[:32]
    if h[:4] == b"DDS ":
        hh, w = struct.unpack("<II", h[12:20])
        return w, hh
    if h[:8] == b"\x89PNG\r\n\x1a\n":
        return struct.unpack(">II", h[16:24])
    return None


def source(name, fbw):
    stem = re.sub(r"(_NML|_MAT)?\.(png|dds)$", "", name, flags=re.I)
    stem = re.sub(r"_K[0-9A-F]{6}.*$|_A\d+$|_MASK\d+$", "", stem)
    for cand in (f"{stem}.PNG.DDS", f"{stem}.png.DDS", f"{stem}.DDS"):
        if (fbw / cand).is_file():
            return fbw / cand
    return None


aircraft, fbw, prefix = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
want = sys.argv[4] if len(sys.argv) > 4 else None
acf = (aircraft / "A380.acf").read_bytes().decode("latin-1")
objs = sorted({v.strip() for v in re.findall(r"^P _obja/\d+/_v10_att_file_stl (.+)$", acf, re.M) if v.strip().startswith(prefix)})
seen = {}
for o in objs:
    p = aircraft / "objects" / o
    if not p.is_file():
        continue
    for line in open(p, encoding="latin-1", errors="replace"):
        w = line.split()
        if not w:
            continue
        if w[0] in ("VT", "POINT_COUNTS"):
            break
        role = ROLES.get(w[0]) or (w[1] if w[0] == "TEXTURE_MAP" and len(w) > 2 else None)
        if role and len(w) > 1:
            seen.setdefault((role, w[-1]), set()).add(o)
rows = []
for (role, name), users in seen.items():
    if want and role != want:
        continue
    p = aircraft / "objects" / name
    if not p.is_file():
        continue
    s = source(name, fbw)
    ss = size(s) if s else None
    have = size(p)
    if not have:
        continue
    share = have[0] / ss[0] if ss else 1.0
    rows.append((share, role, name, have, ss, len(users)))
for share, role, name, have, ss, n in sorted(rows):
    print(f"{role:14} {name:58} {have[0]:>5} src {ss[0] if ss else '-':>5} x{share:.3f} objs {n}")
