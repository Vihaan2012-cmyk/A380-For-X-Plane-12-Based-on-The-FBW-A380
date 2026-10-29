"""usage: copilot_voices.py <app/ui/index.html> <piper.exe> <voices dir> <out dir>"""

import json
import re
import subprocess
import sys
from pathlib import Path

VOICES = {
    "northern": "en_GB-northern_english_male-medium",
    "joe": "en_US-joe-medium",
    "cori": "en_GB-cori-high",
}

CALLOUTS = {
    "thrust_set": "Thrust set.",
    "100kt": "One hundred knots.",
    "rotate": "Rotate.",
    "pos_climb": "Positive climb.",
    "gear_up": "Gear up.",
    "stabilised": "One thousand. Stabilised.",
    "not_stabilised": "One thousand. Not stabilised.",
    "spoilers": "Spoilers.",
    "no_spoilers": "No spoilers.",
    "reverse_green": "Reverse green.",
    "no_reverse": "No reverse.",
    "decel": "Decel.",
    "no_decel": "No decel.",
    "70kt": "Seventy knots.",
    "checklist_complete": "Checklist complete.",
    "checked": "Checked.",
    "1000_to_go": "One thousand to go.",
    "transition_std": "Transition. Standard set.",
    "transition_qnh": "Transition. Q N H set.",
    "taxi_speed": "Check taxi speed.",
    "flaps_0": "Flaps zero.",
    "flaps_1": "Flaps one.",
    "flaps_2": "Flaps two.",
    "flaps_3": "Flaps three.",
    "flaps_full": "Flaps full.",
    "gear_down": "Gear down.",
    "not_set": "Not set.",
}

SPEECH = [
    (r"<<|>>", ""),
    (r"\(BOTH\)", ""),
    (r"_{2,}", ""),
    (r"\bT\.O\b", "takeoff"),
    (r"\bLDG\b", "landing"),
    (r"\bRWY\b", "runway"),
    (r"\bSID\b", "S I D"),
    (r"\bGND SPLRs\b", "ground spoilers"),
    (r"\bFUEL PUMPs\b", "fuel pumps"),
    (r"\bEFBs\b", "E F Bs"),
    (r"\bBARO REF\b", "baro reference"),
    (r"\bFCU ALT\b", "F C U altitude"),
    (r"\bAUTO BRK\b", "auto brake"),
    (r"\bRTO\b", "R T O"),
    (r"\bCONF\b", "config"),
    (r"\bKG\b", "kilos"),
    (r"\bEMER\b", "emergency"),
    (r"\bPACK 1 & 2\b", "packs one and two"),
    (r"&", " and "),
    (r"\s+", " "),
]


def slug(text):
    return re.sub(r"_+", "_", re.sub(r"[^a-z0-9]+", "_", text.lower())).strip("_")[:60]


def speech(text):
    out = text
    for a, b in SPEECH:
        out = re.sub(a, b, out)
    out = out.strip(" ,.")
    return (out[:1].upper() + out[1:].lower()) if out else ""


def phrases(index_html):
    raw = Path(index_html).read_text(encoding="utf-8")
    docs = json.loads(re.search(r"window\.__DOCS__=(\{.*?\});</script><!--efb-docs-->", raw, re.S).group(1))
    out = {k: v for k, v in CALLOUTS.items()}
    for c in docs["normal"]:
        if not c["i"]:
            continue
        title = speech(c["t"])
        if title:
            out["title_" + slug(c["t"])] = f"{title} checklist."
        for name, value, *_ in c["i"]:
            if speech(name):
                out["c_" + slug(name)] = speech(name) + "?"
            if speech(value) and "__" not in value:
                out["r_" + slug(value)] = speech(value) + "."
    return out


def main(index_html, piper, voices_dir, out_dir):
    table = phrases(index_html)
    for voice, model in VOICES.items():
        target = Path(out_dir) / voice
        target.mkdir(parents=True, exist_ok=True)
        for key, text in table.items():
            wav = target / f"fo_{key}.wav"
            if wav.exists():
                continue
            subprocess.run([piper, "--model", str(Path(voices_dir) / f"{model}.onnx"), "--output_file", str(wav), "--sentence_silence", "0.05"],
                           input=text.encode("utf-8"), check=True, capture_output=True)
        print(voice, len(table), "phrases ->", target)
    (Path(out_dir) / "phrases.json").write_text(json.dumps(table, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main(*sys.argv[1:])
