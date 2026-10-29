"""OBJ8 reader for the panel tools: geometry in rest pose (every dataref 0)."""
import math
import numpy as np


def _mat_trans(t):
    m = np.eye(4)
    m[:3, 3] = t
    return m


def _mat_rot(axis, deg):
    a = np.asarray(axis, float)
    n = np.linalg.norm(a)
    if n == 0 or deg == 0:
        return np.eye(4)
    x, y, z = a / n
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    C = 1 - c
    m = np.eye(4)
    m[:3, :3] = [[c + x * x * C, x * y * C - z * s, x * z * C + y * s],
                 [y * x * C + z * s, c + y * y * C, y * z * C - x * s],
                 [z * x * C - y * s, z * y * C + x * s, c + z * z * C]]
    return m


def _interp(keys, v):
    if not keys:
        return None
    if v <= keys[0][0]:
        return keys[0][1]
    if v >= keys[-1][0]:
        return keys[-1][1]
    for (k0, a), (k1, b) in zip(keys, keys[1:]):
        if k0 <= v <= k1:
            f = 0.0 if k1 == k0 else (v - k0) / (k1 - k0)
            return a + (b - a) * f
    return keys[-1][1]


class Draw:
    __slots__ = ("start", "count", "matrix", "state", "lod", "hidden")

    def __init__(self, start, count, matrix, state, lod, hidden):
        self.start, self.count, self.matrix, self.state, self.lod, self.hidden = start, count, matrix, state, lod, hidden


class Obj8:
    def __init__(self, path, rest=None):
        self.path = path
        self.rest = rest or {}
        self.textures = {}
        self.vt = np.zeros((0, 8))
        self.idx = np.zeros(0, np.int64)
        self.draws = []
        self._parse()

    def _val(self, dref):
        return float(self.rest.get(dref, 0.0))

    def _parse(self):
        with open(self.path, encoding="latin-1") as f:
            lines = f.read().splitlines()
        vt_rows, idx_vals = [], []
        for l in lines:
            if l.startswith("VT "):
                vt_rows.append(l[3:])
            elif l.startswith("IDX10 "):
                idx_vals.append(l[6:])
            elif l.startswith("IDX "):
                idx_vals.append(l[4:])
        if vt_rows:
            self.vt = np.array(" ".join(vt_rows).split(), dtype=np.float64).reshape(-1, 8)
        if idx_vals:
            self.idx = np.array(" ".join(idx_vals).split(), dtype=np.int64)

        stack = [np.eye(4)]
        hide_stack = [False]
        state = {}
        lod = -1
        pending = None
        for l in lines:
            if not l or l[0] in "VI" and (l.startswith("VT ") or l.startswith("IDX")):
                continue
            p = l.split()
            if not p:
                continue
            k = p[0]
            if k in ("TEXTURE", "TEXTURE_LIT", "TEXTURE_NORMAL"):
                self.textures[k] = p[1] if len(p) > 1 else None
            elif k == "TEXTURE_MAP" and len(p) > 2:
                self.textures["MAP_" + p[1]] = p[2]
            elif k == "ATTR_LOD":
                lod += 1
            elif k == "ANIM_begin":
                stack.append(stack[-1].copy())
                hide_stack.append(hide_stack[-1])
            elif k == "ANIM_end":
                stack.pop()
                hide_stack.pop()
            elif k == "ANIM_trans" and len(p) >= 7:
                a = np.array(p[1:4], float)
                b = np.array(p[4:7], float)
                if len(p) >= 10:
                    v1, v2 = float(p[7]), float(p[8])
                    v = self._val(p[9])
                    f = 0.0 if v2 == v1 else min(max((v - v1) / (v2 - v1), 0.0), 1.0)
                    t = a + (b - a) * f
                else:
                    t = a
                stack[-1] = stack[-1] @ _mat_trans(t)
            elif k == "ANIM_rotate" and len(p) >= 9:
                axis = [float(x) for x in p[1:4]]
                r1, r2, v1, v2 = map(float, p[4:8])
                v = self._val(p[8]) if len(p) > 8 else 0.0
                f = 0.0 if v2 == v1 else min(max((v - v1) / (v2 - v1), 0.0), 1.0)
                stack[-1] = stack[-1] @ _mat_rot(axis, r1 + (r2 - r1) * f)
            elif k == "ANIM_trans_begin":
                pending = ("t", p[1], [])
            elif k == "ANIM_rotate_begin":
                pending = ("r", p[4], [], [float(x) for x in p[1:4]])
            elif k == "ANIM_trans_key" and pending:
                pending[2].append((float(p[1]), np.array(p[2:5], float)))
            elif k == "ANIM_rotate_key" and pending:
                pending[2].append((float(p[1]), float(p[2])))
            elif k == "ANIM_trans_end" and pending:
                t = _interp(pending[2], self._val(pending[1]))
                if t is not None:
                    stack[-1] = stack[-1] @ _mat_trans(t)
                pending = None
            elif k == "ANIM_rotate_end" and pending:
                d = _interp(pending[2], self._val(pending[1]))
                if d is not None:
                    stack[-1] = stack[-1] @ _mat_rot(pending[3], d)
                pending = None
            elif k in ("ANIM_hide", "ANIM_show") and len(p) >= 4:
                v1, v2 = float(p[1]), float(p[2])
                v = self._val(p[3])
                inside = min(v1, v2) <= v <= max(v1, v2)
                if inside:
                    hide_stack[-1] = k == "ANIM_hide"
            elif k.startswith("ATTR_manip_"):
                if k == "ATTR_manip_none":
                    state.pop("manip", None)
                    state.pop("wheel", None)
                elif k == "ATTR_manip_wheel":
                    state["wheel"] = float(p[1])
                else:
                    state["manip"] = l.strip()
                    state.pop("wheel", None)
            elif k == "ATTR_light_level":
                state["light"] = (float(p[1]), float(p[2]), p[3])
            elif k == "ATTR_light_level_reset":
                state.pop("light", None)
            elif k == "ATTR_cockpit_device":
                state["device"] = p[1]
            elif k in ("ATTR_cockpit", "ATTR_cockpit_region", "ATTR_no_cockpit"):
                state.pop("device", None)
            elif k == "ATTR_blend":
                state.pop("no_blend", None)
            elif k == "ATTR_no_blend":
                state["no_blend"] = float(p[1]) if len(p) > 1 else 0.5
            elif k == "ATTR_shade_flat":
                pass
            elif k == "ATTR_draw_disable":
                state["disabled"] = True
            elif k == "ATTR_draw_enable":
                state.pop("disabled", None)
            elif k == "ATTR_reset":
                state.clear()
            elif k == "TRIS":
                s, n = int(p[1]), int(p[2])
                self.draws.append(Draw(s, n, stack[-1].copy(), dict(state), max(lod, 0), hide_stack[-1]))

    def positions(self, d):
        ii = self.idx[d.start:d.start + d.count]
        v = self.vt[ii, :3]
        return v @ d.matrix[:3, :3].T + d.matrix[:3, 3]

    def triangles(self, d):
        ii = self.idx[d.start:d.start + d.count]
        v = self.vt[ii]
        pos = v[:, :3] @ d.matrix[:3, :3].T + d.matrix[:3, 3]
        nrm = v[:, 3:6] @ d.matrix[:3, :3].T
        return pos, nrm, v[:, 6:8]
