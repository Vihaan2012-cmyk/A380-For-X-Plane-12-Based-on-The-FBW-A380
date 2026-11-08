"""usage: region_split.py <aircraft dir> <backup dir> [--dry-run] [--regions N]

Cuts the big flight-deck objects into a few regions of the cockpit, so X-Plane's per-object view culling skips
what is behind the camera (FlightFactor builds its cockpit from per-section objects). Geometry aft of the
flight-deck door goes into its own part, hidden by fbw/anim/cabin_hidden. The largest part keeps the original
file name and acf slot; the others get new slots copying its acf properties. Objects are copied to the backup
dir first; re-running on a split install does nothing."""

import re
import shutil
import sys
from pathlib import Path

import numpy as np

CABIN_AFT_OF = -29.8
CABIN = "fbw/anim/cabin_hidden"
MIN_OBJECT_TRIS = 20000
MIN_PART_TRIS = 2000
SKIP = {"a380_cockpit_000.obj", "a380_cockpit_003.obj", "a380_cockpit_click.obj", "a380_cockpit_042_lamps.obj"}
CLICK_FLAG, TRANSLUCENT_FLAG = 0x800, 0x2000
UNSAFE = ("ATTR_cockpit", "ATTR_manip", "LIGHT", "VLIGHT", "LINES", "VLINE", "SMOKE", "MAGNET", "EMITTER")


class Obj:
    def __init__(self, path):
        self.lines = path.read_text(encoding="latin-1").split("\n")
        self.header, self.vt, idx, self.commands = [], [], [], []
        stage = 0
        for line in self.lines:
            w = line.split()
            head = w[0] if w else ""
            if head == "VT":
                stage = 1
                self.vt.append(line)
            elif head == "IDX10":
                stage = 1
                idx.extend(int(x) for x in w[1:])
            elif head == "IDX":
                stage = 1
                idx.append(int(w[1]))
            elif stage == 0:
                self.header.append(line)
            else:
                self.commands.append(line)
        self.idx = np.array(idx, dtype=np.int64)
        pos = np.array([[float(x) for x in l.split()[1:4]] for l in self.vt])
        self.centroids = pos[self.idx[: len(self.idx) // 3 * 3].reshape(-1, 3)].mean(axis=1)

    def unsafe(self):
        return any(l.split()[:1] and l.split()[0].startswith(UNSAFE) for l in self.commands)

    def tree(self):
        root, stack = [], []
        cur = root
        for line in self.commands:
            w = line.split()
            if w[:1] == ["ANIM_begin"]:
                node = ["block", [line]]
                cur.append(node)
                stack.append(cur)
                cur = node[1]
            elif w[:1] == ["ANIM_end"]:
                cur.append(line)
                cur = stack.pop()
            elif w[:1] == ["TRIS"]:
                cur.append(("tris", int(w[1]), int(w[2])))
            else:
                cur.append(line)
        return root

    def write(self, path, keep):
        """Writes the triangles whose mask is set, dropping animation blocks left without geometry."""
        remap, vt, idx = {}, [], []

        def emit(nodes):
            out, geo = [], False
            for n in nodes:
                if isinstance(n, str):
                    out.append(n)
                elif n[0] == "tris":
                    t = np.arange(n[1] // 3, (n[1] + n[2]) // 3)
                    t = t[keep[t]]
                    if len(t):
                        start = len(idx)
                        for v in self.idx[(t[:, None] * 3 + np.arange(3)).ravel()]:
                            v = int(v)
                            if v not in remap:
                                remap[v] = len(vt)
                                vt.append(self.vt[v])
                            idx.append(remap[v])
                        out.append(f"TRIS {start} {len(idx) - start}")
                        geo = True
                else:
                    body, g = emit(n[1])
                    if g:
                        out.extend(body)
                        geo = True
                    else:
                        out.extend(l for l in body if l.split()[:1] and l.split()[0].startswith("ATTR_"))
            return out, geo

        commands, _ = emit(self.tree())
        header = [f"POINT_COUNTS {len(vt)} 0 0 {len(idx)}" if l.startswith("POINT_COUNTS") else l for l in self.header]
        body = []
        for i in range(0, len(idx) - len(idx) % 10, 10):
            body.append("IDX10 " + " ".join(str(x) for x in idx[i:i + 10]))
        for x in idx[len(idx) - len(idx) % 10:]:
            body.append(f"IDX {x}")
        path.write_text("\n".join(header + vt + body + commands), encoding="latin-1", newline="\n")


def acf_objects(text):
    props = {}
    for n, k, v in re.findall(r"^P _obja/(\d+)/(\S+) ?(.*)$", text, re.M):
        props.setdefault(int(n), {})[k] = v
    return props


def kmeans(points, k, seed=1):
    rng = np.random.default_rng(seed)
    centers = points[rng.choice(len(points), k, replace=False)]
    for _ in range(40):
        lab = np.argmin(((points[:, None, :] - centers[None]) ** 2).sum(-1), axis=1)
        centers = np.array([points[lab == i].mean(0) if (lab == i).any() else centers[i] for i in range(k)])
    return centers


def main(aircraft_dir, backup_dir, dry, k):
    aircraft, backup = Path(aircraft_dir), Path(backup_dir)
    acf_path = aircraft / "A380.acf"
    text = acf_path.read_text(encoding="latin-1")
    props = acf_objects(text)
    existing = {p.get("_v10_att_file_stl", "").strip() for p in props.values()}
    if any(re.search(r"^a380_cockpit_.*(_r\d+|_cabin)\.obj$", e) for e in existing):
        print("already split; restore the backup to split again")
        return
    candidates = {}
    for n, p in props.items():
        name = p.get("_v10_att_file_stl", "").strip()
        flags = int(p.get("_obj_flags", "0") or 0)
        if not name.startswith("a380_cockpit_") or name in SKIP or p.get("_obj_hide_dataref") or flags & (CLICK_FLAG | TRANSLUCENT_FLAG):
            continue
        if re.search(r"_r\d+\.obj$|_cabin\.obj$", name) or any(e.startswith(name[:-4] + "_r") for e in existing):
            continue
        path = aircraft / "objects" / name
        if not path.is_file():
            continue
        obj = Obj(path)
        if len(obj.centroids) < MIN_OBJECT_TRIS or obj.unsafe():
            continue
        candidates[n] = (name, obj)
    deck = np.concatenate([o.centroids[o.centroids[:, 2] < CABIN_AFT_OF] for _, o in candidates.values()])
    centers = kmeans(deck, k)
    plan = []
    for n, (name, obj) in sorted(candidates.items()):
        c = obj.centroids
        cabin = c[:, 2] >= CABIN_AFT_OF
        lab = np.argmin(((c[:, None, :] - centers[None]) ** 2).sum(-1), axis=1)
        lab[cabin] = -1
        counts = {int(r): int((lab == r).sum()) for r in np.unique(lab)}
        kept = [r for r, m in counts.items() if r >= 0 and m >= MIN_PART_TRIS]
        if not kept:
            kept = [max((r for r in counts if r >= 0), key=lambda r: counts[r])]
        for r in counts:
            if r >= 0 and r not in kept:
                sel = lab == r
                d = ((c[sel][:, None, :] - centers[kept][None]) ** 2).sum(-1)
                lab[sel] = np.array(kept)[np.argmin(d, axis=1)]
        if cabin.sum() < MIN_PART_TRIS:
            lab[cabin] = kept[0] if len(kept) == 1 else np.array(kept)[np.argmin(((c[cabin][:, None, :] - centers[kept][None]) ** 2).sum(-1), axis=1)]
        parts = [(r, int((lab == r).sum())) for r in sorted(set(lab.tolist()))]
        if len(parts) < 2:
            continue
        parts.sort(key=lambda p: (p[0] == -1, -p[1]))
        plan.append((n, name, obj, lab, parts))
        print(f"{name:24} {len(c):8,d} tris -> " + ", ".join(f"{'cabin' if r < 0 else 'r' + str(r)} {m:,d}" for r, m in parts))
    print(f"{len(plan)} objects split into {sum(len(p[4]) for p in plan)} parts")
    if dry:
        return
    backup.mkdir(parents=True, exist_ok=True)
    if not (backup / "A380.acf").exists():
        shutil.copy2(acf_path, backup / "A380.acf")
    count = int(re.search(r"^P _obja/count (\d+)", text, re.M)[1])
    new_lines = []
    for n, name, obj, lab, parts in plan:
        if not (backup / name).exists():
            shutil.copy2(aircraft / "objects" / name, backup / name)
        stem = name[:-4]
        for i, (r, _) in enumerate(parts):
            out = name if i == 0 else (f"{stem}_cabin.obj" if r < 0 else f"{stem}_r{r}.obj")
            obj.write(aircraft / "objects" / out, lab == r)
            if i == 0:
                continue
            for key, value in props[n].items():
                v = out if key == "_v10_att_file_stl" else value
                new_lines.append(f"P _obja/{count}/{key} {v}".rstrip())
            if r < 0:
                new_lines.append(f"P _obja/{count}/_obj_hide_dataref {CABIN}")
            count += 1
    text = re.sub(r"^P _obja/count \d+$", f"P _obja/count {count}", text, flags=re.M)
    anchor = re.search(r"^P _obja/count \d+$", text, re.M)
    text = text[:anchor.end()] + "\n" + "\n".join(new_lines) + text[anchor.end():]
    acf_path.write_text(text, encoding="latin-1", newline="\n")
    print(f"A380.acf: {count} objects")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    k = int(sys.argv[sys.argv.index("--regions") + 1]) if "--regions" in sys.argv else 10
    if "--regions" in sys.argv:
        args = [a for a in args if a != str(k)]
    main(args[0], args[1], "--dry-run" in sys.argv, k)
