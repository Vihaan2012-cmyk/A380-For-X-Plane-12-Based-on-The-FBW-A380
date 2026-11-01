#!/usr/bin/env python3
"""One flight's numbers from X-Plane's Log.txt and the graphics memory log.

    python perf_report.py <Log.txt> [vram log]
"""
import re
import statistics
import sys
from pathlib import Path

PERF = re.compile(
    r"perf: X-Plane ([\d.]+) fps \(frame ([\d.]+) ms, GPU ([\d.]+) ms, worst frame ([\d.]+) ms\)"
    r"(?:, CPU (.*?))?; [\d.]+ plugin frames/s, ([\d.]+) ms per frame in the plugin, "
    r"([\d.]+) MB of screen pixels uploaded per frame \((.*?)\); ms per frame: (.*)$"
)
SECTION = re.compile(r"(.+?) ([\d.]+)$")
SETTLE_SAMPLES = 2
SLOW_LOAD_FPS = 5.0
NOTES = r"our switches|Chromium mode|Chromium is starting|Chromium's start|X-Plane already called|main thread|Chromium settings|Chromium command line|paints a second|paints copied|uploads through|uploads stay|light spills"


def sections(text):
    out = {}
    for part in text.split(", "):
        m = SECTION.match(part.strip())
        if m:
            out[m.group(1)] = float(m.group(2))
    return out


def report(log_path):
    samples = []
    notes = []
    for line in Path(log_path).read_text(errors="replace").splitlines():
        m = PERF.search(line)
        if m:
            samples.append(
                {
                    "fps": float(m.group(1)),
                    "frame": float(m.group(2)),
                    "gpu": float(m.group(3)),
                    "worst": float(m.group(4)),
                    "cpu": m.group(5) or "",
                    "plugin": float(m.group(6)),
                    "mb": float(m.group(7)),
                    "screens": m.group(8),
                    "sections": sections(m.group(9)),
                }
            )
        elif re.search(NOTES, line):
            notes.append(line.split("systems: ", 1)[-1])
    flying = [s for s in samples[SETTLE_SAMPLES:] if s["fps"] >= SLOW_LOAD_FPS]
    print(f"{len(samples)} samples, {len(flying)} after settling")
    if not flying:
        return
    med = lambda key: statistics.median(s[key] for s in flying)
    print(f"fps median {med('fps'):.1f}, lowest {min(s['fps'] for s in flying):.1f}")
    print(f"frame {med('frame'):.1f} ms, GPU {med('gpu'):.1f} ms, plugin {med('plugin'):.2f} ms, worst frame {max(s['worst'] for s in flying):.0f} ms")
    print(f"uploaded {med('mb'):.1f} MB per frame; last split: {flying[-1]['screens']}")
    print(f"CPU: {flying[-1]['cpu']}")
    names = {n for s in flying for n in s["sections"]}
    rows = [(statistics.median(s["sections"].get(n, 0.0) for s in flying), n) for n in names]
    for value, name in sorted(rows, reverse=True):
        if value >= 0.05:
            print(f"  {value:6.2f} ms  {name}")
    for note in notes[-12:]:
        print(note[:1400])


def memory(vram_path):
    peak = {}
    for line in Path(vram_path).read_text(encoding="utf-8-sig", errors="replace").replace("\0", "").splitlines()[1:]:
        parts = line.split("\t")
        if len(parts) >= 4:
            old = peak.get(parts[1], (0, 0))
            peak[parts[1]] = (max(old[0], int(parts[2])), max(old[1], int(parts[3])))
    for name, (dedicated, shared) in sorted(peak.items(), key=lambda kv: -kv[1][0])[:8]:
        print(f"  {name}: dedicated {dedicated} MB, shared {shared} MB (peaks)")


if __name__ == "__main__":
    report(sys.argv[1])
    if len(sys.argv) > 2:
        memory(sys.argv[2])
