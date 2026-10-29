"""usage: wing_bend.py <objects dir> [names...]

Bends the wings with FBW's four WING_FLEX joints. Skin and wing panels are cut exactly on each joint
plane and every strip carries the full chain of the joints inboard of it, so neighbouring panels always
agree. Each joint hinges on the wing's lower surface when the angle is positive and on its upper
surface when negative, so the cut closes like a real hinge instead of opening a gap. Gear, RAT and
engine parts stay rigid and follow the joints by how far along them they reach. Files bent before are
unbent first (old chains removed, the duplicated overlap bands dropped), so the tool can be rerun."""

import re
import sys
from pathlib import Path

import numpy as np

STATIONS = [6.5, 14.55, 25.1, 32.05]
PIVOT_LOW = [-0.80, 1.33, 1.82, 1.98]
PIVOT_HIGH = [0.20, 1.70, 2.27, 2.36]
JOINTS = ["INBOARD", "INBOARD_MID", "OUTBOARD_MID", "OUTBOARD"]
MAX_Z = 22.0
EPS = 1e-5
SPLIT = re.compile(r"^a380_exterior_02[6-9](_hail)?\.obj$|^a380_sep_scars\.obj$")
RIGID = ("_w_gear", "RAT_STOW")
MARK = "fbw/anim/wing_flex/"
TRANSFORM = re.compile(r"^(ANIM_trans|ANIM_trans_begin|ANIM_rotate|ANIM_rotate_begin)\b")
PIVOT = re.compile(r"^\s*ANIM_trans (-?[\d.]+) (-?[\d.]+) 0 \1 \2 0\s*$")
FLEX = re.compile(r"^\s*ANIM_rotate_begin 0 0 -?1 fbw/anim/wing_flex/\S+\s*$")
KEY = re.compile(r"^\s*ANIM_rotate_key \S+ \S+\s*$")


class Tris:
    def __init__(self, indent, tri):
        self.indent = indent
        self.tri = tri


def dataref(side, j):
    return f"fbw/anim/wing_flex/{'left' if side < 0 else 'right'}_{JOINTS[j].lower()}"


def chain(side, wts):
    out = []
    axis = 1 if side > 0 else -1
    for j, w in enumerate(wts):
        if w <= 0:
            continue
        x = side * STATIONS[j]
        for y, keys in ((PIVOT_LOW[j], ((-30, 0), (0, 0), (30, 30 * w))), (PIVOT_HIGH[j], ((-30, -30 * w), (0, 0), (30, 0)))):
            out.append(f"ANIM_trans {x:.4f} {y:.4f} 0 {x:.4f} {y:.4f} 0")
            out.append(f"ANIM_rotate_begin 0 0 {axis} {dataref(side, j)}")
            out += [f"ANIM_rotate_key {v} {a:.3f}" for v, a in keys]
            out.append("ANIM_rotate_end")
            out.append(f"ANIM_trans {-x:.4f} {-y:.4f} 0 {-x:.4f} {-y:.4f} 0")
    return out


def full(q):
    return [1.0 if j < q else 0.0 for j in range(len(STATIONS))]


def weights(lo, hi):
    out = []
    for s in STATIONS:
        if s <= lo:
            out.append(1.0)
        elif s >= hi:
            out.append(0.0)
        else:
            out.append((hi - s) / (hi - lo))
    return out


def chain_end(lines, i):
    j = i
    while j + 1 < len(lines) and PIVOT.match(lines[j]) and FLEX.match(lines[j + 1]):
        k = j + 2
        while k < len(lines) and KEY.match(lines[k]):
            k += 1
        if k + 1 < len(lines) and lines[k].strip() == "ANIM_rotate_end" and PIVOT.match(lines[k + 1]):
            j = k + 2
        else:
            break
    return j


def tris_of(line, idx):
    t = line.strip().split()
    first, count = int(t[1]), int(t[2])
    return Tris(line[:len(line) - len(line.lstrip())], idx[first:first + count].reshape(-1, 3).copy())


def dedupe(tri):
    rot = np.argmin(tri, axis=1)
    canon = np.stack([tri[np.arange(len(tri)), (rot + k) % 3] for k in range(3)], axis=1)
    _, keep = np.unique(canon, axis=0, return_index=True)
    return tri[np.sort(keep)]


def unbend(lines, idx):
    out = []
    skin = None
    i = 0
    while i < len(lines):
        t = lines[i].strip()
        if t == "ANIM_begin" and i + 2 < len(lines) and MARK in lines[i + 2]:
            j = chain_end(lines, i + 1)
            k = j
            while k < len(lines) and lines[k].strip().startswith("TRIS "):
                k += 1
            if k > j and k < len(lines) and lines[k].strip() == "ANIM_end":
                parts = [tris_of(lines[m], idx) for m in range(j, k)]
                if skin is None:
                    skin = Tris(lines[i][:len(lines[i]) - len(lines[i].lstrip())], np.zeros((0, 3), np.int64))
                    out.append(skin)
                skin.tri = np.concatenate([skin.tri] + [p.tri for p in parts])
                i = k + 1
                continue
            if skin is not None:
                skin.tri = dedupe(skin.tri)
                skin = None
            out.append(lines[i])
            i = j
            continue
        if t.startswith("TRIS ") and skin is not None:
            skin.tri = np.concatenate([skin.tri, tris_of(lines[i], idx).tri])
            i += 1
            continue
        if skin is not None:
            skin.tri = dedupe(skin.tri)
            skin = None
        out.append(tris_of(lines[i], idx) if t.startswith("TRIS ") else lines[i])
        i += 1
    if skin is not None:
        skin.tri = dedupe(skin.tri)
    return out


class Mesh:
    def __init__(self, vt):
        self.base = len(vt)
        self.v = np.zeros((len(vt) * 2 + 1024, 8))
        self.v[:len(vt)] = vt
        self.n = len(vt)
        self.cut = {}

    @property
    def vt(self):
        return self.v[:self.n]

    @property
    def new(self):
        return self.v[self.base:self.n]

    def point(self, a, b, plane, c):
        key = (min(a, b), max(a, b), plane)
        if key not in self.cut:
            p, q = self.v[key[0]], self.v[key[1]]
            t = (p[0] - c) / (p[0] - q[0])
            v = p + (q - p) * t
            v[0] = c
            length = np.linalg.norm(v[3:6])
            if length > 0:
                v[3:6] /= length
            if self.n == len(self.v):
                self.v = np.concatenate([self.v, np.zeros_like(self.v)])
            self.v[self.n] = v
            self.cut[key] = self.n
            self.n += 1
        return self.cut[key]

    def clip(self, tri):
        if not len(tri):
            return tri
        for plane, c in enumerate(sorted({s * x for x in STATIONS for s in (-1, 1)})):
            xs = self.v[tri, 0] - c
            zs = self.v[tri, 2].mean(axis=1)
            cross = (xs.min(axis=1) < -EPS) & (xs.max(axis=1) > EPS) & (zs <= MAX_Z)
            if not cross.any():
                continue
            keep = [tri[~cross]]
            for t, d in zip(tri[cross], xs[cross]):
                keep.append(np.array(self.split(t, d, plane, c), np.int64).reshape(-1, 3))
            tri = np.concatenate(keep)
        return tri

    def split(self, t, d, plane, c):
        s = [0 if abs(v) <= EPS else (1 if v > 0 else -1) for v in d]
        for r in range(3):
            a, b, e = t[r], t[(r + 1) % 3], t[(r + 2) % 3]
            sa, sb, se = s[r], s[(r + 1) % 3], s[(r + 2) % 3]
            if sa == 0 and sb * se < 0:
                m = self.point(b, e, plane, c)
                return [(a, b, m), (a, m, e)]
            if sa != 0 and sb == se == -sa:
                ab, ae = self.point(a, b, plane, c), self.point(a, e, plane, c)
                return [(a, ab, ae), (ab, b, e), (ab, e, ae)]
        return [tuple(t)]

    def strips(self, tri):
        c = self.v[tri, :3].mean(axis=1)
        side = np.where(c[:, 0] > 0, 1, -1)
        q = np.searchsorted(STATIONS, np.abs(c[:, 0]), side="right")
        q[c[:, 2] > MAX_Z] = 0
        return side, q


def block_end(items, b):
    depth = 0
    for k in range(b + 1, len(items)):
        if isinstance(items[k], str):
            t = items[k].strip()
            if t == "ANIM_begin":
                depth += 1
            elif t == "ANIM_end":
                if depth == 0:
                    return k
                depth -= 1
    raise ValueError("unbalanced ANIM_begin")


def owners(items):
    stack, has_tf, owner = [], {}, {}
    for i, it in enumerate(items):
        if isinstance(it, Tris):
            owner[i] = next((b for b in stack if has_tf.get(b)), None)
            continue
        t = it.strip()
        if t == "ANIM_begin":
            stack.append(i)
            has_tf[i] = False
        elif t == "ANIM_end":
            if stack:
                stack.pop()
        elif TRANSFORM.match(t) and stack:
            has_tf[stack[-1]] = True
    return owner


def bend(items, mesh, splittable):
    owner = owners(items)
    parts = {}
    for i, b in owner.items():
        if b is not None:
            parts.setdefault(b, []).append(i)
    out = []
    stats = {"strips": 0, "split": 0, "rigid": 0}
    i = 0
    while i < len(items):
        it = items[i]
        if isinstance(it, Tris) and owner[i] is None and splittable:
            tri = mesh.clip(it.tri)
            side, q = mesh.strips(tri) if len(tri) else (np.zeros(0), np.zeros(0))
            for key in sorted({(int(s), int(k)) for s, k in zip(side, q)}, key=lambda t: (t[1] == 0, t)):
                sel = tri[(side == key[0]) & (q == key[1])]
                if key[1] == 0:
                    out.append(Tris(it.indent, sel))
                else:
                    out += ["ANIM_begin"] + chain(key[0], full(key[1])) + [Tris(it.indent, sel), "ANIM_end"]
                    stats["strips"] += 1
            i += 1
            continue
        if i in parts:
            end = block_end(items, i)
            members = parts[i]
            text = " ".join(x for x in items[i:end] if isinstance(x, str))
            rigid = not splittable or any(r in text for r in RIGID)
            if rigid:
                pts = np.concatenate([mesh.v[items[m].tri.ravel(), :3] for m in members])
                c = pts.mean(axis=0)
                w = weights(np.abs(pts[:, 0]).min(), np.abs(pts[:, 0]).max()) if c[2] <= MAX_Z else [0.0] * 4
                out.append(items[i])
                if any(v > 0 for v in w):
                    out += chain(1 if c[0] > 0 else -1, w)
                    stats["rigid"] += 1
                out += items[i + 1:end + 1]
                i = end + 1
                continue
            clipped = {m: mesh.clip(items[m].tri) for m in members}
            keyed = {m: mesh.strips(t) if len(t) else (np.zeros(0), np.zeros(0)) for m, t in clipped.items()}
            keys = sorted({(int(s), int(k)) for m in members for s, k in zip(*keyed[m])}, key=lambda t: (t[1] == 0, t))
            for key in keys:
                copy = [items[i]] + (chain(key[0], full(key[1])) if key[1] else [])
                for k in range(i + 1, end + 1):
                    x = items[k]
                    if isinstance(x, Tris):
                        side, q = keyed[k]
                        sel = clipped[k][(side == key[0]) & (q == key[1])]
                        if len(sel):
                            copy.append(Tris(x.indent, sel))
                    else:
                        copy.append(x)
                out += copy
            stats["split"] += len(keys) > 1
            i = end + 1
            continue
        out.append(it)
        i += 1
    return out, stats


def drop_empty_blocks(items):
    out = []
    for it in items:
        out.append(it)
        if isinstance(it, str) and it.strip() == "ANIM_end":
            k = len(out) - 2
            while k >= 0 and isinstance(out[k], str) and out[k].strip().startswith(("ANIM_hide", "ANIM_show")):
                k -= 1
            if k >= 0 and isinstance(out[k], str) and out[k].strip() == "ANIM_begin":
                del out[k:]
    return out


def process(path, prepare=None):
    lines = path.read_text(encoding="utf-8", errors="replace").split("\n")
    vt_rows = [i for i, l in enumerate(lines) if l.startswith("VT")]
    if not vt_rows:
        return None
    vt = np.array([[float(v) for v in lines[i].split()[1:9]] for i in vt_rows])
    idx = np.array([int(v) for l in lines if l.startswith(("IDX10", "IDX ")) for v in l.split()[1:]], dtype=np.int64)
    body = [l for l in lines if not l.startswith(("VT", "IDX10", "IDX "))]
    head_len = next(i for i, l in enumerate(lines) if l.startswith("VT"))
    items = unbend(body[head_len:], idx)
    mesh = Mesh(vt)
    if prepare:
        items = prepare(items, mesh)
    items, stats = bend(items, mesh, bool(SPLIT.match(path.name)))
    items = drop_empty_blocks([it for it in items if not (isinstance(it, Tris) and not len(it.tri))])
    new_idx, out = [], []
    at = 0
    for it in items:
        if isinstance(it, Tris):
            if not len(it.tri):
                continue
            out.append(f"{it.indent}TRIS {at} {it.tri.size}")
            new_idx.append(it.tri.ravel())
            at += it.tri.size
        else:
            out.append(it)
    flat = np.concatenate(new_idx) if new_idx else np.zeros(0, np.int64)
    head = []
    for l in body[:head_len]:
        if l.startswith("POINT_COUNTS"):
            p = l.split()
            l = f"POINT_COUNTS {len(mesh.vt)} {p[2]} {p[3]} {len(flat)}"
        head.append(l)
    rows = [lines[i] for i in vt_rows]
    rows += ["VT " + " ".join(f"{x:.6f}" for x in v) for v in mesh.new]
    rows += [f"IDX10 {' '.join(map(str, flat[k:k + 10]))}" for k in range(0, len(flat) - len(flat) % 10, 10)]
    rows += [f"IDX {v}" for v in flat[len(flat) - len(flat) % 10:]]
    path.write_text("\n".join(head + rows + out), encoding="utf-8", newline="\n")
    return f"{stats['strips']} skin strips, {stats['split']} panels cut, {stats['rigid']} rigid parts, +{len(mesh.new)} vertices, {len(idx) // 3} -> {len(flat) // 3} tris"


def main(objects_dir, *names):
    d = Path(objects_dir)
    files = [d / n for n in names] if names else sorted(d.glob("a380_exterior_*.obj")) + [d / "a380_sep_scars.obj"]
    for f in files:
        if f.is_file():
            r = process(f)
            if r:
                print(f.name, r)


if __name__ == "__main__":
    main(*sys.argv[1:])
