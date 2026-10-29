import sys

from PIL import Image, ImageDraw, ImageFont

VEHICLES = ['fuel_tanker', 'water_truck', 'lav_truck', 'clean_van', 'deice_truck']
LIVERIES = ['default', 'dnata', 'swissport', 'menzies']
W, H = 480, 300
LABEL = 28


def main():
    checks, out = sys.argv[1], sys.argv[2]
    font = ImageFont.truetype('C:/Windows/Fonts/arialbd.ttf', 18)
    sheet = Image.new('RGB', (W * len(LIVERIES), (H + LABEL) * len(VEHICLES)), (30, 30, 32))
    d = ImageDraw.Draw(sheet)
    for r, v in enumerate(VEHICLES):
        for c, lv in enumerate(LIVERIES):
            x, y = c * W, r * (H + LABEL)
            d.text((x + 8, y + 4), f'{v} / {lv}', font=font, fill=(235, 235, 235))
            try:
                img = Image.open(f'{checks}/{v}_{lv}.png').convert('RGB').resize((W, H), Image.LANCZOS)
                sheet.paste(img, (x, y + LABEL))
            except FileNotFoundError:
                d.text((x + 8, y + LABEL + 8), 'missing', font=font, fill=(255, 90, 90))
    sheet.save(out)


main()
