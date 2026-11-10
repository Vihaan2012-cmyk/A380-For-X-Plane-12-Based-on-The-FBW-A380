"""usage: opaque_to_dxt1.py <objects dir> [--apply] NAME...

DXT5 textures whose alpha is effectively opaque (every texel at 250 or above) take twice the video memory
of DXT1 for nothing; they are re-encoded as DXT1 with the same size and mip count."""
import io
import sys
from pathlib import Path

import numpy as np
from PIL import Image


def payload(img):
    buf = io.BytesIO()
    img.save(buf, "DDS", pixel_format="DXT1")
    return buf.getvalue()[128:]


def main(folder, names, apply):
    for n in names:
        p = Path(folder) / n
        raw = p.read_bytes()
        if raw[84:88] != b"DXT5":
            print(f"  {n:52} {raw[84:88]!r}, left")
            continue
        img = Image.open(p).convert("RGBA")
        alpha = np.asarray(img)[..., 3]
        lo = int(alpha.min())
        if lo < 250:
            print(f"  {n:52} alpha min {lo}, keeps DXT5")
            continue
        if not apply:
            print(f"  {n:52} opaque, would become DXT1")
            continue
        mips = int.from_bytes(raw[28:32], "little") or 1
        header = bytearray(raw[:128])
        header[84:88] = b"DXT1"
        w, h = img.size
        header[20:24] = (max(1, w // 4) * max(1, h // 4) * 8).to_bytes(4, "little")
        level, out = img.convert("RGB").convert("RGBA"), []
        for _ in range(mips):
            if level.size[0] < 4 or level.size[1] < 4:
                pad = Image.new("RGBA", (max(4, level.size[0]), max(4, level.size[1])))
                pad.paste(level, (0, 0))
                out.append(payload(pad))
            else:
                out.append(payload(level))
            level = level.resize((max(1, level.size[0] // 2), max(1, level.size[1] // 2)), Image.Resampling.LANCZOS)
        p.write_bytes(bytes(header) + b"".join(out))
        print(f"  {n:52} DXT5 -> DXT1, {len(raw) >> 20} -> {p.stat().st_size >> 20} MB")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--apply"]
    main(args[0], args[1:], "--apply" in sys.argv)
