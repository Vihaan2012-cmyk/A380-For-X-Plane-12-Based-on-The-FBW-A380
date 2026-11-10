"""Builds "A380 for X-Plane 11" from the installed "A380 for X-Plane 12".

Unchanged files are hard links into the X-Plane 12 copy (same drive, no extra space); the acf, the objects
that change and the plugin folder's small files are real files. Run:
  python -I make_xp11.py --xp12 "<X-Plane 12>/Aircraft/A380 for X-Plane 12" --xp11 "<X-Plane 11>" [--plugin <win.xpl>]
"""
import argparse
import json
import os
import re
import shutil
import struct
import sys
import time
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import acf11
import obj11

PANEL = (4096, 4096)
PAD = 2
NAME = "A380 for X-Plane 11"
MARKER = "xp11_build.json"
SCREEN_OBJ = "objects/a380_cockpit_003.obj"
COCKPIT_OBJ = "objects/a380_cockpit_click.obj"
SHRINK = ["FCU", "SCREEN_DU_RMP_1", "SCREEN_DU_RMP_2", "SCREEN_DU_RMP_3", "SCREEN_EFB", "SCREEN_OIT_LEFT", "SCREEN_OIT_RIGHT", "SCREEN_DU_MFD"]
SKIP_DIRS = {"Output"}
REAL_COPY_DIRS = ("plugins/fbw_a380_systems/64/",)
REAL_COPY_MAX = 1 << 20


def screen_sizes(screens_rs):
    text = Path(screens_rs).read_text(encoding="utf-8")
    out = {}
    for m in re.finditer(r'id:\s*"([^"]+)".*?width:\s*(\d+),\s*height:\s*(\d+)', text, re.S):
        out.setdefault(m.group(1), (int(m.group(2)), int(m.group(3))))
    return out


def shelf(sizes, panel=PANEL, pad=PAD):
    """Rectangles from the bottom left, tallest first along shelves; None if they do not fit."""
    order = sorted(sizes, key=lambda k: (-sizes[k][1], -sizes[k][0], k))
    x = y = shelf_h = 0
    out = {}
    for k in order:
        w, h = sizes[k]
        if w > panel[0]:
            return None
        if x + w > panel[0]:
            x, y, shelf_h = 0, y + shelf_h + pad, 0
        if y + h > panel[1]:
            return None
        out[k] = {"x": x, "y": y, "w": w, "h": h}
        x += w + pad
        shelf_h = max(shelf_h, h)
    return out


def pack(native, panel=PANEL):
    """Each screen at full size if the panel holds them all, else the screens in SHRINK halved in turn."""
    scale = {k: 1.0 for k in native}
    turn = [k for k in SHRINK if k in native] + sorted((k for k in native if k not in SHRINK), key=lambda k: -native[k][0] * native[k][1])
    for step in range(len(turn) * 3 + 1):
        sizes = {k: (max(1, round(w * scale[k])), max(1, round(h * scale[k]))) for k, (w, h) in native.items()}
        placed = shelf(sizes, panel)
        if placed:
            return placed
        k = turn[step % len(turn)]
        scale[k] *= 0.5
    raise SystemExit("the screens do not fit on the panel")


MAX_TEXTURE = 4096
DDS_BLOCK = {b"DXT1": 8, b"ATI1": 8, b"BC4U": 8, b"DXT3": 16, b"DXT5": 16, b"ATI2": 16, b"BC5U": 16}
DXGI_BLOCK = {70: 8, 71: 8, 72: 8, 79: 8, 80: 8, 81: 8, 73: 16, 74: 16, 75: 16, 76: 16, 77: 16, 78: 16, 82: 16, 83: 16, 84: 16, 94: 16, 95: 16, 96: 16, 97: 16, 98: 16, 99: 16}


def dds_size(path):
    with open(path, "rb") as f:
        h = f.read(20)
    if len(h) < 20 or h[:4] != b"DDS ":
        return None
    return struct.unpack("<I", h[16:20])[0], struct.unpack("<I", h[12:16])[0]


def shrink_dds(src, dst, limit=MAX_TEXTURE):
    """X-Plane 11 refuses a texture over 4096 on a side: drop the top mip levels until it fits."""
    data = Path(src).read_bytes()
    height, width = struct.unpack("<II", data[12:20])
    mips = max(1, struct.unpack("<I", data[28:32])[0])
    four = data[84:88]
    offset = 128
    if four == b"DX10":
        block = DXGI_BLOCK.get(struct.unpack("<I", data[128:132])[0])
        offset = 148
    else:
        block = DDS_BLOCK.get(four)
    if block is None:
        raise ValueError(f"{src}: not a block-compressed DDS")
    level = lambda w, h: max(1, (w + 3) // 4) * max(1, (h + 3) // 4) * block
    skip = 0
    while (width > limit or height > limit) and mips > 1:
        skip += level(width, height)
        width, height, mips = max(1, width // 2), max(1, height // 2), mips - 1
    header = bytearray(data[:offset])
    header[12:20] = struct.pack("<II", height, width)
    header[20:24] = struct.pack("<I", level(width, height))
    header[28:32] = struct.pack("<I", mips)
    Path(dst).parent.mkdir(parents=True, exist_ok=True)
    if Path(dst).exists():
        Path(dst).unlink()
    Path(dst).write_bytes(bytes(header) + data[offset + skip:])
    return width, height


def panel_3d(atlas, images):
    """A black gen_rotary under each screen, as ToLiss does, so X-Plane 11 renders those parts of the panel."""
    lines = ["PANEL_3D_BEGIN", "GROUP fbw_xp11_screens", "  SHOW_LESS 0.000000 [0]", "  SHOW_EQUAL 0.000000 [0]", "  SHOW_GREATER 0.000000 [0]", "  GROUP_OPEN"]
    for sid, r in sorted(atlas.items()):
        lines += [f"  gen_rotary {sid}", f"    POS {r['x']:.6f} {r['y']:.6f}", f"    IMAGE {images[(r['w'], r['h'])]}", "    DATAREF ",
                  "    LIGHT_MODE MECHANICAL", "    LIGHT_RHEOSTAT 0", "    KEY_FRAME 0.000000 0.000000 1.000000", "    KEY_FRAME 1.000000 1.000000",
                  "    DIGITS 1", "    ROTARY_TYPE NO_CLICK", "    CLICK_RADIUS 10.000000", "    CLICK_RADIUS_DY 0.000000", "    OFFSET 0.000000",
                  "    STEP1 1.000000", "    STEP2 1.000000", "    CURSOR DEFAULT", "    CLICK_DX 0", "    CLICK_DY 0", ""]
    return lines + ["END_GROUP", "PANEL_3D_END"]


def with_panel_3d(tail, block):
    out, skipping = [], False
    for line in tail:
        s = line.strip()
        if s == "PANEL_3D_BEGIN":
            skipping = True
            out.extend(block)
            continue
        if skipping:
            skipping = s != "PANEL_3D_END"
            continue
        out.append(line)
    if "PANEL_3D_BEGIN" not in [l.strip() for l in tail]:
        out.extend(block)
    return out


def black_png(path, w, h):
    raw = (b"\x00" * (w * 3 + 1)) * h

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_bytes(data)


def packed_name(normal, material):
    """The XP11 normal map packing an XP12 normal and material_gloss pair."""
    stem = Path(normal or material).stem
    tag = zlib.crc32(f"{normal}|{material}".encode()) & 0xFFFFFFFF
    return f"{stem}_xp11_{tag:08x}.png"


def pack_normal(normal, material, out):
    """XP12 keeps the normal's X and Y in RG (B empty) and metalness and gloss in a separate map's R and G;
    XP11's NORMAL_METALNESS reads one map: normal in RG, metalness in B, gloss in A."""
    from PIL import Image
    import numpy as np
    n = Image.open(normal).convert("RGB") if normal else None
    m = Image.open(material).convert("RGB") if material else None
    size = n.size if n else m.size
    if m is not None and m.size != size:
        m = m.resize(size, Image.BILINEAR)
    h, w = size[1], size[0]
    nn = np.asarray(n) if n is not None else np.full((h, w, 3), 128, np.uint8)
    mm = np.asarray(m) if m is not None else np.dstack([np.zeros((h, w), np.uint8), np.full((h, w), 128, np.uint8), np.zeros((h, w), np.uint8)])
    rgba = np.dstack([nn[..., 0], nn[..., 1], mm[..., 0], mm[..., 1]])
    Path(out).parent.mkdir(parents=True, exist_ok=True)
    tmp = Path(str(out) + ".tmp")
    Image.fromarray(rgba, "RGBA").save(tmp, format="PNG", compress_level=1)
    os.replace(tmp, out)


def place(src, dst, real):
    dst.parent.mkdir(parents=True, exist_ok=True)
    if dst.exists() or dst.is_symlink():
        dst.unlink()
    if not real:
        try:
            os.link(src, dst)
            return "link"
        except OSError:
            pass
    shutil.copy2(src, dst)
    return "copy"


def write(dst, text, encoding="utf-8"):
    dst.parent.mkdir(parents=True, exist_ok=True)
    if dst.exists():
        dst.unlink()
    dst.write_bytes(text.encode(encoding))


def build(xp12_dir, xp11_root, plugin=None, server=None, screens_rs=None):
    src = Path(xp12_dir)
    dst = Path(xp11_root) / "Aircraft" / NAME
    if dst.exists() and not (dst / MARKER).exists():
        raise SystemExit(f"{dst} exists and was not made by this tool; move it away first")
    donor_path = Path(xp11_root) / "Aircraft" / "Laminar Research" / "Boeing B747-400" / "747-400.acf"
    _, donor, _ = acf11.read_acf(donor_path)
    native_all = screen_sizes(screens_rs or Path(__file__).resolve().parents[2] / "src" / "display" / "screens.rs")
    screens_text = (src / SCREEN_OBJ).read_text(encoding="latin-1")
    devices = obj11.screen_devices(screens_text)
    missing = [d for d in devices if d not in native_all]
    if missing:
        raise SystemExit(f"no size for screens {missing} in screens.rs")
    atlas = pack({d: native_all[d] for d in devices})
    images = {}
    for r in atlas.values():
        key = (r["w"], r["h"])
        if key not in images:
            images[key] = f"fbw_xp11/black_{key[0]}x{key[1]}"
            for suffix in ("", "-1"):
                black_png(dst / "cockpit_3d" / "generic" / f"{images[key]}{suffix}.png", *key)
    stats = {"link": 0, "copy": 0, "converted": 0, "normal maps": 0}
    t0 = time.time()
    jobs = {}

    def combiner(obj_path):
        def combined(normal, material):
            name = packed_name(normal, material)
            out_dir = dst / obj_path.parent.relative_to(src)
            find = lambda f: next((p for p in (obj_path.parent / f, src / "objects" / f) if p.exists()), None) if f else None
            n, m = find(normal), find(material)
            if (normal and n is None) or (material and m is None):
                return None
            jobs.setdefault(out_dir / name, (n, m))
            return name
        return combined

    for path in sorted(src.rglob("*")):
        rel = path.relative_to(src).as_posix()
        if path.is_dir() or rel.split("/")[0] in SKIP_DIRS:
            continue
        name = path.name
        if name.endswith(".acf~") or name.endswith("_prefs.txt") or (name.endswith(".acf") and name != "A380.acf") or "crashfx" in name.lower():
            continue
        out = dst / rel
        if rel == "A380.acf":
            head, props, tail = acf11.read_acf(path)
            conv = acf11.convert(props, donor)
            out.parent.mkdir(parents=True, exist_ok=True)
            if out.exists():
                out.unlink()
            acf11.write_acf(out, head, conv, with_panel_3d(tail, panel_3d(atlas, images)))
            stats["converted"] += 1
            continue
        if name.lower().endswith(".dds"):
            size = dds_size(path)
            if size and max(size) > MAX_TEXTURE:
                shrink_dds(path, out)
                stats["converted"] += 1
                continue
        if rel == SCREEN_OBJ:
            write(out, obj11.screens_outside_only(screens_text), "latin-1")
            stats["converted"] += 1
            continue
        if rel == COCKPIT_OBJ:
            click = path.read_text(encoding="latin-1")
            text = obj11.cockpit_with_screens(click, screens_text, atlas, PANEL)
            text, _ = obj11.generic(text, combiner(path))
            write(out, text, "latin-1")
            stats["converted"] += 1
            continue
        if name.endswith(".obj"):
            text, changed = obj11.generic(path.read_text(encoding="latin-1"), combiner(path))
            if changed:
                write(out, text, "latin-1")
                stats["converted"] += 1
                continue
        real = rel.startswith("plugins/") and (rel.startswith(REAL_COPY_DIRS) or path.stat().st_size <= REAL_COPY_MAX)
        stats[place(path, out, real)] += 1
    todo = [(out, n, m) for out, (n, m) in jobs.items()
            if not (out.exists() and out.stat().st_mtime >= max(p.stat().st_mtime for p in (n, m) if p))]
    from concurrent.futures import ThreadPoolExecutor
    with ThreadPoolExecutor(max_workers=8) as pool:
        for f in [pool.submit(pack_normal, n, m, out) for out, n, m in todo]:
            f.result()
    stats["normal maps"] = len(jobs)
    for stale in sorted((p for p in dst.rglob("*") if "crashfx" in p.name.lower()), key=lambda p: -len(p.parts)):
        if stale.is_dir():
            shutil.rmtree(stale)
        elif stale.exists():
            stale.unlink()
    black_png(dst / "cockpit_3d" / "-PANELS-" / "Panel.png", *PANEL)
    plugin_dir = dst / "plugins" / "fbw_a380_systems"
    (plugin_dir / "xp11_panel.json").write_text(json.dumps({"panel": list(PANEL), "screens": atlas}, indent=1), encoding="utf-8")
    for given, name in ((plugin, "win.xpl"), (server, "fbw_a380_systems_server.exe")):
        if given:
            place(Path(given), plugin_dir / "64" / name, True)
    (dst / MARKER).write_text(json.dumps({"from": str(src), "built": time.strftime("%Y-%m-%d %H:%M:%S"), "stats": stats}, indent=1), encoding="utf-8")
    print(f"{dst}: {stats['converted']} converted, {stats['link']} linked, {stats['copy']} copied, {stats['normal maps']} normal maps ({len(todo)} made) in {time.time() - t0:.0f} s")
    return dst, atlas


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--xp12", required=True)
    ap.add_argument("--xp11", required=True)
    ap.add_argument("--plugin")
    ap.add_argument("--server")
    ap.add_argument("--screens-rs")
    a = ap.parse_args()
    build(a.xp12, a.xp11, a.plugin, a.server, a.screens_rs)


if __name__ == "__main__":
    main()
