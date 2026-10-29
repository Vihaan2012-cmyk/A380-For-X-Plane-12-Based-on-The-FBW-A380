"""usage: restore_lost.py <aircraft objects dir> <pre-bake objects dir> <ink dir> <text-free dir> [--dry-run]

The 12 Oct bake removed from the decal objects every lettering triangle it believed it had baked, but some
never reached a panel (both EFIS control panels share their texture space, so their labels were refused)
and went missing. This puts back, from the pre-bake decal objects, every triangle whose ink is not in the
panel texture under it: a ray along the decal's normal finds the panel triangle and its UV there, and the
baked panel (<ink dir>/<texture>) is compared with the same panel without lettering (<text-free dir>).
Decal triangles still present are left alone; the restored ones keep their own animation blocks."""

import glob
import os
import sys
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.spatial import cKDTree

DECALS = [f"a380_cockpit_{n:03d}.obj" for n in range(20, 26)]
INK = 24


def load(path):
    lines = Path(path).read_text(encoding="latin-1").split("\n")
    v = np.array([[float(x) for x in l.split()[1:9]] for l in lines if l.startswith("VT")])
    idx = np.array([int(x) for l in lines if l.startswith(("IDX10", "IDX ")) for x in l.split()[1:]], dtype=np.int64)
    tex = next((l.split()[1] for l in lines if l.startswith("TEXTURE ")), None)
    return lines, v, idx, tex


class Panels:
    def __init__(self, objects):
        tris, uvs, texs = [], [], []
        for f in glob.glob(os.path.join(objects, "a380_cockpit_*.obj")):
            b = os.path.basename(f)
            if b in DECALS or "_clean" in b or "click" in b:
                continue
            _, v, idx, tex = load(f)
            if not len(v) or not len(idx) or tex is None:
                continue
            t = idx[: len(idx) // 3 * 3].reshape(-1, 3)
            tris.append(v[t][:, :, :3])
            uvs.append(v[t][:, :, 6:8])
            texs += [tex] * len(t)
        self.p = np.concatenate(tris)
        self.uv = np.concatenate(uvs)
        self.tex = np.array(texs)
        self.tree = cKDTree(self.p.mean(axis=1))

    def under(self, o, d):
        k = np.array(self.tree.query_ball_point(o, 0.05))
        if not len(k):
            return None
        a, b, c = self.p[k, 0], self.p[k, 1], self.p[k, 2]
        e1, e2 = b - a, c - a
        h = np.cross(d, e2)
        det = np.einsum("ij,ij->i", e1, h)
        ok = np.abs(det) > 1e-14
        det = np.where(ok, det, 1)
        s = o - a
        u = np.einsum("ij,ij->i", s, h) / det
        q = np.cross(s, e1)
        w = (q @ d) / det
        t = np.einsum("ij,ij->i", e2, q) / det
        hit = ok & (u >= 0) & (w >= 0) & (u + w <= 1) & (t > 0) & (t < 0.03)
        if not hit.any():
            return None
        j = np.argmin(np.where(hit, t, np.inf))
        kk = k[j]
        uv = self.uv[kk, 0] + u[j] * (self.uv[kk, 1] - self.uv[kk, 0]) + w[j] * (self.uv[kk, 2] - self.uv[kk, 0])
        return self.tex[kk], uv


class Ink:
    def __init__(self, ink_dir, free_dir):
        self.ink_dir, self.free_dir, self.cache = Path(ink_dir), Path(free_dir), {}

    def maps(self, tex):
        if tex not in self.cache:
            a, b = self.ink_dir / tex, self.free_dir / tex
            if a.is_file() and b.is_file():
                x = np.asarray(Image.open(a).convert("RGB")).astype(np.int16)
                y = Image.open(b).convert("RGB")
                if y.size != (x.shape[1], x.shape[0]):
                    y = y.resize((x.shape[1], x.shape[0]), Image.BOX)
                self.cache[tex] = np.abs(x - np.asarray(y).astype(np.int16)).max(axis=2)
            else:
                self.cache[tex] = None
        return self.cache[tex]

    def present(self, tex, uvs):
        m = self.maps(tex)
        if m is None:
            return None
        h, w = m.shape
        px = np.clip((uvs[:, 0] % 1) * w, 0, w - 1).astype(int)
        py = np.clip((1 - uvs[:, 1] % 1) * h, 0, h - 1).astype(int)
        r = 2
        best = 0
        for x, y in zip(px, py):
            best = max(best, int(m[max(0, y - r):y + r + 1, max(0, x - r):x + r + 1].max()))
        return best > INK


BARY = np.array([(i / 6, j / 6, 1 - i / 6 - j / 6) for i in range(7) for j in range(7 - i)])


def atlas_ink(alpha, uv):
    h, w = alpha.shape
    return alpha[int(np.clip((1 - uv[1] % 1) * h, 0, h - 1)), int(np.clip((uv[0] % 1) * w, 0, w - 1))]


def triangles(lines, idx):
    out = []
    for i, l in enumerate(lines):
        t = l.strip()
        if t.startswith("TRIS "):
            a, c = map(int, t.split()[1:3])
            out.append((i, a, c))
    return out


def main(objects, prebake, ink_dir, free_dir, dry=False, region=None, only=None):
    panels = Panels(objects)
    ink = Ink(ink_dir, free_dir)
    atlases = {}
    for name in only or DECALS:
        pre_lines, pv, pidx, tex = load(Path(prebake) / name)
        if tex not in atlases:
            atlases[tex] = np.asarray(Image.open(Path(prebake) / tex).convert("RGBA"))[..., 3].astype(np.float32) / 255
        atlas = atlases[tex]
        cur_lines, cv, cidx, _ = load(Path(objects) / name)
        have = cKDTree(cv[cidx[: len(cidx) // 3 * 3].reshape(-1, 3)][:, :, :3].mean(axis=1)) if len(cidx) else None
        keep_lines = {}
        restored = unknown = 0
        for line_no, a, c in triangles(pre_lines, pidx):
            keep = []
            for k in range(c // 3):
                tri = pidx[a + 3 * k:a + 3 * k + 3]
                p = pv[tri, :3]
                cen = p.mean(axis=0)
                if have is not None and have.query(cen)[0] < 1e-5:
                    keep.append(tri)
                    continue
                if region is not None and not (np.all(cen > region[0]) and np.all(cen < region[1])):
                    continue
                n = pv[tri, 3:6].mean(axis=0)
                n /= np.linalg.norm(n)
                verdicts = []
                for w in BARY:
                    uv = w @ pv[tri, 6:8]
                    if atlas_ink(atlas, uv) < 0.5:
                        continue
                    hit = panels.under(w @ p + n * 0.01, -n)
                    if hit is not None:
                        verdicts.append(ink.present(hit[0], np.vstack([hit[1]])))
                    if len(verdicts) >= 6:
                        break
                known = [x for x in verdicts if x is not None]
                gone = False
                if not known:
                    unknown += any(atlas_ink(atlas, w @ pv[tri, 6:8]) >= 0.5 for w in BARY)
                else:
                    gone = sum(known) == 0
                if gone:
                    keep.append(tri)
                    restored += 1
            keep_lines[line_no] = keep
        cur_count = len(cidx) // 3
        print(f"{name}: {cur_count} decal triangles now, {restored} lost ones restored, {unknown} left out (no panel found or no text-free copy)")
        if dry or not restored:
            continue
        flat = []
        out = []
        for i, l in enumerate(pre_lines):
            if l.startswith(("IDX10", "IDX ")):
                continue
            if i in keep_lines:
                tris = keep_lines[i]
                if tris:
                    indent = l[: len(l) - len(l.lstrip())]
                    out.append(f"{indent}TRIS {len(flat)} {len(tris) * 3}")
                    for t in tris:
                        flat += t.tolist()
                continue
            out.append(l)
        body_start = next(i for i, l in enumerate(out) if l.startswith("VT"))
        vt_end = max(i for i, l in enumerate(out) if l.startswith("VT")) + 1
        full = len(flat) - len(flat) % 10
        rows = ["IDX10 " + " ".join(map(str, flat[k:k + 10])) for k in range(0, full, 10)] + [f"IDX {x}" for x in flat[full:]]
        out = out[:vt_end] + rows + out[vt_end:]
        for i, l in enumerate(out):
            if l.startswith("POINT_COUNTS"):
                p = l.split()
                out[i] = f"POINT_COUNTS {p[1]} {p[2]} {p[3]} {len(flat)}"
        cur_head = [l for l in cur_lines[: next(i for i, l in enumerate(cur_lines) if l.startswith("POINT_COUNTS"))]]
        pre_pc = next(i for i, l in enumerate(out) if l.startswith("POINT_COUNTS"))
        out = cur_head + out[pre_pc:]
        Path(objects, name).write_text("\n".join(out), encoding="latin-1", newline="\n")


if __name__ == "__main__":
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    r = next((x[len("--region="):] for x in sys.argv if x.startswith("--region=")), None)
    region = tuple(np.array([float(v) for v in s.split(",")]) for s in r.split(";")) if r else None
    only = next((x[len("--only="):].split(",") for x in sys.argv if x.startswith("--only=")), None)
    main(*a[:4], dry="--dry-run" in sys.argv or region is not None, region=region, only=only)
