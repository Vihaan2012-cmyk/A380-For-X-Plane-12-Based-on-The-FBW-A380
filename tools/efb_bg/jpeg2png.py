"""Convert the EFB background JPEGs to PNG assets for the native efbx display.

Inputs: app/ui/bg/*.jpg and the window.__BG__ data URL inlined in app/ui/index.html.
Output: <out>/<name>.png plus manifest.json (name, size, source, sha256 of the PNG).
The inlined background is written as inline.png.
"""

import argparse
import base64
import hashlib
import io
import json
import os
import re

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
UI = os.path.normpath(os.path.join(HERE, '..', '..', 'app', 'ui'))
INLINE = re.compile(r'window\.__BG__\s*=\s*window\.__BG__\s*\|\|\s*"data:image/jpeg;base64,([A-Za-z0-9+/=]+)"')


def to_png(data):
    img = Image.open(io.BytesIO(data))
    img = img.convert('RGB')
    out = io.BytesIO()
    img.save(out, 'PNG', optimize=False, compress_level=6)
    return img.size, out.getvalue()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--ui', default=UI)
    ap.add_argument('--out', default='E:/fbw-build/efb-native/bg')
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    sources = []
    bg_dir = os.path.join(args.ui, 'bg')
    for name in sorted(os.listdir(bg_dir)):
        if name.lower().endswith(('.jpg', '.jpeg')):
            with open(os.path.join(bg_dir, name), 'rb') as f:
                sources.append((os.path.splitext(name)[0], 'bg/' + name, f.read()))
    with open(os.path.join(args.ui, 'index.html'), encoding='utf-8') as f:
        m = INLINE.search(f.read())
    if m:
        sources.append(('inline', 'index.html window.__BG__', base64.b64decode(m.group(1))))
    manifest = []
    for name, src, data in sources:
        size, png = to_png(data)
        with open(os.path.join(args.out, name + '.png'), 'wb') as f:
            f.write(png)
        manifest.append({'name': name, 'file': name + '.png', 'width': size[0], 'height': size[1], 'source': src,
                         'sha256': hashlib.sha256(png).hexdigest()})
        print(f'{name}.png {size[0]}x{size[1]} {len(png)} bytes from {src}')
    with open(os.path.join(args.out, 'manifest.json'), 'w', encoding='utf-8', newline='\n') as f:
        json.dump(manifest, f, indent=1)
        f.write('\n')
    if not m:
        print('no inlined background found')


if __name__ == '__main__':
    main()
