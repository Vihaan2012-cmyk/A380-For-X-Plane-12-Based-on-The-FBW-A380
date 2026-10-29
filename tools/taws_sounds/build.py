"""usage: build.py <voices dir> <out dir> [--obstacle WAV] [--cut STEM=SECONDS ...]

Builds the four TAWS look-ahead aurals (names as in crates/gpws/src/sound.rs)
from recordings the aircraft already plays: the voice pack in
plugins/fbw_a380_systems/sound/voices. A phrase the pack already holds is
used as recorded; otherwise it is spliced from single words. "Obstacle" is
in none of the packs, so the two obstacle aurals are only built when a
one-word recording is given with --obstacle. Writes 48 kHz mono 16-bit WAVs
and manifest.json listing each output's sources."""

import argparse
import json
import sys
import wave
from pathlib import Path

import numpy as np

RATE = 48000
HOP = RATE // 100
WORD_GAP_S = 0.06
FADE_S = 0.008
FLOOR_DB = 40.0

OUTPUTS = {
    "aural_terrain_ahead": [("phrase", "co_terrahead"), ("splice", ["terrain", "ahead"])],
    "aural_terrain_ahead_pull_up": [("phrase", "co_tapull"), ("splice", ["terrain", "ahead", "pull_up"])],
    "aural_obstacle_ahead": [("splice", ["obstacle", "ahead"])],
    "aural_obstacle_ahead_pull_up": [("splice", ["obstacle", "ahead", "pull_up"])],
}


def read(path: Path) -> np.ndarray:
    with wave.open(str(path)) as w:
        if w.getsampwidth() != 2:
            raise ValueError(f"{path}: only 16-bit PCM is read")
        a = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(np.float64)
        a = a.reshape(-1, w.getnchannels()).mean(axis=1)
        if w.getframerate() != RATE:
            t = np.arange(0, len(a) * RATE / w.getframerate()) * w.getframerate() / RATE
            a = np.interp(t, np.arange(len(a)), a)
    return a


def write(path: Path, a: np.ndarray) -> None:
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(np.clip(np.round(a), -32768, 32767).astype("<i2").tobytes())


def envelope_db(a: np.ndarray) -> np.ndarray:
    n = len(a) // HOP
    frames = a[: n * HOP].reshape(n, HOP)
    return 20 * np.log10(np.sqrt((frames**2).mean(axis=1)) + 1e-9)


def voiced_span(a: np.ndarray) -> tuple[int, int]:
    e = envelope_db(a)
    loud = np.flatnonzero(e >= FLOOR_DB + 15)
    if loud.size == 0:
        return 0, len(a)
    return max(0, loud[0] - 2) * HOP, min(len(e), loud[-1] + 3) * HOP


def trim(a: np.ndarray) -> np.ndarray:
    s, e = voiced_span(a)
    return fade(a[s:e])


def fade(a: np.ndarray) -> np.ndarray:
    n = min(int(FADE_S * RATE), len(a) // 2)
    if n:
        ramp = np.linspace(0, 1, n)
        a = a.copy()
        a[:n] *= ramp
        a[-n:] *= ramp[::-1]
    return a


def word_split(a: np.ndarray, lo: float = 0.3, hi: float = 0.75) -> int:
    s, e = voiced_span(a)
    env = envelope_db(a)
    i0, i1 = (s + int((e - s) * lo)) // HOP, (s + int((e - s) * hi)) // HOP
    smooth = np.convolve(env, np.ones(3) / 3, mode="same")
    return (i0 + int(np.argmin(smooth[i0:i1]))) * HOP


def words(voices: Path, obstacle: Path | None, cuts: dict[str, float]) -> tuple[dict, dict]:
    found, sources = {}, {}

    def load(stem: str):
        p = voices / f"{stem}.wav"
        return read(p) if p.exists() else None

    def cut(stem: str, a: np.ndarray) -> int:
        return int(cuts[stem] * RATE) if stem in cuts else word_split(a)

    terr = load("co_terr")
    if terr is not None:
        found["terrain"] = trim(terr[: cut("co_terr", terr)])
        sources["terrain"] = "voices/co_terr.wav (first word)"
    ta = load("co_terrahead")
    if ta is not None:
        k = cut("co_terrahead", ta)
        found["ahead"] = trim(ta[k:])
        sources["ahead"] = f"voices/co_terrahead.wav (after {k / RATE:.2f} s)"
        if "terrain" not in found:
            found["terrain"] = trim(ta[:k])
            sources["terrain"] = f"voices/co_terrahead.wav (before {k / RATE:.2f} s)"
    pull = load("co_pull")
    if pull is not None:
        found["pull_up"] = trim(pull)
        sources["pull_up"] = "voices/co_pull.wav"
    if obstacle is not None:
        found["obstacle"] = trim(read(obstacle))
        sources["obstacle"] = obstacle.name
    return found, sources


def join(parts: list[np.ndarray]) -> np.ndarray:
    gap = np.zeros(int(WORD_GAP_S * RATE))
    out = []
    for i, p in enumerate(parts):
        if i:
            out.append(gap)
        out.append(p)
    return np.concatenate(out)


def level(a: np.ndarray, peak: float) -> np.ndarray:
    m = np.abs(a).max()
    return a * (peak / m) if m > 0 else a


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("voices", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--obstacle", type=Path)
    ap.add_argument("--cut", action="append", default=[])
    ap.add_argument("--splice-only", action="store_true")
    args = ap.parse_args()
    cuts = {k: float(v) for k, v in (c.split("=", 1) for c in args.cut)}

    w, word_sources = words(args.voices, args.obstacle, cuts)
    peaks = [np.abs(read(p)).max() for p in args.voices.glob("co_*.wav")]
    peak = float(np.median(peaks)) if peaks else 20000.0
    args.out.mkdir(parents=True, exist_ok=True)
    manifest, missing = {}, {}
    for name, recipes in OUTPUTS.items():
        for kind, spec in recipes:
            if kind == "phrase":
                if args.splice_only or not (args.voices / f"{spec}.wav").exists():
                    continue
                audio, src = trim(read(args.voices / f"{spec}.wav")), [f"voices/{spec}.wav (whole phrase)"]
            else:
                lack = [x for x in spec if x not in w]
                if lack:
                    missing[name] = lack
                    continue
                audio, src = join([w[x] for x in spec]), [word_sources[x] for x in spec]
            audio = level(audio, peak)
            write(args.out / f"{name}.wav", audio)
            manifest[name] = {"length_s": round(len(audio) / RATE, 3), "kind": kind, "sources": src}
            missing.pop(name, None)
            break
    (args.out / "manifest.json").write_text(json.dumps({"built": manifest, "missing_words": missing}, indent=2) + "\n", encoding="utf-8")
    for name, m in manifest.items():
        print(f"{name}: {m['length_s']:.2f} s, {m['kind']}: {'; '.join(m['sources'])}")
    for name, lack in missing.items():
        print(f"{name}: NOT BUILT, no recording of {', '.join(lack)}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
