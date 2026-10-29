import os
import sys

OPERATORS = {
    'dnata': dict(text='dnata', accent=(200, 30, 45), ink=(200, 30, 45), plate_ink=(200, 30, 45)),
    'swissport': dict(text='SWISSPORT', accent=(0, 70, 150), ink=(0, 70, 150), plate_ink=(0, 70, 150)),
    'menzies': dict(text='MENZIES', accent=(240, 130, 20), ink=(40, 40, 40), plate_ink=(40, 40, 40)),
}
NEUTRAL_ACCENT = (96, 100, 104)
FONT = 'C:/Windows/Fonts/arialbd.ttf'


def _font(size):
    from PIL import ImageFont
    try:
        return ImageFont.truetype(FONT, size)
    except OSError:
        return ImageFont.load_default()


def panel_image(path, operator, size, plate=False, subtitle=None):
    from PIL import Image, ImageDraw
    w, h = size
    spec = OPERATORS.get(operator)
    accent = spec['accent'] if spec else NEUTRAL_ACCENT
    img = Image.new('RGBA', (w, h), (255, 255, 255, 255) if plate else (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    band = max(6, int(h * 0.17))
    d.rectangle([0, h - band, w, h], fill=accent + (255,))
    d.rectangle([0, 0, w, max(3, int(h * 0.05))], fill=accent + (255,))
    sub_h = int(h * 0.16) if subtitle else 0
    if subtitle:
        sfont = _font(sub_h)
        l, t, r, b = d.textbbox((0, 0), subtitle, font=sfont)
        if r - l > w * 0.9:
            sfont = _font(int(sub_h * w * 0.9 / (r - l)))
            l, t, r, b = d.textbbox((0, 0), subtitle, font=sfont)
        d.text(((w - (r - l)) / 2 - l, h - band - sub_h * 1.25 - t), subtitle, font=sfont, fill=(spec['plate_ink'] if spec and plate else (spec['ink'] if spec else (60, 60, 60))) + (255,))
    if spec:
        avail_h = (h - band - sub_h * 1.4) * 0.78
        fs = int(avail_h)
        font = _font(fs)
        l, t, r, b = d.textbbox((0, 0), spec['text'], font=font)
        if r - l > w * 0.9:
            fs = int(fs * w * 0.9 / (r - l))
            font = _font(fs)
            l, t, r, b = d.textbbox((0, 0), spec['text'], font=font)
        x = (w - (r - l)) / 2 - l
        y = (h - band - sub_h * 1.4 - (b - t)) / 2 - t
        ink = spec['plate_ink'] if plate else spec['ink']
        d.text((x, y), spec['text'], font=font, fill=ink + (255,))
    os.makedirs(os.path.dirname(path), exist_ok=True)
    img.save(path)
    return path




PANELS = {
    'gpu_tld409': {'side': ((1024, 364), False, None)},
    'asu_unit': {'side': ((1536, 240), False, None)},
    'pca_unit': {'side': ((1536, 260), False, None)},
    'tow_tpx500': {'side': ((1536, 387), True, None)},
    'fuel_hydrant': {'side': ((2048, 550), True, 'AVIATION FUEL  JET A-1  HYDRANT DISPENSER')},
}


def panel_path(out_dir, tag, operator):
    return os.path.join(out_dir, 'liveries', f'{operator or "default"}_{tag}.png')


def draw_all(name, out_dir):
    for tag, (size, plate, subtitle) in PANELS[name].items():
        for key in (None,) + tuple(OPERATORS):
            panel_image(panel_path(out_dir, tag, key), key, size, plate, subtitle)


if __name__ == '__main__':
    draw_all(sys.argv[1], sys.argv[2])
