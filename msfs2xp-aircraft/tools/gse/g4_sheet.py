import os
import sys

from PIL import Image, ImageDraw, ImageFont

EXPORT = 'E:/fbw-gse/export'
VEHICLES = ['stairs_main', 'stairs_upper', 'cat_highloader', 'udcat_highloader']
COLS = [('min_front34', 'min height, front'), ('max_front34', 'max height, front'), ('min_side', 'min height, side'), ('max_side', 'max height, side'),
        ('min_door', 'min, platform edge'), ('max_door', 'max, platform edge'), ('dnata_front34', 'dnata'), ('swissport_front34', 'Swissport'), ('menzies_front34', 'Menzies')]
TW, TH = 520, 324


def main():
    font = ImageFont.truetype('C:/Windows/Fonts/arialbd.ttf', 20)
    small = ImageFont.truetype('C:/Windows/Fonts/arial.ttf', 16)
    sheet = Image.new('RGB', (TW * len(COLS), 30 + (TH + 28) * len(VEHICLES)), (24, 24, 26))
    d = ImageDraw.Draw(sheet)
    for ci, (_, label) in enumerate(COLS):
        d.text((ci * TW + 8, 4), label, fill=(230, 230, 230), font=small)
    for ri, v in enumerate(VEHICLES):
        y = 30 + ri * (TH + 28)
        d.text((8, y), v, fill=(255, 220, 90), font=font)
        for ci, (key, _) in enumerate(COLS):
            p = os.path.join(EXPORT, v, 'renders', f'{v}_{key}.png')
            if os.path.exists(p):
                im = Image.open(p).convert('RGB')
                im.thumbnail((TW - 4, TH))
                sheet.paste(im, (ci * TW + 2, y + 26))
    out = os.path.join(EXPORT, 'group4_contact.png')
    sheet.save(out)
    print(out, sheet.size)


main()
