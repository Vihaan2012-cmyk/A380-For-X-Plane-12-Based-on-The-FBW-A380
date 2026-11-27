"""Gives an installed aircraft's F/O sidestick its own datarefs, so it no longer follows the captain's.

fo_sidestick.py <aircraft dir> [--dry-run]

FlyByWire's glTF drives both sticks from yoke_lever_stick1_*; the converter now splits them (src/sidesticks.rs).
This is the installed-aircraft twin: the F/O stick object and its click blocks move to yoke_lever_stick2_*, and
main.lua creates those at 0.5 (centred). Records itself in objects/optimize_state.json and runs once.
"""
import json
import os
import sys

STEP = 'fo_sidestick'
RENAME = {'fbw/cockpit/yoke_lever_stick1_l_r': 'fbw/cockpit/yoke_lever_stick2_l_r',
          'fbw/cockpit/yoke_lever_stick_fore1_aft': 'fbw/cockpit/yoke_lever_stick_fore2_aft'}
FO_OBJ = 'a380_cockpit_045_r9.obj'
CLICK = 'a380_cockpit_click.obj'


def rename(line):
    for a, b in RENAME.items():
        line = line.replace(a, b)
    return line


def patch_click(lines):
    """Renames inside each innermost ANIM block that holds a stick and pivots right of centre (first line ANIM_trans x > 0)."""
    out, starts, n = list(lines), [], 0
    for i, line in enumerate(lines):
        t = line.split()
        if t and t[0] == 'ANIM_begin':
            starts.append(i)
        elif t and t[0] == 'ANIM_end' and starts:
            s = starts.pop()
            body = lines[s + 1:i]
            leaf = not any(b.split()[:1] == ['ANIM_begin'] for b in body)
            first = body[0].split() if body else []
            right = first[:1] == ['ANIM_trans'] and len(first) > 1 and float(first[1]) > 0
            if leaf and right and any('yoke_lever_stick' in b for b in body):
                out[s + 1:i] = [rename(b) for b in body]
                n += 1
    return out, n


def patch_lua(text):
    if 'yoke_lever_stick2_l_r' in text:
        return text, 0
    out, n = [], 0
    for line in text.split('\n'):
        out.append(line)
        for a, b in RENAME.items():
            if line.startswith(f'D["{a}"] = createGlobalPropertyf('):
                out.append(line.replace(a, b))
                n += 1
    return '\n'.join(out), n


def self_check():
    click = ['ANIM_begin', 'ANIM_trans 0 0 0 0 0 0 -1 1 fbw/anim/buffet_x', 'ANIM_begin', 'ANIM_trans -0.87 2 -32 -0.87 2 -32', 'ANIM_rotate_begin 0 0 1 fbw/cockpit/yoke_lever_stick1_l_r', 'ANIM_end',
             'ANIM_begin', 'ANIM_trans 0.87 2 -32 0.87 2 -32', 'ANIM_rotate_begin 0 0 1 fbw/cockpit/yoke_lever_stick_fore1_aft',
             'ATTR_manip_drag_axis hand 0 0 1 0 1 fbw/cockpit/yoke_lever_stick_fore1_aft x', 'ANIM_end',
             'ANIM_begin', 'ANIM_trans 0.87 2 -32 0.87 2 -32', 'ANIM_trans_begin fbw/cockpit/PUSH_YOKE_RIGHT', 'ANIM_end', 'ANIM_end']
    out, n = patch_click(click)
    assert n == 1 and 'stick1_l_r' in out[4] and 'fore2_aft' in out[8] and 'fore2_aft' in out[9] and len(out) == len(click), out
    lua, n = patch_lua('D["fbw/cockpit/yoke_lever_stick1_l_r"] = createGlobalPropertyf("fbw/cockpit/yoke_lever_stick1_l_r", 0.5, false, true, false)')
    assert n == 1 and lua.split('\n')[1].startswith('D["fbw/cockpit/yoke_lever_stick2_l_r"] = createGlobalPropertyf("fbw/cockpit/yoke_lever_stick2_l_r", 0.5'), lua
    assert patch_lua(lua) == (lua, 0)


def main():
    self_check()
    aircraft = sys.argv[1]
    dry = '--dry-run' in sys.argv
    objects = os.path.join(aircraft, 'objects')
    state_path = os.path.join(objects, 'optimize_state.json')
    state = json.load(open(state_path)) if os.path.exists(state_path) else {}
    if state.get(STEP):
        print(f'{STEP}: already done')
        return
    read = lambda p: open(p, encoding='utf-8', errors='replace', newline='').read()
    fo = os.path.join(objects, FO_OBJ)
    fo_text = read(fo)
    fo_new = rename(fo_text)
    click = os.path.join(objects, CLICK)
    click_text = read(click)
    nl = '\r\n' if '\r\n' in click_text else '\n'
    click_lines, blocks = patch_click(click_text.split(nl))
    lua = os.path.join(aircraft, 'plugins', 'sasl', 'data', 'modules', 'main.lua')
    lua_text = read(lua)
    lua_nl = '\r\n' if '\r\n' in lua_text else '\n'
    lua_new, created = patch_lua(lua_text.replace(lua_nl, '\n'))
    print(f'{STEP}: {FO_OBJ}: {sum(fo_text.count(a) for a in RENAME)} refs; {CLICK}: {blocks} blocks; main.lua: {created} datarefs'
          + (' (dry run)' if dry else ''))
    assert blocks and created == 2 and fo_new != fo_text, 'nothing to do: already split or a different build'
    if not dry:
        open(fo, 'w', encoding='utf-8', newline='').write(fo_new)
        open(click, 'w', encoding='utf-8', newline='').write(nl.join(click_lines))
        open(lua, 'w', encoding='utf-8', newline='').write(lua_new.replace('\n', lua_nl))
        state[STEP] = True
        json.dump(state, open(state_path, 'w'), indent=1)


if __name__ == '__main__':
    main()
