"""usage: make_hud_font.py <out.ttf>

The HUD's stroke font: each glyph is drawn as centre lines on a 4 x 7 cell
(x 0..4, y 0..7 from the baseline, cap height 7), stroked to one width with
round caps and joins, and written as a monospaced TrueType font, family
"A380X-HUD". Shapes follow the Airbus HUD's monoline lettering: plain,
slightly condensed, open counters, a 1 with a short flag and no foot."""

import math
import sys

import pathops
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.cu2quPen import Cu2QuPen
from fontTools.pens.ttGlyphPen import TTGlyphPen

UPM = 1000
CELL = 100.0
STROKE = 78.0
ADVANCE = 640
LEFT = 120
ASCENT, DESCENT = 900, 250


def arc(cx, cy, rx, ry, a0, a1, steps=None):
    steps = steps or max(6, int(abs(a1 - a0) / 8))
    return [(cx + rx * math.cos(math.radians(a0 + (a1 - a0) * k / steps)), cy + ry * math.sin(math.radians(a0 + (a1 - a0) * k / steps))) for k in range(steps + 1)]


def oval(cx, cy, rx, ry):
    return arc(cx, cy, rx, ry, 0, 360, 48)


def stadium(x0, y0, x1, y1):
    r = (x1 - x0) / 2
    cx = (x0 + x1) / 2
    return arc(cx, y1 - r, r, r, 0, 180, 18) + arc(cx, y0 + r, r, r, 180, 360, 18) + [(x1, y1 - r)]


G = {}
G["0"] = [stadium(0, 0, 4, 7)]
G["1"] = [[(0.9, 5.6), (2.2, 7), (2.2, 0)]]
G["2"] = [arc(2, 5, 2, 2, 160, 0, 16) + arc(2, 5, 2, 2, 0, -40, 6) + [(0, 0), (4, 0)]]
G["3"] = [arc(2, 5.25, 1.85, 1.75, 150, -90, 22), arc(2, 1.85, 2, 1.85, 90, -160, 24)]
G["4"] = [[(3, 0), (3, 7), (0, 2), (4, 2)]]
G["5"] = [[(3.8, 7), (0.3, 7), (0.1, 4.1)] + arc(2, 2.3, 2, 2.3, 140, -160, 26)]
G["6"] = [arc(2, 4.6, 2, 2.4, 60, 180, 12) + [(0, 2)], oval(2, 2, 2, 2)]
G["7"] = [[(0, 7), (4, 7), (1.2, 0)]]
G["8"] = [oval(2, 5.3, 1.75, 1.7), oval(2, 1.85, 2, 1.85)]
G["9"] = [oval(2, 5, 2, 2), [(4, 5), (4, 2.4)] + arc(2, 2.4, 2, 2.4, 0, -120, 12)]
G["A"] = [[(0, 0), (2, 7), (4, 0)], [(0.7, 2.4), (3.3, 2.4)]]
G["B"] = [[(0, 0), (0, 7), (2.6, 7)] + arc(2.6, 5.3, 1.4, 1.7, 90, -90, 14) + [(0, 3.6)], [(2.7, 3.6)] + arc(2.7, 1.8, 1.3, 1.8, 90, -90, 14) + [(0, 0)]]
G["C"] = [arc(2.1, 3.5, 1.9, 3.5, 40, 320, 40)]
G["D"] = [[(0, 0), (0, 7), (1.8, 7)] + arc(1.8, 3.5, 2.2, 3.5, 90, -90, 30) + [(0, 0)]]
G["E"] = [[(4, 7), (0, 7), (0, 0), (4, 0)], [(0, 3.6), (3, 3.6)]]
G["F"] = [[(4, 7), (0, 7), (0, 0)], [(0, 3.6), (3, 3.6)]]
G["G"] = [arc(2.1, 3.5, 1.9, 3.5, 40, 340, 40) + [(4, 3.2), (2.4, 3.2)]]
G["H"] = [[(0, 0), (0, 7)], [(4, 0), (4, 7)], [(0, 3.6), (4, 3.6)]]
G["I"] = [[(2, 0), (2, 7)], [(0.9, 7), (3.1, 7)], [(0.9, 0), (3.1, 0)]]
G["J"] = [[(3.6, 7), (3.6, 1.8)] + arc(1.9, 1.8, 1.7, 1.8, 0, -180, 16)]
G["K"] = [[(0, 0), (0, 7)], [(4, 7), (0, 2.6)], [(1.4, 4.1), (4, 0)]]
G["L"] = [[(0, 7), (0, 0), (4, 0)]]
G["M"] = [[(0, 0), (0, 7), (2, 3), (4, 7), (4, 0)]]
G["N"] = [[(0, 0), (0, 7), (4, 0), (4, 7)]]
G["O"] = [stadium(0, 0, 4, 7)]
G["P"] = [[(0, 0), (0, 7), (2.4, 7)] + arc(2.4, 5.2, 1.6, 1.8, 90, -90, 14) + [(0, 3.4)]]
G["Q"] = [stadium(0, 0, 4, 7), [(2.4, 1.6), (4.1, -0.4)]]
G["R"] = [[(0, 0), (0, 7), (2.4, 7)] + arc(2.4, 5.2, 1.6, 1.8, 90, -90, 14) + [(0, 3.4)], [(2.2, 3.4), (4, 0)]]
G["S"] = [arc(2, 5.3, 1.9, 1.7, 30, 270, 26) + arc(2, 1.8, 2, 1.8, 90, -150, 26)]
G["T"] = [[(0, 7), (4, 7)], [(2, 7), (2, 0)]]
G["U"] = [[(0, 7), (0, 2)] + arc(2, 2, 2, 2, 180, 360, 18) + [(4, 7)]]
G["V"] = [[(0, 7), (2, 0), (4, 7)]]
G["W"] = [[(0, 7), (0.9, 0), (2, 4.4), (3.1, 0), (4, 7)]]
G["X"] = [[(0, 7), (4, 0)], [(4, 7), (0, 0)]]
G["Y"] = [[(0, 7), (2, 3.6), (4, 7)], [(2, 3.6), (2, 0)]]
G["Z"] = [[(0, 7), (4, 7), (0, 0), (4, 0)]]
G["-"] = [[(0.6, 3.5), (3.4, 3.5)]]
G["+"] = [[(0.5, 3.5), (3.5, 3.5)], [(2, 2), (2, 5)]]
G["/"] = [[(0.4, -0.4), (3.6, 7.4)]]
G["."] = [[(2, 0), (2, 0.01)]]
G[","] = [[(2.2, 0.3), (1.6, -1)]]
G[":"] = [[(2, 1.2), (2, 1.21)], [(2, 4.8), (2, 4.81)]]
G["("] = [arc(3.6, 3.5, 2.2, 4.2, 120, 240, 16)]
G[")"] = [arc(0.4, 3.5, 2.2, 4.2, 60, -60, 16)]
G["["] = [[(3, 7.6), (1.2, 7.6), (1.2, -0.6), (3, -0.6)]]
G["]"] = [[(1, 7.6), (2.8, 7.6), (2.8, -0.6), (1, -0.6)]]
G["°"] = [oval(2, 6, 0.9, 0.9)]
G["*"] = [[(2, 1.5), (2, 5.5)], [(0.3, 2.5), (3.7, 4.5)], [(0.3, 4.5), (3.7, 2.5)]]
G["="] = [[(0.5, 2.4), (3.5, 2.4)], [(0.5, 4.6), (3.5, 4.6)]]
G["<"] = [[(3.6, 6), (0.4, 3.5), (3.6, 1)]]
G[">"] = [[(0.4, 6), (3.6, 3.5), (0.4, 1)]]
G["'"] = [[(2, 7), (2, 5.4)]]
G["%"] = [oval(0.9, 5.8, 0.8, 1.1), oval(3.1, 1.2, 0.8, 1.1), [(3.8, 7), (0.2, 0)]]
G["←"] = [[(4, 3.5), (0, 3.5)], [(1.5, 5), (0, 3.5), (1.5, 2)]]
G["→"] = [[(0, 3.5), (4, 3.5)], [(2.5, 5), (4, 3.5), (2.5, 2)]]
G["↑"] = [[(2, 0), (2, 7)], [(0.5, 5.5), (2, 7), (3.5, 5.5)]]
G["↓"] = [[(2, 7), (2, 0)], [(0.5, 1.5), (2, 0), (3.5, 1.5)]]
for lower, upper in zip("abcdefghijklmnopqrstuvwxyz", "ABCDEFGHIJKLMNOPQRSTUVWXYZ"):
    G[lower] = G[upper]


def glyph_path(strokes):
    out = pathops.Path()
    for pts in strokes:
        p = pathops.Path()
        (x0, y0), rest = pts[0], pts[1:]
        p.moveTo(LEFT + x0 * CELL, y0 * CELL)
        for x, y in rest or [(x0 + 1e-3, y0)]:
            p.lineTo(LEFT + x * CELL, y * CELL)
        p.stroke(STROKE, pathops.LineCap.ROUND_CAP, pathops.LineJoin.ROUND_JOIN, 4)
        p.convertConicsToQuads()
        out = pathops.op(out, p, pathops.PathOp.UNION) if len(out) else p
    out.simplify()
    return out


def main(out):
    names = {c: (f"uni{ord(c):04X}" if not c.isalnum() else (c if c.isupper() or c.isdigit() else c + ".lc")) for c in G}
    order = [".notdef", "space"] + [names[c] for c in G]
    fb = FontBuilder(UPM, isTTF=True)
    fb.setupGlyphOrder(order)
    cmap = {32: "space"}
    cmap.update({ord(c): names[c] for c in G})
    fb.setupCharacterMap(cmap)
    glyphs, metrics = {}, {}
    empty = TTGlyphPen(None).glyph()
    glyphs[".notdef"], glyphs["space"] = empty, empty
    metrics[".notdef"], metrics["space"] = (ADVANCE, 0), (ADVANCE, 0)
    for c, strokes in G.items():
        path = glyph_path(strokes)
        pen = TTGlyphPen(None)
        path.draw(Cu2QuPen(pen, max_err=1.0, reverse_direction=True))
        glyphs[names[c]] = pen.glyph()
        bounds = path.bounds
        metrics[names[c]] = (ADVANCE, int(bounds[0]) if bounds else 0)
    fb.setupGlyf(glyphs)
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=ASCENT, descent=-DESCENT)
    fb.setupNameTable({"familyName": "A380X-HUD", "styleName": "Regular", "uniqueFontIdentifier": "A380X-HUD Regular", "fullName": "A380X-HUD Regular", "psName": "A380X-HUD-Regular", "version": "Version 1.000"})
    fb.setupOS2(sTypoAscender=ASCENT, sTypoDescender=-DESCENT, usWinAscent=ASCENT, usWinDescent=DESCENT, sCapHeight=700, sxHeight=700, achVendID="FBWX")
    fb.setupPost(isFixedPitch=1)
    fb.save(out)
    print(out, len(G), "glyphs")


if __name__ == "__main__":
    main(sys.argv[1])
