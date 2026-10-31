"""Give every detached cowl piece an inside: reversed triangles on a flat texel, so a shell seen from within is not see-through."""
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from gp7200_obj import Obj, write_obj  # noqa: E402

MARKER = "# two-sided cowl piece"
INSIDE_TEXEL = {
    "A380_EXTERIOR_ENG_LH": (0.41187, 0.84497),
    "A380_EXTERIOR_ENG_RH": (0.84058, 0.41772),
    "A380_EXTERIOR_ENG_CORES_LH": (0.0361, 0.5894),
    "A380_EXTERIOR_ENG_CORES_RH": (0.0996, 0.5557),
}


def texel_of(texture):
    for prefix, uv in sorted(INSIDE_TEXEL.items(), key=lambda kv: -len(kv[0])):
        if texture.startswith(prefix):
            return uv
    raise ValueError(f"no inside texel for {texture}")


def two_sided(path):
    o = Obj(path)
    if any(l.strip() == MARKER for l in o.pre):
        return False
    tris = [i for i, l in enumerate(o.post) if l.strip().startswith("TRIS ")]
    assert len(tris) == 1, f"{path.name}: {len(tris)} TRIS lines"
    back = o.vt.copy()
    back[:, 3:6] *= -1
    back[:, 6:8] = texel_of(o.texture())
    tri = o.idx.reshape(-1, 3)
    reversed_idx = tri[:, ::-1].reshape(-1) + len(o.vt)
    idx = np.concatenate([o.idx, reversed_idx])
    post = list(o.post)
    post[tris[0]] = f"TRIS 0 {len(idx)}"
    write_obj(path, o.pre + [MARKER], np.vstack([o.vt, back]), idx, post)
    return True


def main(objects_dir):
    changed = 0
    pieces = sorted(Path(objects_dir).glob("a380_cowldetach_*.obj"))
    for p in pieces:
        changed += two_sided(p)
    print(f"two-sided cowl pieces: {changed} changed of {len(pieces)} in {objects_dir}")


if __name__ == "__main__":
    for d in sys.argv[1:]:
        main(d)
