"""usage: fan_blur_opacity.py <aircraft dir> <backup dir>

Makes the fan blur discs read as a spinning fan instead of a ghost: each of
the ~15 stacked blur rings in ENGINE_BLUR2..4 (and their _DECAL copies) gets
its opacity raised, a = 1 - (1 - a)^gain, harder for the faster stages, in
the base objects and in every livery's own copy (liveries carry their own
spinner art in these atlases). The DDS keeps its exact format: DXT5, same
size, full mip chain rebuilt from the edited top level, original header."""

import io
import shutil
import sys
from pathlib import Path

import numpy as np
from PIL import Image

GAIN = {2: 1.4, 3: 1.9, 4: 2.4}


def dxt5_payload(img):
    w, h = img.size
    if w < 4 or h < 4:
        pad = Image.new("RGBA", (4, 4))
        pad.paste(img, (0, 0))
        img = pad
    buf = io.BytesIO()
    img.save(buf, "DDS", pixel_format="DXT5")
    data = buf.getvalue()
    return data[128:]


def rebuild(path, gain):
    raw = path.read_bytes()
    header = raw[:128]
    if header[84:88] != b"DXT5":
        return f"skipped ({header[84:88]!r})"
    mips = int.from_bytes(header[28:32], "little") or 1
    img = Image.open(path).convert("RGBA")
    a = np.asarray(img).astype(np.float32)
    alpha = a[..., 3] / 255.0
    solid = alpha > 0.98
    boosted = 1.0 - (1.0 - alpha) ** gain
    boosted[solid] = alpha[solid]
    a[..., 3] = boosted * 255.0
    level = Image.fromarray((a + 0.5).astype(np.uint8), "RGBA")
    payload = []
    for _ in range(mips):
        payload.append(dxt5_payload(level))
        w, h = level.size
        level = level.resize((max(1, w // 2), max(1, h // 2)), Image.Resampling.LANCZOS)
    out = header + b"".join(payload)
    if len(out) != len(raw):
        return f"size mismatch {len(out)} vs {len(raw)}, left unchanged"
    path.write_bytes(out)
    return "rebuilt"


def main(aircraft, backup):
    aircraft, backup = Path(aircraft), Path(backup)
    folders = [aircraft / "objects"] + sorted(p / "objects" for p in (aircraft / "liveries").iterdir() if (p / "objects").is_dir())
    for folder in folders:
        for stage, gain in GAIN.items():
            for decal in ("", "_DECAL"):
                path = folder / f"ENGINE_BLUR{stage}_ALBEDO_KE7E7E7{decal}.dds"
                if not path.is_file():
                    continue
                rel = path.relative_to(aircraft)
                saved = backup / rel
                if not saved.is_file():
                    saved.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(path, saved)
                print(rel, rebuild(path, gain) if path.read_bytes() == saved.read_bytes() else "already rebuilt")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
