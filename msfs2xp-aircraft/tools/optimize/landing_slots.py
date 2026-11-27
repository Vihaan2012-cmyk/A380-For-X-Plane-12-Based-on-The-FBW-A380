"""Moves an installed aircraft's main landing lights to landing_lights_switch slots 0-3, nose lights to 4-5.

landing_slots.py <aircraft dir> [--dry-run]

X-Plane's landing_lights_on, which VATSIM clients read, is slot 0 ("affects the first landing light",
DataRefs.txt). The converter used to give the nose-gear lights 0/1 and the main lights 2-5, so the main
LAND switch never showed on it (#66). The converter (lights.rs, behaviour/events.rs) now emits the new slots;
this remaps lights.obj's airplane_landing_* index and main.lua's switch writes in place, keeping the
hand-tuned light values. Records itself in objects/optimize_state.json and runs once.
"""
import json
import os
import re
import sys

STEP = 'landing_slots'
REMAP = {0: 4, 1: 5, 2: 0, 3: 1, 4: 2, 5: 3}
LUA_REMAP = {0: 4, 1: 5, 2: 0}  # main.lua only addresses each group's first slot (and nose 0/1 at cold start)
LUA = re.compile(r'(sim/cockpit2/switches/landing_lights_switch\[)(\d+)(\])')


def patch_obj(text):
    out, n = [], 0
    for line in text.split('\n'):
        t = line.split(' ')
        if len(t) > 8 and t[0] == 'LIGHT_PARAM' and t[1].startswith('airplane_landing_'):
            t[8] = str(REMAP[int(t[8])])
            line, n = ' '.join(t), n + 1
        out.append(line)
    return '\n'.join(out), n


def patch_lua(text):
    return LUA.subn(lambda m: f'{m[1]}{LUA_REMAP[int(m[2])]}{m[3]}', text)


def self_check():
    obj, n = patch_obj('LIGHT_PARAM airplane_landing_pm 1 2 3 0.7 0.6 0.5 2 2524500cd 0 0 -1 0.9\nLIGHT_PARAM airplane_nav_pm 1 2 3 0')
    assert n == 1 and ' 0.5 0 2524500cd ' in obj and obj.endswith('airplane_nav_pm 1 2 3 0'), obj
    lua, n = patch_lua('XSET("sim/cockpit2/switches/landing_lights_switch[0]", 0) XGET("sim/cockpit2/switches/landing_lights_switch[2]")')
    assert n == 2 and 'switch[4]' in lua and 'switch[0]")' in lua, lua


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
    for path, fn in [(os.path.join(objects, 'lights.obj'), patch_obj),
                     (os.path.join(aircraft, 'plugins', 'sasl', 'data', 'modules', 'main.lua'), patch_lua)]:
        text = open(path, encoding='utf-8', newline='').read()
        out, n = fn(text)
        print(f'{STEP}: {os.path.basename(path)}: {n} slot references remapped' + (' (dry run)' if dry else ''))
        if not dry:
            open(path, 'w', encoding='utf-8', newline='').write(out)
    if not dry:
        state[STEP] = True
        json.dump(state, open(state_path, 'w'), indent=1)


if __name__ == '__main__':
    main()
