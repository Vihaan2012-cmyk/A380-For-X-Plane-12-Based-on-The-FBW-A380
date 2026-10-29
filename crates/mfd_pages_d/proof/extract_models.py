import json
import os
import re
import sys

sys.path.insert(0, 'E:/fbw-int/a380-displays/tools/layout_lab')
import dom2tree  # noqa: E402

CAPTURES = 'E:/golden-tour/yssy-omdb-v15/captures'
OUT = 'E:/fbw-build/mfd-pages-d'


def find_all(node, pred, acc=None):
    acc = [] if acc is None else acc
    if 'tag' in node:
        if pred(node):
            acc.append(node)
        for k in node.get('kids', []):
            find_all(k, pred, acc)
    return acc


def has(cls):
    return lambda n: cls in n.get('cls', [])


def first(node, cls):
    r = find_all(node, has(cls))
    return r[0] if r else None


def raw_text(node):
    if 'text' in node:
        return node['text']
    return ''.join(raw_text(k) for k in node.get('kids', []))


def text(node):
    return ' '.join(raw_text(node).replace('\u00a0', ' ').split())


def inline(node):
    return node.get('inline') or ''


def footer_model(nav):
    f = first(nav, 'mfd-footer')
    area = first(f, 'mfd-footer-message-area')
    msg = text(area)
    btn = first(f, 'mfd-button')
    return {'message': msg or None, 'button_text': raw_text(btn)}


def dropdown_model(dd):
    inp = first(dd, 'mfd-input-field-text-input')
    menu = first(dd, 'mfd-dropdown-menu')
    values = [text(s) for s in menu.get('kids', []) if 'tag' in s]
    t = text(inp)
    return {
        'values': values,
        'selected': values.index(t) if t in values else None,
        'open': 'block' in inline(menu),
        'disabled': False,
    }


def surv_button_state(btn):
    kids = btn['kids']
    disabled = 'disabled' in btn.get('cls', [])
    if disabled:
        return (('hidden' in inline(kids[1])), True)
    return ('mfd-value' in kids[0].get('cls', []), False)


def connect(nav):
    pc = first(nav, 'mfd-page-container')
    dd = first(pc, 'mfd-dropdown-container')
    vals = find_all(pc, has('mfd-value'))
    adsc = find_all(pc, has('mfd-adsc-button'))
    rows = find_all(pc, has('mfd-atccom-connect-row'))
    return {
        'notify_to_atc': dropdown_model(dd),
        'active_atc': text(vals[0]),
        'next_atc': text(vals[1]),
        'adsc_armed': 'mfd-value' in adsc[0]['kids'][0].get('cls', []),
        'adsc_emer_armed': 'mfd-value' in adsc[1]['kids'][0].get('cls', []),
        'connected_stations': text(rows[-2]['kids'][1]),
        'footer': footer_model(nav),
    }


def request(nav):
    pc = first(nav, 'mfd-page-container')
    menus = find_all(pc, has('mfd-atccom-request-button-dropdown-menu'))
    open_menu = None
    for i, m in enumerate(menus):
        if 'block' in inline(m):
            open_menu = i
    return {'open_menu': open_menu, 'frames': [], 'footer': footer_model(nav)}


def msg_record(nav):
    return {'footer': footer_model(nav)}


def datis(nav):
    blocks = []
    for b in find_all(nav, has('mfd-atccom-datis-block')):
        header = first(b, 'mfd-atccom-datis-block-header-row')
        fields = find_all(header, has('mfd-input-field-text-input'))
        icao = text(fields[0])
        dds = find_all(header, has('mfd-dropdown-container'))
        type_dd = dropdown_model(dds[0])
        type_dd['open'] = 'block' in inline(first(dds[0], 'mfd-dropdown-menu'))
        code = text(first(header, 'atis-version'))
        stamp = text(first(header, 'atis-time'))
        icons = {c: 'hidden' not in inline(first(header, c)) for c in ('new-atis-symbol', 'auto-update-symbol')}
        msg = text(first(b, 'mfd-atccom-datis-block-msgarea'))
        status_c = dds[1]
        combined_c = dds[2]
        status_visible = 'hidden' not in inline(status_c)
        combined_visible = 'hidden' not in inline(combined_c)
        status_label = text(first(status_c, 'mfd-button')) if status_visible else ''
        if status_label:
            sl = status_label
        elif combined_visible and not msg:
            sl = ''
        else:
            sl = None
        menu = first(combined_c, 'mfd-dropdown-menu')
        blocks.append({
            'icao': '' if icao == '----' else icao,
            'type_dropdown': type_dd,
            'atis_code': code,
            'timestamp': stamp,
            'new_atis': icons['new-atis-symbol'],
            'auto_update': icons['auto-update-symbol'],
            'message': msg or None,
            'status_label': sl,
            'update_menu_open': 'block' in inline(menu),
            'update_menu_right_set': 'right' in inline(menu),
            'icao_editing': 'editing' in fields[0].get('cls', []),
        })
    return {'blocks': blocks, 'footer': footer_model(nav)}


def radio_info(root, prefix):
    labels = [n for n in find_all(root, lambda n: n.get('tag') == 'label' and (n.get('id') or '').startswith(prefix + '_label_'))]
    colour = ''
    for c in ('green', 'white', 'amber', 'cyan', 'yellow'):
        if labels and c in labels[0].get('cls', []):
            colour = c
    return colour, bool(labels and 'disabled' in labels[0].get('cls', []))


def controls(nav):
    pc = first(nav, 'mfd-page-container')
    m = {'footer': footer_model(nav)}
    heads = {c: first(pc, c) for c in ('mfd-surv-xpdr-label', 'mfd-surv-tcas-label', 'mfd-surv-wxr-label', 'mfd-surv-taws-label')}
    m['xpdr_failed'] = 'failed' in heads['mfd-surv-xpdr-label'].get('cls', [])
    m['tcas_failed'] = 'failed' in heads['mfd-surv-tcas-label'].get('cls', [])
    m['wxr_failed'] = 'failed' in heads['mfd-surv-wxr-label'].get('cls', [])
    m['all_taws_failed'] = 'failed' in heads['mfd-surv-taws-label'].get('cls', [])
    left = first(pc, 'mfd-surv-controls-xpdr-left')
    m['squawk_text'] = text(first(left, 'mfd-input-field-text-input'))
    st, dis = surv_button_state(first(left, 'mfd-surv-button'))
    m['alt_rptg_on'], m['alt_rptg_disabled'] = st, dis
    m['xpdr_radio_color'], _ = radio_info(pc, 'CAPT_MFD_survControlsXpdrStatus')
    m['tcas_radio_color'], m['tcas_radio_disabled'] = radio_info(pc, 'CAPT_MFD_survControlsTcasTara')
    m['elevn_tilt_color'], m['elevn_tilt_disabled'] = radio_info(pc, 'CAPT_MFD_survControlswxrElevnTilt')
    entry = first(pc, 'mfd-surv-controls-wxr-entry')
    shown = ['block' in inline(k) for k in entry['kids']]
    txt = [text(first(k, 'mfd-input-field-text-input')) for k in entry['kids']]
    m['elevn_unit'] = 'FT' if shown[0] else ('FL' if shown[1] else '')
    m['tilt_selected'] = shown[2]
    m['elevn_ft_text'], m['elevn_fl_text'], m['tilt_text'] = txt
    cells = find_all(pc, has('mfd-surv-controls-wxr-grid-cell'))
    names = ['wxr', 'pred', 'turb', 'gain', 'mode', 'vd']
    state = {}
    for n, c in zip(names, cells):
        state[n] = surv_button_state(first(c, 'mfd-surv-button'))
    m['wxr_auto'] = state['wxr'][0]
    m['pred_ws_auto'] = state['pred'][0]
    m['turb_auto'] = state['turb'][0]
    m['gain_auto'] = state['gain'][0]
    m['mode_wx'] = state['mode'][0]
    m['vd_auto'] = state['vd'][0]
    m['wxr_inhibited'] = state['pred'][1]
    m['wxr_off'] = 'hidden' in inline(cells[3])
    ge = first(pc, 'mfd-surv-controls-wxr-gain-entry')
    m['gain_text'] = text(first(ge, 'mfd-input-field-text-input'))
    m['gain_field_shown'] = 'block' in inline(ge)
    dlg = first(pc, 'mfd-dialog')
    wrap = [n for n in find_all(pc, lambda n: any(isinstance(k, dict) and 'mfd-dialog' in k.get('cls', []) for k in n.get('kids', [])))][0]
    m['confirm_visible'] = 'block' in inline(wrap)
    m['confirm_label'] = text(first(dlg, 'mfd-dialog-title'))
    taws = find_all(pc, has('mfd-surv-controls-taws-element'))
    ts = [surv_button_state(first(t, 'mfd-surv-button')) for t in taws]
    m['terr_on'], m['terr_failed'] = ts[0]
    m['gpws_on'], m['gpws_failed'] = ts[1]
    m['gs_on'] = ts[2][0]
    m['flap_on'] = ts[3][0]
    return m


def status(nav):
    pc = first(nav, 'mfd-page-container')
    boxes = find_all(pc, has('sys-box'))

    def side(box, n):
        items = find_all(box, has('mfd-surv-status-item'))
        keys = 'abcde'
        d = {'active': 'active' in first(box, 'sys-group').get('cls', [])}
        for k, it in zip(keys, items):
            d['failed_' + k] = 'failed' in it.get('cls', [])
        return d

    return {
        'footer': footer_model(nav),
        'wxr_taws': [side(boxes[0], 5), side(boxes[1], 5)],
        'xpdr_tcas': [side(boxes[2], 2), side(boxes[3], 2)],
    }


def lights(pc):
    d = {}
    for g in find_all(pc, has('mfd-fcubkup-light-group')):
        label = text(first(g, 'mfd-fcubkup-light-caption'))
        d[label] = 'on' in first(g, 'mfd-fcubkup-bars').get('cls', [])
    return d


def afs(nav):
    pc = first(nav, 'mfd-page-container')
    l = lights(pc)
    wins = [(text(first(w, 'mfd-fcubkup-window-value')), text(first(w, 'mfd-fcubkup-window-unit'))) for w in find_all(pc, has('mfd-fcubkup-window'))]
    buttons = find_all(pc, has('mfd-button'))
    btxt = [text(b) for b in buttons]
    labels = [text(x) for x in find_all(pc, has('mfd-fcubkup-label'))]
    return {
        'footer': footer_model(nav),
        'backup_on': l['AFS CP BACKUP'],
        'trk_fpa_mode': btxt[0].startswith('TRK'),
        'metric': 'selected' in buttons[1].get('cls', []),
        'mach_mode': btxt[2].startswith('MACH'),
        'true_mode': btxt[3].startswith('TRUE'),
        'spd_text': wins[0][0], 'hdg_text': wins[1][0], 'alt_text': wins[2][0], 'vs_text': wins[3][0],
        'loc_light': l['LOC'], 'alt_light': l['ALT'], 'appr_light': l['APPR'], 'fd_light': l['FD'],
        'ap1_light': l['AP1'], 'ap2_light': l['AP2'], 'athr_light': l['A/THR'],
    }


def efis(nav):
    pc = first(nav, 'mfd-page-container')
    l = lights(pc)
    w = first(pc, 'mfd-fcubkup-window')
    buttons = find_all(pc, has('mfd-button'))
    f = 0
    for i, k in enumerate(('WPT', 'VOR.D', 'NDB')):
        if l[k]:
            f = i + 1
    o = 0
    for i, k in enumerate(('WX', 'TERR')):
        if l[k]:
            o = i + 1
    title = text(first(nav, 'mfd-title-bar-text'))
    return {
        'footer': footer_model(nav),
        'captain_side': title.startswith('CAPT'),
        'backup_on': l['EFIS CP BACKUP'],
        'baro_text': text(first(w, 'mfd-fcubkup-window-value')),
        'baro_unit': text(first(w, 'mfd-fcubkup-window-unit')),
        'baro_std': 'selected' in buttons[1].get('cls', []),
        'ls': l['LS'], 'vv': l['VV'], 'cstr': l['CSTR'], 'filter': f, 'arpt': l['ARPT'], 'overlay': o, 'traf': l['TRAF'],
    }


KINDS = {
    'connect': connect, 'request': request, 'msg_record': msg_record, 'datis': datis,
    'controls': controls, 'status': status, 'afs': afs, 'efis': efis,
}


def kind_for(name):
    if 'ATCCOM_CONNECT' in name:
        return 'connect'
    if 'ATCCOM_REQUEST' in name or 'ATCCOM_REPORT' in name:
        return 'request'
    if 'ATCCOM_MSG_RECORD' in name:
        return 'msg_record'
    if 'ATCCOM_D-ATIS' in name or 'ATCCOM_EMER' in name:
        return 'datis'
    if 'SURV_CONTROLS' in name:
        return 'controls'
    if 'SURV_STATUS' in name:
        return 'status'
    if 'FCU_BKUP_AFS' in name:
        return 'afs'
    if 'FCU_BKUP_EFIS' in name:
        return 'efis'
    return None


def main():
    os.makedirs(OUT + '/models', exist_ok=True)
    manifest = []
    for name in sorted(os.listdir(CAPTURES)):
        if '_mfd_' not in name:
            continue
        kind = kind_for(name)
        if not kind:
            continue
        tree = dom2tree.convert(f'{CAPTURES}/{name}/mfd.dom.html')
        nav = first(tree, 'mfd-navigator-container')
        model = KINDS[kind](nav)
        path = f'{OUT}/models/{name}.model.json'
        with open(path, 'w', encoding='utf-8') as fh:
            json.dump(model, fh, ensure_ascii=False)
        manifest.append(f'{kind}\t{path}\t{OUT}/{name}.json\t{CAPTURES}/{name}/mfd.dom.html')
    with open(OUT + '/manifest.tsv', 'w', encoding='utf-8') as fh:
        fh.write('\n'.join(manifest) + '\n')
    print(len(manifest), 'captures')


if __name__ == '__main__':
    main()
