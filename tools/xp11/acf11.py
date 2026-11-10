"""X-Plane 12 acf -> X-Plane 11 acf.

XP12 split XP11's _part array into _body (XP11 parts 56+), _wing (parts 8+), engines (parts 0-7) and gear
struts (parts 85+): slots learned by acf_learn.py from aircraft shipped in both versions. Values XP12
stores differently (trim ratios, hydraulic bitmasks, strut forces, SFC points) are converted back; wing
rib arms and gear springs, which Plane Maker 11 computes, are computed the same way. Numeric keys
XP11 needs that XP12 has no source for come from a donor XP11 acf (Laminar's 747-400 ships with every
X-Plane 11); its names and notes never do.
Validate against the pairs: python -I acf11.py validate <xp11 root>
"""
import math
import re
import sys
from collections import defaultdict
from pathlib import Path

BODY0, WING0, GEAR0, ENGINES = 56, 8, 85, 8
PART_SUBS = {"_part_x", "_part_y", "_part_z", "_part_phi", "_part_psi", "_part_the", "_part_specs_eq", "_part_specs_invis",
             "_part_tex", "_r_dim", "_s_dim", "_top_s1", "_top_s2", "_top_t1", "_top_t2", "_bot_s1", "_bot_s2", "_bot_t1", "_bot_t2",
             "_part_rad", "_part_cd", "_part_area_rule"}
DROP_SUBS = {"_engn_for_body", "_gear_for_body", "_part_specs_rmod"}
RENAMES = {
    "acf/_selective_ties": "acf/_esys_bus_bit_add",
    "acf/_tire_s1": "acf/_wheel_tire_s1",
    "acf/_tire_s2": "acf/_wheel_tire_s2",
    "acf/_tire_t1": "acf/_wheel_tire_t1",
    "acf/_tire_t2": "acf/_wheel_tire_t2",
}
HYD_USERS = ("acf/_hyd_flaps", "acf/_hyd_gear", "acf/_hyd_nw_steer", "acf/_hyd_brakes")
NO_DONOR = ("_cgpt/", "_door/", "acf/_part_descrip/", "_part/", "_wing/", "_gear/", "_engn/", "_prop/", "_obja/", "_lite/", "_panel_inst_3d")
STATIC_DEF_AT_MAX_FT = 2.4
FLAG_XP12_ONLY = 0x2000 | 0x8000


def read_acf(path):
    """(header lines, ordered props dict, trailer lines from PROPERTIES_END)."""
    lines = Path(path).read_text(encoding="latin-1").splitlines()
    props, head, tail = {}, [], []
    stage = 0
    for line in lines:
        if stage == 0:
            head.append(line)
            if line.strip() == "PROPERTIES_BEGIN":
                stage = 1
        elif stage == 1:
            if line.startswith("P "):
                parts = line[2:].split(" ", 1)
                props[parts[0]] = parts[1] if len(parts) > 1 else ""
            elif line.strip() == "PROPERTIES_END":
                stage = 2
                tail.append(line)
        else:
            tail.append(line)
    return head, props, tail


def num(v, default=None):
    try:
        return float(v)
    except (TypeError, ValueError):
        return default


def fmt(x):
    return f"{x:.9f}"


def normalise(key):
    return re.sub(r"/(\d+)(?=/|$)", "/#", key)


def hyd_0123(v):
    """XP12 hydraulic system bitmask (1, 2, 4) -> XP11 0 none, 1 system 1, 2 system 2, 3 both."""
    b = int(num(v, 0))
    return str((1 if b & 1 else 0) | (2 if b & 6 else 0))


def _att(p12, prefix):
    for kind, base in (("_v10_att_body", BODY0), ("_v10_att_wing", WING0), ("_v10_att_gear", GEAR0)):
        v = p12.get(f"{prefix}/{kind}")
        if v is not None and v.strip() not in ("-1", ""):
            return str(base + int(float(v)))
    return "-1"


def crib_arms(out):
    """Rib positions along each wing, as Plane Maker 11 writes them."""
    for k in range(WING0, WING0 + 48):
        els = int(num(out.get(f"_wing/{k}/_els"), 0) or 0)
        semilen = num(out.get(f"_wing/{k}/_semilen_SEG"), 0.0)
        if els <= 0 or not semilen:
            continue
        side = num(out.get(f"_wing/{k}/_is_right_mult"), 1.0) or 1.0
        x0, y0, z0 = (num(out.get(f"_part/{k}/_part_{a}"), 0.0) for a in "xyz")
        dih = math.radians(num(out.get(f"_wing/{k}/_dihed_design"), 0.0))
        swp = math.radians(num(out.get(f"_wing/{k}/_sweep_design"), 0.0))
        step = semilen / els
        for r in range(11):
            on = r <= els
            out[f"_wing/{k}/_crib_x_arm/{r}"] = fmt(x0 + side * r * step * math.cos(dih) * math.cos(swp) if on else 0.0)
            out[f"_wing/{k}/_crib_y_arm/{r}"] = fmt(y0 + r * step * math.sin(dih) * math.cos(swp) if on else 0.0)
            out[f"_wing/{k}/_crib_z_arm/{r}"] = fmt(z0 + r * step * math.sin(swp) if on else 0.0)
        for a in "xyz":
            out[f"_wing/{k}/_crib_{a}_arm/count"] = "11"
        out.setdefault(f"_wing/{k}/_foil_interp_power", "1.0")
        out.setdefault(f"_wing/{k}/_semilen_JND", "0.0")


def gear_springs(p12, out):
    """XP11 strut springs: one spring constant for every strut, each strut's share of the weight from a rigid
    airframe on equal springs, and leg lengths at static compression (XP12 stores them extended)."""
    count = int(num(p12.get("_gear/count"), 0) or 0)
    wheels = [g for g in range(count) if int(num(p12.get(f"_gear/{g}/_gear_type"), 0) or 0) >= 2]
    m_max = num(p12.get("acf/_m_max"), 0.0) or 0.0
    zcg = num(p12.get("acf/_cgZ"), 0.0)
    frac = {g: 0.0 for g in range(count)}
    if wheels:
        z = [num(p12.get(f"_gear/{g}/_gear_z"), 0.0) for g in wheels]
        n, sz, szz = len(z), sum(z), sum(v * v for v in z)
        det = n * szz - sz * sz
        if abs(det) > 1e-9:
            a = (szz - sz * zcg) / det
            b = (n * zcg - sz) / det
            shares = [max(0.0, a + b * v) for v in z]
        else:
            shares = [1.0] * n
        total = sum(shares) or 1.0
        for g, s in zip(wheels, shares):
            frac[g] = s / total
    scon = m_max / STATIC_DEF_AT_MAX_FT if m_max else 0.0
    for g in range(count):
        if int(num(p12.get(f"_gear/{g}/_gear_type"), 0) or 0) == 0:
            continue
        stat = frac[g] * STATIC_DEF_AT_MAX_FT
        leg = num(p12.get(f"_gear/{g}/_leg_len"), 0.0)
        out[f"_gear/{g}/_leg_len"] = fmt(max(leg * 0.5, leg - stat))
        out[f"_gear/{g}/_gear_load_fraction"] = fmt(frac[g])
        out[f"_gear/{g}/_stat_def"] = fmt(stat)
        out[f"_gear/{g}/_scon"] = fmt(scon)
        out[f"_gear/{g}/_gear_def_empty"] = p12.get(f"_gear/{g}/_strut_preload_def", "0.5")
        out[f"_gear/{g}/_gear_def_gross"] = p12.get(f"_gear/{g}/_strut_max_wgt_def", "1.0")
        for a in ("lat", "lon", "axi"):
            out[f"_gear/{g}/_{a}N"] = p12.get(f"_gear/{g}/_{a}E", "0.0")


def scalars(p12, out):
    g = lambda k, d=0.0: num(p12.get(k), d)
    out["acf/_SFC_full_lo_JET"] = fmt(g("acf/_jet_SFC_takeoff"))
    out["acf/_SFC_half_lo_JET"] = fmt(g("acf/_jet_SFC_locrz"))
    out["acf/_SFC_half_hi_JET"] = fmt(g("acf/_jet_SFC_hicrz"))
    out["acf/_SFC_full_hi_JET"] = fmt(g("acf/_jet_SFC_climb"))
    out["acf/_SFC_alt_lo_JET"] = fmt(0.0)
    out["acf/_SFC_alt_hi_JET"] = fmt(35000.0)
    out["acf/_min_n2"] = fmt(g("acf/_min_n2_EGT", 20.0))
    out["acf/_elev_up_trim"] = fmt(g("acf/_ptch_acft_up_trim_rat") * g("acf/_elev1_up"))
    out["acf/_elev_dn_trim"] = fmt(g("acf/_ptch_acft_dn_trim_rat") * g("acf/_elev1_dn"))
    out["acf/_ailn_up_trim"] = fmt(g("acf/_roll_acft_rt_trim_rat") * g("acf/_ailn1_dn"))
    out["acf/_ailn_dn_trim"] = fmt(g("acf/_roll_acft_lf_trim_rat") * g("acf/_ailn1_up"))
    out["acf/_rudd_lf_trim"] = fmt(g("acf/_hdng_acft_lf_trim_rat") * g("acf/_rudd1_lf"))
    out["acf/_rudd_rt_trim"] = fmt(g("acf/_hdng_acft_rt_trim_rat") * g("acf/_rudd1_rt"))
    out["acf/_hyd_ele_pump_0123"] = hyd_0123(p12.get("acf/_hyd_ele_pump_0123/0", "0"))
    out["acf/_beacon_is_strobe"] = "1"
    out["acf/_studio"] = "A380 for X-Plane"
    for k in ("acf/_name", "acf/_descrip"):
        if k in p12:
            out[k] = p12[k].replace("X-Plane 12", "X-Plane 11")
    out["_panel_inst_3d"] = "0"
    out["_part/count"] = "95"
    out["_wing/count"] = "56"
    out["_engn/count"] = str(ENGINES)
    out["_prop/count"] = str(ENGINES)
    out["acf/_part_descrip/count"] = "95"


def convert(p12, donor):
    out = {}
    att_seen = set()
    for k, v in p12.items():
        m = re.match(r"^_body/(\d+)/(.+)$", k)
        if m:
            j, sub = int(m.group(1)), m.group(2)
            if sub == "_descrip":
                out[f"acf/_part_descrip/{BODY0 + j}"] = v
            elif sub not in DROP_SUBS:
                out[f"_part/{BODY0 + j}/{sub}"] = v
            continue
        if k in ("_body/count", "_wing/count", "_blad/count", "_engn/count", "_prop/count") or k.startswith("_blad/"):
            continue
        m = re.match(r"^_wing/(\d+)/(.+)$", k)
        if m:
            i, sub = int(m.group(1)), m.group(2)
            slot = WING0 + i
            if sub in PART_SUBS or sub.startswith("_geo_xyz/") or sub.startswith("_locked/"):
                out[f"_part/{slot}/{sub}"] = v
            elif sub == "_afl_file_1":
                out[f"_wing/{slot}/_afl_file_R0"] = v
                out[f"_wing/{slot}/_afl_file_R1"] = v
            elif sub == "_afl_file_3":
                out[f"_wing/{slot}/_afl_file_T0"] = v
                out[f"_wing/{slot}/_afl_file_T1"] = v
            elif sub == "_afl_file_2" or sub.endswith("_hyd0123") or sub in DROP_SUBS:
                pass
            else:
                out[f"_wing/{slot}/{sub}"] = v
            continue
        m = re.match(r"^_(engn|prop)/(\d+)/(.+)$", k)
        if m:
            e, sub = int(m.group(2)), m.group(3)
            if e >= ENGINES:
                continue
            if m.group(1) == "engn" and sub in ("_part_x", "_part_y", "_part_z", "_part_psi", "_part_the"):
                out[f"_part/{e}/{sub}"] = v
            else:
                out[k] = v
            continue
        m = re.match(r"^(_obja|_lite)/(\d+)/(.+)$", k)
        if m:
            prefix, sub = f"{m.group(1)}/{m.group(2)}", m.group(3)
            if sub in ("_v10_att_body", "_v10_att_wing", "_v10_att_gear"):
                if prefix not in att_seen:
                    att_seen.add(prefix)
                    out[f"{prefix}/_v10_att_part"] = _att(p12, prefix)
            elif sub == "_obj_flags":
                out[k] = str(int(num(v, 0)) & ~FLAG_XP12_ONLY)
            else:
                out[k] = v
            continue
        if k.startswith("acf/_tank_part_en/") and not k.endswith("/count"):
            out[k] = "0"
            continue
        if k in HYD_USERS:
            out[k] = "1" if int(num(v, 0)) else "0"
            continue
        m = re.match(r"^acf/_hyd_engn_pump_0123/(\d+)$", k)
        if m:
            if int(m.group(1)) < ENGINES:
                out[k] = hyd_0123(v)
            continue
        if k.startswith("acf/_hyd_ele_pump_0123/"):
            continue
        base = re.match(r"^(acf/_[A-Za-z0-9_]+)(/.*)?$", k)
        if base and base.group(1) in RENAMES:
            out[RENAMES[base.group(1)] + (base.group(2) or "")] = v
            continue
        m = re.match(r"^(acf/_[A-Za-z0-9_]+)/(\d+)$", k)
        if m and donor.get(f"{m.group(1)}/count") == str(ENGINES) and p12.get(f"{m.group(1)}/count") == "16":
            if int(m.group(2)) < ENGINES:
                out[k] = v
            continue
        if base and donor.get(f"{base.group(1)}/count") == str(ENGINES) and k.endswith("/count") and v == "16":
            out[k] = str(ENGINES)
            continue
        out[k] = v
    scalars(p12, out)
    gear_springs(p12, out)
    crib_arms(out)
    have = {normalise(k) for k in out}
    for k, v in donor.items():
        if k in out or k.startswith(NO_DONOR) or num(v) is None:
            continue
        nk = normalise(k)
        if nk not in have or re.match(r"^acf/_[A-Za-z0-9_]+/\d+$", k):
            out[k] = v
    return out


def write_acf(path, head, props, tail):
    head = [("1100 Version" if h.strip() == "1200 Version" else h) for h in head]
    body = [f"P {k} {v}" for k, v in props.items()]
    Path(path).write_text("\n".join(head + body + tail) + "\n", encoding="latin-1", newline="\n")


def compare(conv, real):
    """normalised key -> [equal, differ, missing] over the XP11 file's keys."""
    stats = defaultdict(lambda: [0, 0, 0])
    for k, v in real.items():
        nk = normalise(k)
        if k not in conv:
            stats[nk][2] += 1
            continue
        a, b = num(conv[k]), num(v)
        same = (a is not None and b is not None and abs(a - b) <= 1e-4 * max(1.0, abs(b))) or conv[k].strip() == v.strip()
        stats[nk][0 if same else 1] += 1
    return stats


def validate(xp11_root):
    sys.path.insert(0, str(Path(__file__).parent))
    from acf_learn import PAIRS
    root = Path(xp11_root) / "Aircraft" / "Laminar Research"
    _, donor, _ = read_acf(root / "Boeing B747-400" / "747-400.acf")
    total = defaultdict(lambda: [0, 0, 0])
    for a12, a11 in PAIRS:
        _, p12, _ = read_acf(root / a12)
        _, p11, _ = read_acf(root / a11)
        for nk, s in compare(convert(p12, donor), p11).items():
            for i in range(3):
                total[nk][i] += s[i]
    eq = sum(s[0] for s in total.values())
    df = sum(s[1] for s in total.values())
    ms = sum(s[2] for s in total.values())
    print(f"keys: {eq} equal, {df} differ, {ms} missing")
    for nk, s in sorted(total.items(), key=lambda x: -(x[1][1] + x[1][2]))[:60]:
        print(f"  {nk}: eq {s[0]} differ {s[1]} missing {s[2]}")


if __name__ == "__main__":
    if len(sys.argv) > 2 and sys.argv[1] == "validate":
        validate(sys.argv[2])
