import sys
from pathlib import Path

from PIL import Image

root = Path(sys.argv[1])
done = 0
for raw in sorted(root.rglob("*.rgba.png")):
    out = raw.with_name(raw.name.replace(".rgba.png", ".png"))
    if out.exists() and out.stat().st_mtime >= raw.stat().st_mtime:
        continue
    page = Image.open(raw).convert("RGBA")
    base = Image.new("RGBA", page.size, (0, 0, 0, 255))
    layer = raw.with_name(raw.name.replace(".rgba.png", ".layer.png"))
    if layer.exists():
        under = Image.open(layer).convert("RGBA").resize(page.size, Image.BILINEAR)
        base = Image.alpha_composite(base, under)
    Image.alpha_composite(base, page).convert("RGB").save(out)
    done += 1
print(f"{done} pictures flattened in {root}")
