import os
import sys

from PIL import Image, ImageDraw, ImageFont

WORK = 'E:/fbw-gse/export/_work4/liveries'
FONT = 'C:/Windows/Fonts/arialbd.ttf'

STYLES = {
    'dnata': dict(bg=(255, 255, 255), band=(210, 35, 42), ink=(0, 32, 91), label='dnata', stripe=(0, 32, 91)),
    'swissport': dict(bg=(226, 35, 26), band=(27, 27, 27), ink=(255, 255, 255), label='Swissport', stripe=(255, 255, 255)),
    'menzies': dict(bg=(29, 63, 122), band=(216, 30, 5), ink=(255, 255, 255), label='MENZIES', stripe=(255, 255, 255)),
    'neutral': dict(bg=(238, 239, 236), band=(150, 154, 158), ink=(238, 239, 236), label='', stripe=(200, 203, 205)),
}
SIZES = {
    'stairs_main': (1024, 256, False),
    'stairs_upper': (1024, 256, False),
    'cat_highloader': (2048, 960, True),
    'udcat_highloader': (2048, 960, True),
}


def fit_font(draw, text, width, height):
    size = height
    while size > 8:
        f = ImageFont.truetype(FONT, size)
        l, t, r, b = draw.textbbox((0, 0), text, font=f)
        if r - l <= width and b - t <= height:
            return f, (l, t, r, b)
        size -= 4
    f = ImageFont.truetype(FONT, 8)
    return f, draw.textbbox((0, 0), text, font=f)


def centered(draw, box, text, color, font_bbox=None):
    x0, y0, x1, y1 = box
    f, (l, t, r, b) = font_bbox or fit_font(draw, text, x1 - x0, y1 - y0)
    draw.text((x0 + (x1 - x0 - (r - l)) / 2 - l, y0 + (y1 - y0 - (b - t)) / 2 - t), text, font=f, fill=color)


def draw_stairs(name, style, w, h):
    img = Image.new('RGBA', (w, h), style['bg'] + (255,))
    d = ImageDraw.Draw(img)
    d.rectangle((0, int(h * 0.80), w, h), fill=style['band'])
    d.rectangle((0, int(h * 0.76), w, int(h * 0.80)), fill=style['stripe'])
    if style['label']:
        centered(d, (int(w * 0.05), int(h * 0.08), int(w * 0.95), int(h * 0.70)), style['label'], style['ink'])
    return img


def draw_catering(name, style, w, h):
    img = Image.new('RGBA', (w, h), style['bg'] + (255,))
    d = ImageDraw.Draw(img)
    d.rectangle((0, h - 230, w, h), fill=style['band'])
    d.rectangle((0, h - 262, w, h - 230), fill=style['stripe'])
    if style['label']:
        centered(d, (int(w * 0.06), int(h * 0.10), int(w * 0.94), int(h * 0.50)), style['label'], style['ink'])
        centered(d, (int(w * 0.06), int(h * 0.56), int(w * 0.60), int(h * 0.70)), 'CATERING', style['ink'])
    return img


def make(vehicle):
    w, h, catering = SIZES[vehicle]
    out = os.path.join(WORK, vehicle)
    os.makedirs(out, exist_ok=True)
    paths = {}
    for key, style in STYLES.items():
        img = (draw_catering if catering else draw_stairs)(key, style, w, h)
        path = os.path.join(out, f'{key}.png')
        img.save(path)
        paths[key] = path
    return paths


if __name__ == '__main__':
    for v in sys.argv[1:] or SIZES:
        print(v, make(v))
