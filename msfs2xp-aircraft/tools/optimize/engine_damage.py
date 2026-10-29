import json
import re
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

SKIP = re.compile(r"ENGINE_BLUR")
CRUSH_FRACTION = 0.3
CRUSH_SQUASH = 0.35
DENTS = 7


def dents(rng, half):
    out = []
    for _ in range(DENTS):
        a = rng.uniform(0, 2 * np.pi)
        z = rng.uniform(-half[2] * 0.8, half[2] * 0.5)
        r = min(half[0], half[1]) * 0.95
        out.append((np.array([r * np.cos(a), r * np.sin(a) - half[1] * 0.2, z]), rng.uniform(0.5, 1.1), rng.uniform(0.12, 0.3)))
    return out


def deform(p, bottom, holes):
    q = p.copy()
    floor = bottom * (1 - CRUSH_FRACTION)
    low = q[:, 1] < floor
    q[low, 1] = floor + (q[low, 1] - floor) * CRUSH_SQUASH
    q[:, 0] *= 1 + 0.08 * np.clip((floor - p[:, 1]) / max(1e-6, -bottom), 0, 1)
    for centre, radius, depth in holes:
        d = np.linalg.norm(p - centre, axis=1)
        w = np.clip(1 - d / radius, 0, 1) ** 2
        radial = q[:, :2] / np.maximum(np.linalg.norm(q[:, :2], axis=1, keepdims=True), 1e-6)
        q[:, :2] -= radial * (w * depth)[:, None]
    q[:, 1] += p[:, 1].min() - q[:, 1].min()
    return q


def burnt(src, dst, seed):
    im = Image.open(src).convert("RGBA")
    w, h = im.size
    if w > 2048:
        im = im.resize((2048, 2048 * h // w), Image.LANCZOS)
        w, h = im.size
    rng = np.random.default_rng(seed)
    a = np.asarray(im).astype(np.float32)
    noise = Image.fromarray((rng.random((h // 8, w // 8)) * 255).astype(np.uint8)).resize((w, h), Image.BICUBIC).filter(ImageFilter.GaussianBlur(6))
    n = np.asarray(noise).astype(np.float32)[..., None] / 255
    soot = np.clip(0.15 + 0.5 * n, 0, 1)
    a[..., :3] = a[..., :3] * soot + np.array([24, 20, 18]) * (1 - soot)
    heat = np.clip((n - 0.7) * 3, 0, 1)
    a[..., :3] = a[..., :3] * (1 - heat * 0.5) + np.array([60, 48, 70]) * heat * 0.5
    Image.fromarray(np.clip(a, 0, 255).astype(np.uint8)).save(dst, optimize=True)


def variant(path, out, holes, bottom, textures=None):
    lines = path.read_text(encoding="utf-8", errors="replace").split("\n")
    vt = [i for i, l in enumerate(lines) if l.startswith("VT")]
    p = np.array([[float(v) for v in lines[i].split()[1:4]] for i in vt])
    q = deform(p, bottom, holes)
    for k, i in enumerate(vt):
        parts = lines[i].split()
        lines[i] = "VT " + " ".join(f"{v:.5f}" for v in q[k]) + " " + " ".join(parts[4:])
    if textures:
        lines = [textures.get(l, l) for l in lines]
    out.write_text("\n".join(lines), encoding="utf-8", newline="\n")


def main(objects_dir):
    objects = Path(objects_dir)
    manifest_path = objects / "a380_engdetach.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    burnt_names = {}
    engines = sorted(manifest["engines"].items()) + sorted(manifest.get("gp", {}).get("engines", {}).items())
    for key, m in engines:
        e = int(key)
        rng = np.random.default_rng(1000 + e)
        holes = dents(rng, np.array(m["half_extent"]))
        crushed, scorched = [], []
        for name in m["objects"]:
            path = objects / name
            text = path.read_text(encoding="utf-8", errors="replace")
            if SKIP.search(" ".join(l for l in text.split("\n")[:60] if l.startswith("TEXTURE"))):
                continue
            textures = {}
            for l in text.split("\n")[:60]:
                if l.startswith("TEXTURE ") and not l.startswith("TEXTURE_"):
                    file = l.split()[-1]
                    stem = Path(file).stem
                    out = f"{stem}_BURNT.png"
                    if file not in burnt_names and (objects / file).is_file():
                        burnt(objects / file, objects / out, len(burnt_names))
                        burnt_names[file] = out
                    if file in burnt_names:
                        textures[l] = f"TEXTURE {burnt_names[file]}"
            c = path.with_name(path.stem + "_crushed.obj")
            b = path.with_name(path.stem + "_burnt.obj")
            variant(path, c, holes, m["bottom"])
            variant(path, b, holes, m["bottom"], textures)
            crushed.append(c.name)
            scorched.append(b.name)
        m["crushed"] = crushed
        m["burnt"] = scorched
        print("engine", e + 1, len(crushed), "damaged objects")
    manifest_path.write_text(json.dumps(manifest, indent=1), encoding="utf-8")
    print("burnt textures:", ", ".join(burnt_names.values()))


if __name__ == "__main__":
    main(*sys.argv[1:])
