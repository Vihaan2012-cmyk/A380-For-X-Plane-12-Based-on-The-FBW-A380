#!/usr/bin/env python3
"""python hold_anim_keys.py <aircraft folder> <backup folder> [--dry-run]

X-Plane carries a key frame table's slope on past its first and last keys. MSFS holds them.
A converted table that starts above 0 or ends below 1 therefore overshoots: the tray tables
stood upright when stowed, because their fold starts at 0.667 and ran backwards from there.
This writes a key at 0 and at 1 holding the end values. Safe to run again.
"""
import shutil
import sys
from pathlib import Path

PREFIXES = ("fbw/cockpit/", "fbw/anim/")


def key_range(line):
    p = line.split()
    return float(p[1]), p[2:]


def hold_table(lines, kind):
    """lines: a table's key lines. Returns them with the ends held, and whether anything was added."""
    newline = "\r\n" if lines[0].endswith("\r\n") else "\n"
    first_at, first_value = key_range(lines[0])
    last_at, last_value = key_range(lines[-1])
    if len(lines) < 2:
        return lines, False
    second_value = key_range(lines[1])[1]
    before_last_value = key_range(lines[-2])[1]
    out = list(lines)
    changed = False
    if first_at > 0 and first_value != second_value:
        out.insert(0, f"ANIM_{kind}_key 0 {' '.join(first_value)}{newline}")
        changed = True
    if last_at < 1 and last_value != before_last_value:
        out.append(f"ANIM_{kind}_key 1 {' '.join(last_value)}{newline}")
        changed = True
    return out, changed


def patch(text):
    lines = text.splitlines(keepends=True)
    out = []
    changed = 0
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("ANIM_trans_begin ") or line.startswith("ANIM_rotate_begin "):
            kind = "trans" if line.startswith("ANIM_trans") else "rotate"
            ref = line.split()[-1]
            j = i + 1
            while not lines[j].startswith(f"ANIM_{kind}_end"):
                j += 1
            keys = lines[i + 1:j]
            out.append(line)
            looped = any(k.startswith("ANIM_keyframe_loop") for k in keys)
            only_keys = all(k.startswith(f"ANIM_{kind}_key") for k in keys)
            if keys and only_keys and not looped and ref.startswith(PREFIXES):
                keys, was_changed = hold_table(keys, kind)
                changed += was_changed
            out.extend(keys)
            i = j
            continue
        out.append(line)
        i += 1
    return "".join(out), changed


def main():
    aircraft = Path(sys.argv[1])
    backup = Path(sys.argv[2])
    dry = "--dry-run" in sys.argv
    tables = 0
    files = 0
    for path in sorted((aircraft / "objects").glob("*.obj")):
        raw = path.read_bytes()
        if b"ANIM_" not in raw:
            continue
        text = raw.decode("latin-1")
        patched, changed = patch(text)
        if not changed:
            continue
        tables += changed
        files += 1
        print(f"{path.name}: {changed} tables")
        if not dry:
            backup.mkdir(parents=True, exist_ok=True)
            if not (backup / path.name).exists():
                shutil.copy2(path, backup / path.name)
            path.write_bytes(patched.encode("latin-1"))
    print(f"{tables} tables in {files} objects")


if __name__ == "__main__":
    main()
