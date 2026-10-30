"""usage: gp7200_verify.py <installed objects dir> <stage objects dir> <out dir>

Checks on the staged GP7200 model: triangle counts per object against the install, winding against the stored normals, duplicate
triangles, the pylon-to-nacelle gap, the Trent files and the Trent half of a380_engdetach.json against the install, the skin UVs on the
Emirates A6-EDY, Air France F-HPJD and Etihad A6-API engine textures, and the profile overlay of the built meshes on spec.json."""

import hashlib
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gp7200_geom import SPEC, frames, old_frames, rb, rt  # noqa: E402
from gp7200_morph import describe, engine_triangles  # noqa: E402
from gp7200_obj import Obj, engine_of_batch  # noqa: E402

GP = ["a380_exterior_001", "a380_exterior_020", "a380_exterior_021", "a380_exterior_022", "a380_exterior_023", "a380_exterior_030", "a380_exterior_031",
      *[f"a380_exterior_0{k}" for k in range(34, 42)], "a380_engine_covers", "a380_engcore", "a380_engdress"]
FN, FO = frames(), old_frames()
LIVERIES = ["Emirates A6-EDY", "Air France F-HPJD", "Etihad A6-API", "Qatar Airways A7-APC", "Korean Air HL7621"]


def triangle_counts(install, stage):
    rows = []
    for stem in GP:
        new = stage / f"{stem}_gp.obj"
        old = install / f"{stem}_gp.obj"
        n = len(Obj(new).idx) // 3 if new.is_file() else 0
        o = len(Obj(old).idx) // 3 if old.is_file() else 0
        rows.append((stem + "_gp.obj", o, n, round(n / o, 2) if o else None))
    return rows


def winding(stage):
    out = {}
    for stem in GP:
        p = stage / f"{stem}_gp.obj"
        if not p.is_file():
            continue
        o = Obj(p)
        T = o.idx[: len(o.idx) // 3 * 3].reshape(-1, 3)
        v = o.vt[T]
        g = np.cross(v[:, 1, :3] - v[:, 0, :3], v[:, 2, :3] - v[:, 0, :3])
        n = v[:, :, 3:6].sum(axis=1)
        d = np.einsum("ij,ij->i", g, n)
        area = np.linalg.norm(g, axis=1)
        ok = (d < 0) | (area < 1e-12)
        out[stem] = (float(ok.mean()), int((~ok).sum()))
    return out


def duplicates(stage):
    out = {}
    for stem in GP:
        p = stage / f"{stem}_gp.obj"
        if not p.is_file():
            continue
        o = Obj(p)
        total = 0
        for b in o.tris():
            T = o.idx[b["start"]: b["start"] + b["count"]].reshape(-1, 3)
            P = np.round(o.vt[T][:, :, :3] * 2000).astype(np.int64)
            key = np.sort(P.reshape(len(T), 3, 3).view([("", P.dtype)] * 3).reshape(len(T), 3), axis=1) if False else None
            tri = [tuple(sorted(map(tuple, t))) for t in P]
            total += len(tri) - len(set(tri))
        out[stem] = total
    return out


def pylon_gap(stage):
    """Lowest pylon vertex above the nacelle crown along the engine's vertical plane, per station."""
    pyl = Obj(stage / "a380_exterior_031_gp.obj")
    nac = {"022": Obj(stage / "a380_exterior_022_gp.obj"), "023": Obj(stage / "a380_exterior_023_gp.obj")}
    res = {}
    for e in (1, 2, 3, 4):
        f = FN[e]
        pts = np.concatenate([pyl.batch_arrays(b)[:, :, :3].reshape(-1, 3) for b in pyl.tris() if engine_of_batch(b) == e - 1])
        loc = f.local(pts)
        sk = nac["022" if e < 3 else "023"]
        sp = np.concatenate([sk.batch_arrays(b)[:, :, :3].reshape(-1, 3) for b in sk.tris() if engine_of_batch(b) == e - 1])
        sl = f.local(sp)
        rows = []
        for s in (0.6, 1.2, 1.8, 2.5, 3.2, 3.9, 4.5):
            top = sl[(np.abs(sl[:, 0]) < 0.12) & (np.abs(sl[:, 2] - s) < 0.1) & (sl[:, 1] > 1.0)][:, 1]
            low = loc[(np.abs(loc[:, 0]) < 0.12) & (np.abs(loc[:, 2] - s) < 0.1) & (loc[:, 1] > 1.0)][:, 1]
            if len(top) and len(low):
                rows.append((s, round(float(top.max()), 3), round(float(low.min()), 3), round(float(low.min() - top.max()), 3)))
        res[e] = rows
    return res


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def trent_identical(install, stage):
    changed = []
    for p in sorted(install.iterdir()):
        if not p.is_file():
            continue
        if "_gp" in p.name or "GPCORE" in p.name or p.name in ("a380_engdetach.json", "a380_cowl_rods.obj"):
            continue
        if p.name.startswith(("a380_engdetach_", "a380_cowldetach_")) and "_gp" in p.name:
            continue
        if sha(p) != sha(stage / p.name):
            changed.append(p.name)
    old = json.loads((install / "a380_engdetach.json").read_text(encoding="utf-8"))
    new = json.loads((stage / "a380_engdetach.json").read_text(encoding="utf-8"))
    halves = {k: old[k] == new[k] for k in old if k != "gp"}
    return changed, halves


def texel(img, uv):
    h, w = img.shape[:2]
    x = np.clip((np.mod(uv[:, 0], 1.0) * w).astype(int), 0, w - 1)
    y = np.clip(((1 - np.mod(uv[:, 1], 1.0)) * h).astype(int), 0, h - 1)
    return img[y, x, :3].astype(float)


def livery_check(install, stage):
    out = {}
    base = np.asarray(Image.open(install / "A380_EXTERIOR_ENG_LH_ALBEDO.dds").convert("RGB"))
    pad = np.median(base[:200, 600:700].reshape(-1, 3), axis=0)
    for name in LIVERIES:
        tex = install.parent / "liveries" / name / "objects" / "A380_EXTERIOR_ENG_LH_ALBEDO.dds"
        if not tex.is_file():
            continue
        img = np.asarray(Image.open(tex).convert("RGB"))
        row = {}
        for tag, obj in (("old", install / "a380_exterior_022_gp.obj"), ("new", stage / "a380_exterior_022_gp.obj")):
            o = Obj(obj)
            tris = engine_triangles(o, 2)
            fr = FO[2] if tag == "old" else FN[2]
            d = describe(tris, fr)
            sel = (d["r"] > 1.55) & (d["nr"] > 0.3) & (d["cen"][:, 2] > -0.9) & (d["cen"][:, 2] < 5.0)
            uv = tris[sel][:, :, 6:8].mean(axis=1)
            col = texel(img, uv)
            gold = (col[:, 0] > 140) & (col[:, 2] < 110) & (col[:, 0] - col[:, 2] > 60)
            navy = (col[:, 2] > col[:, 0] + 20) & (col.sum(axis=1) < 330)
            white = col.min(axis=1) > 190
            blue_pad = np.linalg.norm(texel(base, uv) - pad, axis=1) < 25
            ext = np.ptp(tris[sel][:, :, 6:8], axis=1).max(axis=1)
            row[tag] = dict(tris=int(sel.sum()), wide_uv_tris=int((ext > 0.03).sum()), wide_uv_frac=round(float((ext > 0.03).mean()), 4), white=round(float(white.mean()), 3), gold=round(float(gold.mean()), 4), navy=round(float(navy.mean()), 4), on_blank_padding=round(float(blue_pad.mean()), 3), mean_rgb=[round(float(x), 1) for x in col.mean(axis=0)])
        out[name] = row
    return out


def profile_overlay(stage, out):
    e = 2
    f = FN[e]
    fig, axs = plt.subplots(2, 1, figsize=(18, 13))
    objs = [("a380_exterior_022_gp.obj", "#2c6fbb"), ("a380_exterior_020_gp.obj", "#d08020"), ("a380_exterior_030_gp.obj", "#2e9d4f"), ("a380_engcore_gp.obj", "#9c3a8a")]
    sx = np.linspace(-1.24, 5.2, 400)
    for ax, plane in zip(axs, ("vertical", "horizontal")):
        for name, colr in objs:
            o = Obj(stage / name)
            pts = []
            for b in o.tris():
                if engine_of_batch(b) != e - 1:
                    continue
                if any("thrust_rev" in t and t.startswith("ANIM_rotate_begin") for t in b["anim"]):
                    continue
                pts.append(o.batch_arrays(b)[:, :, :3].reshape(-1, 3))
            if not pts:
                continue
            loc = f.local(np.concatenate(pts))
            sel = np.abs(loc[:, 0] if plane == "vertical" else loc[:, 1]) < 0.025
            r = loc[sel, 1] if plane == "vertical" else loc[sel, 0]
            ax.scatter(loc[sel, 2], r, s=1.2, c=colr, label=name)
        if plane == "vertical":
            ax.plot(*np.array(SPEC["outer_nacelle"]["top"]).T, "k", lw=1.2, label="spec top outline")
            b = np.array(SPEC["outer_nacelle"]["bottom"])
            ax.plot(b[:, 0], -b[:, 1], "k", lw=1.2, label="spec bottom outline")
            ce = np.array(SPEC["core"]["casing_outer_envelope"]["rows"])
            ax.plot(ce[:, 0], ce[:, 1], color="#b8860b", lw=1.0, label="spec casing envelope")
            ax.plot(ce[:, 0], -ce[:, 2], color="#b8860b", lw=1.0)
            n = SPEC["core_nozzle"]
            ax.plot(*np.array(n["top"]).T, "k", lw=1.0)
            ax.plot(*np.array(n["bottom"]).T * np.array([[1], [-1]]), "k", lw=1.0)
            pl = np.array(SPEC["plug"]["profile"])
            ax.plot(pl[:, 0], pl[:, 1], "k", lw=1.0)
            ax.plot(pl[:, 0], -pl[:, 1], "k", lw=1.0)
            ax.set_title("Engine 2, vertical meridian (|a| < 25 mm): built meshes (dots) on spec.json outlines (black)")
        else:
            t = np.array(SPEC["outer_nacelle"]["inboard_outboard_estimate"]["rows"])
            ax.plot(t[:, 0], t[:, 1], "k", lw=1.2, label="spec half width (estimate)")
            ax.plot(t[:, 0], -t[:, 1], "k", lw=1.2)
            ax.set_title("Engine 2, horizontal section (|b| < 25 mm)")
        ax.set_aspect("equal")
        ax.grid(True, alpha=0.3)
        ax.set_xlim(-1.5, 7.3)
        ax.legend(loc="lower left", markerscale=6, fontsize=8)
    plt.tight_layout()
    plt.savefig(out / "profile_overlay_e2.png", dpi=70)
    plt.close(fig)
    dev = {}
    o = Obj(stage / "a380_exterior_022_gp.obj")
    pts = np.concatenate([o.batch_arrays(b)[:, :, :3].reshape(-1, 3) for b in o.tris() if engine_of_batch(b) == e - 1 and not any("thrust_rev" in t and t.startswith("ANIM_rotate_begin") for t in b["anim"])])
    loc = f.local(pts)
    band = np.abs(loc[:, 0]) < 0.03
    for name, sign, spec in (("top", 1, rt), ("bottom", -1, rb)):
        errs = []
        for s in np.arange(-0.8, 4.9, 0.05):
            m = band & (np.abs(loc[:, 2] - s) < 0.03) & (np.sign(loc[:, 1]) == sign) & (np.abs(loc[:, 1]) > 1.62)
            if m.any():
                errs.append(float(np.abs(np.abs(loc[m, 1]).max() - spec(s))))
        dev[name] = (round(max(errs), 4), round(float(np.mean(errs)), 4))
    return dev


def lone_show(stage):
    """ANIM_show lines with no full-range ANIM_hide of the same dataref earlier in their own block, over every GP object and falling copy."""
    bad = {}
    for p in sorted(stage.iterdir()):
        if "_gp" not in p.name or p.suffix != ".obj":
            continue
        stack = []
        n = 0
        for l in p.read_text(encoding="utf-8", errors="replace").splitlines():
            t = l.split()
            if not t:
                continue
            if t[0] == "ANIM_begin":
                stack.append([])
            elif t[0] == "ANIM_end":
                stack.pop()
            elif t[0] == "ANIM_hide" and stack and len(t) == 4:
                stack[-1].append((t[3], float(t[1]), float(t[2])))
            elif t[0] == "ANIM_show" and stack and len(t) == 4:
                if not any(r == t[3] and lo <= -1e4 and hi >= 1e4 for r, lo, hi in stack[-1]):
                    n += 1
        if n:
            bad[p.name] = n
    return bad


def dressing_report(install, stage):
    """The GP fan-case dressing against the spec's fan case (s 0.213-2.143) and the closed door's inner skin (gp7200_nacelle.door_inner geometry)."""
    from gp7200_geom import rb as _rb, rt as _rt
    out = {}
    o = Obj(stage / "a380_engdress_gp.obj")
    for b in o.tris():
        e = engine_of_batch(b)
        v = o.batch_arrays(b)[:, :, :3].reshape(-1, 3)
        loc = FN[e + 1].local(v)
        r = np.hypot(loc[:, 0], loc[:, 1])
        th = np.arctan2(loc[:, 0], loc[:, 1])
        s = loc[:, 2]
        rt_, rb_ = _rt(np.clip(s, 0.3, 2.4)), _rb(np.clip(s, 0.3, 2.4))
        c, rs = (rt_ - rb_) / 2, (rt_ + rb_) / 2 - 0.10
        cu = c * np.cos(th)
        lim = cu + np.sqrt(np.maximum(rs * rs - (c * c - cu * cu), 0))
        out[e + 1] = dict(tris=b["count"] // 3, s_min=round(float(s.min()), 3), s_max=round(float(s.max()), 3), r_max=round(float(r.max()), 3), min_gap_to_door_inner_skin=round(float((lim - r).min()), 4), over_skin_vertices=int((r > lim).sum()))
    old = Obj(install / "a380_engdress_gp.obj")
    out["installed_triangles_per_engine"] = len(old.idx) // 12
    return out


def main(install, stage, out):
    install, stage, out = Path(install), Path(stage), Path(out)
    out.mkdir(parents=True, exist_ok=True)
    report = {}
    report["triangles"] = triangle_counts(install, stage)
    report["winding"] = winding(stage)
    report["duplicates"] = duplicates(stage)
    report["pylon_gap"] = pylon_gap(stage)
    report["pylon_gap_installed"] = pylon_gap(install)
    report["trent_changed"], report["json_trent_halves_equal"] = trent_identical(install, stage)
    report["livery"] = livery_check(install, stage)
    report["lone_show"] = lone_show(stage)
    report["dressing"] = dressing_report(install, stage)
    report["profile_deviation_top_bottom_max_mean"] = profile_overlay(stage, out)
    (out / "verify.json").write_text(json.dumps(report, indent=1), encoding="utf-8")
    print(json.dumps(report, indent=1))


if __name__ == "__main__":
    main(*sys.argv[1:4])
