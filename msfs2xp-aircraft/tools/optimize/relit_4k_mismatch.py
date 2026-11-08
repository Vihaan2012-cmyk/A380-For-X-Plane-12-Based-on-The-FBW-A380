"""usage: relit_4k_mismatch.py <backup dir> <4K pack objects dir> <names json>

Per relit texture: the share of 32-texel tiles where the installed 2K and the 4K pack shrunk to 2K show different
content, not only different shading (art moved or redrawn since the pack was built)."""

import json
import subprocess
import sys
from pathlib import Path

import numpy as np

DDSTOOL = "E:/fbw-build/target/release/ddstool.exe"
TILE = 32


def decode(path, work):
    raw = work / (path.stem + ".chk.rgba")
    out = subprocess.run([DDSTOOL, "decode", str(path), str(raw)], capture_output=True, text=True, check=True).stdout.split()
    w, h = int(out[0]), int(out[1])
    a = np.fromfile(raw, dtype=np.uint8).reshape(h, w, 4).astype(np.float32) / 255
    raw.unlink()
    return a


def tile_mismatch(inst, small):
    h, w = inst.shape[0] // TILE, inst.shape[1] // TILE
    a = inst[: h * TILE, : w * TILE, :3].mean(2).reshape(h, TILE, w, TILE).transpose(0, 2, 1, 3).reshape(h, w, -1)
    b = small[: h * TILE, : w * TILE, :3].mean(2).reshape(h, TILE, w, TILE).transpose(0, 2, 1, 3).reshape(h, w, -1)
    a0, b0 = a - a.mean(-1, keepdims=True), b - b.mean(-1, keepdims=True)
    sa, sb = np.sqrt((a0 ** 2).mean(-1)), np.sqrt((b0 ** 2).mean(-1))
    corr = (a0 * b0).mean(-1) / np.maximum(sa * sb, 1e-6)
    busy = np.maximum(sa, sb) > 4 / 255
    return busy & (corr < 0.6), busy


def main(backup, pack, names_path):
    backup, pack = Path(backup), Path(pack)
    work = Path("E:/fbw-build/tmp/relit4k")
    out = {}
    for name in json.loads(Path(names_path).read_text()):
        if not (backup / name).is_file() or not (pack / name).is_file():
            continue
        inst = decode(backup / name, work)
        big = decode(pack / name, work)
        small = big.reshape(big.shape[0] // 2, 2, big.shape[1] // 2, 2, 4).mean(axis=(1, 3))
        if small.shape != inst.shape:
            continue
        bad, busy = tile_mismatch(inst, small)
        out[name] = (int(bad.sum()), int(busy.sum()))
        print(f"{name:55} {bad.sum():5d} mismatched of {busy.sum():5d} detailed tiles ({100 * bad.sum() / max(busy.sum(), 1):.1f}%)", flush=True)
    (work / "mismatch.json").write_text(json.dumps(out, indent=1))


if __name__ == "__main__":
    main(*sys.argv[1:4])
