"""A380 for X-Plane 12 paint kit: the Kit panel (N panel, tab "A380 Kit").

Engine type, load a livery, reset, helper overlays and export. Runs from the
"kit_panel.py" text in the kit's .blend (Register is on) or with Run Script.
"""
import hashlib
import json
import math
import os
import re
import shutil
import sys

import bpy
import numpy as np
from bpy.props import BoolProperty, EnumProperty, PointerProperty, StringProperty
from bpy.types import Operator, Panel, PropertyGroup

ICON_SIZE = (1672, 941)
CAPS = (('', 2048), (' 4K', 4096))
ENGINES = (('T970', 'Trent 970', 'Trent 900', '970', 'A380-841'),
           ('T972', 'Trent 972', 'Trent 900', '972', 'A380-842'),
           ('GP7270', 'GP7270', 'GP7200', 'GP7270', 'A380-861'))
MODELS = ('A380-841', 'A380-842', 'A380-861')
TRENT_COLLECTION = 'Engines - Trent 900'
GP_COLLECTION = 'Engines - GP7200'
HELPERS_COLLECTION = 'Kit helpers'
OVERLAY_KINDS = ('LINES', 'ISLANDS')

_items_cache = {}


def kit_dir():
    return os.path.dirname(bpy.data.filepath)


def dds():
    scripts = os.path.join(kit_dir(), 'scripts')
    if scripts not in sys.path:
        sys.path.insert(0, scripts)
    import kit_dds
    return kit_dds


def sheets():
    text = bpy.data.texts.get('kit_sheets.json')
    return json.loads(text.as_string())['sheets'] if text else []


def originals():
    text = bpy.data.texts.get('kit_originals.json')
    return json.loads(text.as_string()) if text else {}


def candidates():
    roots = [os.path.normpath(os.path.join(kit_dir(), '..'))]
    for drive in 'CDEFGH':
        for rel in ('Program Files (x86)/Steam/steamapps/common', 'Steam/steamapps/common', 'SteamLibrary/steamapps/common', 'Steam Games/steamapps/common', 'Games'):
            roots.append(f'{drive}:/{rel}/X-Plane 12')
    return [os.path.join(r, 'Aircraft', 'A380 for X-Plane 12') for r in roots] + [os.path.join(r, 'A380 for X-Plane 12') for r in roots[:1]]


def aircraft_dir(settings):
    given = bpy.path.abspath(settings.aircraft_dir) if settings.aircraft_dir else ''
    if given and os.path.isdir(os.path.join(given, 'liveries')):
        return given
    return next((c for c in candidates() if os.path.isdir(os.path.join(c, 'liveries'))), '')


def livery_roots(settings):
    roots = []
    ac = aircraft_dir(settings)
    if ac:
        roots.append(('ac', os.path.join(ac, 'liveries')))
    roots.append(('kit', os.path.join(kit_dir(), 'liveries')))
    return roots


def strip_edition(name):
    return re.sub(r'\s+[24]K$', '', name, flags=re.I)


def livery_items(self, context):
    settings = context.scene.a380_kit
    roots = livery_roots(settings)
    key = tuple((tag, path, os.path.getmtime(path) if os.path.isdir(path) else 0) for tag, path in roots)
    if _items_cache.get('key') != key:
        found = {}
        for tag, path in roots:
            if os.path.isdir(path):
                for d in sorted(os.listdir(path)):
                    if os.path.isdir(os.path.join(path, d, 'objects')):
                        found.setdefault((tag, strip_edition(d)), None)
        items = [(f'{tag}:{name}', name + (' (kit)' if tag == 'kit' else ''), f'From the {"kit" if tag == "kit" else "aircraft"} liveries folder') for (tag, name) in sorted(found, key=lambda k: (k[0] != 'kit', k[1].lower()))]
        _items_cache['key'] = key
        _items_cache['items'] = items or [('NONE', 'No liveries found', '')]
    return _items_cache['items']


def resolve_livery(settings):
    value = settings.livery
    if ':' not in value:
        return None
    tag, name = value.split(':', 1)
    root = dict(livery_roots(settings)).get(tag)
    if not root:
        return None
    order = [f'{name} 4K', name] if settings.prefer_4k else [name, f'{name} 4K']
    return next((os.path.join(root, n) for n in order if os.path.isdir(os.path.join(root, n, 'objects'))), None)


def overlay_toggle(kind, on):
    for mat in bpy.data.materials:
        if mat.use_nodes:
            node = mat.node_tree.nodes.get(f'KIT_{kind}_ON')
            if node:
                node.outputs[0].default_value = 1.0 if on else 0.0


def set_overlays(lines, islands):
    overlay_toggle('LINES', lines)
    overlay_toggle('ISLANDS', islands)


def apply_overlays(self, context):
    s = context.scene.a380_kit
    set_overlays(s.show_lines, s.show_islands)


def apply_arrows(self, context):
    col = bpy.data.collections.get(HELPERS_COLLECTION)
    if col:
        col.hide_viewport = not context.scene.a380_kit.show_arrows


def engine_row(identifier):
    return next(e for e in ENGINES if e[0] == identifier)


def apply_engine(self, context):
    s = context.scene.a380_kit
    gp = s.engine == 'GP'
    for name, hidden in ((TRENT_COLLECTION, gp), (GP_COLLECTION, not gp)):
        col = bpy.data.collections.get(name)
        if col:
            col.hide_viewport = hidden
            col.hide_render = hidden
    family = engine_row(s.export_engine)[2]
    if (family == 'GP7200') != gp:
        s.export_engine = 'GP7270' if gp else 'T970'


def apply_export_engine(self, context):
    s = context.scene.a380_kit
    row = engine_row(s.export_engine)
    s.model = row[4]
    view = 'GP' if row[2] == 'GP7200' else 'TRENT'
    if s.engine != view:
        s.engine = view


class KitSettings(PropertyGroup):
    engine: EnumProperty(name='Engine type', items=[('TRENT', 'Trent 900', 'Rolls-Royce Trent 900 engines'), ('GP', 'GP7200', 'Engine Alliance GP7200 engines')], default='TRENT', update=apply_engine)
    aircraft_dir: StringProperty(name='Aircraft folder', subtype='DIR_PATH', description='The installed "A380 for X-Plane 12" folder; empty looks for it next to the kit and in the usual Steam places')
    livery: EnumProperty(name='Livery', items=livery_items)
    prefer_4k: BoolProperty(name='Use 4K edition when there is one', default=True)
    load_scope: EnumProperty(name='Load', items=[('ALL', 'Everything', 'Exterior, cabin, flight deck, IFE look and cabin lighting'), ('EXTERIOR', 'Exterior only', 'Leave the cabin and flight deck sheets as they are')], default='ALL')
    show_lines: BoolProperty(name='Panel lines', default=False, update=apply_overlays)
    show_islands: BoolProperty(name='UV island outlines', default=False, update=apply_overlays)
    show_arrows: BoolProperty(name='FWD / UP arrows', default=False, update=apply_arrows)
    name: StringProperty(name='Livery name', default='My Livery')
    registration: StringProperty(name='Registration')
    airline: StringProperty(name='Airline')
    icao: StringProperty(name='ICAO')
    iata: StringProperty(name='IATA')
    export_engine: EnumProperty(name='Engines', items=[(e[0], e[1], '') for e in ENGINES], default='T970', update=apply_export_engine)
    model: EnumProperty(name='Model', items=[(m, m, '') for m in MODELS], default='A380-841')
    creator: StringProperty(name='Creator')


def base_pixels(sheet, ac):
    d = dds()
    local = os.path.join(kit_dir(), sheet['base']) if sheet.get('base') else ''
    path = local if local and os.path.exists(local) else os.path.join(ac, 'objects', sheet['file']) if ac else ''
    if not path or not os.path.exists(path):
        raise FileNotFoundError(f"base {sheet['file']} is not in the kit; set the Aircraft folder")
    return d.read_dds(path)


def set_image(img, rgba):
    h, w = rgba.shape[:2]
    if tuple(img.size) != (w, h):
        img.scale(w, h)
    img.pixels.foreach_set((rgba[::-1].astype(np.float32) / 255.0).ravel())
    img.update()


def reset_sheets(settings, mode, scope):
    ac = aircraft_dir(settings)
    done = 0
    for sheet in sheets():
        if scope == 'EXTERIOR' and sheet['role'] != 'exterior':
            continue
        img = bpy.data.images.get(sheet['stem'])
        if img is None:
            continue
        w, h = sheet['size']
        if mode == 'WHITE' and sheet.get('white'):
            rgba = np.full((h, w, 4), 255, np.uint8)
        else:
            rgba = base_pixels(sheet, ac)
        set_image(img, rgba)
        done += 1
    return done


def looks_like_registration(token):
    if not 4 <= len(token) <= 8 or not re.fullmatch(r'[A-Z0-9-]+', token):
        return False
    letters, digits = re.search(r'[A-Z]', token), re.search(r'\d', token)
    return bool('-' in token and letters and not token.startswith('-') and not token.endswith('-') or letters and digits)


def livery_identity(folder):
    path = os.path.join(folder, 'operator.json')
    data = {}
    if os.path.exists(path):
        try:
            data = json.load(open(path, encoding='utf-8'))
        except (OSError, ValueError):
            data = {}
    words = strip_edition(os.path.basename(folder)).split()
    if words and looks_like_registration(words[-1]):
        data.setdefault('registration', words.pop())
    data.setdefault('airline', ' '.join(words))
    return data


def load_livery(settings, folder, scope):
    d = dds()
    ac = aircraft_dir(settings)
    objects = os.path.join(folder, 'objects')
    present = {f.lower(): f for f in os.listdir(objects)}
    loaded = []
    for sheet in sheets():
        if scope == 'EXTERIOR' and sheet['role'] != 'exterior':
            continue
        img = bpy.data.images.get(sheet['stem'])
        if img is None:
            continue
        hit = present.get(sheet['file'].lower())
        if hit:
            rgba = d.shrink(d.read_dds(os.path.join(objects, hit)), 4096)
            loaded.append(sheet['file'])
        else:
            rgba = base_pixels(sheet, ac)
        set_image(img, rgba)
    if scope == 'ALL':
        cabin = os.path.join(folder, 'cabin.json')
        text = bpy.data.texts.get('cabin.json')
        if os.path.exists(cabin) and text:
            text.clear()
            text.write(open(cabin, encoding='utf-8').read())
        for name in ('ife_logo', 'ife_background'):
            src = os.path.join(folder, name + '.png')
            img = bpy.data.images.get(name)
            if os.path.exists(src) and img:
                shutil.copy2(src, bpy.path.abspath(img.filepath))
                img.reload()
    identity = livery_identity(folder)
    s = settings
    s.registration = identity.get('registration', '') or s.registration
    s.airline = identity.get('airline', '') or s.airline
    s.icao = identity.get('icao', s.icao)
    s.iata = identity.get('iata', s.iata)
    engine = identity.get('xphfbw.engineRating')
    if engine:
        s.export_engine = {'970': 'T970', '972': 'T972', 'GP7270': 'GP7270'}.get(str(engine), s.export_engine)
    if identity.get('model') in MODELS:
        s.model = identity['model']
    return loaded


class KIT_OT_load_livery(Operator):
    bl_idname = 'a380_kit.load_livery'
    bl_label = 'Load livery'
    bl_description = 'Load the chosen livery into the paint sheets; sheets it does not override go back to the base'

    def execute(self, context):
        s = context.scene.a380_kit
        folder = resolve_livery(s)
        if not folder:
            self.report({'ERROR'}, 'Pick a livery; set the Aircraft folder if the list is empty')
            return {'CANCELLED'}
        try:
            loaded = load_livery(s, folder, s.load_scope)
        except (OSError, ValueError) as e:
            self.report({'ERROR'}, str(e))
            return {'CANCELLED'}
        self.report({'INFO'}, f'{os.path.basename(folder)}: {len(loaded)} sheets loaded')
        return {'FINISHED'}


class KIT_OT_reset(Operator):
    bl_idname = 'a380_kit.reset_paint'
    bl_label = 'Reset paint'
    bl_description = 'Put every paint sheet back to the base texture, or to plain white for the body sheets'

    mode: EnumProperty(name='To', items=[('BASE', 'Base', 'The aircraft\'s own texture'), ('WHITE', 'White', 'Plain white body sheets, the rest at base')], default='BASE')
    scope: EnumProperty(name='Sheets', items=[('ALL', 'Everything', ''), ('EXTERIOR', 'Exterior only', '')], default='ALL')

    def invoke(self, context, event):
        return context.window_manager.invoke_confirm(self, event)

    def execute(self, context):
        try:
            n = reset_sheets(context.scene.a380_kit, self.mode, self.scope)
        except (OSError, ValueError) as e:
            self.report({'ERROR'}, str(e))
            return {'CANCELLED'}
        self.report({'INFO'}, f'{n} sheets reset')
        return {'FINISHED'}


def rgba8(img):
    w, h = img.size
    px = np.empty(w * h * 4, np.float32)
    img.pixels.foreach_get(px)
    return np.clip(px.reshape(h, w, 4)[::-1] * 255 + 0.5, 0, 255).astype(np.uint8)


def safe_name(text):
    return re.sub(r'[<>:"/\\|?*\x00-\x1f]', '', text).strip().strip('.')


def operator_json(s):
    row = engine_row(s.export_engine)
    out = {'airline': s.airline.strip(), 'registration': s.registration.strip().upper(), 'icao': s.icao.strip().upper(), 'iata': s.iata.strip().upper(),
           'model': s.model, 'xphfbw.engineType': 'GP7200' if row[2] == 'GP7200' else 'TRENT900', 'xphfbw.engineRating': row[3], 'creator': s.creator.strip()}
    return {k: v for k, v in out.items() if v}


def silhouette_points(objects):
    pts = []
    for o in objects:
        co = np.empty(len(o.data.vertices) * 3, np.float32)
        o.data.vertices.foreach_get('co', co)
        co = co.reshape(-1, 3)[::max(1, len(co) // 3 // 1500)]
        m = np.array(o.matrix_world)
        pts.append(co @ m[:3, :3].T + m[:3, 3])
    return np.concatenate(pts)


def render_icon(context, path):
    from mathutils import Matrix, Vector
    s = context.scene.a380_kit
    keep = [(m, k, m.node_tree.nodes[f'KIT_{k}_ON'].outputs[0].default_value) for m in bpy.data.materials if m.use_nodes for k in OVERLAY_KINDS if f'KIT_{k}_ON' in m.node_tree.nodes]
    set_overlays(False, False)
    scene = bpy.data.scenes.new('kit_icon')
    engine_col = bpy.data.collections.get(GP_COLLECTION if s.engine == 'GP' else TRENT_COLLECTION)
    shown = [bpy.data.collections.get(n) for n in ('Exterior - paintable', 'Exterior - reference')] + [engine_col]
    for col in shown:
        if col:
            scene.collection.children.link(col)
    objects = [o for col in shown if col for o in col.all_objects if o.type == 'MESH' and not o.hide_render and len(o.data.vertices)]
    points = silhouette_points(objects)
    centre = Vector((points.min(axis=0) + points.max(axis=0)) / 2)
    cam_data = bpy.data.cameras.new('kit_icon')
    cam_data.lens = 50
    cam = bpy.data.objects.new('kit_icon', cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    scene.render.resolution_x, scene.render.resolution_y = ICON_SIZE
    scene.render.resolution_percentage = 100
    aspect = ICON_SIZE[0] / ICON_SIZE[1]
    direction = Vector((-math.cos(math.radians(34)), math.sin(math.radians(34)), 0.16)).normalized()

    def place(distance):
        cam.location = centre + direction * distance
        cam.rotation_euler = (centre - cam.location).to_track_quat('-Z', 'Y').to_euler()
        cam.matrix_world = Matrix.Translation(cam.location) @ cam.rotation_euler.to_matrix().to_4x4()
        local = (points - np.array(cam.location)) @ np.array(cam.matrix_world.to_3x3())
        k = cam_data.lens / cam_data.sensor_width
        x = 0.5 + local[:, 0] / -local[:, 2] * k
        y = 0.5 + local[:, 1] / -local[:, 2] * k * aspect
        return x.min(), x.max(), y.min(), y.max()

    lo, hi = 40.0, 800.0
    for _ in range(30):
        mid = (lo + hi) / 2
        x0, x1, y0, y1 = place(mid)
        lo, hi = (lo, mid) if (x1 - x0) < 0.9 and (y1 - y0) < 0.86 else (mid, hi)
    x0, x1, y0, y1 = place(hi)
    cam_data.shift_x = (x0 + x1) / 2 - 0.5
    cam_data.shift_y = ((y0 + y1) / 2 - 0.5) / aspect
    world = bpy.data.worlds.new('kit_icon')
    world.use_nodes = True
    bg = world.node_tree.nodes['Background']
    bg.inputs['Color'].default_value = (0.2, 0.3, 0.45, 1.0)
    bg.inputs['Strength'].default_value = 1.0
    scene.world = world
    sun_data = bpy.data.lights.new('kit_icon', 'SUN')
    sun_data.energy = 3.0
    sun = bpy.data.objects.new('kit_icon', sun_data)
    sun.rotation_euler = (-Vector((-0.55, 0.4, 0.75))).to_track_quat('-Z', 'Y').to_euler()
    scene.collection.objects.link(sun)
    scene.view_settings.view_transform = 'Standard'
    scene.render.image_settings.file_format = 'PNG'
    scene.render.image_settings.color_mode = 'RGB'
    scene.render.filepath = path
    names = [e.identifier for e in bpy.types.RenderSettings.bl_rna.properties['engine'].enum_items]
    used = None
    for wanted in [n for n in names if n.startswith('BLENDER_EEVEE')] + ['BLENDER_WORKBENCH']:
        if wanted not in names:
            continue
        scene.render.engine = wanted
        if wanted == 'BLENDER_WORKBENCH':
            scene.display.shading.light = 'STUDIO'
            scene.display.shading.color_type = 'TEXTURE'
        else:
            scene.eevee.taa_render_samples = 24
        try:
            bpy.ops.render.render(write_still=True, scene=scene.name)
            used = wanted
            break
        except RuntimeError:
            continue
    for col in shown:
        if col:
            scene.collection.children.unlink(col)
    for data, coll in ((scene, bpy.data.scenes), (cam, bpy.data.objects), (sun, bpy.data.objects), (cam_data, bpy.data.cameras), (sun_data, bpy.data.lights), (world, bpy.data.worlds)):
        coll.remove(data)
    for m, k, v in keep:
        m.node_tree.nodes[f'KIT_{k}_ON'].outputs[0].default_value = v
    return used


def run_export(context):
    d = dds()
    s = context.scene.a380_kit
    name = safe_name(s.name)
    if not name:
        raise ValueError('Give the livery a name')
    root = os.path.join(kit_dir(), 'liveries')
    base_originals = originals()
    wm = context.window_manager
    info = sheets()
    wm.progress_begin(0, len(info) * 2 + 2)
    step = 0
    pixels = {}
    for sheet in info:
        img = bpy.data.images.get(sheet['stem'])
        if img is None:
            continue
        if img.is_dirty:
            img.save()
        rgba = rgba8(img)
        if sheet['role'] != 'exterior' and hashlib.md5(np.ascontiguousarray(rgba).tobytes()).hexdigest() == base_originals.get(sheet['stem']):
            continue
        pixels[sheet['file']] = rgba
    cabin = bpy.data.texts.get('cabin.json')
    cabin_text = cabin.as_string() if cabin else open(os.path.join(kit_dir(), 'cabin.json'), encoding='utf-8').read()
    json.loads(cabin_text)
    ife = [bpy.path.abspath(img.filepath) for img in bpy.data.images if os.path.basename(os.path.dirname(bpy.path.abspath(img.filepath))) == 'ife']
    icon = os.path.join(bpy.app.tempdir, 'kit_icon.png')
    renderer = render_icon(context, icon)
    result = []
    for suffix, cap in CAPS:
        folder = os.path.join(root, name + suffix)
        out = os.path.join(folder, 'objects')
        shutil.rmtree(out, ignore_errors=True)
        os.makedirs(out, exist_ok=True)
        report = []
        for file, rgba in pixels.items():
            w, h, mips, size = d.write_dxt5(d.shrink(rgba, cap), os.path.join(out, file))
            sha = hashlib.sha256(open(os.path.join(out, file), 'rb').read()).hexdigest()
            report.append({'filename': file, 'width': w, 'height': h, 'format': 'DXT5', 'mip_levels': mips, 'bytes': size, 'sha256': sha})
            step += 1
            wm.progress_update(step)
        open(os.path.join(folder, 'cabin.json'), 'w', encoding='utf-8').write(cabin_text)
        for path in ife:
            shutil.copy2(path, os.path.join(folder, os.path.basename(path)))
        json.dump(operator_json(s), open(os.path.join(folder, 'operator.json'), 'w', encoding='utf-8'), indent=2)
        json.dump({'registration': s.registration.strip().upper(), 'textures': report}, open(os.path.join(folder, 'validation.json'), 'w', encoding='utf-8'), indent=2)
        open(os.path.join(folder, 'README.txt'), 'w', encoding='utf-8').write(f'{name + suffix} - X-Plane 12 A380\n\nINSTALL: Copy the "{name + suffix}" folder into the aircraft liveries folder.\n')
        if os.path.exists(icon):
            for target in ('A380_icon11.png', 'A380_icon11_thumb.png'):
                shutil.copy2(icon, os.path.join(folder, target))
        result.append((folder, len(report), cap))
    wm.progress_end()
    return result, renderer


class KIT_OT_export(Operator):
    bl_idname = 'a380_kit.export_livery'
    bl_label = 'Export livery'
    bl_description = 'Write the 2K and 4K livery folders into the kit\'s liveries folder'

    def execute(self, context):
        try:
            result, renderer = run_export(context)
        except (OSError, ValueError) as e:
            self.report({'ERROR'}, str(e))
            return {'CANCELLED'}
        self.report({'INFO'}, ' and '.join(f'{os.path.basename(f)} ({n} sheets)' for f, n, _ in result) + (' written' if renderer else ' written, icon not rendered'))
        return {'FINISHED'}


class KIT_OT_save_images(Operator):
    bl_idname = 'a380_kit.save_images'
    bl_label = 'Save paint'
    bl_description = 'Save every changed paint sheet'

    def execute(self, context):
        n = 0
        for img in bpy.data.images:
            if img.is_dirty and img.filepath:
                img.save()
                n += 1
        self.report({'INFO'}, f'{n} images saved')
        return {'FINISHED'}


class KIT_PT_main(Panel):
    bl_label = 'A380 Paint Kit'
    bl_idname = 'KIT_PT_main'
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'A380 Kit'

    def draw(self, context):
        s = context.scene.a380_kit
        col = self.layout.column()
        col.label(text='Engine type')
        col.row().prop(s, 'engine', expand=True)
        col.operator('a380_kit.save_images', icon='FILE_TICK')


class KIT_PT_livery(Panel):
    bl_label = 'Start from a livery'
    bl_idname = 'KIT_PT_livery'
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'A380 Kit'
    bl_parent_id = 'KIT_PT_main'

    def draw(self, context):
        s = context.scene.a380_kit
        col = self.layout.column()
        col.prop(s, 'aircraft_dir')
        col.prop(s, 'livery', text='')
        col.prop(s, 'prefer_4k')
        col.prop(s, 'load_scope', text='')
        col.operator('a380_kit.load_livery', icon='IMPORT')
        row = col.row(align=True)
        op = row.operator('a380_kit.reset_paint', text='Reset to base')
        op.mode = 'BASE'
        op = row.operator('a380_kit.reset_paint', text='Reset to white')
        op.mode = 'WHITE'


class KIT_PT_overlays(Panel):
    bl_label = 'Helper overlays'
    bl_idname = 'KIT_PT_overlays'
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'A380 Kit'
    bl_parent_id = 'KIT_PT_main'

    def draw(self, context):
        s = context.scene.a380_kit
        col = self.layout.column()
        col.prop(s, 'show_lines')
        col.prop(s, 'show_islands')
        col.prop(s, 'show_arrows')
        col.label(text='Display only: never exported')


class KIT_PT_export(Panel):
    bl_label = 'Export'
    bl_idname = 'KIT_PT_export'
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'A380 Kit'
    bl_parent_id = 'KIT_PT_main'

    def draw(self, context):
        s = context.scene.a380_kit
        col = self.layout.column()
        for prop in ('name', 'registration', 'airline', 'icao', 'iata', 'export_engine', 'model', 'creator'):
            col.prop(s, prop)
        col.operator('a380_kit.export_livery', icon='EXPORT')


classes = (KitSettings, KIT_OT_load_livery, KIT_OT_reset, KIT_OT_export, KIT_OT_save_images, KIT_PT_main, KIT_PT_livery, KIT_PT_overlays, KIT_PT_export)


def unregister():
    for c in reversed(classes):
        try:
            bpy.utils.unregister_class(c)
        except (RuntimeError, ValueError):
            pass
    if hasattr(bpy.types.Scene, 'a380_kit'):
        del bpy.types.Scene.a380_kit


def register():
    unregister()
    for c in classes:
        bpy.utils.register_class(c)
    bpy.types.Scene.a380_kit = PointerProperty(type=KitSettings)


register()
