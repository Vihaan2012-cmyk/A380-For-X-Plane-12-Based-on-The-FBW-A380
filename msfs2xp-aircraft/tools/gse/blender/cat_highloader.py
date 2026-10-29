import math
import os

import g4
import g4_detail
import g4_lift
import gselib as g

NAME = 'cat_highloader'
TEXTURE_SIZE = 2048

LENGTH = 8200.0
WIDTH = 2500.0
HY = WIDTH / 2
CHASSIS_TOP = 1100.0
BOX_X0 = 250.0
BOX_X1 = 5350.0
BOX_W = 2400.0
BOX_H = 2300.0
BOX_FLOOR = 1800.0
LIFT_RISE = 3800.0
BRIDGE_TRAVEL = 1300.0
BRIDGE_FACE = BOX_X1 + 120.0
AXLES = (1300.0, 2600.0, 6500.0)
TYRE_D = 1020.0
TYRE_W = 315.0
TRACK = 2040.0

LIFT_DATAREF = 'fbw/gse/cat_highloader/lift'
BRIDGE_DATAREF = 'fbw/gse/cat_highloader/bridge'
LIFT_ANIM = [g4.trans(LIFT_DATAREF, (0.0, 0.0, LIFT_RISE / 1000.0))]
BRIDGE_ANIM = LIFT_ANIM + [g4.trans(BRIDGE_DATAREF, (0.0, BRIDGE_TRAVEL / 1000.0, 0.0))]

SC_N = 3
SC_L = 2800.0
SC_X_R = 900.0
SC_Z_BASE = 1260.0
SC_THETA0 = 2.1
SC_YS = (-620.0, 620.0)

VIEWS = {}
FRONT_OBJECTS = ['cab_shell']


def materials():
    return {
        'white': g.mat('gse_white', (236, 237, 233), roughness=0.35),
        'box': g.mat('gse_box_white', (240, 240, 238), roughness=0.3),
        'cab': g.mat('gse_cab', (230, 230, 228), roughness=0.35),
        'chassis': g.mat('gse_chassis', (40, 42, 45), metallic=0.5, roughness=0.5),
        'steel': g.mat('gse_steel', (150, 153, 156), metallic=0.85, roughness=0.4),
        'yellow': g.mat('gse_yellow', (245, 200, 0), roughness=0.4),
        'black': g.mat('gse_black', (22, 22, 23), roughness=0.55),
        'rubber': g.mat('gse_rubber', (28, 28, 28), roughness=0.88),
        'rim': g.mat('gse_rim', (170, 172, 170), metallic=0.7, roughness=0.4),
        'glass': g.mat('gse_glass', (30, 45, 55), roughness=0.05),
        'amber': g.mat('gse_amber_lens', (255, 140, 10), roughness=0.12),
        'red': g.mat('gse_red', (185, 18, 20), roughness=0.3),
        'floor': g.mat('gse_deck_floor', (90, 92, 95), metallic=0.4, roughness=0.7),
        'seal': g.mat('gse_seal', (15, 15, 15), roughness=0.9),
    }


def chassis(f, m, c):
    for k in (-1, 1):
        g.box(f, f'frame_rail_{k}', 0, LENGTH - 300, k * 480 - 90, k * 480 + 90, 700, CHASSIS_TOP, m['chassis'], c, bevel=12)
    for i, x in enumerate((300, 2000, 3700, 5300, 6900)):
        g.box(f, f'cross_member_{i}', x - 60, x + 60, -480, 480, 780, 1000, m['chassis'], c, bevel=8)
    g.box(f, 'rear_bumper', -60, 40, -1100, 1100, 620, 900, m['yellow'], c, bevel=10)
    g.box(f, 'subframe', BOX_X0 - 40, BOX_X1 + 40, -1150, 1150, CHASSIS_TOP, CHASSIS_TOP + 60, m['steel'], c, bevel=10)
    for k in (-1, 1):
        g.box(f, f'stabiliser_housing_{k}', 3000, 3300, k * 1250, k * 1560, 1050, 1250, m['chassis'], c, bevel=8)
        g.cylinder(f, f'stabiliser_leg_{k}', (3150, k * 1480, 700), 'z', 70, 560, m['steel'], c, segments=16, bevel=4)
        g.cylinder(f, f'stabiliser_foot_{k}', (3150, k * 1480, 120), 'z', 170, 40, m['black'], c, segments=20, bevel=6)
        g.box(f, f'side_marker_{k}', 3600, 3800, k * (HY - 80), k * (HY - 60), 800, 860, m['amber'], c)
        g.box(f, f'fuel_tank_{k}', 3850, 5000, k * 1000, k * 1200, 650, 1050, m['steel'], c, bevel=20)
    for x in AXLES:
        g.cylinder(f, f'axle_{int(x)}', (x, 0, TYRE_D / 2), 'y', 70, TRACK, m['chassis'], c, segments=16)
    for x in (1950, 6500):
        for k in (-1, 1):
            g.box(f, f'mudguard_{int(x)}_{k}', x - 650 if x < 3000 else x - 520, x + 650 if x < 3000 else x + 520, k * (TRACK / 2 + 190), k * (TRACK / 2 - 190), 880, 900, m['chassis'], c, bevel=3)


def wheels(f, m, c):
    r = TYRE_D / 2
    profile = [(190, -TYRE_W / 2 + 25), (210, -TYRE_W / 2), (r - 40, -TYRE_W / 2), (r - 8, -TYRE_W / 2 + 25), (r, -TYRE_W / 2 + 70),
               (r, TYRE_W / 2 - 70), (r - 8, TYRE_W / 2 - 25), (r - 40, TYRE_W / 2), (210, TYRE_W / 2), (190, TYRE_W / 2 - 25)]
    for x in AXLES:
        for k in (-1, 1):
            y = k * TRACK / 2
            tag = f'{int(x)}_{"L" if k < 0 else "R"}'
            g4_detail.tyre(f, f'tyre_{tag}', (x, y, r), (k, 0, 0), profile, m['rubber'], c)
            g.lathe(f, f'rim_{tag}', (x, y, r), (k, 0, 0), [(0.0, 40.0), (120.0, 40.0), (200.0, 30.0), (240.0, 20.0), (250.0, 80.0)], m['rim'], c, segments=32)
            for b in range(8):
                a = 2 * math.pi * b / 8
                g.cylinder(f, f'lug_{tag}_{b}', (x + 90 * math.cos(a), y + k * 48, r + 90 * math.sin(a)), 'y', 14, 22, m['steel'], c, segments=8)


def cab(f, m, c):
    x0, x1 = 5700.0, LENGTH
    g.prism_xz(f, 'cab_shell', [(x0, 1000), (x0, 3050), (7350, 3050), (7600, 2900), (x1, 2600), (x1, 1000)], -HY + 60, HY - 60, m['cab'], c, bevel=30)
    g.prism_xz(f, 'windscreen', [(7620, 1850), (7640, 2850), (x1 - 40, 2560), (x1 - 40, 1850)], -HY + 190, HY - 190, m['glass'], c, bevel=5)
    for k in (-1, 1):
        y = k * (HY - 55)
        g.box(f, f'door_window_{k}', 6350, 7250, y - k * 5, y + k * 10, 1850, 2700, m['glass'], c, bevel=8)
        g.box(f, f'door_line_{k}', 6300, 7300, y - k * 3, y + k * 5, 1050, 1060, m['black'], c)
        g.box(f, f'door_step_{k}', 6300, 7300, k * (HY - 20), k * (HY + 180), 980, 1010, m['steel'], c, bevel=4)
        g.box(f, f'mirror_arm_{k}', 7500, 7560, k * (HY - 40), k * (HY + 280), 2300, 2330, m['black'], c)
        g.box(f, f'mirror_{k}', 7480, 7580, k * (HY + 240), k * (HY + 320), 2000, 2500, m['black'], c, bevel=12)
        g.cylinder(f, f'headlight_{k}', (LENGTH - 5, k * 900, 1350), 'x', 90, 30, m['white'], c, segments=20)
        g.box(f, f'indicator_{k}', LENGTH - 10, LENGTH, k * 1050, k * 1150, 1250, 1450, m['amber'], c)
    g.box(f, 'front_grille', LENGTH - 8, LENGTH + 10, -700, 700, 1150, 1700, m['black'], c, bevel=8)
    g.box(f, 'front_bumper', LENGTH - 60, LENGTH + 80, -1150, 1150, 600, 950, m['yellow'], c, bevel=15)
    g.box(f, 'cab_roof_light_bar', 7000, 7150, -600, 600, 3050, 3110, m['black'], c, bevel=6)
    g.cylinder(f, 'beacon_base', (6200, 0, 3070), 'z', 80, 30, m['black'], c, segments=20)
    g.cylinder(f, 'beacon_lens', (6200, 0, 3130), 'z', 70, 90, m['amber'], c, segments=20, bevel=8)


def lift_mechanism(f, m, c):
    parts = g4_lift.scissor_stack(f, m, c, 'sc', SC_YS, SC_X_R, SC_Z_BASE, SC_N, SC_L, SC_THETA0, LIFT_RISE, LIFT_DATAREF)
    for k in (-1, 1):
        y = k * 620.0
        parts.append(g.box(f, f'sc_track_{k}', SC_X_R - 250, SC_X_R + SC_L + 250, y - 110, y + 110, SC_Z_BASE - 80, SC_Z_BASE - 40, m['steel'], c, bevel=4))
        parts.append(g.box(f, f'sc_pivot_block_{k}', SC_X_R - 120, SC_X_R + 120, y - 160, y + 160, SC_Z_BASE - 40, SC_Z_BASE + 20, m['chassis'], c, bevel=4))
        top = g.box(f, f'sc_top_rail_{k}', SC_X_R - 250, SC_X_R + SC_L + 250, y - 110, y + 110, SC_Z_BASE + SC_N * SC_L * math.sin(math.radians(SC_THETA0)) + 40, BOX_FLOOR - 160, m['steel'], c, bevel=4)
        g.set_anim(top, LIFT_ANIM)
        parts.append(top)
    theta0 = SC_THETA0
    parts += g4_lift.hydraulic_ram(f, m, c, 'sc', (-300.0, 300.0), 620.0, (SC_X_R + 2100.0, 1215.0), (SC_X_R, SC_Z_BASE), 1500.0, theta0, SC_N, SC_L, LIFT_RISE, LIFT_DATAREF)
    return parts


def box_body(f, m, c):
    parts = []
    zt = BOX_FLOOR + BOX_H
    parts.append(g.box(f, 'box_shell', BOX_X0, BOX_X1, -BOX_W / 2, BOX_W / 2, BOX_FLOOR, zt, m['box'], c, bevel=35, segments=3))
    parts.append(g.box(f, 'box_floor_frame', BOX_X0 - 40, BOX_X1 + 40, -BOX_W / 2 - 40, BOX_W / 2 + 40, BOX_FLOOR - 160, BOX_FLOOR, m['steel'], c, bevel=8))
    parts.append(g.box(f, 'box_roof_cap', BOX_X0 + 40, BOX_X1 - 40, -BOX_W / 2 + 60, BOX_W / 2 - 60, zt, zt + 40, m['white'], c, bevel=15))
    for k in (-1, 1):
        y = k * BOX_W / 2
        parts.append(g.box(f, f'bumper_strip_{k}', BOX_X0 + 20, BOX_X1 - 20, y - k * 5, y + k * 20, BOX_FLOOR + 80, BOX_FLOOR + 160, m['black'], c, bevel=4))
        parts.append(g.box(f, f'side_marker_box_{k}', BOX_X1 - 120, BOX_X1 - 20, y, y + k * 10, BOX_FLOOR + 400, BOX_FLOOR + 460, m['amber'], c))
        parts.append(g.box(f, f'hinge_rail_{k}', BOX_X0 + 20, BOX_X1 - 20, y - k * 5, y + k * 12, zt - 160, zt - 130, m['steel'], c))
        parts.append(g.box(f, f'rear_light_{k}', BOX_X0 - 12, BOX_X0, k * 1090, k * 1170, BOX_FLOOR + 100, BOX_FLOOR + 400, m['red'], c))
        for i in range(8):
            x = BOX_X0 + 350 + i * 600
            parts.append(g.box(f, f'panel_rib_{k}_{i}', x - 15, x + 15, y, y + k * 8, BOX_FLOOR + 220, zt - 100, m['white'], c, bevel=3))
    for k in (-1, 1):
        for i in range(76):
            x = BOX_X0 + 40 + i * 66.0
            parts.append(g.cylinder(f, f'rivet_{k}_{i}', (x, k * (BOX_W / 2 + 24), BOX_FLOOR + 120), 'y', 12, 10, m['steel'], c, segments=8))
    parts.append(g.box(f, 'rear_door_roller', BOX_X0 - 12, BOX_X0, -1000, 1000, BOX_FLOOR + 150, zt - 150, m['white'], c, bevel=4))
    for i in range(10):
        z = BOX_FLOOR + 220 + i * 190
        parts.append(g.box(f, f'rear_door_rib_{i}', BOX_X0 - 16, BOX_X0 - 10, -980, 980, z, z + 20, m['steel'], c))
    parts.append(g.box(f, 'rear_door_handle', BOX_X0 - 40, BOX_X0 - 12, -150, 150, BOX_FLOOR + 800, BOX_FLOOR + 860, m['black'], c, bevel=6))
    parts.append(g.box(f, 'door_frame_front', BOX_X1, BOX_X1 + 60, -1050, 1050, BOX_FLOOR + 60, BOX_FLOOR + 2150, m['seal'], c, bevel=10))
    parts.append(g.box(f, 'door_opening_front', BOX_X1 + 20, BOX_X1 + 62, -950, 950, BOX_FLOOR + 140, BOX_FLOOR + 2060, m['black'], c, bevel=6))
    for p in parts:
        g.set_anim(p, LIFT_ANIM)
    return parts


def bridge(f, m, c):
    z0 = BOX_FLOOR
    parts = []
    xs = BOX_X1 - 1000
    xa = BRIDGE_FACE - 130
    parts.append(g.box(f, 'bridge_deck', xs, xa, -940, 940, z0 - 80, z0, m['floor'], c, bevel=4))
    parts.append(g.box(f, 'bridge_apron', xa, xa + 130, -940, 940, z0 - 60, z0 + 2, m['yellow'], c, bevel=4))
    parts.append(g.box(f, 'bridge_seal', xa + 120, xa + 240, -960, 960, z0 - 100, z0 + 700, m['seal'], c, bevel=20))
    for k in (-1, 1):
        y = k * 940
        parts.append(g.box(f, f'bridge_side_{k}', xs, xa, y - k * 20, y + k * 15, z0 - 80, z0 + 120, m['steel'], c, bevel=4))
        parts.append(g.tube(f, f'bridge_rail_{k}', [(xa - 600, y, z0 + 1000), (xa + 100, y, z0 + 1000)], 22, m['yellow'], c, segments=10))
        parts.append(g.tube(f, f'bridge_rail_low_{k}', [(xa - 600, y, z0 + 550), (xa + 100, y, z0 + 550)], 18, m['yellow'], c, segments=8))
        for i in range(3):
            x = xa - 600 + i * 350
            parts.append(g.cylinder(f, f'bridge_post_{k}_{i}', (x, y, z0 + 500), 'z', 20, 1000, m['yellow'], c, segments=8))
    for p in parts:
        g.set_anim(p, BRIDGE_ANIM)
    return parts


def decals(f, c):
    mat = g.image_mat('decal_livery', g4.livery_path(NAME, 'neutral'), roughness=0.4)
    zb, zt = BOX_FLOOR + 160, BOX_FLOOR + 160 + 1900
    off = BOX_W / 2 + 3
    x0, x1 = BOX_X0 + 400, BOX_X0 + 400 + 1900 * 2048 / 960
    for k in (-1, 1):
        if k > 0:
            corners = [(x0, k * off, zb), (x1, k * off, zb), (x1, k * off, zt), (x0, k * off, zt)]
        else:
            corners = [(x1, k * off, zb), (x0, k * off, zb), (x0, k * off, zt), (x1, k * off, zt)]
        d = g.decal(f, f'livery_side_{k}', corners, mat, c)
        g.set_anim(d, LIFT_ANIM)


def build(with_connection=True):
    g.reset_scene()
    f = g.Frame(BRIDGE_FACE + 110.0)
    c = g.collection(NAME)
    m = materials()
    chassis(f, m, c)
    wheels(f, m, c)
    cab(f, m, c)
    lift_mechanism(f, m, c)
    box_body(f, m, c)
    bridge(f, m, c)
    decals(f, c)
    return c


def meta():
    return {
        'front_marker_xp': [[0.0, 1.0, -(LENGTH - (BRIDGE_FACE + 110.0)) / 1000.0]],
        'rear_marker_xp': [[0.0, 1.0, -(0.0 - (BRIDGE_FACE + 110.0)) / 1000.0]],
        'anims': [
            {
                'dataref': LIFT_DATAREF,
                'meaning': 'box, bridge and livery rise on a three-stage scissor lift; the bridge deck top is the door-sill reference height',
                'min': 0.0,
                'max': 1.0,
                'platform_height_at_min_m': BOX_FLOOR / 1000.0,
                'platform_height_at_max_m': (BOX_FLOOR + LIFT_RISE) / 1000.0,
            },
            {
                'dataref': BRIDGE_DATAREF,
                'meaning': 'bridge deck and seal extend towards the aircraft (-Z) by 1.30 m',
                'min': 0.0,
                'max': 1.0,
                'platform_height_at_min_m': None,
                'platform_height_at_max_m': None,
            },
        ],
        'attach': {
            'platform_front_edge': [0.0, BOX_FLOOR / 1000.0, 0.0],
            'how': 'the origin is the ground point below the bridge seal face (retracted) on the centreline; the front edge sits at x 0, z 0 and at the bridge deck top height; fbw/gse/cat_highloader/lift adds 0 to 3.80 m to y; fbw/gse/cat_highloader/bridge moves the seal face 1.30 m towards -Z',
        },
    }
