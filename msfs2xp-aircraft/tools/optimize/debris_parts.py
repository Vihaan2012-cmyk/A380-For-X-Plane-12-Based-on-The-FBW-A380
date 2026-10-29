import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

TEXTURE = "a380_debris.png"


def texture(path):
    rng = np.random.default_rng(7)
    w = h = 256
    base = np.zeros((h, w, 3), np.float32)
    streak = np.asarray(Image.fromarray((rng.random((h, 8)) * 255).astype(np.uint8)).resize((w, h), Image.BICUBIC)).astype(np.float32) / 255
    base[:, : w // 2] = (150 + 50 * streak[:, : w // 2, None]) * np.array([1.0, 1.0, 1.04])
    soot = np.asarray(Image.fromarray((rng.random((32, 16)) * 255).astype(np.uint8)).resize((w // 2, h), Image.BICUBIC).filter(ImageFilter.GaussianBlur(3))).astype(np.float32) / 255
    base[:, w // 2:] = (30 + 70 * soot[..., None]) * np.array([1.05, 0.95, 0.9])
    Image.fromarray(np.clip(base, 0, 255).astype(np.uint8)).save(path)


def prism(outline, thickness, u0, twist=0.0):
    outline = np.asarray(outline, float)
    n = len(outline)
    span = outline[:, 0].max() - outline[:, 0].min() + 1e-6
    verts, tris = [], []

    def add(p, nrm, uv):
        verts.append((*p, *nrm, *uv))
        return len(verts) - 1

    for side, sign in ((0, 1), (1, -1)):
        ids = []
        for x, z in outline:
            y = sign * thickness / 2 + twist * x * x
            ids.append(add((x, y, z), (0, sign, 0), (u0 + 0.45 * (x - outline[:, 0].min()) / span, 0.5 + z)))
        for k in range(1, n - 1):
            tris += [ids[0], ids[k], ids[k + 1]] if sign > 0 else [ids[0], ids[k + 1], ids[k]]
    for k in range(n):
        a, b = outline[k], outline[(k + 1) % n]
        e = b - a
        nrm = np.array([e[1], 0, -e[0]]) / (np.linalg.norm(e) + 1e-9)
        q = [add((a[0], thickness / 2 + twist * a[0] ** 2, a[1]), nrm, (u0 + 0.5, 0.1)),
             add((b[0], thickness / 2 + twist * b[0] ** 2, b[1]), nrm, (u0 + 0.5, 0.2)),
             add((b[0], -thickness / 2 + twist * b[0] ** 2, b[1]), nrm, (u0 + 0.52, 0.2)),
             add((a[0], -thickness / 2 + twist * a[0] ** 2, a[1]), nrm, (u0 + 0.52, 0.1))]
        tris += [q[0], q[2], q[1], q[0], q[3], q[2]]
    return verts, tris


def write(path, verts, tris):
    body = ["I", "800", "OBJ", "", f"TEXTURE {TEXTURE}", f"POINT_COUNTS {len(verts)} 0 0 {len(tris)}", ""]
    body += ["VT " + " ".join(f"{v:.4f}" for v in vt) for vt in verts]
    body += ["IDX10 " + " ".join(map(str, tris[k:k + 10])) for k in range(0, len(tris) - len(tris) % 10, 10)]
    body += [f"IDX {t}" for t in tris[len(tris) - len(tris) % 10:]]
    body += ["", "ATTR_no_cull", f"TRIS 0 {len(tris)}", ""]
    path.write_text("\n".join(body), encoding="utf-8", newline="\n")


def main(objects_dir):
    d = Path(objects_dir)
    texture(d / TEXTURE)
    blade = [(-0.22, -0.05), (0.2, -0.04), (0.24, 0.0), (0.12, 0.05), (-0.18, 0.06), (-0.24, 0.02)]
    chunk = [(-0.14, -0.1), (0.1, -0.12), (0.16, 0.02), (0.05, 0.13), (-0.12, 0.09)]
    plate = [(-0.3, -0.18), (-0.05, -0.22), (0.28, -0.12), (0.22, 0.05), (0.3, 0.17), (0.0, 0.2), (-0.26, 0.1)]
    write(d / "a380_debris_0.obj", *prism(blade, 0.012, 0.0, twist=0.6))
    write(d / "a380_debris_1.obj", *prism(chunk, 0.08, 0.5))
    write(d / "a380_debris_2.obj", *prism(plate, 0.006, 0.0, twist=-0.3))
    print("debris: 3 shapes,", TEXTURE)


if __name__ == "__main__":
    main(*sys.argv[1:])
