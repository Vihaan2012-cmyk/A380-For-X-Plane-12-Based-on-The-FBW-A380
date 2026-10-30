"""DXT1/DXT5 DDS reading and DXT5 writing with numpy only, shared by the kit builder and the kit panel."""
import os
import struct
from concurrent.futures import ThreadPoolExecutor

import numpy as np

from dds import _box_filter_half, _dds_header, _pad_to_multiple_of_4, encode_dxt5

STRIP_ROWS = 64


def read_header(path):
    with open(path, 'rb') as f:
        d = f.read(128)
    if d[:4] != b'DDS ':
        raise ValueError(f'{path}: not a DDS file')
    h, w = struct.unpack('<II', d[12:20])
    mips = max(1, struct.unpack('<I', d[28:32])[0])
    return w, h, mips, d[84:88].decode('ascii', 'replace')


def _rgb565(v):
    v = v.astype(np.int32)
    r, g, b = (v >> 11) & 31, (v >> 5) & 63, v & 31
    return np.stack([(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)], axis=-1)


def _bc1_colors(blocks):
    c0 = blocks[:, 0].astype(np.int32) | (blocks[:, 1].astype(np.int32) << 8)
    c1 = blocks[:, 2].astype(np.int32) | (blocks[:, 3].astype(np.int32) << 8)
    p0, p1 = _rgb565(c0), _rgb565(c1)
    four = (c0 > c1)[:, None]
    p2 = np.where(four, (2 * p0 + p1) // 3, (p0 + p1) // 2)
    p3 = np.where(four, (p0 + 2 * p1) // 3, 0)
    pal = np.stack([p0, p1, p2, p3], axis=1)
    alpha = np.stack([np.full(len(blocks), 255), np.full(len(blocks), 255), np.full(len(blocks), 255), np.where(four[:, 0], 255, 0)], axis=1)
    bits = blocks[:, 4:8].astype(np.uint32)
    word = bits[:, 0] | (bits[:, 1] << 8) | (bits[:, 2] << 16) | (bits[:, 3] << 24)
    idx = ((word[:, None] >> (2 * np.arange(16, dtype=np.uint32))) & 3).astype(np.int64)
    rgb = np.take_along_axis(pal, idx[:, :, None], axis=1)
    a = np.take_along_axis(alpha, idx, axis=1)
    return rgb, a


def _bc3_alpha(blocks):
    a0 = blocks[:, 0].astype(np.int32)
    a1 = blocks[:, 1].astype(np.int32)
    k = np.arange(1, 7, dtype=np.int32)[None, :]
    hi = (a0[:, None] * (7 - k) + a1[:, None] * k) // 7
    k5 = np.arange(1, 5, dtype=np.int32)[None, :]
    lo = (a0[:, None] * (5 - k5) + a1[:, None] * k5) // 5
    n = len(blocks)
    pal8 = np.concatenate([a0[:, None], a1[:, None], hi], axis=1)
    pal6 = np.concatenate([a0[:, None], a1[:, None], lo, np.zeros((n, 1), np.int32), np.full((n, 1), 255, np.int32)], axis=1)
    pal = np.where((a0 > a1)[:, None], pal8, pal6)
    raw = np.zeros(n, np.uint64)
    for i in range(6):
        raw |= blocks[:, 2 + i].astype(np.uint64) << np.uint64(8 * i)
    idx = ((raw[:, None] >> (np.uint64(3) * np.arange(16, dtype=np.uint64))) & np.uint64(7)).astype(np.int64)
    return np.take_along_axis(pal, idx, axis=1)


def read_dds(path):
    """Top level of a DXT1 or DXT5 DDS as (H, W, 4) uint8, rows top to bottom."""
    w, h, _, fourcc = read_header(path)
    if fourcc not in ('DXT1', 'DXT5'):
        raise ValueError(f'{path}: {fourcc} is not supported, use DXT1 or DXT5')
    size = 8 if fourcc == 'DXT1' else 16
    bw, bh = (w + 3) // 4, (h + 3) // 4
    data = np.fromfile(path, np.uint8, count=bw * bh * size, offset=128).reshape(bh * bw, size)
    out = np.empty((bh * 4, bw * 4, 4), np.uint8)
    step = max(1, 65536 // bw) * bw
    for s in range(0, len(data), step):
        blk = data[s:s + step]
        if fourcc == 'DXT1':
            rgb, a = _bc1_colors(blk)
        else:
            rgb, _ = _bc1_colors(blk[:, 8:])
            a = _bc3_alpha(blk[:, :8])
        px = np.concatenate([rgb, a[:, :, None]], axis=2).astype(np.uint8)
        r0 = s // bw
        rows = len(blk) // bw
        out[r0 * 4:(r0 + rows) * 4] = px.reshape(rows, bw, 4, 4, 4).transpose(0, 2, 1, 3, 4).reshape(rows * 4, bw * 4, 4)
    return out[:h, :w]


def shrink(rgba, longest):
    """Box-filter halving until the longest side is at most `longest`; never enlarges."""
    while max(rgba.shape[:2]) > longest and min(rgba.shape[:2]) > 1:
        rgba = _box_filter_half(rgba)
    return rgba


def upscale2(a):
    """Bilinear 2x enlargement of an (H, W, 4) uint8 image."""
    f = a.astype(np.float32)
    for axis in (0, 1):
        prev = np.concatenate([np.take(f, [0], axis=axis), np.delete(f, -1, axis=axis)], axis=axis)
        nxt = np.concatenate([np.delete(f, 0, axis=axis), np.take(f, [-1], axis=axis)], axis=axis)
        out = np.empty(f.shape[:axis] + (f.shape[axis] * 2,) + f.shape[axis + 1:], np.float32)
        lo = [slice(None)] * 3
        hi = [slice(None)] * 3
        lo[axis] = slice(0, None, 2)
        hi[axis] = slice(1, None, 2)
        out[tuple(lo)] = 0.75 * f + 0.25 * prev
        out[tuple(hi)] = 0.75 * f + 0.25 * nxt
        f = out
    return np.clip(f + 0.5, 0, 255).astype(np.uint8)


def fit(rgba, width, height):
    """Resample to width x height; both are power-of-two multiples or divisors of the source."""
    while rgba.shape[1] < width:
        rgba = upscale2(rgba)
    while rgba.shape[1] > width:
        rgba = _box_filter_half(rgba)
    return rgba


def _encode_level(level, pool):
    padded = _pad_to_multiple_of_4(level)
    h = padded.shape[0]
    strips = [padded[r:r + STRIP_ROWS] for r in range(0, h, STRIP_ROWS)]
    return b''.join(pool.map(encode_dxt5, strips))


def write_dxt5(rgba, path):
    """DXT5 DDS with a full mip chain. Returns (width, height, mip count, bytes)."""
    h, w = rgba.shape[:2]
    levels = [rgba]
    while levels[-1].shape[0] > 1 or levels[-1].shape[1] > 1:
        levels.append(_box_filter_half(levels[-1]))
    with ThreadPoolExecutor(max_workers=min(16, os.cpu_count() or 4)) as pool:
        encoded = [_encode_level(lv, pool) for lv in levels]
    with open(path, 'wb') as f:
        f.write(_dds_header(w, h, len(levels), len(encoded[0])))
        for e in encoded:
            f.write(e)
    return w, h, len(levels), 128 + sum(len(e) for e in encoded)
