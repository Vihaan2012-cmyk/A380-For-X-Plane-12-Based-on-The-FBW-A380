"""Moves the A380's exterior animations off SASL in an installed aircraft: every OBJ block keyed on a
`fbw/anim/<clip>` dataref that SASL used to recompute each frame is rekeyed on the dataref it was computed
from, in that dataref's own units, and main.lua stops creating and setting those datarefs.

obj_anims_keyed.py <aircraft dir> [--dry-run]

It is the installed-aircraft twin of the converter's keyed clips (msfs2xp-aircraft/src/rig.rs `Source::Key`,
model/anim.rs `Keys`): a fresh conversion already writes these blocks; this brings an aircraft converted
before that, and post-processed since, to the same place. KEYS was taken from the converter itself (old vs
new output of the same package, blocks matched by axis and key values) and checked against main.lua's own
formulas; change both together. Records itself in objects/optimize_state.json and runs once.
"""
import json
import os
import re
import sys

# clip -> (source dataref, value at clip fraction 0, value at fraction 1[, loop period])
KEYS = {
    'A32NX_HYD_RAT_PROPELLER_ANGLE': ('fbw/A32NX_RAT_PROPELLER_ANGLE', 0, 1),
    'A32NX_HYD_RAT_STOW_POSITION': ('fbw/A32NX_RAT_STOW_POSITION', 0, 1),
    'b_tire_key': ('sim/flightmodel2/gear/tire_rotation_angle_deg[1]', 0, 360, 360),
    'c_gear_door1': ('fbw/A32NX_GEAR_DOOR_CENTER_POSITION', 0, 100),
    'c_tire_anim_LH': ('sim/flightmodel2/gear/tire_rotation_angle_deg[0]', 0, 360, 360),
    'c_tire_anim_RH': ('sim/flightmodel2/gear/tire_rotation_angle_deg[0]', 0, 360, 360),
    'c_wheel': ('sim/flightmodel2/gear/tire_steer_actual_deg[0]', -69.999995, 69.999995),
    'l_aileron1_percent_key': ('fbw/A32NX_HYD_AIL_LEFT_INWARD_DEFLECTION', 1, -0.2),
    'l_aileron2_percent_key': ('fbw/A32NX_HYD_AIL_LEFT_MIDDLE_DEFLECTION', 1, -0.2),
    'l_aileron3_percent_key': ('fbw/A32NX_HYD_AIL_LEFT_OUTWARD_DEFLECTION', 1, -0.2),
    'l_b_gear_tilt': ('fbw/A32NX_GEAR_1_TILT_POSITION', 0, 1),
    'l_blg_inner_door': ('fbw/A32NX_SECONDARY_GEAR_DOOR_LEFT_POSITION', 0, 100),
    'l_body_gear_door': ('fbw/A32NX_GEAR_DOOR_LEFT_POSITION', 0, 100),
    'l_flap_percent_key': ('fbw/A32NX_LEFT_FLAPS_ANIMATION_POSITION', 0, 100),
    'l_inner_elevator_deflection': ('fbw/A32NX_HYD_ELEV_LEFT_INWARD_DEFLECTION', -0.2, 1),
    'l_outer_elevator_deflection': ('fbw/A32NX_HYD_ELEV_LEFT_OUTWARD_DEFLECTION', -0.2, 1),
    'l_slat_percent_key': ('fbw/A32NX_LEFT_SLATS_POSITION_PERCENT', 0, 100),
    'l_spoiler1_key': ('fbw/A32NX_HYD_SPOILER_1_LEFT_DEFLECTION', 0, 1.3),
    'l_spoiler2_key': ('fbw/A32NX_HYD_SPOILER_2_LEFT_DEFLECTION', 0, 1.3),
    'l_spoiler3_key': ('fbw/A32NX_HYD_SPOILER_3_LEFT_DEFLECTION', 0, 1.3),
    'l_spoiler4_key': ('fbw/A32NX_HYD_SPOILER_4_LEFT_DEFLECTION', 0, 1.3),
    'l_spoiler5_key': ('fbw/A32NX_HYD_SPOILER_5_LEFT_DEFLECTION', 0, 1.3),
    'l_spoiler6_key': ('fbw/A32NX_HYD_SPOILER_6_LEFT_DEFLECTION', 0, 1.3),
    'l_spoiler7_key': ('fbw/A32NX_HYD_SPOILER_7_LEFT_DEFLECTION', 0, 1.3),
    'l_spoiler8_key': ('fbw/A32NX_HYD_SPOILER_8_LEFT_DEFLECTION', 0, 1.3),
    'l_tire_key': ('sim/flightmodel2/gear/tire_rotation_angle_deg[3]', 0, 360, 360),
    'l_w_gear_tilt': ('fbw/A32NX_GEAR_3_TILT_POSITION', 0, 1),
    'l_wing_gear_door': ('fbw/A32NX_GEAR_DOOR_LEFT_POSITION', 0, 100),
    'll_tire_key': ('sim/flightmodel2/gear/tire_rotation_angle_deg[1]', 0, 360, 360),
    'lower_rudder_percent_key': ('fbw/A32NX_HYD_LOWER_RUD_DEFLECTION', 0, 1),
    'lr_tire_key': ('sim/flightmodel2/gear/tire_rotation_angle_deg[1]', 0, 360, 360),
    'outflow_valve_1': ('fbw/A32NX_PRESS_MAN_OUTFLOW_VALVE_1_OPEN_PERCENTAGE', 0, 100),
    'outflow_valve_2': ('fbw/A32NX_PRESS_MAN_OUTFLOW_VALVE_2_OPEN_PERCENTAGE', 0, 100),
    'outflow_valve_3': ('fbw/A32NX_PRESS_MAN_OUTFLOW_VALVE_3_OPEN_PERCENTAGE', 0, 100),
    'outflow_valve_4': ('fbw/A32NX_PRESS_MAN_OUTFLOW_VALVE_4_OPEN_PERCENTAGE', 0, 100),
    'r_aileron1_percent_key': ('fbw/A32NX_HYD_AIL_RIGHT_INWARD_DEFLECTION', -0.2, 1),
    'r_aileron2_percent_key': ('fbw/A32NX_HYD_AIL_RIGHT_MIDDLE_DEFLECTION', -0.2, 1),
    'r_aileron3_percent_key': ('fbw/A32NX_HYD_AIL_RIGHT_OUTWARD_DEFLECTION', -0.2, 1),
    'r_b_gear_tilt': ('fbw/A32NX_GEAR_2_TILT_POSITION', 0, 1),
    'r_blg_inner_door': ('fbw/A32NX_SECONDARY_GEAR_DOOR_RIGHT_POSITION', 0, 100),
    'r_body_gear_door': ('fbw/A32NX_GEAR_DOOR_RIGHT_POSITION', 0, 100),
    'r_flap_percent_key': ('fbw/A32NX_RIGHT_FLAPS_ANIMATION_POSITION', 0, 100),
    'r_inner_elevator_deflection': ('fbw/A32NX_HYD_ELEV_RIGHT_INWARD_DEFLECTION', -0.2, 1),
    'r_outer_elevator_deflection': ('fbw/A32NX_HYD_ELEV_RIGHT_OUTWARD_DEFLECTION', -0.2, 1),
    'r_slat_percent_key': ('fbw/A32NX_RIGHT_SLATS_POSITION_PERCENT', 0, 100),
    'r_spoiler1_key': ('fbw/A32NX_HYD_SPOILER_1_RIGHT_DEFLECTION', 0, 1.3),
    'r_spoiler2_key': ('fbw/A32NX_HYD_SPOILER_2_RIGHT_DEFLECTION', 0, 1.3),
    'r_spoiler3_key': ('fbw/A32NX_HYD_SPOILER_3_RIGHT_DEFLECTION', 0, 1.3),
    'r_spoiler4_key': ('fbw/A32NX_HYD_SPOILER_4_RIGHT_DEFLECTION', 0, 1.3),
    'r_spoiler5_key': ('fbw/A32NX_HYD_SPOILER_5_RIGHT_DEFLECTION', 0, 1.3),
    'r_spoiler6_key': ('fbw/A32NX_HYD_SPOILER_6_RIGHT_DEFLECTION', 0, 1.3),
    'r_spoiler7_key': ('fbw/A32NX_HYD_SPOILER_7_RIGHT_DEFLECTION', 0, 1.3),
    'r_spoiler8_key': ('fbw/A32NX_HYD_SPOILER_8_RIGHT_DEFLECTION', 0, 1.3),
    'r_w_gear_tilt': ('fbw/A32NX_GEAR_4_TILT_POSITION', 0, 1),
    'r_wing_gear_door': ('fbw/A32NX_GEAR_DOOR_RIGHT_POSITION', 0, 100),
    'rat_blur': ('fbw/A32NX_RAT_RPM', 0, 1000),
    'rl_tire_key': ('sim/flightmodel2/gear/tire_rotation_angle_deg[2]', 0, 360, 360),
    'rr_tire_key': ('sim/flightmodel2/gear/tire_rotation_angle_deg[2]', 0, 360, 360),
    'thrust_rev_1': ('sim/flightmodel2/engines/thrust_reverser_deploy_ratio[1]', 0, 1),
    'thrust_rev_2': ('sim/flightmodel2/engines/thrust_reverser_deploy_ratio[2]', 0, 1),
    'trimtab_elevator_key': ('sim/cockpit2/controls/elevator_trim', 1, -1),
    'upper_rudder_percent_key': ('fbw/A32NX_HYD_UPPER_RUD_DEFLECTION', 0, 1),
}
# ANIM_hide/show helpers SASL computed for a visibility test: name -> (source, value at 0, value at 1)
HIDES = {
    'N1_pct_1': ('sim/cockpit2/engine/indicators/N1_percent[0]', 0, 1),
    'N1_pct_2': ('sim/cockpit2/engine/indicators/N1_percent[1]', 0, 1),
    'N1_pct_3': ('sim/cockpit2/engine/indicators/N1_percent[2]', 0, 1),
    'N1_pct_4': ('sim/cockpit2/engine/indicators/N1_percent[3]', 0, 1),
    'rat_blur_disc': ('fbw/A32NX_RAT_RPM', 0, 1000),
}
ALL = {f'fbw/anim/{k}': v for k, v in {**KEYS, **HIDES}.items()}
STEP = 'obj_anims_keyed'


def num(x):
    s = f'{x:.6f}'.rstrip('0').rstrip('.')
    return '0' if s in ('-0', '') else s


def indent(line):
    return line[:len(line) - len(line.lstrip())]


def rekey(lines):
    """Rewritten OBJ lines and how many blocks changed."""
    out, n, i = [], 0, 0
    while i < len(lines):
        line = lines[i]
        t = line.split()
        if t and t[0] in ('ANIM_rotate_begin', 'ANIM_trans_begin') and t[-1] in ALL:
            src, lo, hi, *period = ALL[t[-1]]
            kw = 'ANIM_rotate_key' if t[0] == 'ANIM_rotate_begin' else 'ANIM_trans_key'
            j, keys = i + 1, []
            while j < len(lines) and lines[j].split()[:1] == [kw]:
                k = lines[j].split()
                keys.append(f'{indent(lines[j])}{kw} {num(lo + float(k[1]) * (hi - lo))} {" ".join(k[2:])}')
                j += 1
            if hi < lo:
                keys.reverse()  # X-Plane wants keys in ascending dataref order
            out.append(indent(line) + ' '.join(t[:-1] + [src]))
            out += keys
            if period and not (j < len(lines) and lines[j].lstrip().startswith('ANIM_keyframe_loop')):
                out.append(f'{indent(line)}ANIM_keyframe_loop {num(period[0])}')
            n += 1
            i = j
            continue
        if t and t[0] in ('ANIM_hide', 'ANIM_show') and len(t) == 4 and t[3] in ALL:
            src, lo, hi, *_ = ALL[t[3]]
            a, b = (lo + float(x) * (hi - lo) for x in t[1:3])
            out.append(f'{indent(line)}{t[0]} {num(min(a, b))} {num(max(a, b))} {src}')
            n += 1
            i += 1
            continue
        out.append(line)
        i += 1
    return out, n


def strip_lua(text):
    """main.lua without the creation and per-frame setting of every rekeyed dataref."""
    names = '|'.join(re.escape(d) for d in ALL)
    drop = re.compile(rf'^\s*(D\["({names})"\] = createGlobalProperty|setc?\(D\["({names})"\], )')
    lines = text.split('\n')
    kept = [l for l in lines if not drop.match(l)]
    left = [l for l in kept if re.search(rf'"({names})"', l)]
    if left:
        raise SystemExit('main.lua still reads a rekeyed dataref:\n' + '\n'.join(left[:10]))
    return '\n'.join(kept), len(lines) - len(kept)


def self_check():
    obj = ['ANIM_rotate_begin 1 0 0 fbw/anim/l_aileron1_percent_key', 'ANIM_rotate_key 0 -30', 'ANIM_rotate_key 1 20',
           'ANIM_rotate_end', 'ANIM_rotate_begin 0 1 0 fbw/anim/c_tire_anim_LH', 'ANIM_rotate_key 0 0',
           'ANIM_rotate_key 1 -360', 'ANIM_rotate_end', 'ANIM_hide -1 0.5 fbw/anim/rat_blur_disc', 'ANIM_show 0 1 fbw/other']
    out, n = rekey(obj)
    assert n == 3, n
    assert out[:4] == ['ANIM_rotate_begin 1 0 0 fbw/A32NX_HYD_AIL_LEFT_INWARD_DEFLECTION',
                       'ANIM_rotate_key -0.2 20', 'ANIM_rotate_key 1 -30', 'ANIM_rotate_end'], out[:4]
    assert out[4:9] == ['ANIM_rotate_begin 0 1 0 sim/flightmodel2/gear/tire_rotation_angle_deg[0]', 'ANIM_rotate_key 0 0',
                        'ANIM_rotate_key 360 -360', 'ANIM_keyframe_loop 360', 'ANIM_rotate_end'], out[4:9]
    assert out[9:] == ['ANIM_hide -1000 500 fbw/A32NX_RAT_RPM', 'ANIM_show 0 1 fbw/other'], out[9:]
    assert rekey(out) == (out, 0), 'a second run changes nothing'
    lua = 'D["fbw/anim/c_wheel"] = createGlobalPropertyf("fbw/anim/c_wheel", 0)\n  setc(D["fbw/anim/c_wheel"], 1)\nkeep'
    assert strip_lua(lua) == ('keep', 2)


def main():
    self_check()
    aircraft = sys.argv[1]
    dry = '--dry-run' in sys.argv
    objects = os.path.join(aircraft, 'objects')
    state_path = os.path.join(objects, 'optimize_state.json')
    state = json.load(open(state_path)) if os.path.exists(state_path) else {}
    if state.get(STEP):
        print('obj_anims_keyed: already done')
        return
    lua_path = os.path.join(aircraft, 'plugins', 'sasl', 'data', 'modules', 'main.lua')
    lua, dropped = strip_lua(open(lua_path, encoding='utf-8').read())
    files = blocks = 0
    for root, _, names in os.walk(objects):
        for name in names:
            if not name.endswith('.obj'):
                continue
            path = os.path.join(root, name)
            out, n = rekey(open(path, encoding='utf-8', errors='replace').read().split('\n'))
            if n:
                files += 1
                blocks += n
                if not dry:
                    open(path, 'w', encoding='utf-8', newline='\n').write('\n'.join(out))
    print(f'obj_anims_keyed: {blocks} blocks in {files} objects rekeyed; {dropped} main.lua lines dropped'
          + (' (dry run)' if dry else ''))
    if not dry:
        open(lua_path, 'w', encoding='utf-8', newline='\n').write(lua)
        state[STEP] = True
        json.dump(state, open(state_path, 'w'), indent=1)


if __name__ == '__main__':
    main()
