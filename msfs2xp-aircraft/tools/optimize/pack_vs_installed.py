"""usage: pack_vs_installed.py <pack objects dir> <installed objects dir> NAME...

For each texture: the pack copy scaled down to the installed size against the installed copy, so a pack
that predates an edit to the installed texture shows up before it is copied over."""
import sys
from pathlib import Path

import numpy as np
from PIL import Image

pack, inst = Path(sys.argv[1]), Path(sys.argv[2])
for name in sys.argv[3:]:
    a, b = Image.open(pack / name).convert("RGBA"), Image.open(inst / name).convert("RGBA")
    a = a.resize(b.size, Image.Resampling.BOX)
    d = np.abs(np.asarray(a).astype(np.int16) - np.asarray(b).astype(np.int16))[..., :3]
    print(f"{name:52} {a.size}->{b.size} mean diff {d.mean():5.2f}  >24: {100 * (d.max(axis=2) > 24).mean():5.2f}%  max {d.max()}")
