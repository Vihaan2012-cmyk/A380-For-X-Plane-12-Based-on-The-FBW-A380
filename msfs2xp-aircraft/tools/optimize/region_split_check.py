"""usage: region_split_check.py <original obj> <part obj>...

Checks that the parts hold exactly the original's triangles, each under the same animation blocks and attribute
state."""

import sys
from collections import Counter


def family(word):
    w = word[5:]
    if w.startswith("no_"):
        w = w[3:]
    for suffix in ("_reset",):
        if w.endswith(suffix):
            w = w[: -len(suffix)]
    if w in ("shadow_blend", "no_blend"):
        w = "blend"
    return w


def triangles(path):
    vt, idx, cmds, started = [], [], [], False
    for line in open(path, encoding="latin-1"):
        line = line.rstrip("\n")
        w = line.split()
        if w[:1] == ["VT"]:
            vt.append(tuple(w[1:]))
            started = True
        elif w[:1] == ["IDX10"]:
            idx.extend(int(x) for x in w[1:])
        elif w[:1] == ["IDX"]:
            idx.append(int(w[1]))
        elif started and w:
            cmds.append(line)
    out = Counter()
    stack, state = [[]], {}
    for line in cmds:
        w = line.split()
        if w[0] == "ANIM_begin":
            stack.append([])
        elif w[0] == "ANIM_end":
            stack.pop()
        elif w[0].startswith("ANIM_"):
            stack[-1].append(line)
        elif w[0].startswith("ATTR_"):
            state[family(w[0])] = line
        elif w[0] == "TRIS":
            o, n = int(w[1]), int(w[2])
            anim = tuple(tuple(s) for s in stack)
            st = tuple(sorted(state.items()))
            for t in range(o, o + n, 3):
                out[(tuple(vt[i] for i in idx[t:t + 3]), anim, st)] += 1
    return out


orig = triangles(sys.argv[1])
parts = Counter()
for p in sys.argv[2:]:
    parts.update(triangles(p))
missing, extra = orig - parts, parts - orig
print(f"{sum(orig.values())} original, {sum(parts.values())} in parts, {sum(missing.values())} missing, {sum(extra.values())} extra")
sys.exit(1 if missing or extra else 0)
