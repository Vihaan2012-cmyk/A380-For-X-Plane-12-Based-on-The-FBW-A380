"""usage: gp7200_chain.py <installed objects dir> <stage objects dir> [--work DIR] [--templates DIR] [--dressing DIR] [--skip-core] [--skip-collision]

--templates: folder holding the pre-chain *_gp.obj twins (gp7200_variant.py output from the Trent files); without it the installed twins are the templates,
which is only right while they are still the old morph. --dressing: folder with the baked fan-case dressing (see gp7200_dressing.py).

Rebuilds the whole GP7200 chain into the stage folder without touching the install or any Trent file:
  1. refresh the stage from the install (files that differ are copied back, so a rerun starts from the install again);
  2. gp7200_model.py writes the new GP twins;
  3. the GP twins and their per-engine detach copies are put in a scratch folder with the three Trent fan files the cowl tools read for
     their axes and tips, and engine_detach (GP half), cowl_separation, cowl_clip, cowl_hinges and engine_damage run there with the
     measured placement (LIFT, cowl plane stations, door hinge line) set for the GP cowls;
  4. the GP results come back into the stage and a380_engdetach.json keeps its Trent half untouched;
  5. a380_engdress_gp.obj is re-seated on the new axes, the engine core object is built, collision spheres are refreshed.
gp7200_exhaust.py is not run (the model builds the exhaust itself and the twins carry its marker). gp_trent_chains.py is not run: the
twins already carry the Trent ANIM lines and its replacement of the pivots would put the fan back on the Trent axis."""

import json
import shutil
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "engine_interior"))

import cowl_clip as cc  # noqa: E402
import cowl_hinges as ch  # noqa: E402
import cowl_rods as rods  # noqa: E402
import reverser_hinges as rh  # noqa: E402
import cowl_separation as cs  # noqa: E402
import engine_damage as dmg  # noqa: E402
import engine_detach as ed  # noqa: E402
import gp7200_model as model  # noqa: E402
import gp7200_variant as gv  # noqa: E402
from gp7200_geom import S_NOSE, S_TR, SPEC  # noqa: E402
from gp7200_obj import Obj, engine_of_batch, write_obj  # noqa: E402

GP_FILES = ["a380_exterior_001", "a380_exterior_020", "a380_exterior_021", "a380_exterior_022", "a380_exterior_023", "a380_exterior_030", "a380_exterior_031"] + model.BLUR
HINGE_ANGLE = 8.5
GP_OPEN_DEG = 45.0
NACELLE_R_HINGE = 2.25
OLD_LIFT = dict(gv.LIFT)
TRENT_HINGE = (ch.HINGE_SIDE_M, ch.HINGE_UP_M)


def new_lift():
    pe = SPEC["engine_positions"]["per_engine"]
    return {e: pe[str(e)]["recommended"]["axis_point_at_fan_disc_z"][1] - pe[str(e)]["trent_mesh"]["fan_disc_centre"][1] for e in (1, 2, 3, 4)}


def sync(install, stage):
    stage.mkdir(parents=True, exist_ok=True)
    n = 0
    for p in install.iterdir():
        d = stage / p.name
        if p.is_dir():
            if not d.exists():
                shutil.copytree(p, d)
            continue
        if not d.exists() or d.stat().st_size != p.stat().st_size or int(d.stat().st_mtime) != int(p.stat().st_mtime):
            shutil.copy2(p, d)
            n += 1
    return n


def textures_needed(install, scratch):
    for p in install.iterdir():
        if p.suffix.lower() in (".dds", ".png") and ("ALBEDO" in p.name and "BURNT" not in p.name) and p.name.startswith(("A380_EXTERIOR_ENG", "A380_EXTERTIOR", "ENGINE_BLUR")):
            shutil.copy2(p, scratch / p.name)


def tip_prime(axes, e):
    """cowl_separation's 'tip' for a GP engine: the along-axis coordinate that puts INLET_AHEAD_OF_TIP behind it on the nose cowl split."""
    p, d = axes[e]
    plane = model.FN[e + 1].point(np.array([0.0, 0.0, S_NOSE]))
    return cs.INLET_AHEAD_OF_TIP + float((plane - (p + np.array([0.0, gv.LIFT[e + 1], 0.0]))) @ d)


def detach_gp(stage, scratch, manifest):
    gp = {"engines": {}}
    for e0 in range(4):
        centre = manifest["engines"][str(e0)]["centre"]
        names, pts_all = [], []
        for stem in GP_FILES:
            path = stage / f"{stem}_gp.obj"
            text = path.read_text(encoding="utf-8", errors="replace")
            lines, vt, idx = ed.read(path)
            found = ed.engine_tris(lines)
            if e0 not in found:
                continue
            header = [l for l in lines[:60] if l.startswith(ed.HEADER_KEEP)]
            name = f"a380_engdetach_{e0}_{stem}_gp.obj"
            ed.write_detached(scratch / name, header, vt, idx, found[e0], centre)
            names.append(name)
            pts_all += [[float(x) for x in vt[i][:3]] for s, c, _ in found[e0] for i in idx[s:s + c]]
        lo = [min(p[a] for p in pts_all) for a in range(3)]
        hi = [max(p[a] for p in pts_all) for a in range(3)]
        gp["engines"][str(e0)] = {"centre": centre, "half_extent": [round((h - l) / 2, 3) for l, h in zip(lo, hi)], "bottom": round(lo[1] - centre[1], 3), "objects": names}
    return gp


def run_tools(stage, install, scratch):
    if scratch.exists():
        shutil.rmtree(scratch)
    scratch.mkdir(parents=True)
    manifest = json.loads((install / "a380_engdetach.json").read_text(encoding="utf-8"))
    for stem in GP_FILES:
        shutil.copy2(stage / f"{stem}_gp.obj", scratch / f"{stem}_gp.obj")
    for name in ("a380_exterior_030.obj", "a380_exterior_022.obj", "a380_exterior_023.obj", *[f"a380_engdetach_{e}_a380_exterior_030.obj" for e in range(4)]):
        shutil.copy2(install / name, scratch / name)
    textures_needed(install, scratch)
    gp = detach_gp(stage, scratch, manifest)
    lean = {"engines": {k: {"centre": v["centre"], "half_extent": v["half_extent"], "bottom": v["bottom"], "objects": [f"a380_engdetach_{k}_a380_exterior_030.obj"]} for k, v in manifest["engines"].items()}, "gp": gp, "cowls": {}}
    (scratch / "a380_engdetach.json").write_text(json.dumps(lean, indent=1), encoding="utf-8")

    gv.LIFT.update(new_lift())
    cs.SEAM_BEHIND_TIP = (S_TR - S_NOSE) + cs.INLET_AHEAD_OF_TIP
    ch.HINGE_SIDE_M = NACELLE_R_HINGE * float(np.sin(np.radians(HINGE_ANGLE)))
    ch.HINGE_UP_M = NACELLE_R_HINGE * float(np.cos(np.radians(HINGE_ANGLE)))
    real_tips = cs.tips

    def tips(objects, axes, centres):
        return {e: tip_prime(axes, e) for e in axes}

    cs.tips = tips
    gp_hinge = (ch.HINGE_SIDE_M, ch.HINGE_UP_M, cs.SEAM_BEHIND_TIP)
    try:
        cs.main(scratch)
        cc.main(scratch)
        ch.main(scratch, GP_OPEN_DEG)
        print("door hinge lines keyed over 0..0.5:", door_keys(scratch))
        dmg.main(scratch)
        run_rods(scratch, real_tips, gp_hinge)
    finally:
        cs.tips = real_tips
        gv.LIFT.update(OLD_LIFT)
        ch.HINGE_SIDE_M, ch.HINGE_UP_M = TRENT_HINGE
        cs.SEAM_BEHIND_TIP = 3.05
    return json.loads((scratch / "a380_engdetach.json").read_text(encoding="utf-8"))


def door_keys(scratch):
    """The installed doors open over cowl_open 0..0.5 (the 0..1 hinge lines cowl_hinges.py writes are keyed the way reverser_hinges.keyed did, without its C-duct blocks)."""
    n = 0
    for stem in ("020", "021", "022", "023"):
        path = scratch / f"a380_exterior_{stem}_gp.obj"
        lines = path.read_text(encoding="utf-8").split("\n")
        out = []
        for l in lines:
            m = rh.DOOR.match(l)
            if m:
                out += rh.keyed(m.group(1), m.group(2), m.group(3), m.group(4), 0.5)
                n += 1
            else:
                out.append(l)
        path.write_text("\n".join(out), encoding="utf-8", newline="\n")
    return n


def run_rods(scratch, real_tips, gp_hinge):
    """cowl_rods builds the Trent and GP sets in one file; the GP set is built with the GP hinge line and cowl planes."""
    side0, up0 = TRENT_HINGE
    open0 = ch.OPEN_DEG
    state = {"gp": False}
    skin = rods.skin_vertices
    frame = ch.frame
    axes, _, _ = cs.main(scratch, preview=True)
    centres = {int(k): np.array(v["centre"]) for k, v in json.loads((scratch / "a380_engdetach.json").read_text(encoding="utf-8"))["engines"].items()}
    trent_tip = real_tips(scratch, axes, centres)

    def skin_vertices(objects, suffix):
        state["gp"] = suffix == "_gp"
        ch.HINGE_SIDE_M, ch.HINGE_UP_M = (gp_hinge[0], gp_hinge[1]) if state["gp"] else (side0, up0)
        ch.OPEN_DEG = GP_OPEN_DEG if state["gp"] else open0
        return skin(objects, suffix)

    def frame_for(axis, tip):
        e = min(axes, key=lambda k: min(np.linalg.norm(axis[0] - axes[k][0]), np.linalg.norm(axis[0] - axes[k][0] - np.array([0.0, gv.LIFT[k + 1], 0.0]))))
        return frame(axis, tip_prime(axes, e) if state["gp"] else trent_tip[e])

    rods.skin_vertices, ch.frame = skin_vertices, frame_for
    try:
        rods.main(scratch, scratch)
    finally:
        rods.skin_vertices, ch.frame = skin, frame
        ch.HINGE_SIDE_M, ch.HINGE_UP_M = side0, up0
        ch.OPEN_DEG = open0


def bring_back(scratch, stage, install, result):
    n = 0
    for p in sorted(scratch.iterdir()):
        if "_gp" not in p.name or p.suffix != ".obj":
            continue
        if p.name.startswith("a380_exterior_") or p.name.startswith(("a380_engdetach_", "a380_cowldetach_")):
            shutil.copy2(p, stage / p.name)
            n += 1
    for name in ("a380_cowl_rods.obj", "a380_cowl_rods_ALB.png", "a380_cowl_rods_MAT.png"):
        if (scratch / name).is_file():
            shutil.copy2(scratch / name, stage / name)
    manifest = json.loads((install / "a380_engdetach.json").read_text(encoding="utf-8"))
    old_gp = manifest.get("gp", {})
    gp = result["gp"]
    for k, entry in gp["engines"].items():
        extra = [n for n in old_gp.get("engines", {}).get(k, {}).get("objects", []) if n not in entry["objects"] and ("engdress" in n or "engcore" in n)]
        entry["objects"] += [n for n in extra if n not in entry["objects"]]
    manifest["gp"] = gp
    (stage / "a380_engdetach.json").write_text(json.dumps(manifest, indent=1), encoding="utf-8")
    return n


def reseat_engdress(install, stage):
    lift = new_lift()
    dy = {e: lift[e] - OLD_LIFT[e] for e in lift}
    done = []
    path = install / "a380_engdress_gp.obj"
    if not path.is_file():
        return done
    src = Obj(path)
    vt = src.vt.copy()
    for b in src.tris():
        e = engine_of_batch(b)
        if e is None:
            continue
        rows = np.unique(src.idx[b["start"]:b["start"] + b["count"]])
        vt[rows, 1] += dy[e + 1]
    write_obj(stage / path.name, src.pre, vt, src.idx, src.post)
    done.append(path.name)
    for e in range(4):
        d = install / f"a380_engdetach_{e}_a380_engdress_gp.obj"
        if d.is_file():
            o = Obj(d)
            v = o.vt.copy()
            v[:, 1] += dy[e + 1]
            write_obj(stage / d.name, o.pre, v, o.idx, o.post)
            done.append(d.name)
    return done


def settle_winding(stage):
    """Per batch: flip the few cut slivers whose winding disagrees with their vertex normals and drop exact duplicate triangles."""
    done = {}
    for stem in ("020", "021", "022", "023"):
        path = stage / f"a380_exterior_{stem}_gp.obj"
        o = Obj(path)
        post = list(o.post)
        chunks, start, flipped, dropped = [], 0, 0, 0
        for b in o.tris():
            T = o.idx[b["start"]:b["start"] + b["count"]].reshape(-1, 3).copy()
            v = o.vt[T]
            g = np.cross(v[:, 1, :3] - v[:, 0, :3], v[:, 2, :3] - v[:, 0, :3])
            bad = (np.einsum("ij,ij->i", g, v[:, :, 3:6].sum(axis=1)) >= 0) & (np.linalg.norm(g, axis=1) >= 1e-12)
            T[bad] = T[bad][:, [0, 2, 1]]
            q = np.round(o.vt[:, :3] * 2000).astype(np.int64)
            vk = q[:, 0] * 73856093 ^ q[:, 1] * 19349663 ^ q[:, 2] * 83492791
            _, first = np.unique(np.sort(vk[T], axis=1), axis=0, return_index=True)
            keep = np.sort(first)
            flipped += int(bad.sum())
            dropped += len(T) - len(keep)
            T = T[keep]
            chunks.append(T.ravel())
            line = post[b["line"]]
            post[b["line"]] = line[:len(line) - len(line.lstrip())] + f"TRIS {start} {len(T) * 3}"
            start += len(T) * 3
        write_obj(path, o.pre, o.vt, np.concatenate(chunks), post)
        done[stem] = (flipped, dropped)
    return done


def changed_files(install, stage):
    """Files under the stage root (the folder holding objects/, A380.acf and liveries/) that differ from, or are missing in, the installed aircraft folder."""
    root_i, root_s = install.parent, stage.parent
    out = []
    for p in sorted(root_s.rglob("*")):
        if not p.is_file() or p.relative_to(root_s).parts[0] in ("scratch", "nacelle_dump") or p.name == "changed_files.txt":
            continue
        rel = p.relative_to(root_s)
        q = root_i / rel
        if not q.is_file():
            out.append((rel.as_posix(), "new"))
        elif p.stat().st_size != q.stat().st_size or p.read_bytes() != q.read_bytes():
            out.append((rel.as_posix(), "changed"))
    return out


def main(install, stage, work=None, skip_core=False, skip_collision=False, templates=None, dressing=None, pylon_base=None, fairing=True, texdetail=False, panelfix=True, parts=True, fandetail=True, pylondetail=True):
    install, stage = Path(install), Path(stage)
    work = Path(work) if work else stage.parent / "scratch"
    templates = Path(templates) if templates else install
    print("synced", sync(install, stage), "files from the install")
    model.main(templates, stage, pylon=templates == install, panel_fix=panelfix, parts=parts, fan_detail=fandetail)
    result = run_tools(stage, install, work)
    print("brought back", bring_back(work, stage, install, result), "GP files")
    print("winding settled", settle_winding(stage), settle_winding(stage))
    import gp7200_pylon as pylon
    if pylon_base:
        for p in Path(pylon_base).glob("*.obj"):
            shutil.copy2(p, stage / p.name)
    report = pylon.main(stage)
    (work / "pylon_report.json").write_text(json.dumps(report, indent=1, default=str), encoding="utf-8")
    print("pylon seated", report["accepted"], report["gap_after"])
    if fairing:
        import gp7200_fairing as fair
        freport = fair.main(stage, panel_detail=pylondetail)
        (work / "fairing_report.json").write_text(json.dumps(freport, indent=1, default=str), encoding="utf-8")
        print("fairing", {e: v["triangles"] for e, v in freport.items()})
    if dressing:
        import gp7200_dressing as dress
        print("engdress", dress.main(Path(dressing), install, stage))
    else:
        print("engdress", reseat_engdress(install, stage))
    if not skip_core:
        import gp7200_core as core
        core.main(install, stage)
    if not skip_collision:
        import collision_data
        acf = stage.parent / "A380.acf"
        shutil.copy2(install.parent / "A380.acf", acf)
        collision_data.main(stage)
    if texdetail:
        import gp7200_texdetail as td
        tdr = td.main(stage, install, stage.parent / "nacelle_dump" / "nacelle_tags.npz")
        (work / "texdetail_report.json").write_text(json.dumps(tdr, indent=1, default=str), encoding="utf-8")
        print("texdetail", {k: (v if k == "retargeted" else v["texels_changed"]) for k, v in tdr.items()})
    report = changed_files(install, stage)
    (stage.parent / "changed_files.txt").write_text("\n".join(f"{n}\t{k}" for n, k in report), encoding="utf-8")
    print(len(report), "files differ from the install")


if __name__ == "__main__":
    args = sys.argv[1:]
    work = args[args.index("--work") + 1] if "--work" in args else None
    templates = args[args.index("--templates") + 1] if "--templates" in args else None
    dressing = args[args.index("--dressing") + 1] if "--dressing" in args else None
    pylon_base = args[args.index("--pylon-base") + 1] if "--pylon-base" in args else None
    main(args[0], args[1], work, "--skip-core" in args, "--skip-collision" in args, templates, dressing, pylon_base, "--no-fairing" not in args, "--texdetail" in args, "--no-panelfix" not in args, "--no-parts" not in args, "--no-fan" not in args, "--no-pylondetail" not in args)
