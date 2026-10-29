import re
import sys
from pathlib import Path

CABIN = re.compile(r"a380_cockpit_(00[489]|010|09[6-9]|10[0-7])\.obj$|a380_cabin_(mood_.*|ife_\d)\.obj$")
COCKPIT = re.compile(r"a380_cockpit_.*\.obj$")
DECK_Y = 3.55
EYE_DECK_Y = 4.0
SLICE_M = 16.0
REACH_M = 40.0
SLICE_MIN_TRIS = 30000
FAR = 99999
AWAY = "fbw/anim/eye_away_from_flight_deck"
IN_DECK = "fbw/anim/eye_in_flight_deck"
EYE_Y = "fbw/anim/eye_obj_y"
EYE_Z = "fbw/anim/eye_obj_z"
MARK = "fbw/anim/eye_"


def parse(lines):
    vt, idx_lines = [], []
    for i, l in enumerate(lines):
        if l.startswith("VT"):
            p = l.split()
            vt.append((float(p[2]), float(p[3])))
        elif l.startswith(("IDX10", "IDX ")):
            idx_lines.append(i)
    idx = []
    for i in idx_lines:
        idx.extend(int(v) for v in lines[i].split()[1:])
    return vt, idx, idx_lines


def write_idx(idx):
    out = []
    for k in range(0, len(idx) - len(idx) % 10, 10):
        out.append("IDX10 " + " ".join(map(str, idx[k:k + 10])))
    for k in range(len(idx) - len(idx) % 10, len(idx)):
        out.append(f"IDX {idx[k]}")
    return out


def bucket_of(vt, tri):
    y = sum(vt[i][0] for i in tri) / 3
    z = sum(vt[i][1] for i in tri) / 3
    return (1 if y > DECK_Y else 0, int((z + 100.0) // SLICE_M))


def visibility(deck, zs, sliced, indent):
    out = [f"{indent}ANIM_begin"]
    if sliced:
        z0, z1 = zs
        out.append(f"{indent}ANIM_hide {-FAR} {FAR} {EYE_Z}")
        out.append(f"{indent}ANIM_show {z0 - REACH_M:.1f} {z1 + REACH_M:.1f} {EYE_Z}")
    if deck:
        out.append(f"{indent}ANIM_hide {-FAR} {EYE_DECK_Y} {EYE_Y}")
    else:
        out.append(f"{indent}ANIM_hide {EYE_DECK_Y} {FAR} {EYE_Y}")
    return out


def cull_cabin(lines):
    vt, idx, idx_lines = parse(lines)
    body = []
    for l in lines:
        s = l.strip()
        if s == "ATTR_shadow":
            continue
        if s.startswith("TRIS "):
            start, count = map(int, s.split()[1:3])
            tris = [idx[start + k:start + k + 3] for k in range(0, count, 3)]
            sliced = len(tris) >= SLICE_MIN_TRIS
            groups = {}
            for t in tris:
                d, z = bucket_of(vt, t)
                groups.setdefault((d, z if sliced else 0), []).append(t)
            if len(groups) == 1:
                body.append(l)
                continue
            indent = l[:len(l) - len(l.lstrip())]
            pos = start
            for (d, zk) in sorted(groups):
                g = groups[(d, zk)]
                flat = [i for t in g for i in t]
                idx[pos:pos + len(flat)] = flat
                zr = (zk * SLICE_M - 100.0, (zk + 1) * SLICE_M - 100.0)
                body.extend(visibility(d, zr, sliced, indent))
                body.append(f"{indent}TRIS {pos} {len(flat)}")
                body.append(f"{indent}ANIM_end")
                pos += len(flat)
            continue
        body.append(l)
        if s.startswith("ATTR_LOD"):
            body.extend(["ANIM_begin", f"ANIM_hide 0.5 1.5 {IN_DECK}", "ATTR_no_shadow"])
    return finish(body, idx, idx_lines)


def finish(body, idx, idx_lines):
    if idx_lines:
        first = body.index(next(l for l in body if l.startswith(("IDX10", "IDX "))))
        body = [l for l in body if not l.startswith(("IDX10", "IDX "))]
        body[first:first] = write_idx(idx)
    return close_lod_groups(body)


def close_lod_groups(body, opener=None):
    out = []
    open_group = False
    for l in body:
        if l.startswith("ATTR_LOD") and open_group:
            out.append("ANIM_end")
            open_group = False
        out.append(l)
        if l.startswith("ATTR_LOD"):
            open_group = True
            if opener:
                out.extend(opener)
    if open_group:
        while out and not out[-1].strip():
            out.pop()
        out.append("ANIM_end")
    return out


def cull_cockpit(lines):
    return close_lod_groups(lines, ["ANIM_begin", f"ANIM_hide 0.5 1.5 {AWAY}"])


def swap_head(lines):
    return [l.replace("sim/graphics/view/pilots_head_", "fbw/anim/eye_cg_") if l.lstrip().startswith(("ANIM_show", "ANIM_hide")) else l for l in lines]


def main(acf, objects_dir, backup):
    t = Path(acf).read_bytes().decode("latin-1")
    files = sorted(set(re.findall(r"P _obja/\d+/_v10_att_file_stl (\S+)", t)))
    d = Path(objects_dir)
    done = {"cabin": 0, "cockpit": 0, "exterior": 0}
    for f in files:
        p = d / f
        if not p.exists():
            continue
        text = p.read_text(encoding="latin-1")
        if MARK in text and "pilots_head_" not in text:
            continue
        lines = text.split("\n")
        if CABIN.match(f):
            new, kind = swap_head(cull_cabin(lines)), "cabin"
        elif COCKPIT.match(f):
            new, kind = swap_head(cull_cockpit(lines)), "cockpit"
        elif "pilots_head_" in text:
            new, kind = swap_head(lines), "exterior"
        else:
            continue
        if new == lines:
            continue
        b = Path(backup) / f
        if not b.exists():
            b.parent.mkdir(parents=True, exist_ok=True)
            b.write_bytes(p.read_bytes())
        p.write_text("\n".join(new).rstrip("\n") + "\n", encoding="latin-1")
        done[kind] += 1
        print(kind, f)
    print(done)


if __name__ == "__main__":
    main(*sys.argv[1:])
