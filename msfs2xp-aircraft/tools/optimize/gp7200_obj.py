import re
from pathlib import Path

import numpy as np

ENGSEP = re.compile(r"ANIM_hide 5\.5 6\.5 sim/operation/failures/rel_engsep(\d)")
COWL = re.compile(r"ANIM_hide 0\.5 1\.5 fbw/fail/sep/cowl_(\d)")


class Obj:
    def __init__(self, path):
        self.path = Path(path)
        lines = self.path.read_text(encoding="utf-8", errors="replace").split("\n")
        first = next(i for i, l in enumerate(lines) if l.startswith("VT"))
        last = max(i for i, l in enumerate(lines) if l.startswith(("VT", "IDX")))
        self.pre = [l for l in lines[:first] if not l.startswith("POINT_COUNTS")]
        self.post = lines[last + 1:]
        data = lines[first:last + 1]
        self.vt = np.array([[float(v) for v in l.split()[1:9]] for l in data if l.startswith("VT")]).reshape(-1, 8)
        self.idx = np.array([int(v) for l in data if l.startswith(("IDX10", "IDX ")) for v in l.split()[1:]], dtype=np.int64)

    def texture(self):
        return next((l.split()[1] for l in self.pre if l.split()[:1] == ["TEXTURE"]), None)

    def tris(self):
        out = []
        stack = []
        for i, l in enumerate(self.post):
            t = l.strip()
            if t == "ANIM_begin":
                stack.append([])
            elif t == "ANIM_end":
                if stack:
                    stack.pop()
            elif t.startswith("TRIS "):
                s, c = map(int, t.split()[1:3])
                out.append(dict(line=i, start=s, count=c, anim=[x for lv in stack for x in lv]))
            elif stack:
                stack[-1].append(t)
            else:
                pass
        return out

    def batch_arrays(self, b):
        tri = self.idx[b["start"]:b["start"] + b["count"]].reshape(-1, 3)
        return self.vt[tri]


def engine_of_batch(b):
    for t in b["anim"]:
        m = ENGSEP.match(t)
        if m:
            return int(m.group(1))
    return None


def has(b, text):
    return any(text in t for t in b["anim"])


def write_obj(path, pre, vt, idx, post):
    out = list(pre)
    while out and out[-1] == "":
        out.pop()
    count_line = f"POINT_COUNTS {len(vt)} 0 0 {len(idx)}"
    out.append(count_line)
    out.append("")
    out += ["VT " + " ".join(f"{v:.6g}" for v in r) for r in vt]
    full = len(idx) - len(idx) % 10
    out += ["IDX10 " + " ".join(map(str, idx[k:k + 10])) for k in range(0, full, 10)]
    out += [f"IDX {i}" for i in idx[full:]]
    out += [""] + post
    Path(path).write_text("\n".join(out), encoding="utf-8", newline="\n")
