import os
import sys

from PIL import Image, ImageDraw, ImageFont

EXPORT = 'E:/fbw-gse/export'
VEHICLES = ['bag_tractor', 'bulk_cart', 'belt_loader', 'ldcl_loader', 'uld_set']
COLUMNS = ['default', 'dnata', 'swissport', 'menzies']
CELL = (560, 373)


def font(size):
    for n in ('C:/Windows/Fonts/arialbd.ttf', 'arialbd.ttf'):
        try:
            return ImageFont.truetype(n, size)
        except OSError:
            continue
    return ImageFont.load_default()


def main():
    shots = os.path.join(EXPORT, '_g3', 'renders')
    rows = []
    for v in VEHICLES:
        cells = []
        for col in COLUMNS:
            p = os.path.join(shots, f'{v}_{col}.png')
            cells.append((f'{v} {col}', p))
        rows.append(cells)
    extra = []
    for v in VEHICLES:
        for name in sorted(os.listdir(shots)):
            if name.startswith(f'{v}_pose'):
                extra.append((name[:-4], os.path.join(shots, name)))
    cols = len(COLUMNS)
    extra_rows = (len(extra) + cols - 1) // cols
    total_rows = len(rows) + extra_rows
    sheet = Image.new('RGB', (cols * CELL[0], total_rows * CELL[1]), (40, 40, 44))
    d = ImageDraw.Draw(sheet)
    f = font(22)
    allcells = [c for r in rows for c in r] + extra
    for i, (label, path) in enumerate(allcells):
        x, y = (i % cols) * CELL[0], (i // cols) * CELL[1]
        if os.path.exists(path):
            im = Image.open(path).convert('RGB')
            im.thumbnail(CELL)
            sheet.paste(im, (x + (CELL[0] - im.width) // 2, y + (CELL[1] - im.height) // 2))
        else:
            d.text((x + 20, y + 20), 'missing', font=f, fill=(255, 80, 80))
        d.rectangle((x, y, x + 12 + len(label) * 12, y + 28), fill=(0, 0, 0))
        d.text((x + 6, y + 2), label, font=f, fill=(255, 255, 255))
    out = os.path.join(EXPORT, 'group3_contact.png')
    sheet.save(out)
    print(out, sheet.size)


if __name__ == '__main__':
    main()
