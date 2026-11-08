"""usage: relit_4k.py <aircraft dir> <4K pack objects dir> <backup dir> <work dir> [--dry-run]

Puts the 4K pack's cockpit textures into the install without undoing what changed at 2K since the pack was
built (relight, AO): where the installed 2K differs from the pack shrunk to 2K, the 4K gets the per-texel ratio
of the two in linear light, upsampled. Each result is checked to shrink back to the installed 2K."""

import json
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.ndimage import binary_dilation, gaussian_filter, zoom

DDSTOOL = "E:/fbw-build/target/release/ddstool.exe"
SAME = 1.5 / 255
MAX_ERROR = 3.0 / 255


def decode(path, work):
    raw = work / (path.stem + ".rgba")
    out = subprocess.run([DDSTOOL, "decode", str(path), str(raw)], capture_output=True, text=True, check=True).stdout.split()
    w, h = int(out[0]), int(out[1])
    a = np.fromfile(raw, dtype=np.uint8).reshape(h, w, 4).astype(np.float32) / 255
    raw.unlink()
    return a, out[2]


def encode(rgba, path, work):
    raw = work / (path.stem + ".rgba")
    np.clip(rgba * 255 + 0.5, 0, 255).astype(np.uint8).tofile(raw)
    subprocess.run([DDSTOOL, "encode", str(raw), str(rgba.shape[1]), str(rgba.shape[0]), str(path)], capture_output=True, check=True)
    raw.unlink()


def mismatched_tiles(inst, small, tile=32, min_corr=0.8):
    """Tiles whose content differs, not only their shading: the art moved or was redrawn since the pack."""
    h, w = inst.shape[0] // tile, inst.shape[1] // tile
    def tiles(x):
        return x[: h * tile, : w * tile, :3].mean(2).reshape(h, tile, w, tile).transpose(0, 2, 1, 3).reshape(h, w, -1)
    a, b = tiles(inst), tiles(small)
    a0, b0 = a - a.mean(-1, keepdims=True), b - b.mean(-1, keepdims=True)
    sa, sb = np.sqrt((a0 ** 2).mean(-1)), np.sqrt((b0 ** 2).mean(-1))
    corr = (a0 * b0).mean(-1) / np.maximum(sa * sb, 1e-6)
    return binary_dilation((np.maximum(sa, sb) > 4 / 255) & (corr < min_corr), iterations=1)


def shrink(a):
    h, w = a.shape[0] // 2, a.shape[1] // 2
    return a.reshape(h, 2, w, 2, 4).mean(axis=(1, 3))


def to_lin(x):
    return np.power(x, 2.2)


def to_srgb(x):
    return np.power(np.clip(x, 0, 1), 1 / 2.2)


def main(aircraft, pack, backup, work, dry):
    objects = Path(aircraft) / "objects"
    pack, backup, work = Path(pack), Path(backup), Path(work)
    work.mkdir(parents=True, exist_ok=True)
    names = json.loads(Path(sys.argv[0]).with_name("relit_4k_list.json").read_text())
    if "--shard" in sys.argv:
        i, n = map(int, sys.argv[sys.argv.index("--shard") + 1].split("/"))
        names = names[i::n]
        work = work / f"shard{i}"
        work.mkdir(parents=True, exist_ok=True)
    report = []
    for name in names:
        inst_p, pack_p = objects / name, pack / name
        src_p = backup / name if (backup / name).is_file() else inst_p
        if not src_p.is_file() or not pack_p.is_file():
            report.append((name, "missing"))
            continue
        inst, fmt = decode(src_p, work)
        if inst.shape[0] >= 4096:
            report.append((name, "already 4K"))
            continue
        big, _ = decode(pack_p, work)
        small = shrink(big)
        if small.shape != inst.shape:
            report.append((name, f"shape {small.shape} vs {inst.shape}"))
            continue
        diff = float(np.abs(small - inst)[..., :3].mean())
        if diff < SAME:
            out, how = big, "pack as is"
        else:
            eps = 1e-3
            ratio = (to_lin(inst[..., :3]) + eps) / (to_lin(small[..., :3]) + eps)
            ratio = np.clip(gaussian_filter(ratio, (0.7, 0.7, 0)), 0.05, 20)
            out = np.empty_like(big)
            out[..., :3] = to_srgb(to_lin(big[..., :3]) * zoom(ratio, (2, 2, 1), order=1))
            out[..., 3] = np.clip(big[..., 3] + zoom(inst[..., 3] - small[..., 3], 2, order=1), 0, 1)
            bad = mismatched_tiles(inst, small)
            tile = inst.shape[0] // bad.shape[0]
            m = np.zeros(big.shape[:2], np.float32)
            m[: bad.shape[0] * tile * 2, : bad.shape[1] * tile * 2] = np.kron(bad, np.ones((tile * 2, tile * 2)))
            m = np.clip(gaussian_filter(m, 6), 0, 1)[..., None]
            out = m * zoom(inst, (2, 2, 1), order=1) + (1 - m) * out
            how = f"relit, {100 * bad.mean():.0f}% of tiles kept at 2K"
        err = float(np.abs(shrink(out) - inst)[..., :3].mean())
        detail = float(np.abs(out - zoom(shrink(out), (2, 2, 1), order=1))[..., :3].mean())
        report.append((name, f"{how}: 2K diff {diff * 255:.1f}, after {err * 255:.2f}, 4K detail {detail * 255:.2f}, {fmt}"))
        print(report[-1][0], report[-1][1], flush=True)
        if err > MAX_ERROR:
            report[-1] = (name, report[-1][1] + " REJECTED")
            continue
        if dry:
            continue
        backup.mkdir(parents=True, exist_ok=True)
        if not (backup / name).exists():
            shutil.copy2(src_p, backup / name)
        if how == "pack as is":
            shutil.copy2(pack_p, inst_p)
        else:
            encode(out, inst_p, work)
    (work / "relit_4k_report.json").write_text(json.dumps(report, indent=1))


if __name__ == "__main__":
    main(*sys.argv[1:5], "--dry-run" in sys.argv)
