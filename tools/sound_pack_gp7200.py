"""usage: sound_pack_gp7200.py <aircraft folder> <xA380 GP pack folder> <plugin src/sound_pack.rs>"""

import re
import shutil
import sys
from pathlib import Path

aircraft, pack, table_rs = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
fmod = aircraft / "fmod"
sets = aircraft / "fmod_sets"
trent, gp = sets / "trent900", sets / "gp7200"
SET_FILES = ["Master Bank.bank", "A380.snd", "GUIDs.txt"]

if not (trent / "Master Bank.bank").exists():
    trent.mkdir(parents=True, exist_ok=True)
    for f in SET_FILES:
        shutil.copy2(fmod / f, trent / f)
gp.mkdir(parents=True, exist_ok=True)

text = table_rs.read_text(encoding="utf-8")
block = text[text.index("pub const BANK_PARAMETERS"):text.index("];", text.index("pub const BANK_PARAMETERS"))]
pairs = re.findall(r'\("([^"]+)", "([^"]+)"\)', block)


def bank_name(original, base):
    bracket = "[" if base.endswith("[") else ""
    base = base[:-1] if bracket else base
    return base + "_" * (len(original) - len(base) - len(bracket)) + bracket


data = (pack / "fmod" / "Master Bank.bank").read_bytes()
counts = []
for o, b in pairs:
    o_b, n_b = o.encode(), bank_name(o, b).encode()
    assert len(o_b) == len(n_b), (o, b)
    pattern = re.escape(o_b) if o_b.endswith(b"[") else re.escape(o_b) + rb"(?![A-Za-z0-9_])"
    data, c = re.subn(pattern, n_b, data)
    if c:
        counts.append(f"{c}x {o}")
(gp / "Master Bank.bank").write_bytes(data)
print("GP master bank:", "; ".join(counts))


def guid_events(path):
    out = {}
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        m = re.match(r"(\{[0-9a-fA-F-]+\})\s+(\S.*)", line.strip())
        if m:
            out[m.group(2).strip()] = line
    return out


def shared_bank_events(banks, guid_lines):
    import ctypes
    import os
    dlls = Path(aircraft).parents[1] / "Resources" / "dlls" / "64"
    os.add_dll_directory(str(dlls))
    os.environ["PATH"] = str(dlls) + os.pathsep + os.environ["PATH"]
    ctypes.CDLL(str(dlls / "fmod.dll"), winmode=0)
    studio = ctypes.CDLL(str(dlls / "fmodstudio.dll"), winmode=0)

    class GUID(ctypes.Structure):
        _fields_ = [("a", ctypes.c_uint32), ("b", ctypes.c_uint16), ("c", ctypes.c_uint16), ("d", ctypes.c_ubyte * 8)]

    system = ctypes.c_void_p()
    assert studio.FMOD_Studio_System_Create(ctypes.byref(system), ctypes.c_uint(0x00020223)) == 0
    assert studio.FMOD_Studio_System_Initialize(system, 32, 0, 0, None) == 0
    by_guid = {}
    for path, line in guid_lines.items():
        m = re.match(r"\{([0-9a-fA-F-]+)\}", line.strip())
        if m:
            by_guid[m.group(1).lower()] = path
    found = set()
    for bank in banks:
        handle = ctypes.c_void_p()
        assert studio.FMOD_Studio_System_LoadBankFile(system, str(bank).encode(), 0, ctypes.byref(handle)) == 0, bank
        count = ctypes.c_int()
        studio.FMOD_Studio_Bank_GetEventCount(handle, ctypes.byref(count))
        arr = (ctypes.c_void_p * max(1, count.value))()
        got = ctypes.c_int()
        studio.FMOD_Studio_Bank_GetEventList(handle, arr, count.value, ctypes.byref(got))
        for k in range(got.value):
            g = GUID()
            studio.FMOD_Studio_EventDescription_GetID(ctypes.c_void_p(arr[k]), ctypes.byref(g))
            text = f"{g.a:08x}-{g.b:04x}-{g.c:04x}-{bytes(g.d[:2]).hex()}-{bytes(g.d[2:]).hex()}"
            if text in by_guid:
                found.add(by_guid[text])
    studio.FMOD_Studio_System_Release(system)
    return found


gp_guids = guid_events(pack / "fmod" / "GUIDs.txt")
rr_guids = guid_events(trent / "GUIDs.txt")
gp_events = {k[len("event:"):] for k in gp_guids if k.startswith("event:")}
shared_banks = {"bank:/Ground", "bank:/Weather", "bank:/RREngines"}
shared = shared_bank_events([gp / "Master Bank.bank", fmod / "Ground.bank", fmod / "Weather.bank", fmod / "RREngines.bank"], rr_guids)
print("events in the shared RR banks:", len(shared))
merged = dict(gp_guids)
for k, line in rr_guids.items():
    if k not in merged and (k in shared_banks or k in shared):
        merged[k] = line
(gp / "GUIDs.txt").write_text("\n".join(merged.values()) + "\n", encoding="utf-8")

RENAME = {}
for e in "ABCD":
    for x in ("", "X"):
        RENAME[f"/Engines/RR/Engine{e}{x}"] = f"/Engines/Engine{e}{x}"
        RENAME[f"/Engines/RR/Start{e}{x}"] = f"/Engines/Start{e}{x}"
for e in "BC":
    for x in ("", "X"):
        RENAME[f"/Engines/RR/Rev{e}{x}"] = f"/Engines/Rev{e}{x}"
available = gp_events | {k[len("event:"):] for k in shared if k.startswith("event:")}


def convert(src, dst):
    raw = src.read_text(encoding="utf-8")
    nl = "\r\n" if "\r\n" in raw else "\n"
    lines = raw.replace("\r\n", "\n").split("\n")
    out, kept, dropped, i = [], 0, [], 0
    while i < len(lines):
        if lines[i].strip() == "BEGIN_SOUND_ATTACHMENT":
            j = i
            while lines[j].strip() != "END_SOUND_ATTACHMENT":
                j += 1
            blk = lines[i:j + 1]
            name = next((l.split()[1] for l in blk if l.strip().startswith("EVENT_NAME")), None)
            snapshot = any(l.strip().startswith("SNAPSHOT_NAME") for l in blk)
            if name in RENAME:
                blk = [l.replace(name, RENAME[name]) if l.strip().startswith("EVENT_NAME") else l for l in blk]
                name = RENAME[name]
            if snapshot or (name in available and not name.startswith("/Engines/RR/")):
                out.extend(blk)
                kept += 1
            else:
                dropped.append(name)
            i = j + 1
            continue
        if lines[i].startswith("REQUIRES_BANK RREngines.bank"):
            i += 1
            continue
        out.append(lines[i])
        i += 1
    body = "\n".join(out).replace("x_A380 Sound Pack (RR)", "x_A380 Sound Pack (GP7200) on the RR ground and weather banks")
    dst.write_text(body.replace("\n", nl), encoding="utf-8", newline="")
    print(f"{dst.name}: kept {kept}, dropped {len(dropped)}: {' '.join(sorted(set(dropped)))}")


for name in ("A380.snd",):
    convert(trent / name, gp / name)
