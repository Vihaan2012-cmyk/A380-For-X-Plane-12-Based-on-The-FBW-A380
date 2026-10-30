"""Smoke test for a built kit: open the kit's .blend headless and run it.

blender -b "<kit>/A380 3D Paint Kit.blend" --enable-autoexec --python smoke_test.py -- <aircraft folder> [livery] [export name]
"""
import json
import os
import struct
import sys

import bpy
import numpy as np

args = sys.argv[sys.argv.index('--') + 1:]
aircraft = args[0]
livery = args[1] if len(args) > 1 else 'Emirates A6-EDY'
export_name = args[2] if len(args) > 2 else 'Kit Test'
kit = os.path.dirname(bpy.data.filepath)
sys.path.insert(0, os.path.join(kit, 'scripts'))
import kit_dds

failures = []


def check(ok, what):
    print(('ok   ' if ok else 'FAIL ') + what)
    if not ok:
        failures.append(what)


scene = bpy.context.scene
s = getattr(scene, 'a380_kit', None)
check(s is not None, 'Kit panel registered from the text in the .blend')
if s is None:
    sys.exit(1)
sheets = json.loads(bpy.data.texts['kit_sheets.json'].as_string())['sheets']
exterior = [x for x in sheets if x['role'] == 'exterior']
print('paint sheets', len(sheets), 'exterior', len(exterior))

s.aircraft_dir = aircraft
names = sorted({n[:-3] if n.endswith(' 4K') else n for n in os.listdir(os.path.join(aircraft, 'liveries'))})
selectable = 0
for n in names:
    try:
        s.livery = f'ac:{n}'
        selectable += s.livery == f'ac:{n}'
    except TypeError:
        pass
check(selectable == len(names), f'{selectable} of {len(names)} installed livery names selectable in the Load livery list')
s.livery = f'ac:{livery}'
s.prefer_4k = True
bpy.ops.a380_kit.load_livery()
loaded = {x['stem']: tuple(bpy.data.images[x['stem']].size) for x in sheets}
print('paint image sizes after loading:', sorted({v for v in loaded.values()}))
check(s.registration != '', f'identity read from the livery: {s.registration!r} / {s.airline!r} / {s.export_engine}')

fuse = bpy.data.images['A380X_FUSE1_ALBEDO_KE7E7E7']
check(tuple(fuse.size) == (4096, 4096), f'FUSE1 paint image is {tuple(fuse.size)} after loading the 4K livery')
bpy.ops.a380_kit.reset_paint(mode='WHITE', scope='EXTERIOR')
px = np.empty(4096 * 4096 * 4, np.float32)
fuse.pixels.foreach_get(px)
check(bool((px == 1.0).all()), 'Reset to white makes the body sheets plain white')
bpy.ops.a380_kit.reset_paint(mode='BASE', scope='ALL')
base = kit_dds.read_dds(os.path.join(aircraft, 'objects', 'A380X_FUSE1_ALBEDO_KE7E7E7.dds'))
px = np.empty(2048 * 2048 * 4, np.float32)
fuse.pixels.foreach_get(px)
now = np.clip(px.reshape(2048, 2048, 4)[::-1] * 255 + 0.5, 0, 255).astype(np.uint8)
check(tuple(fuse.size) == (2048, 2048) and int(np.abs(now.astype(int) - base.astype(int)).max()) == 0, 'Reset to base puts the stock 2048 FUSE1 back')
bpy.ops.a380_kit.load_livery()
for engine in ('GP', 'TRENT', 'GP'):
    s.engine = engine
    trent, gp = bpy.data.collections['Engines - Trent 900'], bpy.data.collections['Engines - GP7200']
    check(gp.hide_viewport == (engine != 'GP') and trent.hide_viewport == (engine == 'GP'), f'engine switch to {engine}: Trent hidden={trent.hide_viewport}, GP hidden={gp.hide_viewport}')
    print('   export engine follows:', s.export_engine, s.model)

s.show_lines = True
s.show_islands = True
s.show_arrows = True
check(all(m.node_tree.nodes['KIT_LINES_ON'].outputs[0].default_value == 1.0 for m in bpy.data.materials if m.use_nodes and 'KIT_LINES_ON' in m.node_tree.nodes), 'panel line overlay on')
check(not bpy.data.collections['Kit helpers'].hide_viewport, 'arrows shown')

s.name = export_name
s.registration = 'A6-EDY'
s.airline = 'Emirates'
s.icao = 'UAE'
s.iata = 'EK'
s.export_engine = 'GP7270'
s.creator = 'Kit smoke test'
result = bpy.ops.a380_kit.export_livery()
check(result == {'FINISHED'}, f'export returned {result}')
check(s.show_lines and all(m.node_tree.nodes['KIT_LINES_ON'].outputs[0].default_value == 1.0 for m in bpy.data.materials if m.use_nodes and 'KIT_LINES_ON' in m.node_tree.nodes), 'overlay toggles restored after the export')

src = os.path.join(aircraft, 'liveries', livery + ' 4K', 'objects')
if not os.path.isdir(src):
    src = os.path.join(aircraft, 'liveries', livery, 'objects')
total_bytes = {}
for suffix, cap in (('', 2048), (' 4K', 4096)):
    folder = os.path.join(kit, 'liveries', export_name + suffix)
    out = os.path.join(folder, 'objects')
    files = sorted(os.listdir(out))
    print(f'-- {export_name + suffix}: {len(files)} sheets')
    check(len(files) >= len(exterior), f'{len(files)} DDS sheets, {len(exterior)} exterior')
    worst, bad_mips, bad_size, over = 0.0, [], [], []
    sizes = {}
    for f in files:
        w, h, mips, fourcc = kit_dds.read_header(os.path.join(out, f))
        full = int(np.floor(np.log2(max(w, h)))) + 1
        sizes[f] = (w, h, mips)
        if mips != full:
            bad_mips.append(f)
        if max(w, h) > cap:
            bad_size.append(f)
        px = kit_dds.read_dds(os.path.join(out, f))
        if os.path.exists(os.path.join(src, f)):
            ref = kit_dds.shrink(kit_dds.read_dds(os.path.join(src, f)), cap)
            if ref.shape == px.shape:
                d = float(np.abs(px.astype(int) - ref.astype(int)).mean())
                worst = max(worst, d)
        cyan = int(((px[..., 0] == 0) & (px[..., 1] == 217) & (px[..., 2] == 255)).sum())
        orange = int(((px[..., 0] == 255) & (px[..., 1] == 140) & (px[..., 2] == 0)).sum())
        if cyan + orange > 50:
            over.append(f)
    check(not bad_mips, 'every sheet has a full mip chain' + (f': {bad_mips}' if bad_mips else ''))
    check(not bad_size, f'no sheet longer than {cap}' + (f': {bad_size}' if bad_size else ''))
    check(not over, 'no overlay colours in the sheets' + (f': {over}' if over else ''))
    print(f'   mean abs difference to the source livery sheets (max over sheets): {worst:.2f}')
    print('   sizes:', ', '.join(sorted({f'{w}x{h}/{m}mips' for w, h, m in sizes.values()})))
    check(all(os.path.exists(os.path.join(folder, n)) for n in ('cabin.json', 'operator.json', 'validation.json', 'A380_icon11.png')), 'cabin.json, operator.json, validation.json and the icon are there')
    icon = os.path.join(folder, 'A380_icon11.png')
    with open(icon, 'rb') as fh:
        head = fh.read(26)
    wh = struct.unpack('>II', head[16:24])
    check(wh == (1672, 941), f'icon is {wh[0]}x{wh[1]}, {os.path.getsize(icon)} bytes')
    print('   operator.json', open(os.path.join(folder, 'operator.json')).read().replace('\n', ' '))
    total_bytes[suffix or '2K'] = sum(os.path.getsize(os.path.join(out, f)) for f in files)
    print('   DDS bytes', total_bytes[suffix or '2K'])
print('FAILURES' if failures else 'ALL OK', failures)
