"""Zip archives at the strongest compression the installer can unpack: every
entry is LZMA (zip method 14, preset 9 extreme, an end marker as zip's LZMA
entries carry), or stored when LZMA would not make it smaller. Entries are
compressed in parallel and written in order.
"""

import lzma
import os
import struct
import zipfile
import zlib
from collections import deque
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

PRESET = 9 | lzma.PRESET_EXTREME
WORKERS = min(os.cpu_count() or 4, 24)


def lzma_payload(data: bytes) -> bytes:
    dict_size = 1 << 16
    while dict_size < len(data) and dict_size < 64 << 20:
        dict_size <<= 1
    filt = {"id": lzma.FILTER_LZMA1, "preset": PRESET, "dict_size": dict_size}
    props = lzma._encode_filter_properties(filt)
    packer = lzma.LZMACompressor(lzma.FORMAT_RAW, filters=[filt])
    return struct.pack("<BBH", 9, 4, len(props)) + props + packer.compress(data) + packer.flush()


def compress_file(path: str) -> tuple[int, int, int, bytes]:
    data = Path(path).read_bytes()
    crc = zlib.crc32(data)
    packed = lzma_payload(data)
    if len(packed) >= len(data):
        return crc, len(data), zipfile.ZIP_STORED, data
    return crc, len(data), zipfile.ZIP_LZMA, packed


def compress_bytes(data: bytes) -> tuple[int, int, int, bytes]:
    packed = lzma_payload(data)
    if len(packed) >= len(data):
        return zlib.crc32(data), len(data), zipfile.ZIP_STORED, data
    return zlib.crc32(data), len(data), zipfile.ZIP_LZMA, packed


def write_entry(z: zipfile.ZipFile, arc: str, result: tuple[int, int, int, bytes], path: Path | None = None) -> None:
    crc, size, method, payload = result
    info = zipfile.ZipInfo.from_file(path, arc) if path is not None else zipfile.ZipInfo(arc, date_time=(2026, 1, 1, 0, 0, 0))
    if path is None:
        info.external_attr = 0o644 << 16
    info.compress_type = method
    if method == zipfile.ZIP_LZMA:
        info.flag_bits |= 0x02
    info.CRC, info.file_size, info.compress_size = crc, size, len(payload)
    with z._lock:
        z._writecheck(info)
        z._didModify = True
        info.header_offset = z.fp.tell()
        z.fp.write(info.FileHeader())
        z.fp.write(payload)
        z.filelist.append(info)
        z.NameToInfo[info.filename] = info
        z.start_dir = z.fp.tell()


def write_bytes(z: zipfile.ZipFile, arc: str, data: bytes) -> None:
    write_entry(z, arc, compress_bytes(data))


def compressed_in_order(paths: list[Path]):
    with ProcessPoolExecutor(max_workers=WORKERS) as pool:
        pending = deque()
        it = iter(paths)
        for path in it:
            pending.append(pool.submit(compress_file, str(path)))
            if len(pending) >= WORKERS * 2:
                break
        while pending:
            yield pending.popleft().result()
            nxt = next(it, None)
            if nxt is not None:
                pending.append(pool.submit(compress_file, str(nxt)))


def write_zip(out: Path, items: list[tuple[Path, str]]) -> None:
    with zipfile.ZipFile(out, "w") as z:
        for (path, arc), result in zip(items, compressed_in_order([p for p, _ in items])):
            write_entry(z, arc, result, path)
