import glob
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, 'E:/fbw-int/a380-displays/tools/layout_lab')
sys.path.insert(0, 'E:/fbw-int/plugin/tools/pagetree')
import compare  # noqa: E402

ROOT = 'E:/golden-tour/yssy-omdb-v15/captures'
OUT = 'E:/fbw-build/oit-pages'
BIN = 'E:/fbw-build/target/release/oit_build.exe'


def tag(n):
    return n.get('tag') if 'tag' in n else None


def cls(n):
    return n.get('cls', [])


def kids(n):
    return n.get('kids', [])


def elems(n):
    return [k for k in kids(n) if 'tag' in k]


def text(n):
    if 'text' in n:
        return ' '.join(n['text'].replace('\u00a0', ' ').split())
    return ' '.join(t for t in (text(k) for k in kids(n)) if t)


def find_all(n, pred, out=None):
    out = [] if out is None else out
    if 'tag' in n and pred(n):
        out.append(n)
    for k in kids(n):
        if 'tag' in k:
            find_all(k, pred, out)
    return out


def first(n, pred):
    r = find_all(n, pred)
    return r[0] if r else None


def has(*names):
    return lambda n: all(c in cls(n) for c in names)


def field(root):
    ti = first(root, has('mfd-input-field-text-input'))
    cs = cls(ti)
    units = find_all(root, has('mfd-input-field-unit'))
    lead = text(first(root, has('mfd-unit-leading')))
    trail = text(first(root, has('mfd-unit-trailing')))
    caret = text(first(root, has('mfd-input-field-caret')))
    f = {
        'text': text(ti),
        'caret': caret,
        'mandatory': 'mandatory' in cs,
        'editing': 'editing' in cs,
        'value_selected': 'valueSelected' in cs,
        'inactive': 'inactive' in cs,
        'disabled': 'disabled' in cs,
        'white': 'white' in cs,
        'validating': 'validating' in cs,
        'computed_by_fms': 'computedByFms' in cs,
        'tmpy': 'tmpy' in cs,
        'leading': lead,
        'trailing': trail,
    }
    del units
    return f


def dropdown(root):
    f = field(first(root, has('mfd-input-field-root')))
    menu = first(root, has('mfd-dropdown-menu'))
    return {
        'values': [text(s) for s in elems(menu)],
        'text': f['text'],
        'editing': f['editing'],
        'value_selected': f['value_selected'],
        'inactive': f['inactive'],
        'disabled': f['disabled'],
        'opened': sel_state(menu),
    }


def sel_state(n):
    return 'inline' in n and 'block' in n['inline']


def num(s, default=None):
    m = re.search(r'-?\d+(\.\d+)?', s)
    return float(m.group(0)) if m else default


def fltfolder(page):
    bar = first(page, has('oefb-bar'))
    sub = text(first(bar, has('oefb-sub')))
    btns = [text(b) for b in find_all(bar, has('oefb-btn'))]
    tree = first(page, has('oefb-tree'))
    view = first(page, has('oefb-view'))
    sections = []
    group = ''
    selected = None
    for k in elems(tree):
        c = cls(k)
        if 'oefb-group' in c:
            group = text(k)
        elif 'oefb-heading' in c:
            sections.append({'group': group, 'title': text(k), 'level': 0})
        elif 'oefb-item' in c:
            if 'sel' in c:
                selected = len(sections)
            sections.append({'group': group, 'title': text(k), 'level': 1 if 'l1' in c else 0, 'text': ' '})
    model = {'kind': 'flt_folder', 'tree_hidden': btns[0] == 'SHOW TREE'}
    mono = first(view, has('oefb-mono'))
    if mono is not None and selected is not None:
        sections[selected]['text'] = text(mono)
    msg = first(view, has('oefb-msg'))
    if sections:
        ofp = {'sections': sections}
        m = re.match(r'^(\S+)\s+(\w{4}) - (\w{4})\s+ALTN (\w{4})\s+(.*)$', sub)
        if m:
            ofp.update(flight=m.group(1), origin=m.group(2), destination=m.group(3), alternate=m.group(4), generated=m.group(5))
        else:
            ofp['flight'] = sub
        model['ofp'] = ofp
        model['selected'] = selected
    if msg is not None:
        model['message'] = {'text': text(msg), 'warn': 'warn' in cls(msg)}
    return model


def sheet_view(inline):
    m = re.search(r'translate\(([-\d.e]+)px, ([-\d.e]+)px\) scale\(([-\d.e]+)\) rotate\(([-\d.e]+)deg\)', inline)
    if not m:
        return {}
    return {'pan_x': float(m.group(1)), 'pan_y': float(m.group(2)), 'zoom': float(m.group(3)),
            'rotation': float(m.group(4)), 'night_filter': 'invert' in inline}


def charts(page):
    bar = first(page, has('oefb-bar'))
    segs = find_all(page, has('oefb-seg'))
    tabs, source = segs[0], segs[1]
    tab_on = text(first(tabs, has('on')))
    src_on = text(first(source, has('on')))
    src_map = {'NAVIGRAPH': 'navigraph', 'FAA': 'faa', 'MY CHARTS': 'local'}
    chips = [{'label': text(first(c, has('k'))), 'icao': text(elems(c)[1])} for c in find_all(bar, has('oefb-chip'))]
    inp = first(bar, lambda n: tag(n) == 'input')
    tree = first(page, has('oefb-tree'))
    items = elems(tree)
    if len(items) == 1 and 'oefb-msg' in cls(items[0]):
        lst = {'kind': 'message', 'text': text(items[0])}
    else:
        groups = []
        flat = 0
        selected = None
        for k in items:
            if 'oefb-group' in cls(k):
                groups.append({'name': text(k), 'items': []})
            else:
                if 'sel' in cls(k):
                    selected = flat
                groups[-1]['items'].append(text(k))
                flat += 1
        lst = {'kind': 'groups', 'groups': groups, 'selected': selected}
    stage_el = first(page, has('oefb-stage'))
    sk = elems(stage_el)[0]
    if tag(sk) == 'img':
        stage = {'kind': 'image', 'view': sheet_view(sk.get('inline', ''))}
    elif tag(sk) == 'iframe':
        stage = {'kind': 'pdf'}
    else:
        stage = {'kind': 'message', 'text': text(sk), 'warn': 'warn' in cls(sk)}
    tools = [text(b) for b in find_all(first(page, has('oefb-tools')), has('oefb-btn'))]
    return {
        'kind': 'charts',
        'enroute': tab_on == 'ENROUTE',
        'source': src_map[src_on],
        'status': text(first(bar, has('oefb-sub'))),
        'chips': chips,
        'icao': inp.get('value', ''),
        'list': lst,
        'stage': stage,
        'night': tools[-1] == 'DAY',
    }


def sts(page):
    reg = dropdown(first(page, has('mfd-dropdown-container')))
    lines = find_all(page, has('oit-flt-ops-sts-line'))
    rights = [first(l, has('oit-flt-ops-sts-line-right')) for l in lines]
    fields = find_all(page, has('mfd-input-field-root'))
    fields = [f for f in fields if first(f, has('mfd-input-field-text-input')) is not None]
    lower = first(page, has('oit-flt-ops-sts-lower'))
    lf = find_all(lower, has('mfd-input-field-root'))
    return {
        'kind': 'sts',
        'registration': reg,
        'msn': text(rights[1]),
        'ois_version': text(rights[2]),
        'flt_number': field(lf[0]),
        'from_airport': field(lf[1]),
        'to_airport': field(lf[2]),
    }


def to_perf(page):
    boxes = find_all(page, has('oit-labeled-box-container'))
    dep, cond, wts, res = boxes[0], boxes[1], boxes[2], boxes[3]
    top_inputs = [f for f in elems(first(dep, has('oit-perf-row'))) if 'mfd-input-field-root' in cls(f)]
    rows = find_all(dep, has('oit-perf-row'))
    airport = field(top_inputs[0])
    drops = [d for d in find_all(dep, has('mfd-dropdown-container')) if first(d, has('mfd-dropdown-outer'))]
    runway_dd = dropdown(drops[0])
    entry = dropdown(drops[1])
    shift = field([f for f in elems(rows[1]) if 'mfd-input-field-root' in cls(f)][0])
    notes = find_all(dep, has('oit-perf-note'))
    db_msg = text(notes[0])
    cond_dd = [dropdown(d) for d in find_all(cond, has('mfd-dropdown-container')) if first(d, has('mfd-dropdown-outer'))]
    cgrid = [text(c) for c in find_all(first(cond, has('oit-perf-grid')), has('oit-label'))]
    cv = dict(zip(cgrid[0::2], cgrid[1::2]))
    wx = {}
    if cv.get('OAT', '---') != '---':
        wx['oat'] = num(cv['OAT'])
    if cv.get('QNH', '----') != '----':
        wx['qnh'] = num(cv['QNH'])
    m = re.match(r'(\d+)°T/(\d+)KT', cv.get('WIND', ''))
    if m:
        wx['wind_direction_true'] = float(m.group(1))
        wx['wind_kt'] = float(m.group(2))
    src = text(find_all(cond, has('oit-perf-note'))[-1])
    wx['source'] = src.replace('WEATHER SOURCE:', '').strip()
    wgrid = [text(c) for c in find_all(first(wts, has('oit-perf-grid')), has('oit-label'))]
    wv = dict(zip(wgrid[0::2], wgrid[1::2]))

    def kg(s):
        n = num(s)
        return None if n is None or '-' in s.split()[0] else n * 1000.0

    def pct(s):
        n = num(s)
        return None if n is None or s.startswith('--') else n

    weights = {
        'zero_fuel_weight_kg': kg(wv['ZFW']),
        'zero_fuel_cg': pct(wv['ZFWCG']),
        'fob_kg': kg(wv['FOB']),
        'gross_weight_kg': kg(wv['TOW']),
        'cg': pct(wv['TOCG']),
    }
    cells = find_all(first(res, has('oit-perf-grid')), has('oit-label'))
    rv = {text(k): v for k, v in zip(cells[0::2], cells[1::2])}
    rtxt = {k: text(v) for k, v in rv.items()}
    v1c = cls(rv['V1'])
    source = 'simbrief' if 'cyan' in v1c else 'model' if 'oit-green-text' in v1c else 'none'

    def alt(s):
        return None if s.startswith('-') else num(s)

    row = first(res, has('oit-perf-row'))
    rwy_text = text(find_all(row, has('oit-green-text'))[0])
    results = {
        'source': source,
        'runway_text': rwy_text,
        'v1': rtxt['V1'], 'vr': rtxt['VR'], 'v2': rtxt['V2'], 'flaps': rtxt['FLAPS'], 'flex': rtxt['FLEX'],
        'ths': rtxt['THS'], 'mtow': rtxt['MTOW (PERF)'], 'stop_margin': rtxt['STOP MARGIN'],
        'note': text(find_all(res, has('oit-perf-note'))[-1]),
    }
    return {
        'kind': 'to_perf',
        'airport': airport,
        'runway_dropdown': runway_dd,
        'database_message': db_msg,
        'shift_field': shift,
        'entry_angle': entry,
        'weather': wx,
        'rwy_cond': cond_dd[0], 'air_cond': cond_dd[1], 'flaps': cond_dd[2], 'anti_ice': cond_dd[3], 'thrust': cond_dd[4],
        'weights': weights,
        'thr_red_ft': alt(rtxt['THR RED']),
        'eo_accel_ft': alt(rtxt['EO ACCEL']),
        'results': results,
    }


def aoc(page):
    files = find_all(first(page, has('oit-avncs-navigator-left')), has('oit-avncs-navigator-file-name'))
    folder = [text(f) for f in files if 'selected' in cls(f)][0]
    fchildren = first(page, has('oit-avncs-navigator-folder-children'))
    right = first(page, has('oit-avncs-navigator-right'))
    list_el, viewer = elems(right)
    messages = []
    for r in elems(list_el)[1:]:
        c = elems(r)
        if 'fr' in cls(r) and len(c) == 3:
            messages.append({'title': text(c[0]), 'zulu': text(c[1]), 'is_new': text(c[2]) == '*'})
    model = {'kind': 'aoc', 'folder': folder, 'inboxes_expanded': 'hidden' not in cls(fchildren), 'messages': messages}
    if elems(viewer):
        head, box = elems(viewer)
        labels = [text(x) for x in find_all(head, has('oit-label'))]
        model['open'] = {'from': labels[1], 'zulu': labels[3], 'lines': text(box).split('\n')}
    return model


PAGES = {
    'FLT OPS MENU': lambda p: {'kind': 'menu'},
    'FLT FOLDER': fltfolder,
    'CHARTS': charts,
    'STATUS PAGE': sts,
    'T.O PERF': to_perf,
    'AOC': aoc,
}


def loading_width(n):
    fill = first(n, has('LoadingProgressBarFill'))
    m = re.search(r'width:\s*([^;]+)', fill.get('inline', '')) if fill else None
    return m.group(1).strip() if m else ''


def model_for(path):
    g = compare.dom2tree.convert(path)
    root = compare.find(g, lambda x: x.get('id') == 'OIT_CONTENT')
    st, wrapper, ansu = elems(root)
    ids = [n.get('id', '') for n in find_all(wrapper, lambda n: bool(n.get('id')))]
    capt = any(i.startswith('CAPT_') for i in ids)
    flt, av = elems(wrapper)
    fcontent, flogin, floading = elems(flt)
    heading = text(first(fcontent, has('oit-heading')))
    labels = find_all(fcontent, has('mfd-page-selector-label'))
    nav = first(fcontent, has('mfd-navigator-container'))
    page_el = elems(nav)[0]
    page = PAGES[heading](page_el)
    acontent, alogin, aloading = elems(av)
    hdr = elems(acontent)[0]
    ahdr_rows = [text(first(hdr, has('oit-avncs-header-flt-nbr')))]
    sub = first(acontent, has('oit-header-row'))
    sub_btns = {text(b): b for b in find_all(elems(acontent)[1], has('mfd-button'))}
    return {
        'capt': capt,
        'self_test_visible': sel_state(st),
        'oit_visible': sel_state(wrapper),
        'ansu_failed': sel_state(ansu),
        'flt_ops': {
            'hidden': 'hidden' in cls(flt),
            'content_hidden': 'hidden' in cls(fcontent),
            'login_hidden': 'hidden' in cls(flogin),
            'loading_hidden': 'hidden' in cls(floading),
            'loading_fill_width': loading_width(floading),
            'menu_opened': 'opened' in cls(labels[0]),
            'functions_opened': 'opened' in cls(labels[1]),
            'page': page,
        },
        'avncs': {
            'hidden': 'hidden' in cls(av),
            'content_hidden': 'hidden' in cls(acontent),
            'login_hidden': 'hidden' in cls(alogin),
            'loading_hidden': 'hidden' in cls(aloading),
            'loading_fill_width': loading_width(aloading),
            'flt_number': ahdr_rows[0],
            'back_disabled': 'disabled' in cls(sub_btns['BACK']),
            'forward_disabled': 'disabled' in cls(sub_btns['FORWARD']),
            'connected_text': text(first(alogin, has('oit-login-page-bottom-left-text'))),
        },
    }


def main():
    os.makedirs(OUT, exist_ok=True)
    only = sys.argv[1] if len(sys.argv) > 1 else None
    results = []
    for d in sorted(os.listdir(ROOT)):
        if 'oit' not in d or (only and only not in d):
            continue
        dom = glob.glob(f'{ROOT}/{d}/oit-*.dom.html')[0]
        model = model_for(dom)
        mp = f'{OUT}/{d}.model.json'
        tp = f'{OUT}/{d}.json'
        json.dump(model, open(mp, 'w', encoding='utf-8'))
        subprocess.run([BIN, mp, tp], check=True)
        r = subprocess.run([sys.executable, 'E:/fbw-int/plugin/tools/pagetree/compare.py', tp, dom, 'id=OIT_CONTENT'],
                           capture_output=True, text=True)
        first_line = r.stdout.splitlines()[0] if r.stdout else r.stderr[-200:]
        results.append((d, first_line))
        print(d, first_line, flush=True)
        if r.returncode != 0:
            for line in r.stdout.splitlines()[1:8]:
                print('   ', line)
    bad = [x for x in results if 'differences 0' not in x[1]]
    print(f'{len(results)} captures, {len(results) - len(bad)} with 0 differences')


if __name__ == '__main__':
    main()
