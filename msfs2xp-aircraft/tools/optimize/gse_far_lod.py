"""Ground vehicles: the 80-250 m mesh draws out to the farthest LOD's range, and that LOD goes.

The farthest LOD came from a decimator that collapsed across UV seams, so its
triangles span the texture atlas and show as black smears beyond 250 m.

    python gse_far_lod.py <folder of vehicle folders> [--apply]
"""
import re
import sys
from pathlib import Path


def plan(text: str):
    lines = text.split("\n")
    lods = [i for i, l in enumerate(lines) if l.startswith("ATTR_LOD")]
    if len(lods) != 3:
        return None
    far = lines[lods[2]].split()[2]
    kept = lines[: lods[2]]
    kept[lods[1]] = f"ATTR_LOD {lines[lods[1]].split()[1]} {far}"
    while kept and not kept[-1].strip():
        kept.pop()

    def tris(rows):
        return [(int(m[1]), int(m[2])) for m in (re.match(r"\s*TRIS\s+(\d+)\s+(\d+)", r) for r in rows) if m]

    keep_groups = tris(kept[lods[0]:])
    drop_groups = tris(lines[lods[2]:])
    idx_end = max(o + c for o, c in keep_groups)
    if min(o for o, _ in drop_groups) < idx_end:
        raise ValueError("the farthest LOD's indices are not at the end of the table")

    indices = []
    for l in kept:
        if l.startswith(("IDX10 ", "IDX ")):
            indices.extend(l.split()[1:])
    indices = indices[:idx_end]
    vert_end = max(int(i) for i in indices) + 1

    out = []
    verts = 0
    table_written = False
    for l in kept:
        if l.startswith("POINT_COUNTS"):
            p = l.split()
            if p[2] != "0" or p[3] != "0":
                raise ValueError("lines or lights in the point table")
            out.append(f"POINT_COUNTS {vert_end} 0 0 {idx_end}")
        elif l.startswith("VT "):
            if verts < vert_end:
                out.append(l)
            verts += 1
        elif l.startswith(("IDX10 ", "IDX ")):
            if not table_written:
                for k in range(0, idx_end - idx_end % 10, 10):
                    out.append("IDX10 " + " ".join(indices[k : k + 10]))
                for k in range(idx_end - idx_end % 10, idx_end):
                    out.append("IDX " + indices[k])
                table_written = True
        else:
            out.append(l)
    return "\n".join(out) + "\n", len(indices) // 3, sum(c for _, c in drop_groups) // 3, verts - vert_end


def main() -> None:
    root = Path(sys.argv[1])
    apply = "--apply" in sys.argv
    changed = skipped = 0
    for obj in sorted(root.glob("*/*.obj")):
        text = obj.read_bytes().decode("latin-1")
        result = plan(text)
        if result is None:
            skipped += 1
            continue
        new, kept_tris, dropped_tris, dropped_verts = result
        changed += 1
        print(f"{obj.relative_to(root)}: {kept_tris} tris kept, {dropped_tris} tris and {dropped_verts} vertices gone, {len(text) // 1024} -> {len(new) // 1024} KB")
        if apply:
            obj.write_bytes(new.encode("latin-1"))
    print(f"{changed} objects {'changed' if apply else 'to change'}, {skipped} left alone")


if __name__ == "__main__":
    main()
