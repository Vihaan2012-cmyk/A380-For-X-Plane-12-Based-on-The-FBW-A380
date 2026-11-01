#!/usr/bin/env python3
"""python bgra2png.py <in.bgra> <width> <height> <out.png> [scale]"""
import struct
import sys
import zlib
from pathlib import Path


def chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))


def main(source, w, h, out, scale=1):
    px = Path(source).read_bytes()
    rows = []
    for y in range(h):
        row = bytearray()
        for x in range(w):
            o = (y * w + x) * 4
            row += bytes((px[o + 2], px[o + 1], px[o])) * scale
        for _ in range(scale):
            rows.append(b"\0" + bytes(row))
    head = struct.pack(">IIBBBBB", w * scale, h * scale, 8, 2, 0, 0, 0)
    Path(out).write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", head) + chunk(b"IDAT", zlib.compress(b"".join(rows))) + chunk(b"IEND", b""))


if __name__ == "__main__":
    a = sys.argv
    main(a[1], int(a[2]), int(a[3]), a[4], int(a[5]) if len(a) > 5 else 1)
