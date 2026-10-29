"""usage: build.py <plugin repo> <models dir> <out dir> [--voices northern,joe,cori] [--only-samples]

First-officer voice set with Kokoro-82M (Apache-2.0) through kokoro-onnx (MIT).
Writes <out>/<voice>/fo_<key>.wav (22.05 kHz mono 16-bit), <out>/manifest.json,
<out>/samples/<voice>/NN_<key>.wav.
"""

import json
import os
import re
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

VOICES = {
    "northern": {"kokoro": "bm_george", "lang": "en-gb", "speed": 1.08, "desc": "British English male"},
    "joe": {"kokoro": "am_michael", "lang": "en-us", "speed": 1.08, "desc": "American English male"},
    "cori": {"kokoro": "bf_emma", "lang": "en-gb", "speed": 1.05, "desc": "British English female"},
}

RATE = 22050

AI_FO = {
    "speed": "Speed.",
    "sink_rate": "Sink rate.",
    "bank": "Bank.",
    "pitch": "Pitch.",
    "speed_check": "Speed check.",
    "confirm": "Confirm?",
    "unable": "Unable.",
    "ecam_actions_complete": "Ee cam actions complete.",
    "takeoff_data_set": "Takeoff data set.",
    "approach_data_set": "Approach data set.",
    "go_around_flaps": "Go around, flaps.",
    "ecam_actions": "Ee cam actions.",
    "stop_ecam": "Stop, ee cam.",
    "contact": "Contact.",
}

PA = {
    "pa_seats_for_takeoff": "Cabin crew, please be seated for takeoff.",
    "pa_prepare_for_landing": "Cabin crew, prepare for landing.",
    "pa_seats_for_landing": "Cabin crew, please be seated for landing.",
    "pa_disarm_doors": "Cabin crew, disarm doors and cross check.",
}

DIGITS = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "niner"]

STATIONS = {
    "delivery": "Delivery",
    "ground": "Ground",
    "tower": "Tower",
    "departure": "Departure",
    "approach": "Approach",
    "centre": "Centre",
    "atis": "Eighttiss",
}

WORDS = {
    "decimal": "decimal",
    "fl": "flight level",
    "heading": "heading",
    "altitude": "altitude",
    "thousand": "thousand",
    "hundred": "hundred",
    "feet": "feet",
    "knots": "knots",
    "squawk": "squawk",
    "qnh": "Q N H",
    "climb": "climb",
    "descend": "descend",
    "maintain": "maintain",
    "speed_w": "speed",
    "left": "left",
    "right": "right",
    "direct": "direct",
    "runway": "runway",
    "cleared": "cleared",
    "on": "on",
    "and": "and",
    "point": "point",
}


def blocks():
    out = {}
    for i, w in enumerate(DIGITS):
        out[f"d{i}"] = (w, "mid")
        out[f"d{i}_end"] = (w + ".", "end")
    for k, w in WORDS.items():
        out["w_" + k] = (w, "mid")
    for k, w in STATIONS.items():
        out["stn_" + k] = (w, "mid")
        out["stn_" + k + "_end"] = (w + ".", "end")
    return out


def rust_str(s):
    s = re.sub(r"\\u\{([0-9a-fA-F]+)\}", lambda m: chr(int(m.group(1), 16)), s)
    return s.replace('\\"', '"').replace("\\n", "\n").replace("\\\\", "\\")


def rust_slug(s):
    out = ""
    for c in s:
        if c.isascii() and c.isalnum():
            out += c.lower()
        elif out and not out.endswith("_"):
            out += "_"
    return out.rstrip("_")


def ecam_voice_key(name, label):
    key = "e_" + rust_slug(name)
    if label:
        key += "_" + rust_slug(label)
    return key[:64].rstrip("_")


ABBR = {
    "ENG": "engine", "ENGS": "engines", "SYS": "system", "SYS1": "system one", "F/CTL": "flight controls",
    "CTL": "control", "PRESS": "pressure", "VLV": "valve", "VLVS": "valves", "TK": "tank", "TKS": "tanks",
    "PMP": "pump", "PMPS": "pumps", "DET": "detection", "LO": "low", "HI": "high", "L": "left", "R": "right",
    "Y": "yellow", "G": "green", "L/G": "landing gear", "L/GS": "landing gears", "FWD": "forward",
    "COND": "conditioning", "CAB": "cabin", "HYD": "hydraulic", "VENT": "ventilation", "FLT": "flight",
    "AVAIL": "available", "BRK": "brake", "BRKS": "brakes", "THR": "thrust", "TEMP": "temperature",
    "FL": "flight level", "MAN": "manual", "CAPT": "captain", "F/O": "first officer", "FO": "first officer",
    "OVHT": "overheat", "LDG": "landing", "ALT": "altitude", "GEN": "generator", "GENS": "generators",
    "SURV": "surveillance", "AVNCS": "avionics", "XFR": "transfer", "OUTFLW": "outflow", "DISC": "disconnect",
    "DIFF": "differential", "EMER": "emergency", "DISCH": "discharge", "RQRD": "required", "T.O": "takeoff",
    "P/B": "push button", "P/BS": "push buttons", "PB": "push button", "TRGT": "target", "INR": "inner",
    "OUTR": "outer", "NORM": "normal", "STBY": "standby", "ESS": "essential", "SEL": "selector",
    "BKUP": "backup", "COOLG": "cooling", "SWTG": "switching", "ABNORM": "abnormal", "ABN": "abnormal",
    "ISOL": "isolation", "FT": "feet", "PWR": "power", "ALTN": "alternate", "EXT": "external",
    "ACFT": "aircraft", "CONF": "config", "A-SKID": "anti skid", "PAX": "passengers", "CKPT": "cockpit",
    "GND": "ground", "SPLRS": "spoilers", "PROT": "protection", "GRVTY": "gravity", "LVR": "lever",
    "LVRS": "levers", "GA": "go around", "PROC": "procedure", "XBLEED": "cross bleed",
    "C/B": "circuit breaker", "OVRD": "override", "V/S": "vertical speed", "KT": "knots", "A-ICE": "anti ice",
    "KG": "kilos", "HDG": "heading", "SPD": "speed", "REF": "reference", "TERR": "terrain",
    "OVTHR": "overthrust", "RSVR": "reservoir", "CHAN": "channel", "LWR": "lower", "BTL": "bottle",
    "CRG": "cargo", "DES": "descent", "DIST": "distance", "EXTN": "extension", "XDPR": "transponder",
    "XPDR": "transponder", "REV": "reverser", "AFFCTD": "affected", "STD": "standard",
    "RECONF": "reconfiguration", "ACCUS": "accumulators", "ACCU": "accumulator", "CONSUMPT": "consumption",
    "LAV": "lavatory", "REDUND": "redundancy", "POS": "position", "INOP": "inoperative", "CLB": "climb",
    "RPTG": "reporting", "SW": "switch", "HR": "hour", "BAT": "battery", "BATS": "batteries",
    "OPT": "optimum", "WINDML": "windmill", "XWIND": "crosswind", "S/F": "slats flaps", "MSG": "message",
    "OVHD": "overhead", "INCRSD": "increased", "INCREASD": "increased", "RECOM": "recommended",
    "CPNY": "company", "APPR": "approach", "INSTR": "instrument", "XCHECK": "cross check", "CRZ": "cruise",
    "EQPT": "equipment", "PRED": "predicted", "INV": "inverter", "ACCELMTR": "accelerometer",
    "OVBD": "overboard", "ARPT": "airport", "PNL": "panel", "PNLS": "panels", "CONDS": "conditions",
    "DU": "display unit", "AP": "autopilot", "A/THR": "autothrust", "AP/FD": "autopilot, flight director",
    "DECEND": "descend", "SUCCESFUL": "successful", "NECCESSARY": "necessary", "LIFESTOCK": "livestock",
    "TCAS": "tee cas", "FADEC": "fay deck", "ECAM": "ee cam", "ATIS": "eighttiss", "MORA": "mora",
    "FCOM": "F com", "CIDS": "sids", "RAT": "rat", "N/W": "nose wheel", "WXR": "weather radar",
    "SATCOM": "satcom", "DATALINK": "datalink", "GDOT": "green dot", "LT": "light", "XMIT": "transmit",
    "IFE": "I F E", "IFEC": "I F E C", "WIP": "", "APU": "A P U", "AOA": "A O A", "GYROMETERS": "gyrometers",
    "RA": "R A", "TA": "T A", "VAPP": "V app", "PRIMS": "prims", "FMCS": "F M Cs", "ADRS": "A D Rs",
    "TO": "to", "ONE": "one", "TWO": "two", "THREE": "three", "STEER": "steering", "BLEED": "bleed",
    "ALIGN": "align", "ELEC": "elec", "NAV": "nav", "AIR": "air", "OIL": "oil", "FUEL": "fuel",
    "FIRE": "fire", "SMOKE": "smoke", "AGENT": "agent", "MASTER": "master", "PACK": "pack", "OXY": "oxygen", "IGN": "ignition", "HUD": "hud", "A": "eigh",
}

SHORT_WORDS = {"OFF", "ON", "NOT", "FOR", "ALL", "IN", "NO", "AND", "IF", "WITH", "AS", "OR", "OUT", "UP",
               "AT", "BY", "OF", "THE", "DO", "SET", "USE", "AFT", "MID", "BAY", "FAN", "TIE", "TOO", "LOW",
               "HOT", "DOT", "TRY", "ICE", "KEY", "LAW", "BUG", "PIN", "SAFE", "GO", "MAX", "MIN", "ANY",
               "OIL", "BUS", "RAM", "TAIL", "FLY", "FLOW", "LAND", "TAXI", "SIDE", "SLAT", "SEAT", "TURN",
               "ONLY", "WHEN", "THEN", "KEEP", "RATE", "DUCT", "ZERO", "TIME", "NOSE", "SLOW", "LAST", "FULL",
               "OPEN", "DOWN", "DATA", "MODE", "LOST", "MAIN", "FEED", "DOOR", "LEAK", "LOOP", "BODY", "DECK",
               "FLAP", "TRIM", "IDLE", "PUSH", "PARK", "TEST", "REST", "OVER", "HEAT", "TYPE", "LEFT", "TIP",
               "WING", "STEP", "LINE", "MORE", "MOST", "PIPE", "VOICE", "GEAR", "PAGE", "SEE", "ARMED",
               "CREW", "CELL", "HOLD", "WILL", "SAME", "BOTH", "LAV", "PART", "LOAD", "STALL", "MASK", "SLIDE",
               "LEVER", "LEVEL", "FAIL", "SHUT", "STILL", "AVOID", "CARE", "EXIT", "NEG", "RISK", "FORCE",
               "PRIM", "SEC", "MEMO", "PUMP", "BRAKES", "CABIN", "CARGO", "AUTO"}


def spell(letters):
    return " ".join("eigh" if c == "A" else c for c in letters)


def say_token(tok):
    up = tok.upper()
    if up in ABBR:
        return ABBR[up]
    if up.endswith("S") and up[:-1] in ABBR and len(up) > 2:
        return ABBR[up[:-1]] + "s"
    if re.fullmatch(r"[A-Z]+[0-9]+", up):
        letters, num = re.match(r"([A-Z]+)([0-9]+)", up).groups()
        return say_token(letters) + " " + " ".join(DIGITS[int(d)].replace("niner", "nine") for d in num)
    if "/" in tok or "-" in tok:
        return " ".join(say_token(p) for p in re.split(r"[/-]", tok) if p)
    if up in SHORT_WORDS:
        return up.lower()
    if up.isalpha() and (len(up) <= 3 or not re.search(r"[AEIOU]", up)):
        return spell(up)
    if up.isalpha() and up.endswith("S") and len(up) <= 4 and not re.search(r"[AEIOU]", up[:-1]):
        return spell(up[:-1]) + "s"
    return tok.lower()


def ecam_speech(text):
    t = text.replace("\ufffd", " degrees ").replace("\u00b0", " degrees ")
    t = re.sub(r"\(WIP\)", "", t)
    t = re.sub(r"(?<=\d)\+(?=\d)", " and ", t)
    t = t.replace("&", " and ").replace("+", " and ")
    t = re.sub(r"\s*:\s*", ", ", t)
    t = re.sub(r"[()]", ", ", t)
    words = []
    for tok in re.findall(r"[A-Za-z][A-Za-z0-9/.\-]*[A-Za-z0-9]|[A-Za-z]|\d+(?:\.\d+)?|/|\.\d+|,", t):
        if tok == ",":
            words.append(",")
        elif tok[0].isdigit() or tok.startswith("."):
            words.append(tok)
        elif tok == "/":
            words.append(" ")
        else:
            words.append(say_token(tok))
    out = " ".join(w for w in words if w).replace(" ,", ",")
    out = re.sub(r"(,\s*)+", ", ", out)
    out = re.sub(r"\s+", " ", out).strip(" ,")
    return out[:1].upper() + out[1:] if out else ""


def ecam_tables(procedures_rs):
    src = Path(procedures_rs).read_text(encoding="utf-8")
    titles, items = {}, {}
    for m in re.finditer(r'Procedure \{ id: (\d+),.*?plain_title: "((?:[^"\\]|\\.)*)"', src):
        spoken = ecam_speech(rust_str(m.group(2)))
        if spoken:
            titles["t_" + m.group(1)] = spoken + "."
    for m in re.finditer(r'Item \{ name: "((?:[^"\\]|\\.)*)", kind: ItemKind::(\w+), [^}]*?label: "((?:[^"\\]|\\.)*)"', src):
        name, kind, label = rust_str(m.group(1)), m.group(2), rust_str(m.group(3))
        key = ecam_voice_key(name, label)
        if key == "e" or key in items:
            continue
        spoken = ecam_speech(name)
        if not spoken:
            continue
        if label:
            lab = ecam_speech(label)
            lab = (lab[:1].lower() + lab[1:] if lab[1:2].islower() else lab).replace(" then ", ", then ")
            spoken = f"{spoken}, {lab}" if lab else spoken
        items[key] = spoken + "."
    return titles, items


def table(repo):
    import copilot_voices
    repo = Path(repo)
    rows = {}
    for k, v in copilot_voices.phrases(repo / "app/ui/index.html").items():
        rows[k] = (v, "callout" if k in copilot_voices.CALLOUTS else "checklist", "full")
    for k, v in AI_FO.items():
        rows[k] = (v, "ai_fo", "full")
    for k, v in PA.items():
        rows[k] = (v, "pa", "full")
    titles, items = ecam_tables(repo / "crates/fws_data/src/generated/procedures.rs")
    for k, v in titles.items():
        rows[k] = (v, "ecam_title", "full")
    for k, v in items.items():
        rows[k] = (v, "ecam_item", "full")
    for k, (v, pos) in blocks().items():
        rows[k] = (v, "block", pos)
    return rows


def process(audio, sr, tight):
    from scipy.signal import butter, resample_poly, sosfilt
    x = np.asarray(audio, dtype=np.float64)
    x = resample_poly(x, 147, 160) if sr == 24000 else x
    x = sosfilt(butter(2, 140, "highpass", fs=RATE, output="sos"), x)
    x = sosfilt(butter(2, 6800, "lowpass", fs=RATE, output="sos"), x)
    peak = np.max(np.abs(x)) or 1.0
    env = np.convolve(np.abs(x), np.ones(220) / 220, mode="same")
    on = np.nonzero(env > peak * 10 ** (-40 / 20))[0]
    if len(on):
        pad = int(RATE * (0.012 if tight else 0.04))
        x = x[max(0, on[0] - pad): on[-1] + pad]
    x = x / peak
    x = np.tanh(1.6 * x) / np.tanh(1.6)
    x *= 10 ** (-3 / 20)
    fade = min(len(x) // 4, int(RATE * 0.006))
    if fade:
        x[:fade] *= np.linspace(0, 1, fade)
        x[-fade:] *= np.linspace(1, 0, fade)
    return (np.clip(x, -1, 1) * 32767).astype(np.int16)


def write_wav(path, pcm):
    import soundfile as sf
    sf.write(str(path), pcm, RATE, subtype="PCM_16")


def worker(args):
    voice, models, out_dir, jobs, threads = args
    import onnxruntime as ort
    from kokoro_onnx import Kokoro
    so = ort.SessionOptions()
    so.intra_op_num_threads = threads
    sess = ort.InferenceSession(str(Path(models) / "kokoro-v1.0.onnx"), so, providers=["CPUExecutionProvider"])
    tts = Kokoro.from_session(sess, str(Path(models) / "voices-v1.0.bin"))
    cfg = VOICES[voice]
    target = Path(out_dir) / voice
    target.mkdir(parents=True, exist_ok=True)
    cache, lengths = {}, {}
    for key, text, pos, stale in jobs:
        wav = target / f"fo_{key}.wav"
        if wav.exists() and not stale:
            import soundfile as sf
            lengths[key] = round(sf.info(str(wav)).duration, 3)
            continue
        if (text, pos) not in cache:
            audio, sr = tts.create(text, voice=cfg["kokoro"], speed=cfg["speed"] * (1.06 if pos != "full" else 1.0), lang=cfg["lang"])
            cache[(text, pos)] = process(audio, sr, pos != "full")
        pcm = cache[(text, pos)]
        write_wav(wav, pcm)
        lengths[key] = round(len(pcm) / RATE, 3)
    return voice, lengths


SAMPLES = ["thrust_set", "rotate", "stabilised", "sink_rate", "ecam_actions_complete",
           "t_260800005", "pa_prepare_for_landing", "c_t_o_config"]


def assemble(dirpath, keys, gap=0.03):
    import soundfile as sf
    parts = []
    for k in keys:
        d, _ = sf.read(str(Path(dirpath) / f"fo_{k}.wav"), dtype="int16")
        parts += [d, np.zeros(int(RATE * gap), dtype=np.int16)]
    return np.concatenate(parts[:-1])


def freq_keys(khz, station):
    mhz, frac = divmod(khz, 1000)
    frac_digits = f"{frac:03d}".rstrip("0") or "0"
    digits = [f"d{c}" for c in str(mhz)] + ["w_decimal"] + [f"d{c}" for c in frac_digits]
    digits[-1] += "_end"
    return [f"stn_{station}"] + digits


def samples(out_dir, rows, voices):
    import soundfile as sf
    picks = [k for k in SAMPLES if k in rows]
    picks += [k for k in rows if k.startswith("e_eng_1_master_off")][:1]
    picks = picks[:9]
    readback = freq_keys(121500, "tower")
    written = {}
    for voice in voices:
        sd = Path(out_dir) / "samples" / voice
        sd.mkdir(parents=True, exist_ok=True)
        names = []
        for i, k in enumerate(picks, 1):
            d, _ = sf.read(str(Path(out_dir) / voice / f"fo_{k}.wav"), dtype="int16")
            name = f"{i:02d}_{k}.wav"
            write_wav(sd / name, d)
            names.append(name)
        name = f"{len(picks) + 1:02d}_assembled_readback.wav"
        write_wav(sd / name, assemble(Path(out_dir) / voice, readback))
        names.append(name)
        written[voice] = names
    return picks, readback, written


def main(repo, models, out_dir, *flags):
    voices = list(VOICES)
    for f in flags:
        if f.startswith("--voices="):
            voices = f.split("=", 1)[1].split(",")
    rows = table(repo)
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    shards = 4
    threads = max(1, (os.cpu_count() or 8) // (len(voices) * shards))
    old = {}
    if (out / "manifest.json").exists():
        old = json.loads((out / "manifest.json").read_text(encoding="utf-8")).get("keys", {})
    jobs = [(k, v[0], v[2], k in old and (old[k]["text"], old[k]["position"]) != (v[0], v[2])) for k, v in rows.items()]
    print(sum(j[3] for j in jobs), "changed keys", flush=True)
    tasks = [(v, models, out_dir, jobs[s::shards], threads) for v in voices for s in range(shards)]
    lengths = {v: {} for v in voices}
    if "--only-samples" not in flags:
        with ProcessPoolExecutor(len(tasks)) as ex:
            for voice, l in ex.map(worker, tasks):
                lengths[voice].update(l)
                print(voice, "shard done", len(l), flush=True)
    picks, readback, written = samples(out_dir, rows, voices)
    manifest = {
        "engine": "Kokoro-82M v1.0 (hexgrad, Apache-2.0) via kokoro-onnx (MIT); phonemes by espeak-ng at build time only",
        "format": {"sample_rate": RATE, "channels": 1, "bits": 16},
        "processing": "resample 22.05 kHz, 2nd-order high-pass 140 Hz, low-pass 6.8 kHz, trim at -40 dB, tanh soft limit, peak -3 dBFS",
        "voices": {v: {**VOICES[v], "files": len(lengths[v]) or len(rows)} for v in voices},
        "keys": {k: {"text": t, "kind": kind, "position": pos, "seconds": {v: lengths[v].get(k) for v in voices}}
                 for k, (t, kind, pos) in rows.items()},
        "samples": {"keys": picks, "assembled_readback": readback, "files": written},
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=1), encoding="utf-8")
    counts = {}
    for _, kind, _ in rows.values():
        counts[kind] = counts.get(kind, 0) + 1
    print(json.dumps(counts), len(rows), "keys")


if __name__ == "__main__":
    main(*sys.argv[1:])
