import json
import os
import sys

EXPORT = 'E:/fbw-gse/export'
OPS = ['default', 'dnata', 'swissport', 'menzies']
POSES = {
    'bag_tractor': [],
    'bulk_cart': [('pose_curtain_up', {'fbw/gse/bulk_cart/curtain': 1}, 'rear_left')],
    'belt_loader': [('pose_boom_20deg', {'fbw/gse/belt_loader/boom': 20}, 'side_right'), ('pose_boom_10deg', {'fbw/gse/belt_loader/boom': 10}, 'side_right')],
    'ldcl_loader': [('pose_platforms_2p3m', {'fbw/gse/ldcl_loader/rear_platform': 2.3, 'fbw/gse/ldcl_loader/bridge_platform': 2.3}, 'front_right'),
                    ('pose_platforms_1p2m', {'fbw/gse/ldcl_loader/rear_platform': 1.2, 'fbw/gse/ldcl_loader/bridge_platform': 1.2}, 'side_right')],
    'uld_set': [],
}
VIEWS = {'bag_tractor': 'front_right', 'bulk_cart': 'front_right', 'belt_loader': 'front_right', 'ldcl_loader': 'front_right', 'uld_set': 'front_right'}


def main():
    names = sys.argv[2:]
    shots = os.path.join(EXPORT, '_g3', 'renders')
    os.makedirs(shots, exist_ok=True)
    jobs = []
    for v in names:
        for op in OPS:
            fn = f'{v}.obj' if op == 'default' else f'{v}_{op}.obj'
            jobs.append({'obj': f'{EXPORT}/{v}/{fn}', 'png': f'{shots}/{v}_{op}.png', 'view': VIEWS[v], 'values': {}})
        for tag, values, view in POSES[v]:
            jobs.append({'obj': f'{EXPORT}/{v}/{v}_dnata.obj', 'png': f'{shots}/{v}_{tag}.png', 'view': view, 'values': values})
    with open(sys.argv[1], 'w', encoding='utf-8') as fh:
        json.dump(jobs, fh, indent=1)
    print(len(jobs), 'jobs')


if __name__ == '__main__':
    main()
