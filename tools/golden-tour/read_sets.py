import glob
import json
import os
import sys

roots = sys.argv[2:]
out = sys.argv[1]
sets = {}


def flatten(j, prefix, out):
    if isinstance(j, bool):
        return
    if isinstance(j, (int, float)):
        out.add(prefix)
        return
    if not isinstance(j, dict):
        return
    for k, v in j.items():
        if "." in k:
            continue
        flatten(v, k if not prefix else prefix + "." + k, out)


for root in roots:
    for f in glob.glob(os.path.join(root, "*", "*.state.json")):
        display = os.path.basename(f)[: -len(".state.json")]
        with open(f, encoding="utf-8") as fh:
            s = json.load(fh)
        d = sets.setdefault(display, {"var": set(), "str": set(), "env": set()})
        for k in s.get("vars", {}):
            name, _, unit = k.rpartition("|")
            d["var"].add((name, unit))
        d["str"].update(s.get("strings", {}))
        flatten(s.get("env"), "", d["env"])
        flatten(s.get("frozenAt"), "frozenAt", d["env"])
lines = []
for display in sorted(sets):
    d = sets[display]
    for name, unit in sorted(d["var"]):
        lines.append(f"{display}\tvar\t{name}\t{unit}")
    for k in sorted(d["str"]):
        lines.append(f"{display}\tstr\t{k}")
    for k in sorted(d["env"]):
        lines.append(f"{display}\tenv\t{k}")

with open(out, "w", encoding="utf-8", newline="\n") as fh:
    fh.write("\n".join(lines) + "\n")
print(len(sets), "displays", len(lines), "lines")
