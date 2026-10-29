import os
import sys

import numpy as np


def flipped(vt, tri):
    p, nrm = vt[:, :3], vt[:, 3:6]
    gn = np.cross(p[tri[:, 1]] - p[tri[:, 0]], p[tri[:, 2]] - p[tri[:, 0]])
    area = np.linalg.norm(gn, axis=1)
    d = np.stack([np.einsum("ij,ij->i", gn, nrm[tri[:, k]]) for k in range(3)], axis=1)
    return (d > 0).all(axis=1) & (area > 1e-5)


def fix(path, write):
    lines = open(path, encoding="utf-8", errors="replace").read().split("\n")
    vt = np.array([[float(v) for v in l.split()[1:7]] for l in lines if l.startswith("VT")])
    if not len(vt):
        return 0
    idx = np.array([int(v) for l in lines if l.startswith(("IDX10", "IDX ")) for v in l.split()[1:]], dtype=np.int64)
    n3 = len(idx) // 3 * 3
    tri = idx[:n3].reshape(-1, 3).copy()
    bad = flipped(vt, tri)
    if not bad.any() or not write:
        return int(bad.sum())
    tri[bad] = tri[bad][:, [0, 2, 1]]
    idx[:n3] = tri.ravel()
    body = [l for l in lines if not l.startswith(("IDX10", "IDX "))]
    at = next(i for i, l in enumerate(body) if l.startswith("VT")) + len(vt)
    rows = [f'IDX10 {" ".join(map(str, idx[i:i + 10]))}' for i in range(0, len(idx) - len(idx) % 10, 10)]
    rows += [f"IDX {v}" for v in idx[len(idx) - len(idx) % 10:]]
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(body[:at] + rows + body[at:]))
    return int(bad.sum())


if __name__ == "__main__":
    write = sys.argv[1] == "fix"
    tot = 0
    for path in sys.argv[2:]:
        n = fix(path, write)
        if n:
            print(os.path.basename(path), n)
        tot += n
    print("total", tot)
