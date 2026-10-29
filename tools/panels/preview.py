"""Offline check: panel art with control, lamp and screen rects drawn on top.

python preview.py [--panels E:/fbw-build/panels] [--only glareshield] [--lit]
"""
import argparse
import json
import os

from PIL import Image, ImageDraw

COLORS = {"button": (0, 220, 255), "switch": (255, 200, 0), "knob": (0, 255, 120), "rotary": (255, 90, 255)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--panels", default="E:/fbw-build/panels")
    ap.add_argument("--only", default="")
    ap.add_argument("--lit", action="store_true")
    args = ap.parse_args()
    only = [s for s in args.only.split(",") if s]
    out_dir = os.path.join(args.panels, "preview")
    os.makedirs(out_dir, exist_ok=True)
    for f in sorted(os.listdir(args.panels)):
        if not f.endswith(".json"):
            continue
        p = json.load(open(os.path.join(args.panels, f), encoding="utf-8"))
        if only and p["id"] not in only:
            continue
        w, h = p["size"]
        off = os.path.join(args.panels, p["art"])
        if not os.path.exists(off):
            print(f"{p['id']}: no art yet")
            continue
        base = Image.open(off).convert("RGB")
        lit_path = os.path.join(args.panels, p["art_lit"])
        if args.lit and os.path.exists(lit_path):
            lit = Image.open(lit_path).convert("RGB")
            mask = Image.new("L", base.size, 0)
            md = ImageDraw.Draw(mask)
            for L in p.get("lamps", []):
                md.rectangle(L["rect"], fill=255)
            base.paste(lit, (0, 0), mask)
        if base.size != (w, h):
            print(f"{p['id']}: art {base.size} != json size {(w, h)}")
        d = ImageDraw.Draw(base)
        for L in p.get("lamps", []):
            d.rectangle(L["rect"], outline=(255, 150, 0), width=1)
        for dev, r in p.get("screens", []):
            d.rectangle(r, outline=(255, 0, 0), width=3)
            d.text((r[0] + 4, r[1] + 4), dev, fill=(255, 0, 0))
        for c in p["controls"]:
            col = COLORS.get(c["kind"], (255, 255, 255))
            d.rectangle(c["rect"], outline=col, width=2)
            d.text((c["rect"][0] + 2, c["rect"][3] + 1), c["id"][:24], fill=col)
        out = os.path.join(out_dir, f"{p['id']}{'_lit' if args.lit else ''}.png")
        base.save(out)
        print(f"{p['id']}: {len(p['controls'])} controls, {len(p.get('lamps', []))} lamps -> {out}")


if __name__ == "__main__":
    main()
