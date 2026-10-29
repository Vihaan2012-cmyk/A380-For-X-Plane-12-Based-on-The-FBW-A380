"""usage: obstacle_word.py <voices dir> <out wav> --piper EXE --model ONNX [--word TEXT] [--tries N]

Speaks one word with a local Piper voice and matches it to the GPWS callout
voice in <voices dir>: median pitch and pitch range (PSOLA, praat-parselmouth),
long-term spectrum (third-octave EQ against the co_* words), voiced RMS
level and crest factor. Piper is not deterministic, so --tries versions are
made and the closest match is kept. The result is a 48 kHz mono 16-bit WAV
for build.py --obstacle.
Needs: pip install praat-parselmouth numpy"""

import argparse
import subprocess
import sys
import tempfile
import wave
from pathlib import Path

import numpy as np
import parselmouth
from parselmouth.praat import call

RATE = 48000
REFERENCE = ["co_terr", "co_terrahead", "co_tooterr", "co_sink", "co_glideslope", "co_pull"]
PITCH_FLOOR, PITCH_CEILING = 60.0, 400.0
EQ_LIMIT_DB = 24.0


def sound(path: Path) -> parselmouth.Sound:
    s = parselmouth.Sound(str(path))
    if s.n_channels > 1:
        s = s.convert_to_mono()
    if s.sampling_frequency != RATE:
        s = s.resample(RATE)
    return s


def pitch_stats(s: parselmouth.Sound) -> tuple[float, float]:
    f0 = s.to_pitch(time_step=0.01, pitch_floor=PITCH_FLOOR, pitch_ceiling=PITCH_CEILING).selected_array["frequency"]
    f0 = f0[f0 > 0]
    st = 12 * np.log2(f0 / np.median(f0))
    return float(np.median(f0)), float(np.percentile(st, 90) - np.percentile(st, 10))


def voiced(a: np.ndarray) -> np.ndarray:
    hop = RATE // 100
    n = len(a) // hop
    fr = a[: n * hop].reshape(n, hop)
    e = 20 * np.log10(np.sqrt((fr**2).mean(axis=1)) + 1e-12)
    return fr[e > e.max() - 30].ravel()


def third_octave(a: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    n = 8192
    win = np.hanning(n)
    frames = [a[i : i + n] * win for i in range(0, len(a) - n, n // 4)] or [np.pad(a, (0, n - len(a))) * win]
    p = np.mean([np.abs(np.fft.rfft(f)) ** 2 for f in frames], axis=0)
    fr = np.fft.rfftfreq(n, 1 / RATE)
    centres = 1000 * 2 ** (np.arange(-13, 14) / 3)
    out = []
    for c in centres:
        m = (fr >= c / 2 ** (1 / 6)) & (fr < c * 2 ** (1 / 6))
        out.append(10 * np.log10(p[m].mean() + 1e-20))
    return centres, np.array(out)


def eq_to(a: np.ndarray, target: tuple[np.ndarray, np.ndarray]) -> np.ndarray:
    centres, want = target
    _, have = third_octave(voiced(a))
    gain = (want - want.max()) - (have - have.max())
    gain = np.clip(gain - np.median(gain[(centres > 200) & (centres < 4000)]), -EQ_LIMIT_DB - 30, EQ_LIMIT_DB)
    n = len(a) + RATE
    spec = np.fft.rfft(a, n)
    fr = np.fft.rfftfreq(n, 1 / RATE)
    g = np.interp(np.log2(np.maximum(fr, 1)), np.log2(centres), gain, left=gain[0], right=gain[-1])
    return np.fft.irfft(spec * 10 ** (g / 20), n)[: len(a)]


def repitch(s: parselmouth.Sound, have_f0: float, want_f0: float, scale: float) -> np.ndarray:
    manip = call(s, "To Manipulation", 0.01, PITCH_FLOOR, PITCH_CEILING)
    tier = call(manip, "Extract pitch tier")
    pts = [(call(tier, "Get time from index", i), call(tier, "Get value at index", i)) for i in range(1, call(tier, "Get number of points") + 1)]
    call(tier, "Remove points between", 0, s.get_total_duration())
    for t, f in pts:
        call(tier, "Add point", t, want_f0 * 2 ** (12 * np.log2(f / have_f0) * scale / 12))
    call([manip, tier], "Replace pitch tier")
    return call(manip, "Get resynthesis (overlap-add)").values[0]


def limit(a: np.ndarray, ceiling: float) -> np.ndarray:
    hold = int(0.003 * RATE)
    env = np.abs(a)
    env = np.max([np.roll(env, k) for k in range(-hold, hold + 1)], axis=0)
    gain = np.minimum(1.0, ceiling / np.maximum(env, 1e-12))
    k = np.hanning(2 * hold + 1)
    gain = np.convolve(gain, k / k.sum(), mode="same")
    return a * np.minimum(gain, ceiling / np.maximum(np.abs(a), 1e-12))


def spectral_distance(a: np.ndarray, target: tuple[np.ndarray, np.ndarray]) -> float:
    centres, want = target
    _, have = third_octave(voiced(a))
    m = (centres >= 100) & (centres <= 6000)
    d = (have - have[m].max()) - (want - want[m].max())
    return float(np.sqrt(((d[m] - d[m].mean()) ** 2).mean()))


def piper(exe: Path, model: Path, text: str, out: Path) -> None:
    subprocess.run([str(exe), "-m", str(model), "-f", str(out), "-q"], input=text.encode(), check=True, stdout=subprocess.DEVNULL)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("voices", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--piper", type=Path, required=True)
    ap.add_argument("--model", type=Path, required=True)
    ap.add_argument("--word", default="Obstacle.")
    ap.add_argument("--tries", type=int, default=8)
    args = ap.parse_args()

    refs = [sound(args.voices / f"{s}.wav") for s in REFERENCE if (args.voices / f"{s}.wav").exists()]
    if not refs:
        print("no co_* reference words found", file=sys.stderr)
        return 1
    ref_f0 = [pitch_stats(r) for r in refs if r.get_total_duration() > 0.2]
    want_f0 = float(np.median([f for f, _ in ref_f0]))
    want_range = float(np.median([r for _, r in ref_f0]))
    ref_audio = np.concatenate([voiced(r.values[0]) for r in refs])
    ref_spec = third_octave(ref_audio)
    ref_rms = float(np.sqrt((ref_audio**2).mean()))
    ref_crest = float(np.median([np.abs(r.values[0]).max() / np.sqrt((voiced(r.values[0]) ** 2).mean()) for r in refs]))

    def process(s: parselmouth.Sound) -> np.ndarray:
        have_f0, have_range = pitch_stats(s)
        scale = want_range / have_range if have_range > 0 else 1.0
        target = want_f0
        for _ in range(3):
            a = repitch(s, have_f0, target, scale)
            got, got_range = pitch_stats(parselmouth.Sound(a, RATE))
            target *= want_f0 / got
            scale *= want_range / got_range if got_range > 0 else 1.0
        a = eq_to(eq_to(a, ref_spec), ref_spec)
        for _ in range(4):
            a *= ref_rms / (np.sqrt((voiced(a) ** 2).mean()) + 1e-12)
            a = limit(a, ref_rms * ref_crest)
        return a * min(1.0, 0.99 / np.abs(a).max())

    best = None
    with tempfile.TemporaryDirectory() as tmp:
        for i in range(args.tries):
            raw = Path(tmp) / f"word{i}.wav"
            piper(args.piper, args.model, args.word, raw)
            a = process(sound(raw))
            f0, rng = pitch_stats(parselmouth.Sound(a, RATE))
            dist = spectral_distance(a, ref_spec)
            score = abs(12 * np.log2(f0 / want_f0)) + 0.5 * abs(rng - want_range) + 0.2 * dist
            if best is None or score < best[0]:
                best = (score, a, f0, rng, dist)
    _, a, out_f0, out_range, dist = best
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(args.out), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(np.clip(np.round(a * 32767), -32768, 32767).astype("<i2").tobytes())
    print(f"reference: f0 {want_f0:.0f} Hz, range {want_range:.1f} st, crest {20 * np.log10(ref_crest):.1f} dB")
    print(f"out: f0 {out_f0:.0f} Hz, range {out_range:.1f} st, third-octave distance {dist:.1f} dB (100 Hz-6 kHz)")
    for r, name in zip(refs, [x for x in REFERENCE if (args.voices / f"{x}.wav").exists()]):
        print(f"  vs {name}: {spectral_distance(a, third_octave(voiced(r.values[0]))):.1f} dB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
