import json
import sys

from PIL import Image, ImageDraw, ImageFont

FONT = 'C:/Windows/Fonts/arialbd.ttf'


def font(size):
    return ImageFont.truetype(FONT, size)


def fit(d, text, width, height, start=400):
    size = start
    while size > 8:
        f = font(size)
        box = d.textbbox((0, 0), text, font=f)
        if box[2] - box[0] <= width and box[3] - box[1] <= height:
            return f, box
        size -= 4
    return font(8), d.textbbox((0, 0), text, font=font(8))


def centred(d, text, area, fill):
    x0, y0, x1, y1 = area
    f, box = fit(d, text, x1 - x0, y1 - y0)
    d.text((x0 + (x1 - x0 - (box[2] - box[0])) / 2 - box[0], y0 + (y1 - y0 - (box[3] - box[1])) / 2 - box[1]), text, font=f, fill=fill)


def panel(path, s):
    w, h = s.get('w', 1024), s.get('h', 256)
    img = Image.new('RGBA', (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    main, accent = tuple(s['main']) + (255,), tuple(s['accent']) + (255,)
    band = int(h * 0.2)
    d.rectangle((0, h - band, w, h), fill=main)
    d.rectangle((0, h - band - int(h * 0.04), w, h - band), fill=accent)
    sub = s.get('subtitle', '')
    top = int(h * 0.04)
    if s['label']:
        lh = int(h * (0.50 if sub else 0.62))
        centred(d, s['label'], (int(w * 0.05), top, int(w * 0.95), top + lh), main)
        if sub:
            centred(d, sub, (int(w * 0.1), top + lh + int(h * 0.03), int(w * 0.9), h - band - int(h * 0.08)), (60, 62, 66, 255))
    elif sub:
        centred(d, sub, (int(w * 0.05), top, int(w * 0.95), h - band - int(h * 0.08)), main)
    img.save(path)


def door(path, s):
    w, h = s.get('w', 700), s.get('h', 420)
    img = Image.new('RGBA', (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    main, accent = tuple(s['main']) + (255,), tuple(s['accent']) + (255,)
    d.rounded_rectangle((0, 0, w - 1, h - 1), 40, fill=main)
    d.rectangle((0, int(h * 0.74), w, int(h * 0.80)), fill=accent)
    label = s['label'] or s.get('subtitle', '')
    centred(d, label, (int(w * 0.06), int(h * 0.1), int(w * 0.94), int(h * 0.68)), (255, 255, 255, 255))
    if s['label'] and s.get('subtitle'):
        centred(d, s['subtitle'], (int(w * 0.1), int(h * 0.83), int(w * 0.9), int(h * 0.96)), (255, 255, 255, 255))
    img.save(path)


def fuel_tank(path, s):
    w, h = 2400, 286
    img = Image.new('RGBA', (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    main, accent = tuple(s['main']) + (255,), tuple(s['accent']) + (255,)
    title = s['label'] or 'AVIATION FUEL'
    d.rectangle((0, h - 80, w, h), fill=main)
    d.rectangle((0, h - 100, w, h - 80), fill=accent)
    d.ellipse((30, 10, 200, 180), fill=main)
    d.polygon([(70, 150), (115, 50), (160, 150), (115, 125)], fill=accent)
    centred(d, title, (260, 10, 2300, 185), main)
    d.text((70, h - 76), 'JET A-1  /  AVIATION FUEL' if s['label'] else 'JET A-1', font=font(60), fill=(255, 255, 255, 255))
    img.save(path)


def sign(path, s):
    img = Image.new('RGBA', (400, 400), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.polygon([(200, 10), (390, 380), (10, 380)], fill=(250, 200, 0, 255), outline=(0, 0, 0, 255))
    d.rectangle((188, 120, 212, 270), fill=(0, 0, 0, 255))
    d.ellipse((186, 296, 214, 324), fill=(0, 0, 0, 255))
    img.save(path)


def plate(path, s):
    img = Image.new('RGBA', (900, 300), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle((0, 0, 899, 299), 20, fill=(255, 140, 0, 255), outline=(0, 0, 0, 255), width=8)
    centred(d, s['text'], (40, 30, 860, 270), (0, 0, 0, 255))
    img.save(path)


KINDS = dict(panel=panel, door=door, fuel_tank=fuel_tank, sign=sign, plate=plate)

if __name__ == '__main__':
    KINDS[sys.argv[1]](sys.argv[2], json.loads(sys.argv[3]))
