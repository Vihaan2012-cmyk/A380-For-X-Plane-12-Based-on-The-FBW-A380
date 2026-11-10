"""usage: halve_cabin_maps.py <aircraft dir> <backup dir> [--min N] [--apply]

Halves every texture only the passenger cabin uses (a380_cockpit_004/005/008/009/010/1xx, cabin_ext,
cabin_mood) and at least --min px (default 1024): a DDS loses its top mip (exact, no re-encode), a PNG is
resized. The cabin is seen only on a walk through it, so this is where video memory comes back cheapest."""
import re
import shutil
import struct
import sys
from pathlib import Path

from PIL import Image

CABIN = re.compile(r"^a380_cockpit_(00[4589]|010|1\d\d)|cabin", re.I)
ROLES = ("TEXTURE", "TEXTURE_LIT", "TEXTURE_NORMAL", "TEXTURE_MAP")


def maps(obj):
    out = []
    for line in open(obj, encoding="latin-1", errors="replace"):
        w = line.split()
        if not w:
            continue
        if w[0] in ("VT", "POINT_COUNTS"):
            break
        if w[0] in ROLES and len(w) > 1:
            out.append(w[-1])
    return out


def drop_top_mip(p):
    raw = p.read_bytes()
    h, w = struct.unpack("<II", raw[12:20])
    mips = struct.unpack("<I", raw[28:32])[0]
    four = raw[84:88]
    block = 8 if four == b"DXT1" else 16
    if mips < 2 or four not in (b"DXT1", b"DXT3", b"DXT5"):
        return False
    top = max(1, w // 4) * max(1, h // 4) * block
    head = bytearray(raw[:128])
    head[12:20] = struct.pack("<II", h // 2, w // 2)
    head[20:24] = struct.pack("<I", max(1, w // 8) * max(1, h // 8) * block)
    head[28:32] = struct.pack("<I", mips - 1)
    p.write_bytes(bytes(head) + raw[128 + top:])
    return True


def main(aircraft, backup, minimum, apply):
    objects = aircraft / "objects"
    acf = (aircraft / "A380.acf").read_bytes().decode("latin-1")
    names = {v.strip() for v in re.findall(r"^P _obja/\d+/_v10_att_file_stl (.+)$", acf, re.M)}
    users = {}
    for n in names:
        p = objects / n
        if p.is_file():
            for t in maps(p):
                users.setdefault(t, set()).add(n)
    saved = 0
    for tex, objs in sorted(users.items()):
        if not all(CABIN.search(o) for o in objs):
            continue
        p = objects / tex
        if not p.is_file():
            continue
        raw = p.read_bytes()[:32]
        if raw[:4] == b"DDS ":
            h, w = struct.unpack("<II", raw[12:20])
        else:
            w, h = struct.unpack(">II", raw[16:24])
        if max(w, h) < minimum:
            continue
        before = p.stat().st_size
        if apply:
            backup.mkdir(parents=True, exist_ok=True)
            if not (backup / tex).exists():
                shutil.copy2(p, backup / tex)
            if raw[:4] == b"DDS ":
                if not drop_top_mip(p):
                    print(f"  {tex}: not a mipped DXT, left")
                    continue
            else:
                im = Image.open(p)
                im.resize((w // 2, h // 2), Image.Resampling.LANCZOS).save(p, compress_level=6)
            saved += before - p.stat().st_size
        print(f"  {tex:58} {w} -> {w // 2}")
    print(f"{saved / 1e6:.0f} MB smaller on disk")


if __name__ == "__main__":
    a = sys.argv[1:]
    m = int(a[a.index("--min") + 1]) if "--min" in a else 1024
    pos = [x for i, x in enumerate(a) if not x.startswith("--") and (i == 0 or a[i - 1] != "--min")]
    main(Path(pos[0]), Path(pos[1]), m, "--apply" in a)
