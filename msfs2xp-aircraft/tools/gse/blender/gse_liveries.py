import os

import bpy

import gselib as g
from livery_art import OPERATORS, PANELS, panel_path


class Livery:
    def __init__(self, name, out_dir=None):
        self.name = name
        self.out_dir = out_dir or os.environ.get('GSE_OUT', 'E:/fbw-gse/export/_work/default')
        self.colors = {}

    def path(self, tag, operator):
        return panel_path(self.out_dir, tag, operator)

    def panel_material(self, tag, roughness=0.4):
        return g.image_mat(f'gse_livery_{tag}', self.path(tag, None), roughness=roughness)

    def recolor(self, mat_name, role):
        self.colors[mat_name] = role

    def variants(self, operators=OPERATORS):
        return {key: self._applier(key) for key in operators}

    def _applier(self, key):
        def apply():
            spec = OPERATORS[key]
            for tag in PANELS[self.name]:
                m = bpy.data.materials['gse_livery_' + tag]
                tex = next(n for n in m.node_tree.nodes if n.type == 'TEX_IMAGE')
                tex.image = bpy.data.images.load(self.path(tag, key), check_existing=True)
                tex.image.reload()
            for name, role in self.colors.items():
                m = bpy.data.materials.get(name)
                if m is None:
                    continue
                p = m.node_tree.nodes.get('Principled BSDF')
                rgb = spec['accent'] if role == 'accent' else spec[role]
                p.inputs['Base Color'].default_value = (*g.srgb_to_linear(rgb), 1.0)
        return apply
