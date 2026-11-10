"""What the X-Plane 12 A380 uses that X-Plane 11.55 lacks: XPLM functions named in the plugin's source,
XPLM functions every bundled .xpl imports, sim/ datarefs and commands named by the source, the OBJs, the
SASL Lua and the FMOD .snd, and the OBJ attributes and light names X-Plane 11 does not know.
python -I tools/xp11/audit.py [--xp11 DIR] [--aircraft DIR] [--src DIR]"""
import argparse
import re
import struct
from pathlib import Path

ap = argparse.ArgumentParser()
ap.add_argument("--xp11", default="D:/Steam Games/steamapps/common/X-Plane 11")
ap.add_argument("--xp12", default="D:/Steam Games/steamapps/common/X-Plane 12")
ap.add_argument("--aircraft", default="D:/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12")
ap.add_argument("--src", default=str(Path(__file__).resolve().parents[2] / "src"))
a = ap.parse_args()
xp11, xp12, acft, src = Path(a.xp11), Path(a.xp12), Path(a.aircraft), Path(a.src)


def names(path):
    out = set()
    for line in path.read_text(encoding="latin-1").splitlines()[2:]:
        w = line.split()
        if w:
            out.add(re.sub(r"\[\d*\]$", "", w[0]))
    return out


def pe_imports(path):
    """Imported function names per DLL of a PE32+ file."""
    data = path.read_bytes()
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    nsec = struct.unpack_from("<H", data, pe + 6)[0]
    opt = pe + 24
    magic = struct.unpack_from("<H", data, opt)[0]
    dirs = opt + (112 if magic == 0x20B else 96)
    imp_rva = struct.unpack_from("<I", data, dirs + 8)[0]
    secs = opt + struct.unpack_from("<H", data, pe + 20)[0]
    sections = [struct.unpack_from("<IIII", data, secs + 40 * i + 8) for i in range(nsec)]

    def off(rva):
        for vsize, vaddr, rsize, raddr in sections:
            if vaddr <= rva < vaddr + max(vsize, rsize):
                return rva - vaddr + raddr
        return None

    out = {}
    p = off(imp_rva)
    while p is not None:
        orig, _, _, name_rva, first = struct.unpack_from("<IIIII", data, p)
        if not name_rva:
            break
        dll = data[off(name_rva):].split(b"\0", 1)[0].decode()
        thunk = off(orig or first)
        funcs = []
        while thunk is not None:
            v = struct.unpack_from("<Q" if magic == 0x20B else "<I", data, thunk)[0]
            if not v:
                break
            if not v >> (63 if magic == 0x20B else 31):
                funcs.append(data[off(v & 0x7FFFFFFF) + 2:].split(b"\0", 1)[0].decode())
            thunk += 8 if magic == 0x20B else 4
        out[dll] = funcs
        p += 20
    return out


dll = (xp11 / "Resources/plugins/XPLM_64.dll").read_bytes()
exported = lambda f: f.encode() + b"\0" in dll
refs11 = names(xp11 / "Resources/plugins/DataRefs.txt")
cmds11 = names(xp11 / "Resources/plugins/Commands.txt")
cmds12 = names(xp12 / "Resources/plugins/Commands.txt")
rust = "\n".join(p.read_text(encoding="utf-8", errors="replace") for p in src.rglob("*.rs"))

funcs = sorted({f for f in re.findall(r'"(XPLM\w+)"', rust) if not exported(f)})
print(f"source XPLM functions missing in X-Plane 11: {len(funcs)}")
for f in funcs:
    print("F", f)

for xpl in sorted((acft / "plugins").rglob("*.xpl")):
    try:
        imps = pe_imports(xpl)
    except Exception as e:
        print(f"P {xpl.relative_to(acft)}: unreadable ({e})")
        continue
    xplm = [f for d, fs in imps.items() if d.lower().startswith("xplm") for f in fs]
    missing = [f for f in xplm if not exported(f)]
    print(f"P {xpl.relative_to(acft)}: {len(xplm)} XPLM imports, missing in X-Plane 11: {', '.join(missing) or 'none'}")

other = []
for g in ("objects/*.obj", "plugins/sasl/**/*.lua", "fmod/*.snd", "*.lua", "plugins/**/*.lua"):
    for p in acft.glob(g):
        other.append(p.read_text(encoding="latin-1"))
used = {re.sub(r"\[\d*\]$", "", s) for s in re.findall(r"\bsim/[A-Za-z0-9_/\[\]]+", rust + "\n".join(other))}
miss_d = sorted(s for s in used if s not in refs11 and s not in cmds11 and s not in cmds12)
miss_c = sorted(s for s in used if s not in refs11 and s not in cmds11 and s in cmds12)
print(f"datarefs missing in X-Plane 11: {len(miss_d)}")
for s in miss_d:
    print("D", s)
print(f"commands missing in X-Plane 11: {len(miss_c)}")
for s in miss_c:
    print("C", s)

pat = re.compile(r"^\s*(ATTR_cockpit_device|ATTR_manip_device|LIGHT_PARAM airplane_\w+_(?:pm|bb))\b", re.M)
for p in sorted((acft / "objects").glob("*.obj")):
    hits = pat.findall(p.read_text(encoding="latin-1"))
    if hits:
        print("O", p.name, len(hits))
acf = (acft / "A380.acf").read_text(encoding="latin-1")
print("obja entries:", len(set(re.findall(r"^P _obja/(\d+)/", acf, re.M))))
