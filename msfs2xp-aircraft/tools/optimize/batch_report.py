"""usage: batch_report.py <objects dir> [glob]

Counts each OBJ's draw batches (TRIS commands) and how many would remain if every batch sharing the same
animation signature (the full chain of enclosing ANIM commands) and the same draw state (every ATTR_ in
force) were drawn as one."""

import glob
import os
import sys

STATE_RESET = {"ATTR_manip_none": "manip", "ATTR_light_level_reset": "light"}


def state_key(name):
    for k, v in (("ATTR_manip", "manip"), ("ATTR_light_level", "light"), ("ATTR_blend", "blend"), ("ATTR_no_blend", "blend"),
                 ("ATTR_shadow", "shadow"), ("ATTR_no_shadow", "shadow"), ("ATTR_cull", "cull"), ("ATTR_no_cull", "cull"),
                 ("ATTR_poly_os", "polyos"), ("ATTR_draped", "draped"), ("ATTR_hard", "hard"), ("ATTR_no_hard", "hard"),
                 ("ATTR_shiny_rat", "shiny"), ("ATTR_cockpit", "cockpit"), ("ATTR_no_cockpit", "cockpit"), ("ATTR_LOD", "lod"),
                 ("ATTR_draw_enable", "draw"), ("ATTR_draw_disable", "draw"), ("ATTR_solid_camera", "camera"), ("ATTR_no_solid_camera", "camera")):
        if name.startswith(k):
            return v
    return name


def analyse(path):
    stack = [[]]
    state = {}
    batches = 0
    keys = set()
    for line in open(path, encoding="latin-1", errors="replace"):
        w = line.split()
        if not w:
            continue
        c = w[0]
        if c == "ANIM_begin":
            stack.append([])
        elif c == "ANIM_end":
            stack.pop()
        elif c.startswith("ANIM_"):
            stack[-1].append(line.strip())
        elif c.startswith("ATTR_"):
            state[STATE_RESET.get(c, state_key(c))] = line.strip()
        elif c == "TRIS":
            batches += 1
            sig = tuple(tuple(s) for s in stack)
            keys.add((sig, tuple(sorted(state.items()))))
    return batches, len(keys)


def main(objects, pattern="a380_cockpit_*.obj"):
    total = mergeable = 0
    rows = []
    for p in sorted(glob.glob(os.path.join(objects, pattern))):
        b, k = analyse(p)
        total += b
        mergeable += k
        rows.append((b - k, b, k, os.path.basename(p)))
    rows.sort(reverse=True)
    for saved, b, k, n in rows[:25]:
        print(f"{n:40} batches {b:5} -> {k:5}  (-{saved})")
    print(f"TOTAL {total} batches -> {mergeable} after merging identical animation + state")


if __name__ == "__main__":
    main(*sys.argv[1:3])
