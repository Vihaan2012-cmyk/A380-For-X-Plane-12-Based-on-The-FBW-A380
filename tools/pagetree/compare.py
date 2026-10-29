import json
import sys

sys.path.insert(0, 'E:/fbw-int/a380-displays/tools/layout_lab')
import dom2tree  # noqa: E402

STYLES = False
KEYS = ('tag', 'id', 'cls', 'value')


def ws(v):
    return ' '.join(str(v).split())


def norm_style(s):
    decls = []
    for part in (s or '').split(';'):
        k, sep, v = part.partition(':')
        if sep and k.strip():
            decls.append(f'{k.strip().lower()}: {ws(v)}')
    return sorted(decls)


def norm_attrs(a):
    return {k.lower(): ws(v) for k, v in sorted((a or {}).items()) if k == '_d' or not k.startswith('_')}


def norm(node):
    if 'text' in node:
        t = ' '.join(node['text'].replace('\u00a0', ' ').split())
        return {'text': t} if t else None
    out = {'tag': node.get('tag')}
    for k in ('id', 'value'):
        if node.get(k) is not None:
            out[k] = node[k]
    if node.get('cls'):
        out['cls'] = sorted(node['cls'])
    if STYLES:
        st = norm_style(node.get('inline'))
        if st:
            out['style'] = st
        if node.get('svgattr'):
            out['svgattr'] = norm_attrs(node['svgattr'])
        if node.get('shapes'):
            out['shapes'] = [[sh.get('t'), norm_attrs(sh.get('a'))] for sh in node['shapes']]
    kids = [norm(k) for k in node.get('kids', [])]
    out['kids'] = [k for k in kids if k is not None]
    return out


def find(node, pred):
    if pred(node):
        return node
    for k in node.get('kids', []):
        if 'tag' in k:
            r = find(k, pred)
            if r is not None:
                return r
    return None


def diff(a, b, path, out, limit):
    if len(out) >= limit:
        return
    if a == b:
        return
    if a is None or b is None or ('text' in a) != ('text' in b):
        out.append(f'{path}: {json.dumps(a)[:120]} != {json.dumps(b)[:120]}')
        return
    if 'text' in a:
        out.append(f'{path}: text {a["text"]!r} != {b["text"]!r}')
        return
    for k in KEYS:
        if a.get(k) != b.get(k):
            if STYLES:
                out.append(f'{path}: {k} {str(a.get(k))[:200]} != {str(b.get(k))[:200]}')
            else:
                out.append(f'{path}: {k} {a.get(k)} != {b.get(k)}')
    ka, kb = a.get('kids', []), b.get('kids', [])
    if len(ka) != len(kb):
        out.append(f'{path}: {len(ka)} kids != {len(kb)}')
    for i in range(min(len(ka), len(kb))):
        name = ka[i].get('tag') or 'text'
        cls = '.'.join(ka[i].get('cls', [])[:2])
        diff(ka[i], kb[i], f'{path}/{name}{("." + cls) if cls else ""}[{i}]', out, limit)


def count(n):
    return 1 + sum(count(k) for k in n.get('kids', []))


def main():
    global STYLES, KEYS
    args = [a for a in sys.argv[1:] if a != '--styles']
    if len(args) != len(sys.argv) - 1:
        STYLES = True
        KEYS = KEYS + ('style', 'svgattr', 'shapes')
    built = json.load(open(args[0], encoding='utf-8'))
    golden = dom2tree.convert(args[1])
    sel = args[2] if len(args) > 2 else None
    if sel:
        key, _, val = sel.partition('=')
        pick = (lambda n: n.get('id') == val) if key == 'id' else (lambda n: val in n.get('cls', []))
        golden = find(golden, pick) or {}
        built = find(built, pick) or built
    a, b = norm(built), norm(golden)
    out = []
    diff(a, b, '', out, 40)
    total = count(b)
    print(f'nodes golden {total}, differences {len(out)}{"+" if len(out) >= 40 else ""}')
    for line in out:
        print(' ', line)
    return 0 if not out else 1


if __name__ == '__main__':
    sys.exit(main())
