import math

import g4
import gselib as g


def steel(m):
    return m.get('steel') or m['galv']


def stack_theta(theta0, n, length, rise, v):
    return math.asin(min(0.999, math.sin(theta0) + rise * v / (n * length)))


def scissor_stack(f, m, c, prefix, ys, x_r, z_base, n, length, theta0_deg, rise, dataref, bar_w=120.0, bar_t=56.0, joint_r=36.0):
    theta0 = math.radians(theta0_deg)
    s0, c0 = math.sin(theta0), math.cos(theta0)
    xc0 = x_r + length / 2 * c0
    parts = []
    moving_keys = []
    for v in g4.KEYS:
        th = stack_theta(theta0, n, length, rise, v)
        moving_keys.append((v, th))

    def arm_profile(zc, sign):
        d = (c0, sign * s0)
        nx, nz = -d[1], d[0]
        hl, ht = length / 2, bar_t / 2
        return [
            (xc0 - d[0] * hl - nx * ht, zc - d[1] * hl - nz * ht),
            (xc0 + d[0] * hl - nx * ht, zc + d[1] * hl - nz * ht),
            (xc0 + d[0] * hl + nx * ht, zc + d[1] * hl + nz * ht),
            (xc0 - d[0] * hl + nx * ht, zc - d[1] * hl + nz * ht),
        ]

    for i in range(n):
        zc0 = z_base + (i + 0.5) * length * s0
        for yi, yc in enumerate(ys):
            side = 'L' if yc < 0 else 'R'
            for kind, sign, lat in (('a', 1.0, -1.0), ('b', -1.0, 1.0)):
                y0, y1 = sorted((yc + lat * (bar_w + 20.0) / 2 - bar_w / 2, yc + lat * (bar_w + 20.0) / 2 + bar_w / 2))
                arm = g.prism_xz(f, f'{prefix}_arm_{kind}{i}_{side}{yi}', arm_profile(zc0, sign), y0, y1, steel(m), c, bevel=3, segments=1)
                trans_keys, rot_keys = [], []
                for v, th in moving_keys:
                    dy = length / 2 * (math.cos(th) - c0) / 1000.0
                    dz = (i + 0.5) * length * (math.sin(th) - s0) / 1000.0
                    trans_keys.append((v, (0.0, dy, dz)))
                    rot_keys.append((v, sign * math.degrees(th - theta0)))
                pivot = f.v(xc0, 0.0, zc0)
                g.set_anim(arm, [g4.trans_keys(dataref, trans_keys), g4.rotate_keys(dataref, tuple(pivot), (1.0, 0.0, 0.0), rot_keys)])
                parts.append(arm)
                pin = g.cylinder(f, f'{prefix}_centre_pin_{kind}{i}_{side}{yi}', (xc0, yc + lat * (bar_w + 20.0) / 2, zc0), 'y', joint_r, bar_w + 30.0, m['black'], c, segments=10)
                g.set_anim(pin, [g4.trans_keys(dataref, [(v, (0.0, length / 2 * (math.cos(th) - c0) / 1000.0, (i + 0.5) * length * (math.sin(th) - s0) / 1000.0)) for v, th in moving_keys])])
                parts.append(pin)
    half = max(abs(y) for y in ys) + bar_w + 40.0
    for j in range(n + 1):
        rear = g.tube(f, f'{prefix}_joint_rear_{j}', [(x_r, -half, z_base + j * length * s0), (x_r, half, z_base + j * length * s0)], joint_r, m['black'], c, segments=10)
        if j:
            g.set_anim(rear, [g4.trans(dataref, (0.0, 0.0, j * rise / n / 1000.0))])
        parts.append(rear)
        front_x = x_r + length * c0
        front = g.tube(f, f'{prefix}_joint_front_{j}', [(front_x, -half, z_base + j * length * s0), (front_x, half, z_base + j * length * s0)], joint_r, m['black'], c, segments=10)
        keys = [(v, (0.0, length * (math.cos(th) - c0) / 1000.0, j * length * (math.sin(th) - s0) / 1000.0)) for v, th in moving_keys]
        g.set_anim(front, [g4.trans_keys(dataref, keys)])
        parts.append(front)
    return parts


def hydraulic_ram(f, m, c, prefix, ys, span, anchor, arm_foot, arm_dist, theta0_deg, n, length, rise, dataref, radius=48.0):
    theta0 = math.radians(theta0_deg)
    px, pz = anchor
    ax, az = arm_foot
    rows = []
    for v in g4.KEYS:
        th = stack_theta(theta0, n, length, rise, v)
        q = (ax + arm_dist * math.cos(th), az + arm_dist * math.sin(th))
        rows.append((v, q, math.hypot(q[0] - px, q[1] - pz), math.atan2(q[1] - pz, q[0] - px)))
    _, q0, l0, a0 = rows[0]
    l_max = max(r[2] for r in rows)
    dirv = (math.cos(a0), math.sin(a0))
    body_len, rod_len = l_max * 0.56, l_max * 0.6
    pivot = f.v(px, 0.0, pz)
    rot_keys = [(v, math.degrees(a - a0)) for v, q, ln, a in rows]
    ext_keys = [(v, (0.0, dirv[0] * (ln - l0) / 1000.0, dirv[1] * (ln - l0) / 1000.0)) for v, q, ln, a in rows]
    brace_keys = [(v, (0.0, (q[0] - q0[0]) / 1000.0, (q[1] - q0[1]) / 1000.0)) for v, q, ln, a in rows]
    parts = []
    for i, y in enumerate(ys):
        body = g.cylinder(f, f'{prefix}_ram_body_{i}', (px + dirv[0] * body_len / 2, y, pz + dirv[1] * body_len / 2), (0.0, dirv[0], dirv[1]), radius, body_len, m['yellow'], c, segments=14)
        rod = g.cylinder(f, f'{prefix}_ram_rod_{i}', (q0[0] - dirv[0] * rod_len / 2, y, q0[1] - dirv[1] * rod_len / 2), (0.0, dirv[0], dirv[1]), radius * 0.6, rod_len, steel(m), c, segments=12)
        g.set_anim(body, [g4.rotate_keys(dataref, tuple(pivot), (1.0, 0.0, 0.0), rot_keys)])
        g.set_anim(rod, [g4.rotate_keys(dataref, tuple(pivot), (1.0, 0.0, 0.0), rot_keys), g4.trans_keys(dataref, ext_keys)])
        parts += [body, rod]
        parts.append(g.cylinder(f, f'{prefix}_ram_anchor_{i}', (px, y, pz), 'y', 40, 180, m['black'], c, segments=10))
    brace = g.tube(f, f'{prefix}_ram_brace', [(q0[0], -span, q0[1]), (q0[0], span, q0[1])], 40, steel(m), c, segments=10)
    g.set_anim(brace, [g4.trans_keys(dataref, brace_keys)])
    parts.append(brace)
    return parts
