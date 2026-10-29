"""usage: ground_voices.py <piper.exe> <voices dir> <copilot sound dir>

Writes sound/copilot/<fo voice>/fo_gnd_<key>.wav: the ground engineer's
interphone lines, spoken by a different Piper voice than the first officer
and band-limited like a headset interphone."""

import subprocess
import sys
import tempfile
import wave
from pathlib import Path

import numpy as np
from scipy.signal import butter, sosfilt

MODELS = {
    "northern": "en_GB-northern_english_male-medium",
    "joe": "en_US-joe-medium",
    "cori": "en_GB-cori-high",
}
GROUND_FOR = {"northern": "joe", "joe": "northern", "cori": "northern"}

LINES = {
    "go_ahead": "Cockpit, ground. Go ahead.",
    "on_headset": "Cockpit, ground. I'm on the headset.",
    "release_brakes": "Roger. Release the parking brake, please.",
    "push_start": "Brakes released. Commencing pushback.",
    "tug_coming": "Standby, cockpit. The tug is on its way.",
    "tug_wait": "Standby. Connecting the tug.",
    "engines_clear": "Area is clear. You are cleared to start engines.",
    "push_complete": "Pushback complete. Set the parking brake, please.",
    "disconnect": "Parking brake set. Disconnecting. Steering pin removed. Hand signals on your left.",
    "bye": "Disconnected. Have a nice flight.",
    "stop": "Stopping the pushback.",
    "arrival": "Cockpit, ground. Chocks are in place. You can release the parking brake.",
    "chocks_in": "Chocks in place.",
    "chocks_out": "Chocks removed.",
    "gpu_on": "Ground power connected and available.",
    "gpu_off": "Ground power disconnected.",
    "asu_on": "Air start unit connected.",
    "asu_off": "Air start unit removed.",
    "pca_on": "Preconditioned air connected.",
    "pca_off": "Preconditioned air disconnected.",
    "covers_off": "All covers and pins removed. Walkaround complete.",
    "refuelling": "Negative, cockpit. Refuelling is still in progress.",
    "doors_open": "Negative, cockpit. A door is still open.",
    "boarding": "Negative, cockpit. Loading is still in progress.",
    "beacon": "Beacon is on. Clearing the area.",
    "roger": "Roger.",
    "not_now": "Unable, cockpit. Not at this time.",
}


def interphone(samples, rate):
    x = samples.astype(np.float64) / 32768.0
    x = sosfilt(butter(4, [300, 3400], btype="band", fs=rate, output="sos"), x)
    x = np.tanh(2.2 * x / (np.max(np.abs(x)) + 1e-9)) / np.tanh(2.2)
    rng = np.random.default_rng(380)
    hiss = sosfilt(butter(2, [400, 3000], btype="band", fs=rate, output="sos"), rng.standard_normal(len(x))) * 0.012
    pad = int(0.06 * rate)
    x = np.concatenate([np.zeros(pad), x, np.zeros(pad)])
    hiss = np.concatenate([hiss[:pad], hiss, hiss[:pad]])[: len(x)]
    y = (x + hiss) * 0.7
    return (np.clip(y, -1, 1) * 32767).astype(np.int16)


def main(piper, voices_dir, out_dir):
    with tempfile.TemporaryDirectory() as tmp:
        for fo, ground in GROUND_FOR.items():
            target = Path(out_dir) / fo
            target.mkdir(parents=True, exist_ok=True)
            for key, text in LINES.items():
                raw = Path(tmp) / f"{ground}_{key}.wav"
                if not raw.exists():
                    subprocess.run([piper, "--model", str(Path(voices_dir) / f"{MODELS[ground]}.onnx"), "--output_file", str(raw), "--sentence_silence", "0.15"],
                                   input=text.encode("utf-8"), check=True, capture_output=True)
                with wave.open(str(raw), "rb") as w:
                    rate, ch = w.getframerate(), w.getnchannels()
                    pcm = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16)
                if ch > 1:
                    pcm = pcm.reshape(-1, ch).mean(axis=1).astype(np.int16)
                with wave.open(str(target / f"fo_gnd_{key}.wav"), "wb") as w:
                    w.setnchannels(1)
                    w.setsampwidth(2)
                    w.setframerate(rate)
                    w.writeframes(interphone(pcm, rate).tobytes())
            print(fo, "<-", ground, len(LINES), "ground lines ->", target)


if __name__ == "__main__":
    main(*sys.argv[1:])
