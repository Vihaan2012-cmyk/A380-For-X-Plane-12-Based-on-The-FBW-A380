import os
import sys

OPS = {
    'dnata': dict(label='dnata', main=(0, 72, 152), accent=(214, 36, 48)),
    'swissport': dict(label='SWISSPORT', main=(214, 28, 40), accent=(70, 72, 76)),
    'menzies': dict(label='MENZIES', main=(20, 40, 92), accent=(238, 112, 28)),
}
WHITE = (240, 240, 238)
FONTS = ('C:/Windows/Fonts/arialbd.ttf', 'arialbd.ttf', 'DejaVuSans-Bold.ttf')


def font(size):
    from PIL import ImageFont
    for n in FONTS:
        try:
            return ImageFont.truetype(n, size)
        except OSError:
            continue
    return ImageFont.load_default()


def fit(draw, text, max_w, max_h):
    size = max_h
    while size > 8:
        f = font(size)
        box = draw.textbbox((0, 0), text, font=f)
        if box[2] - box[0] <= max_w and box[3] - box[1] <= max_h:
            return f, box
        size -= 4
    f = font(8)
    return f, draw.textbbox((0, 0), text, font=f)


def band(path, w, h, op):
    from PIL import Image, ImageDraw
    s = OPS[op]
    im = Image.new('RGBA', (w, h), WHITE + (255,))
    d = ImageDraw.Draw(im)
    d.rectangle((0, int(h * 0.60), w, h), fill=s['main'] + (255,))
    d.rectangle((0, int(h * 0.52), w, int(h * 0.58)), fill=s['accent'] + (255,))
    f, box = fit(d, s['label'], int(w * 0.5), int(h * 0.42))
    d.text((int(w * 0.03) - box[0], int(h * 0.05) - box[1]), s['label'], font=f, fill=s['main'] + (255,))
    f2, box2 = fit(d, 'GROUND HANDLING', int(w * 0.3), int(h * 0.16))
    d.text((int(w * 0.97) - box2[2], int(h * 0.20) - box2[1]), 'GROUND HANDLING', font=f2, fill=s['main'] + (255,))
    im.save(path)


def panel(path, w, h, op):
    from PIL import Image, ImageDraw
    s = OPS[op]
    im = Image.new('RGBA', (w, h), s['main'] + (0,))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle((0, 0, w - 1, h - 1), int(h * 0.12), fill=s['main'] + (255,))
    d.rectangle((int(w * 0.04), int(h * 0.80), int(w * 0.96), int(h * 0.88)), fill=s['accent'] + (255,))
    f, box = fit(d, s['label'], int(w * 0.84), int(h * 0.50))
    d.text(((w - (box[2] - box[0])) // 2 - box[0], int(h * 0.14) - box[1]), s['label'], font=f, fill=(255, 255, 255, 255))
    im.save(path)


def label(path, w, h, op):
    from PIL import Image, ImageDraw
    s = OPS[op]
    im = Image.new('RGBA', (w, h), (255, 255, 255, 0))
    d = ImageDraw.Draw(im)
    f, box = fit(d, s['label'], int(w * 0.8), int(h * 0.52))
    d.text(((w - (box[2] - box[0])) // 2 - box[0], int(h * 0.12) - box[1]), s['label'], font=f, fill=(255, 255, 255, 255))
    d.rectangle((int(w * 0.10), int(h * 0.76), int(w * 0.90), int(h * 0.84)), fill=s['accent'] + (255,))
    d.rectangle((int(w * 0.10), int(h * 0.88), int(w * 0.90), int(h * 0.92)), fill=(255, 255, 255, 255))
    im.save(path)


def dollyband(path, w, h, op):
    from PIL import Image, ImageDraw
    s = OPS[op]
    im = Image.new('RGBA', (w, h), (255, 255, 255, 0))
    d = ImageDraw.Draw(im)
    d.rectangle((0, 0, w, h), fill=WHITE + (255,))
    d.rectangle((0, 0, w, int(h * 0.12)), fill=s['accent'] + (255,))
    f, box = fit(d, s['label'], int(w * 0.6), int(h * 0.62))
    d.text(((w - (box[2] - box[0])) // 2 - box[0], int(h * 0.22) - box[1]), s['label'], font=f, fill=s['main'] + (255,))
    im.save(path)


KINDS = {
    'belt_boom': (band, 2048, 104),
    'belt_chassis': (band, 1024, 192),
    'bag_hood': (panel, 1024, 472),
    'cart_curtain': (label, 2048, 612),
    'ldcl_side': (band, 2048, 150),
    'dolly_short': (dollyband, 1024, 100),
    'dolly_long': (dollyband, 2048, 100),
}


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for kind, (fn, w, h) in KINDS.items():
        for op in OPS:
            fn(os.path.join(out, f'{kind}_{op}.png'), w, h, op)


if __name__ == '__main__':
    main()
