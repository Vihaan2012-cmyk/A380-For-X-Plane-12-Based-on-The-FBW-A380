# Chromium Displays Optimisation Implementation Plan

> **For agentic workers:** use superpowers:executing-plans and work task by task, solo. The user has not allowed subagents on this work. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** the A380's Chromium displays cost X-Plane's main thread under 2 ms a frame and almost no memory on the RTX 4060, with every PFD and ND repainting once per sim frame, and nothing running outside X-Plane.

**Architecture:** the pages keep running in X-Plane's own Chromium 117 through the hand-written CEF binding (`own_cef.rs`). Chromium's compositing leaves the RTX, pixel copying leaves the main thread, uploads go through a mapped buffer, unchanged pixels stop being sent, and the pages stop redrawing what has not changed. Each step is measured in the sim before the next one is chosen.

**Tech Stack:** Rust (plugin, GNU toolchain), CEF 117.1.4 C API, OpenGL through X-Plane's plugin context, Win32 (delay-import hook, `ReadProcessMemory`, thread suspension), Python and PowerShell for tools, Node for DevTools scripts.

**Spec:** this document. The requirements are Global Constraints and Targets below.

## Global Constraints

- Windows first. New Windows-only calls get `#[cfg(windows)]` and a non-Windows path. After each phase run the port check in Task 16.
- Every PFD and ND repaints at the same rate, 1x the sim's frame rate (user's ruling, 1 Nov). Only an outside view may slow them.
- Nothing external runs on this route: no XPHFBW, no systems server (user's ruling, 1 Nov).
- X-Plane is launched only when the user says so for that launch. Never relaunch on your own. Kill X-Plane by PID, only when told. Never kill `chrome.exe` by name.
- Tests must not open windows on the user's screen. Own-browser tests are windowless. Run Chromium headless when a browser is needed for a test.
- The user's motion vector / TAA plugin is the user's: report what it costs, do not change it.
- Keep the app route working (`browserDisplays` off) and keep the native display code, until the user says otherwise.
- User-visible text says "A380 for X-Plane 12", never XPH or XPHFBW. No realism toggles. Settings are for preferences only.
- No explanatory paragraph comments in code. Match the file around the edit.
- Output goes on E:, build folder `E:/A380/fbw-build/target` only. No whole-disk `find`.
- Commit often, as `git -c user.name=vihaan2012-cmyk -c user.email=vihaan2012-cmyk@users.noreply.github.com commit`, with no co-author line. Never push unless asked. Branch `xp-own-api`, repo `E:/A380/fbw-int/plugin`.
- Never discard results: flights go in `E:/A380/fbw-build/perf-runs/<time>-<label>`, and stay.
- Edit source files by bytes or with the Edit tool. Bash heredocs mangle backslashes, and some files are CRLF.

### Named procedures

**BUILD:**
```bash
cd /e/A380/fbw-int/plugin
export CARGO_TARGET_DIR=/e/A380/fbw-build/target TMP='E:\A380\fbw-build\tmp' TEMP='E:\A380\fbw-build\tmp'
cargo +stable-x86_64-pc-windows-gnu build --release --features js --lib
```

**TEST `<filter>`:** the same environment, then:
```bash
cargo +stable-x86_64-pc-windows-gnu test --release --features js --lib <filter>
```

**INSTALL** (only with X-Plane closed; check with `tasklist | grep -i x-plane.exe`):
```bash
PL="/d/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12/plugins/fbw_a380_systems/64"
mkdir -p /e/A380/fbw-backup/<date>-<what> && cp "$PL/win.xpl" /e/A380/fbw-backup/<date>-<what>/win.xpl
cp /e/A380/fbw-build/target/release/fbw_a380_systems.dll "$PL/win.xpl"
cmp /e/A380/fbw-build/target/release/fbw_a380_systems.dll "$PL/win.xpl" && echo installed
```

**FLIGHT `<label>` `<settings>`** (exists after Task 1, only when the user says launch; run in the background so its report arrives when X-Plane closes):
```bash
FBW_XPLANE_ROOT='D:\Steam Games\steamapps\common\X-Plane 12' powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'E:\A380\fbw-int\plugin\tools\browser-screens\flight.ps1' -Label <label> -Set '<NAME=value;NAME=value>'
```

**Reference flying**, so numbers compare. The user does the same two things each time:
- **R1:** runway start, VOBL 09R, engines running, cockpit view, 90 s without touching anything.
- **R2:** the slow part: air start or approach into the same airport, at least 3 minutes, cockpit view.

---

## Where things stand (1 Nov 2026)

### Flights

| When | Route | Frame ms | GPU ms | Plugin ms | Uploaded MB/frame |
|---|---|---|---|---|---|
| 12 Oct, ground | app | 28 | 21 | 5.4 | 8.6 |
| 12 Oct, air | app | 42 | 21 | not logged | not logged |
| 1 Nov 14:00, parked | own browsers | 30-32 | 18-22 | 6.5 (paints not timed yet) | 6-9 |
| 1 Nov 14:03, air | own browsers | 43-46 | 18-22 | 9.4-9.8 (paints not timed yet) | 17-18 |
| 1 Nov 15:46, runway, engines on | own browsers | 37-42 | 20 | about 11 (logged 14-15) | 11-13 |
| 1 Nov 15:46, worst 20 s | own browsers | 64-101 | 35-46 | about 25 (logged 32-40) | 10-11 |

The perf line's plugin total counts the `of the draw callbacks` notes twice, so the logged 14 ms is really about 11. Task 2 fixes that.

Frame minus plugin is X-Plane's own share: about 22-26 ms on the ground (12 Oct: 23) and about 31-34 ms in the air. So:

- On the ground the plugin is where the gap to the old app is.
- In the air X-Plane's own share sets the ceiling. With the plugin at 6 ms that is about 25-27 fps, unless Task 14 finds more.

The plugin's 11 ms on the runway: browser paints 3.7, draw callbacks 3.4 (uploads 2.9 of that), deep 1.2, `xphfbw_host` 0.7, `extra_backend_fbw` 0.6, native shadows about 0.5, about 1 ms of small sections.

The worst moments are memory, not the plugin's own work: uploads jump from 3 to 13 ms and GPU doubles when the card is over-committed.

### Main thread, sampled (2,321 samples, 15:46 flight)

| Where | Share | ms of a 40 ms frame | Note |
|---|---|---|---|
| `X-Plane.exe` | 26.4% | 10.6 | |
| `ntdll.dll` | 26.6% | 10.6 | waits, locks, heap; 10.7% with our plugin on the stack, not yet explained |
| `ucrtbase.dll` | 11.4% | 4.6 | memcpy; 10.2% is our `on_paint` |
| `VkLayer_mv.dll` | 8.7% | 3.5 | the user's motion vector layer |
| our plugin's own code | 7.4% | 3.0 | |
| `nvoglv64.dll` | 7.2% | 2.9 | GL driver |
| SASL | 2.5% | 1.0 | |
| `XPLM_64.dll` | 2.0% | 0.8 | |
| no module | 1.9% | 0.8 | |
| `win32u.dll`, FlyWithLua, `libcef.dll` | 3.2% | 1.3 | |

The "on the stack" flags come from scanning stack words, so stale slots over-count them (Chromium's 23.5% includes almost all plugin time). Task 3 replaces this with real call stacks.

### Graphics memory (15:46 peaks, card 7,956 MB)

| Process | Dedicated MB | Shared MB |
|---|---|---|
| X-Plane | 5,259 | 890 (spilled to system memory) |
| cefclient (Chromium's GPU process) | 1,207 | 127 |
| dwm | 1,055 | 69 |

Earlier on 1 Nov dwm held 6,037 MB with X-Plane closed; restarting it freed that. A restart is worth checking for before any flight that is being compared.

**Why the old app looked better:** `xphfbw.json` has `"displayGpu": "integrated"` and Windows has `GpuPreference=1` for `XPHFBW.exe`. The app's Chromium ran on the Intel UHD 770, so it took nothing from the RTX. X-Plane's `cefclient.exe` has no such preference, so the same work now sits on the RTX. The user's `chromiumSwitches` do not reach X-Plane's Chromium either.

### Offline probe: six 1152x1536 browsers on a test page

| Setup | RTX dedicated MB | Shared MB | ms per 7 MB paint | Page faults per MB | Helper CPU, % of a core | Paints/s of 180 |
|---|---|---|---|---|---|---|
| Today: GPU compositing on the RTX, shared-texture flag on | 351 | 138 | 2.3 | 245 | 323 | 179 |
| Same, flag off | 343 | 82 | 2.3 | 245 | 284 | 180 |
| Flag off, MSAA 0, 512 MB cap | 334 | 76 | 1.9 | 245 | 198 | 180 |
| Flag off, GPU process on the Intel | 0 | 418 (on the Intel) | 2.3 | 245 | 297 | 170 |
| Flag off, `disable-gpu-compositing` | 8 | 1 | 1.1-1.6 | 0 | 160-370 | 154-180 |

Run to run, ms per paint moves by about 25%. What this shows:
- GPU compositing maps a fresh buffer for every paint (one address across all six browsers, 245 page faults per MB, 4 GB/s). Plain cached memory copies at 22 GB/s here.
- Software compositing keeps one buffer per browser, stays mapped, and takes no faults. That is what makes a copy thread possible.
- The test page is far simpler than the real pages. Older notes record CPU raster holding 18 views to 1-5 Hz, in a session where the GPU process had crashed. Only the sim can settle this, which is what Task 5 is for.

Frames from the test page are equally sharp in both compositing modes, so the capture path is not what softens the screens.

### Already in the tree, not yet seen working in the sim

- The `cef_initialize` hook that adds our switches (c7b9ff0, 36b1b80). The last flight logged them as not applied. 36b1b80 adds an app when X-Plane passes none, and logs what it saw.
- The terrain probe no longer runs in shadow (c7b9ff0).
- `of the draw callbacks: uploads` and `mipmaps` in the perf line (c7b9ff0).
- The simple main thread sampler (3c626d7). Task 3 replaces it.

## Targets

| Measure | Now | Target |
|---|---|---|
| Chromium on the RTX | 1,207 MB | under 50 MB |
| X-Plane spilled to shared memory (R2) | 890 MB | under 200 MB |
| Paints + uploads on the main thread | 6.6-7 ms | under 1.5 ms |
| Plugin total, R1 | about 11 ms | 6 ms or less |
| R1 sim fps | 24-27 | about 33 |
| R2 sim fps | 16-24 | 25-27, lowest 20-s stretch above 20 |
| PFD and ND paints/s | 14-26, uneven | within 10% of sim fps, all four equal |
| Screens at the seat | "cooked" | the user says they look right |

The fps figures are estimates from the table above, not promises.

## Order and gates

| Phase | Tasks | In-sim checkpoint | Why this order |
|---|---|---|---|
| A. Measure | 1, 2, 3 | none | Every later choice reads these numbers |
| B. Chromium off the RTX | 4, 5 | **C1**: two launches, software and Intel | Biggest single win; removes the stalls; decides whether Task 7 is possible |
| C. Pixel work off the main thread | 6, 7, 8, 9 | **C2**: one launch | 6.6 ms of the 11 |
| D. Paint less | 10, 11 | **C3**: one launch | NDs resend 8 MB a frame parked |
| E. Plugin tick | 12 | **C4**: one launch | Real call stacks say what to fix |
| F. Resolution | 13 | **C5**: one launch, user judges by eye | Independent of the rest |
| G. X-Plane's own share | 14 | **C6**: a few launches, report only | Sets what is possible in the air |
| H. Ports | 15, 16 | none | Keep Linux and Mac building |
| I. Ship | 17 | user's flight | Only when the user approves |

## Already tried: do not repeat

| Idea | Result |
|---|---|
| Shared textures on CEF 117 | `on_accelerated_paint` is never called. In the app's newer CEF: 2.5 GB of card, JFK 16.6 fps against 25.9 without (14 Oct) |
| `force-gpu-mem-available-mb` alone | No effect on the probe (343 to 334 MB). 256 made the MFD lag (14 Oct) |
| `PrefetchVirtualMemory`, `VirtualLock`, 4-thread copy of a mapped paint | 3.0, 2.3, 2.6 ms per paint, all worse than 2.0 |
| Uploading straight from `on_paint` to save a copy | Warm memcpy costs 0.3 ms a screen, so there is little to save; the driver's upload path is the cost |
| Lower paint rates, far-side displays slower | User ruled them out |
| X-Plane browser devices on Windows | About 30 Hz, device pixel ratio 2, not ours to control. Still the Linux and Mac route |
| Native Glass, inline native displays | Killed fps, 5-23 ms per screen; the user went back to Chromium |
| A separate process for screens or systems | User wants none |

## File map

| File | Job |
|---|---|
| `tools/browser-screens/flight.ps1`, `vram_log.ps1`, `perf_report.py` | Create: launch, log memory, report one flight |
| `tools/browser-screens/symbolize.py` | Create: names for the sampler's addresses |
| `tools/browser-screens/paint_probe.sh`, `chromium_vram.ps1`, `paint_page.html` | Create: offline paint cost |
| `tools/browser-screens/paint_audit.mjs` | Create: which page repaints and why |
| `tools/browser-screens/bgra2png.py` | Create: view raw frames |
| `tools/linux/plugin_check.sh` | Create: Mac check and Linux tests |
| `src/perf.rs` | Modify: full section list, helper CPU |
| `src/browser_screens/main_thread_profile.rs` | Modify: real call stacks, off unless asked |
| `src/browser_screens/own_cef.rs` | Modify: Chromium modes, hook logs, paint path, begin frames |
| `src/browser_screens/paint_copy.rs` | Create: compare and copy changed rows |
| `src/browser_screens/paint_thread.rs` | Create: copy thread |
| `src/display/upload_ring.rs` | Create: ring bookkeeping |
| `src/display/gl.rs`, `src/display/mod.rs` | Modify: uploads through the ring, prefilter setting |
| `src/native_mode.rs`, `src/lib.rs` | Modify: skip native work nothing uses |
| `src/page_paint_patches.rs` | Create: patches that stop needless repaints |
| `docs/browser-screens.md` | Modify at ship: it still describes browser devices |

---

## Phase A: Measure

### Task 1: Flight kit

**Files:**
- Create: `tools/browser-screens/vram_log.ps1`, `tools/browser-screens/flight.ps1`, `tools/browser-screens/perf_report.py`

**Interfaces:**
- Produces: the FLIGHT procedure; `E:/A380/fbw-build/perf-runs/<time>-<label>/{Log.txt,vram.txt,report.txt,win.xpl}`.

- [ ] **Step 1: Write `vram_log.ps1`**

```powershell
param([string]$Out = "E:\A380\fbw-build\tmp\vram-flight.txt")
"time`tprocess`tdedicated_MB`tshared_MB" | Out-File -Encoding utf8 $Out
$idle = 0
for ($n = 0; $n -lt 720; $n++) {
  Start-Sleep -Seconds 10
  if (-not (Get-Process X-Plane -ErrorAction SilentlyContinue)) {
    $idle++
    if ($idle -gt 3) { break }
    continue
  }
  $idle = 0
  $t = Get-Date -Format HH:mm:ss
  try {
    $ded = (Get-Counter '\GPU Process Memory(*)\Dedicated Usage' -ErrorAction Stop).CounterSamples
    $sh = (Get-Counter '\GPU Process Memory(*)\Shared Usage' -ErrorAction Stop).CounterSamples
  } catch { continue }
  $rows = @{}
  foreach ($pair in @(@($ded, 0), @($sh, 1))) {
    foreach ($s in $pair[0]) {
      if ($s.CookedValue -lt 1MB) { continue }
      $id = [int][regex]::Match($s.InstanceName, 'pid_(\d+)_').Groups[1].Value
      $name = (Get-Process -Id $id -ErrorAction SilentlyContinue).ProcessName
      if (-not $name) { continue }
      if (-not $rows.ContainsKey($name)) { $rows[$name] = @(0.0, 0.0) }
      $rows[$name][$pair[1]] += $s.CookedValue / 1MB
    }
  }
  foreach ($k in $rows.Keys) {
    if ($rows[$k][0] -gt 20 -or $rows[$k][1] -gt 20) {
      "$t`t$k`t$([math]::Round($rows[$k][0]))`t$([math]::Round($rows[$k][1]))" | Out-File -Append -Encoding utf8 $Out
    }
  }
}
```

- [ ] **Step 2: Write `flight.ps1`**

```powershell
param([string]$Label = "run", [string]$Set = "", [string]$XPlane = $env:FBW_XPLANE_ROOT)
if (-not $XPlane) { "set FBW_XPLANE_ROOT to the X-Plane folder"; exit 2 }
if (Get-Process -Name "X-Plane" -ErrorAction SilentlyContinue) { "X-Plane is already running"; exit 1 }
foreach ($pair in ($Set -split ";" | Where-Object { $_ })) {
  $name, $value = $pair -split "=", 2
  Set-Item -Path "env:$name" -Value $value
}
$dir = "E:\A380\fbw-build\perf-runs\{0:yyyyMMdd-HHmm}-{1}" -f (Get-Date), $Label
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item "$XPlane\Aircraft\A380 for X-Plane 12\plugins\fbw_a380_systems\64\win.xpl" "$dir\win.xpl"
$xp = Start-Process -FilePath "$XPlane\X-Plane.exe" -WorkingDirectory $XPlane -PassThru
"X-Plane started, PID $($xp.Id), results in $dir"
& "$PSScriptRoot\vram_log.ps1" -Out "$dir\vram.txt"
Copy-Item "$XPlane\Log.txt" "$dir\Log.txt"
python "$PSScriptRoot\perf_report.py" "$dir\Log.txt" "$dir\vram.txt" | Tee-Object "$dir\report.txt"
```

- [ ] **Step 3: Write `perf_report.py`**

This version has already been run against a real `Log.txt`.

```python
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
NOTES = r"our switches|Chromium is starting|Chromium's start|X-Plane already called|main thread|Chromium settings|Chromium command line|paints a second"


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
    for note in notes[-8:]:
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
```

- [ ] **Step 4: Check it against the last flight**

Run: `python3 tools/browser-screens/perf_report.py "/d/Steam Games/steamapps/common/X-Plane 12/Log.txt" /e/A380/fbw-build/tmp/vram-flight.txt`
Expected: a section list, and per-process peaks. If X-Plane has been launched since, any `Log.txt` with `perf:` lines will do.

- [ ] **Step 5: Commit**

```bash
git add tools/browser-screens/vram_log.ps1 tools/browser-screens/flight.ps1 tools/browser-screens/perf_report.py
git commit -m "feat(tools): one command for a measured flight, kept under perf-runs"
```

### Task 2: What X-Plane gives Chromium, and a complete perf line

**Files:**
- Modify: `src/perf.rs:71`, `src/perf.rs:151`
- Modify: `src/browser_screens/own_cef.rs` (hook, `Settings` moved out of `test_support`, paint counters, test URL)
- Modify: `src/browser_screens/mod.rs` (probe test)
- Create: `tools/browser-screens/paint_probe.sh`, `tools/browser-screens/chromium_vram.ps1`, `tools/browser-screens/paint_page.html`

**Interfaces:**
- Produces: log lines `Chromium settings: ...`, `Chromium command line: ...`; ignored test `paint_cost_probe`; `paint_probe.sh <label> "<switches>"`.

- [ ] **Step 1: Show everything in the perf line and count the helpers' CPU**

In `src/perf.rs`:

```rust
#[cfg(windows)]
const CPU_PROCESSES: [&str; 3] = ["X-Plane.exe", "XPHFBW.exe", "cefclient.exe"];
```

and in `end_frame`, replacing `totals.iter().take(12)`:

```rust
let top: Vec<String> = totals
    .iter()
    .filter(|(_, d)| d.as_secs_f64() * 1000. / frames >= 0.02)
    .map(|(label, d)| format!("{label} {:.2}", d.as_secs_f64() * 1000. / frames))
    .collect();
```

The report's `sum` adds up every section, so a `perf::note` section counts twice in it, which is why the log said 14 ms for about 11:

```rust
let sum: f64 = totals.iter().filter(|(label, _)| !label.starts_with("of the ")).map(|(_, d)| d.as_secs_f64()).sum::<f64>() * 1000. / frames;
```

- [ ] **Step 2: Log what X-Plane starts Chromium with**

Move `struct Settings` out of `mod test_support` to module level in `own_cef.rs`. Add:

```rust
unsafe fn text(s: &CefString) -> String {
    if s.str_.is_null() {
        return String::new();
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(s.str_, s.length))
}

unsafe fn log_settings(settings: *const c_void) {
    let s = &*(settings as *const Settings);
    if s.size < std::mem::size_of::<Settings>() {
        super::log(&format!("Chromium settings: struct is {} bytes, ours {}", s.size, std::mem::size_of::<Settings>()));
        return;
    }
    super::log(&format!(
        "Chromium settings: multi_threaded={} external_pump={} windowless={} args_disabled={} no_sandbox={} chrome_runtime={} helper={} cache={} js_flags={} debug_port={} background={:#x}",
        s.multi_threaded_message_loop,
        s.external_message_pump,
        s.windowless_rendering_enabled,
        s.command_line_args_disabled,
        s.no_sandbox,
        s.chrome_runtime,
        text(&s.browser_subprocess_path),
        text(&s.cache_path),
        text(&s.javascript_flags),
        s.remote_debugging_port,
        s.background_color,
    ));
}

unsafe fn log_command_line(line: *mut CommandLine) {
    const GET_COMMAND_LINE_STRING: usize = 7;
    let get = std::mem::transmute::<Slot, unsafe extern "system" fn(*mut CommandLine) -> *mut CefString>((*line).before[GET_COMMAND_LINE_STRING]);
    let string = get(line);
    if string.is_null() {
        return;
    }
    super::log(&format!("Chromium command line: {}", text(&*string)));
    let free = GetProcAddress(GetModuleHandleA(c"libcef.dll".as_ptr()), c"cef_string_userfree_utf16_free".as_ptr());
    if !free.is_null() {
        std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut CefString)>(free)(string);
    }
}
```

Call `log_settings(settings)` at the top of `cef_initialize_hook`, after restoring the slot. Call `log_command_line(line)` in `on_command_line`, after `append_switches(line)`.

- [ ] **Step 3: Keep the paint probe as a test**

Add to `own_cef.rs`, with `use std::sync::atomic::AtomicU64` already present:

```rust
#[cfg(test)]
pub mod probe {
    use super::*;
    pub static NS: AtomicU64 = AtomicU64::new(0);
    pub static FAULTS: AtomicU64 = AtomicU64::new(0);
    pub static PAINTS: AtomicU64 = AtomicU64::new(0);
    pub static BYTES: AtomicU64 = AtomicU64::new(0);

    pub fn page_faults() -> u32 {
        extern "system" {
            fn K32GetProcessMemoryInfo(process: *mut c_void, counters: *mut u32, bytes: u32) -> i32;
            fn GetCurrentProcess() -> *mut c_void;
        }
        let mut c = [0u32; 20];
        c[0] = 80;
        unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), c.as_mut_ptr(), 80) };
        c[1]
    }

    pub fn reset() {
        for a in [&NS, &FAULTS, &PAINTS, &BYTES] {
            a.store(0, Ordering::Relaxed);
        }
    }
}
```

In `on_paint`, around the copy:

```rust
#[cfg(test)]
let probe_started = (Instant::now(), probe::page_faults());
// the existing copy
#[cfg(test)]
{
    probe::PAINTS.fetch_add(1, Ordering::Relaxed);
    probe::BYTES.fetch_add(list.iter().map(|r| r.width.max(0) as u64 * r.height.max(0) as u64 * 4).sum(), Ordering::Relaxed);
    probe::NS.fetch_add(probe_started.0.elapsed().as_nanos() as u64, Ordering::Relaxed);
    probe::FAULTS.fetch_add((probe::page_faults() - probe_started.1) as u64, Ordering::Relaxed);
}
```

In `open_all`, build the URL so a test can replace it:

```rust
let url = format!("http://s{i}.localhost:{port}/__b/boot/{}", w.key);
#[cfg(test)]
let url = std::env::var("FBW_BENCH_URL").unwrap_or(url);
let url: Vec<u16> = url.encode_utf16().collect();
```

In `src/browser_screens/mod.rs`, in the tests module:

```rust
#[test]
#[ignore = "offline, with X-Plane's Chromium: FBW_BENCH_CEF, FBW_BENCH_CACHE, FBW_BENCH_URL"]
#[cfg(windows)]
fn paint_cost_probe() {
    use own_cef::probe as p;
    use own_cef::test_support as t;
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k}"));
    t::start_chromium(&env("FBW_BENCH_CEF"), &env("FBW_BENCH_CACHE"));
    let tag = format!("probe_{}", std::process::id());
    let keys = ["SCREEN_DU_PFDL", "SCREEN_DU_PFDR", "SCREEN_DU_NDL", "SCREEN_DU_NDR", "SCREEN_DU_EWD", "SCREEN_DU_SD"];
    let wanted: Vec<own_cef::Wanted> = keys.iter().map(|k| own_cef::Wanted { key: k.to_string(), screen: Some(k.to_string()), width: 768, height: 1024 }).collect();
    let mut blocks = Vec::new();
    for k in keys {
        let (dw, dh) = views::device_size(k, 768, 1024);
        blocks.push(crate::xphfbw_bridge::ScreenBlock::create(&tag, k, dw, dh).unwrap());
    }
    let session = crate::xphfbw_bridge::Session::create(&tag).unwrap();
    let mut browsers = own_cef::open_all(&tag, 1, &wanted, 30).expect("browsers");
    let started = Instant::now();
    let mut marked = false;
    while started.elapsed().as_secs_f64() < 30. {
        browsers.tick(&session);
        let until = Instant::now() + Duration::from_millis(28);
        while Instant::now() < until {
            t::pump();
            std::thread::sleep(Duration::from_millis(1));
        }
        if !marked && started.elapsed().as_secs_f64() > 12. {
            marked = true;
            p::reset();
        }
    }
    let get = |a: &std::sync::atomic::AtomicU64| a.load(Ordering::Relaxed) as f64;
    let (b, f, n, c) = (get(&p::BYTES), get(&p::FAULTS), get(&p::NS), get(&p::PAINTS));
    println!("PROBE paints/s {:.1}; MB per paint {:.2}; ms per paint {:.3}; GB/s {:.2}; faults per MB {:.0}", c / 18., b / c.max(1.) / 1e6, n / c.max(1.) / 1e6, b / n.max(1.), f / (b / 1e6).max(1.));
    let (w, h, px) = t::tga_like_dump(&blocks[0]);
    std::fs::write(format!("E:/A380/fbw-build/tmp/probe-{w}x{h}.bgra"), &px).unwrap();
    browsers.close();
}
```

Also print the hook's lines in tests: after `t::start_chromium` in `own_browsers_in_x_planes_chromium`, add `for line in logs().lock().unwrap().drain(..) { println!("log: {line}"); }`.

- [ ] **Step 4: Write the probe's tools**

`tools/browser-screens/paint_page.html`:

```html
<!doctype html>
<html><head><meta charset="utf-8"><style>
html,body{margin:0;padding:0;width:100%;height:100%;overflow:hidden;background:#000}
svg{position:absolute;left:0;top:0}
text{font:22px monospace;fill:#0f0}
</style></head><body><svg id="s" width="768" height="1024" viewBox="0 0 768 1024"></svg>
<script>
const NS='http://www.w3.org/2000/svg';
const s=document.getElementById('s');
const g=document.createElementNS(NS,'g');s.appendChild(g);
for(let i=0;i<40;i++){for(let j=0;j<10;j++){
 const t=document.createElementNS(NS,'text');t.setAttribute('x',-300+j*90);t.setAttribute('y',-500+i*28);t.textContent=(i*j+100)+'';g.appendChild(t);
}}
const p=document.createElementNS(NS,'path');p.setAttribute('d','M-800 0 L800 0 L800 900 L-800 900 Z');p.setAttribute('fill','#553311');g.insertBefore(p,g.firstChild);
const st=document.createElementNS(NS,'g');s.appendChild(st);st.innerHTML='<rect x="0" y="0" width="420" height="96" fill="#000"/><text x="8" y="26" style="font:bold 20px sans-serif;fill:#0f0">FL 340 HDG 157 SPD 250 ILS</text><text x="8" y="52" style="font:14px sans-serif;fill:#fff">The quick brown fox 0123456789</text><path d="M8 66 H400 M8 76 H400" stroke="#0ff" stroke-width="1"/>';
let a=0;
function f(){a=(a+0.7)%360;g.setAttribute('transform','translate(384 512) rotate('+a.toFixed(1)+')');requestAnimationFrame(f);}
f();
</script></body></html>
```

`tools/browser-screens/chromium_vram.ps1`:

```powershell
$ids = @((Get-Process cefclient -ErrorAction SilentlyContinue).Id)
$ded = (Get-Counter '\GPU Process Memory(*)\Dedicated Usage').CounterSamples
$sh = (Get-Counter '\GPU Process Memory(*)\Shared Usage').CounterSamples
$by = @{}
foreach ($pair in @(@($ded, 0), @($sh, 1))) {
  foreach ($s in $pair[0]) {
    $m = [regex]::Match($s.InstanceName, 'pid_(\d+)_luid_0x[0-9a-f]+_0x([0-9a-f]+)_')
    if (-not $m.Success) { continue }
    if ($ids -notcontains [int]$m.Groups[1].Value) { continue }
    $l = $m.Groups[2].Value
    if (-not $by.ContainsKey($l)) { $by[$l] = @(0.0, 0.0) }
    $by[$l][$pair[1]] += $s.CookedValue / 1MB
  }
}
$rows = foreach ($k in $by.Keys) { "{0}: dedicated {1:N0} MB, shared {2:N0} MB" -f $k.Substring($k.Length - 4), $by[$k][0], $by[$k][1] }
$c1 = (Get-Process cefclient | Measure-Object CPU -Sum).Sum
Start-Sleep -Seconds 2
$c2 = (Get-Process cefclient | Measure-Object CPU -Sum).Sum
"helpers memory by adapter: " + ($rows -join "; ")
"helpers CPU {0:N0}% of a core" -f (($c2 - $c1) / 2 * 100)
```

On this PC adapter `03bd` is the RTX and `1863` the Intel.

`tools/browser-screens/paint_probe.sh`:

```sh
#!/bin/sh
# paint_probe.sh <label> "<chromium switches>"
cd /e/A380/fbw-int/plugin
export CARGO_TARGET_DIR=/e/A380/fbw-build/target TMP='E:\A380\fbw-build\tmp' TEMP='E:\A380\fbw-build\tmp'
export FBW_BROWSER_ROUTE=own FBW_BENCH_CEF="E:/A380/fbw-build/cef117" FBW_BENCH_CACHE="E:\\A380\\fbw-build\\tmp\\probe-cache-$1"
export FBW_BENCH_URL="file:///E:/A380/fbw-int/plugin/tools/browser-screens/paint_page.html" FBW_CHROMIUM_SWITCHES="$2"
rm -rf "/e/A380/fbw-build/tmp/probe-cache-$1"; mkdir -p "/e/A380/fbw-build/tmp/probe-cache-$1"
(sleep 22; powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'E:\A380\fbw-int\plugin\tools\browser-screens\chromium_vram.ps1' > "/e/A380/fbw-build/tmp/probe-vram-$1.txt" 2>&1) &
timeout 120 cargo +stable-x86_64-pc-windows-gnu test --release --features js --lib paint_cost_probe -- --ignored --nocapture > "/e/A380/fbw-build/tmp/probe-$1.txt" 2>&1
wait
echo "== $1 [$2]"
grep -aE "^PROBE|^error|panicked" "/e/A380/fbw-build/tmp/probe-$1.txt" | cut -c1-170
cut -c1-140 "/e/A380/fbw-build/tmp/probe-vram-$1.txt"
```

- [ ] **Step 5: Run it**

Run: `sh tools/browser-screens/paint_probe.sh soft "disable-gpu-compositing"`
Expected: paints/s near 180, faults per MB 0, about 8 MB dedicated.

- [ ] **Step 6: Commit**

```bash
git add src/perf.rs src/browser_screens/own_cef.rs src/browser_screens/mod.rs tools/browser-screens/paint_probe.sh tools/browser-screens/chromium_vram.ps1 tools/browser-screens/paint_page.html
git commit -m "feat(browser-screens): log how X-Plane starts Chromium, keep the paint probe"
```

### Task 3: Real call stacks from the main thread

**Files:**
- Modify: `src/browser_screens/main_thread_profile.rs`
- Create: `tools/browser-screens/symbolize.py`

**Interfaces:**
- Produces: log lines `main thread: ...` and `main thread in our plugin: rva=<hex> leaf=<module> <percent>, ...`, only with `FBW_MAIN_THREAD_PROFILE=1`.
- The plugin keeps its symbols in release builds: `nm` lists 98,000 of them from the installed DLL, so no special build is needed.

The stack is copied while the thread is suspended and unwound after it resumes. The unwinder may take locks the suspended thread could be holding, so it must not run during the suspension.

- [ ] **Step 1: Write the failing test**

```rust
#[inline(never)]
fn spin_leaf(stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        std::hint::spin_loop();
    }
}

#[inline(never)]
fn spin_caller(stop: &AtomicBool) -> u32 {
    spin_leaf(stop);
    std::hint::black_box(7)
}

#[test]
fn a_stack_is_unwound_through_its_callers() {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let spinner = std::thread::spawn(move || {
        tx.send((open_this_thread() as usize, stack_top_here())).unwrap();
        spin_caller(&flag);
    });
    let (handle, top) = rx.recv().unwrap();
    std::thread::sleep(Duration::from_millis(50));
    let mut copy = vec![0u8; STACK_COPY_BYTES + STACK_SLACK_BYTES];
    let mut frames = Vec::with_capacity(MAX_FRAMES);
    let mut found = false;
    for _ in 0..50 {
        unsafe { capture(handle as *mut c_void, top, &mut copy, &mut frames) };
        found |= frames.iter().any(|(_, start)| *start == spin_caller as *const () as usize);
    }
    stop.store(true, Ordering::Relaxed);
    spinner.join().unwrap();
    unsafe { CloseHandle(handle as *mut c_void) };
    assert!(found, "spin_caller is on the stack: {frames:x?}");
}
```

- [ ] **Step 2: Run it to see it fail**

Run TEST `main_thread_profile`. Expected: fails to compile, `capture` not found.

- [ ] **Step 3: Implement the capture**

```rust
const CONTEXT_CONTROL_INTEGER: u32 = 0x0010_0003;
const CONTEXT_NONVOLATILE_AT: [usize; 8] = [0x90, 0xA0, 0xA8, 0xB0, 0xD8, 0xE0, 0xE8, 0xF0];
const STACK_COPY_BYTES: usize = 128 << 10;
const STACK_SLACK_BYTES: usize = 128 << 10;
const MAX_FRAMES: usize = 64;

#[repr(C)]
struct RuntimeFunction {
    begin: u32,
    end: u32,
    unwind: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn RtlLookupFunctionEntry(pc: u64, image_base: *mut u64, history: *mut c_void) -> *const RuntimeFunction;
    fn RtlVirtualUnwind(kind: u32, image_base: u64, pc: u64, entry: *const RuntimeFunction, context: *mut Context, data: *mut *mut c_void, establisher: *mut u64, pointers: *mut c_void) -> *mut c_void;
}

fn word(context: &Context, at: usize) -> usize {
    usize::from_le_bytes(context.0[at..at + 8].try_into().unwrap_or([0; 8]))
}

fn set_word(context: &mut Context, at: usize, value: usize) {
    context.0[at..at + 8].copy_from_slice(&value.to_le_bytes());
}

/// `frames` gets (return address, start of its function), leaf first.
unsafe fn capture(thread: *mut c_void, stack_top: usize, copy: &mut [u8], frames: &mut Vec<(usize, usize)>) {
    frames.clear();
    let mut context = Context([0; 1232]);
    context.0[CONTEXT_FLAGS_AT..CONTEXT_FLAGS_AT + 4].copy_from_slice(&CONTEXT_CONTROL_INTEGER.to_le_bytes());
    if SuspendThread(thread) == u32::MAX {
        return;
    }
    let got = GetThreadContext(thread, &mut context) != 0;
    let rsp = word(&context, CONTEXT_RSP_AT);
    let used = if got && rsp != 0 && rsp < stack_top { (stack_top - rsp).min(STACK_COPY_BYTES) } else { 0 };
    if used > 0 {
        std::ptr::copy_nonoverlapping(rsp as *const u8, copy.as_mut_ptr(), used);
    }
    ResumeThread(thread);
    if used == 0 {
        return;
    }
    let base = copy.as_ptr() as usize;
    let relocate = |context: &mut Context| {
        for at in CONTEXT_NONVOLATILE_AT {
            let v = word(context, at);
            if v >= rsp && v < rsp + used {
                set_word(context, at, v - rsp + base);
            }
        }
    };
    set_word(&mut context, CONTEXT_RSP_AT, base);
    relocate(&mut context);
    while frames.len() < MAX_FRAMES {
        let pc = word(&context, CONTEXT_RIP_AT);
        if pc == 0 {
            break;
        }
        let mut image = 0u64;
        let entry = RtlLookupFunctionEntry(pc as u64, &mut image, std::ptr::null_mut());
        frames.push((pc, if entry.is_null() { pc } else { image as usize + (*entry).begin as usize }));
        let sp = word(&context, CONTEXT_RSP_AT);
        if sp < base || sp + 8 > base + used {
            break;
        }
        if entry.is_null() {
            set_word(&mut context, CONTEXT_RIP_AT, std::ptr::read_unaligned(sp as *const usize));
            set_word(&mut context, CONTEXT_RSP_AT, sp + 8);
        } else {
            let (mut data, mut establisher) = (std::ptr::null_mut(), 0u64);
            RtlVirtualUnwind(0, image, pc as u64, entry, &mut context, &mut data, &mut establisher, std::ptr::null_mut());
        }
        relocate(&mut context);
    }
}
```

- [ ] **Step 4: Replace `sample` and `report` with stack totals**

```rust
#[derive(Default)]
struct Stacks {
    total: u32,
    under_plugin: u32,
    under_chromium: u32,
    leaf: std::collections::HashMap<usize, u32>,
    chromium_leaf: std::collections::HashMap<usize, u32>,
    plugin_functions: std::collections::HashMap<(usize, usize), u32>,
}

impl Stacks {
    /// `frames` from `capture`. A leaf outside every module counts under `list.len()`.
    fn add(&mut self, frames: &[(usize, usize)], list: &[Module], ours: Option<usize>, chromium: Option<usize>) {
        let Some(&(pc, _)) = frames.first() else { return };
        let leaf = find(list, pc).unwrap_or(list.len());
        self.total += 1;
        *self.leaf.entry(leaf).or_default() += 1;
        let in_module = |m: Option<usize>, a: usize| m.is_some_and(|m| a >= list[m].base && a < list[m].end);
        if frames.iter().any(|(pc, _)| in_module(chromium, *pc)) {
            self.under_chromium += 1;
            *self.chromium_leaf.entry(leaf).or_default() += 1;
        }
        if let Some(&(_, start)) = frames.iter().find(|(pc, _)| in_module(ours, *pc)) {
            self.under_plugin += 1;
            let rva = start - ours.map_or(0, |m| list[m].base);
            *self.plugin_functions.entry((rva, leaf)).or_default() += 1;
        }
    }

    fn report(&self, list: &[Module]) -> Vec<String> {
        if self.total == 0 {
            return Vec::new();
        }
        let pct = |n: u32| n as f64 * 100. / self.total as f64;
        let name = |m: usize| list.get(m).map_or("(no module)", |m| m.label.as_str());
        let by_module = |map: &std::collections::HashMap<usize, u32>| {
            let mut rows: Vec<_> = map.iter().collect();
            rows.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
            rows.iter().take(16).map(|(m, n)| format!("{} {:.1}", name(**m), pct(**n))).collect::<Vec<_>>().join(", ")
        };
        let mut functions: Vec<_> = self.plugin_functions.iter().collect();
        functions.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        let functions: Vec<String> = functions.iter().take(40).map(|((rva, leaf), n)| format!("rva={rva:x} leaf={} {:.1}", name(*leaf), pct(**n))).collect();
        vec![
            format!("main thread: {} samples, our plugin in the stack {:.1}%, Chromium {:.1}%; leaf: {}", self.total, pct(self.under_plugin), pct(self.under_chromium), by_module(&self.leaf)),
            format!("main thread under Chromium, by leaf: {}", by_module(&self.chromium_leaf)),
            format!("main thread in our plugin: {}", functions.join(", ")),
        ]
    }
}
```

The worker loop in `start` becomes:

```rust
let list = modules();
let (ours, chromium) = (find(&list, ours()), chromium_of(&list));
let mut stacks = Stacks::default();
let started = Instant::now();
while started.elapsed() < WINDOW && !STOP.load(Ordering::Relaxed) {
    unsafe { capture(thread, stack_top, &mut copy, &mut frames) };
    stacks.add(&frames, &list, ours, chromium);
    unsafe { Sleep(SAMPLE_EVERY_MS) };
}
for line in stacks.report(&list) {
    super::log(&line);
}
```

with `let mut copy = vec![0u8; STACK_COPY_BYTES + STACK_SLACK_BYTES];` and `let mut frames = Vec::with_capacity(MAX_FRAMES);` before the loop. Delete the old `sample`, `report`, `Tally`, and the test that used them, keeping `other_plugins_are_named_by_their_folder`.

At the top of `pub fn start()`:

```rust
static WANTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
if !*WANTED.get_or_init(|| std::env::var("FBW_MAIN_THREAD_PROFILE").is_ok_and(|v| v == "1")) {
    return;
}
```

- [ ] **Step 5: Run the tests**

Run TEST `main_thread_profile`. Expected: all pass, including `a_stack_is_unwound_through_its_callers`.

- [ ] **Step 6: Write `symbolize.py`**

```python
#!/usr/bin/env python3
"""Names for the sampler's addresses: python symbolize.py <Log.txt> <win.xpl>"""
import bisect
import re
import struct
import subprocess
import sys
from pathlib import Path


def image_base(path):
    data = Path(path).read_bytes()[:4096]
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    return struct.unpack_from("<Q", data, pe + 24 + 24)[0]


def symbols(path):
    out = subprocess.run(["nm", "-n", "-C", "--defined-only", path], capture_output=True, text=True, errors="replace").stdout
    rows = []
    for line in out.splitlines():
        parts = line.split(" ", 2)
        if len(parts) == 3 and parts[1] in "tT":
            rows.append((int(parts[0], 16), parts[2]))
    return rows


def main(log, xpl):
    base = image_base(xpl)
    rows = symbols(xpl)
    starts = [r[0] for r in rows]
    for line in Path(log).read_text(errors="replace").splitlines():
        if "main thread in our plugin:" not in line:
            continue
        print("--- window")
        for rva, leaf, share in re.findall(r"rva=([0-9a-f]+) leaf=(.+?) ([\d.]+)(?:,|$)", line):
            i = bisect.bisect_right(starts, base + int(rva, 16)) - 1
            print(f"{float(share):5.1f}%  leaf {leaf:<22} {rows[i][1][:110] if i >= 0 else rva}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
```

- [ ] **Step 7: Commit**

```bash
git add src/browser_screens/main_thread_profile.rs tools/browser-screens/symbolize.py
git commit -m "feat(browser-screens): sample the main thread as whole call stacks, on request"
```

---

## Phase B: Chromium off the RTX

### Task 4: Three Chromium modes

**Files:**
- Modify: `src/browser_screens/own_cef.rs`
- Modify: `src/lib.rs` (`set_gpu_preference` becomes `pub(crate)`)

**Interfaces:**
- Produces: `own_cef::ChromiumMode { Software, Integrated, Card }`, `own_cef::chromium_mode() -> ChromiumMode`, env `FBW_CHROMIUM_MODE=software|integrated|card`. `FBW_CHROMIUM_SWITCHES` still replaces the switches outright.

`Integrated` uses the same Windows per-app preference the old app had. It needs no hook, so it still works if the hook never fires.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod mode_tests {
    use super::*;

    #[test]
    fn each_mode_has_its_switches() {
        assert_eq!(ChromiumMode::parse("software"), Some(ChromiumMode::Software));
        assert_eq!(ChromiumMode::parse("integrated"), Some(ChromiumMode::Integrated));
        assert_eq!(ChromiumMode::parse("card"), Some(ChromiumMode::Card));
        assert_eq!(ChromiumMode::parse("nonsense"), None);
        assert!(ChromiumMode::Software.switches().contains("disable-gpu-compositing"));
        assert!(!ChromiumMode::Integrated.switches().contains("disable-gpu-compositing"));
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run TEST `mode_tests`. Expected: `ChromiumMode` not found.

- [ ] **Step 3: Implement**

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChromiumMode {
    Software,
    Integrated,
    Card,
}

impl ChromiumMode {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "software" => Some(Self::Software),
            "integrated" => Some(Self::Integrated),
            "card" => Some(Self::Card),
            _ => None,
        }
    }

    fn switches(self) -> &'static str {
        match self {
            Self::Software => "disable-gpu-compositing",
            Self::Integrated | Self::Card => "gpu-rasterization-msaa-sample-count=0",
        }
    }
}

pub fn chromium_mode() -> ChromiumMode {
    static MODE: std::sync::OnceLock<ChromiumMode> = std::sync::OnceLock::new();
    *MODE.get_or_init(|| std::env::var("FBW_CHROMIUM_MODE").ok().and_then(|v| ChromiumMode::parse(&v)).unwrap_or(ChromiumMode::Software))
}

fn chromium_switches() -> String {
    std::env::var("FBW_CHROMIUM_SWITCHES").unwrap_or_else(|_| chromium_mode().switches().to_string())
}

fn chromium_helper() -> Option<std::path::PathBuf> {
    Some(crate::xp::system_path()?.join("Resources").join("dlls").join("64").join("cef").join("win").join("cefclient.exe"))
}
```

Delete the `CHROMIUM_SWITCHES` const. At the top of `prepare_chromium_start`, after the `ONCE` check:

```rust
if let Some(helper) = chromium_helper() {
    crate::set_gpu_preference(&helper, chromium_mode() == ChromiumMode::Integrated);
}
super::log(&format!("Chromium mode: {:?}", chromium_mode()));
```

`set_gpu_preference(_, false)` deletes the value, so switching back to another mode leaves nothing behind. In `forget_chromium_start`:

```rust
if chromium_mode() == ChromiumMode::Integrated {
    if let Some(helper) = chromium_helper() {
        crate::set_gpu_preference(&helper, false);
    }
}
```

- [ ] **Step 4: Drop the shared-texture path**

CEF 117 never calls `on_accelerated_paint`, and the flag costs about 55 MB of shared memory (probe table). In `own_cef.rs`:
- `open_all`: `shared_texture_enabled: 0`, and remove the `gpu_share::set_logger` line.
- `RenderVt`: `on_accelerated_paint: Slot`; in `new_view` set it to `0`.
- Delete `fn on_accelerated_paint`, and the `View` fields `out`, `staging`, `gpu_paints`.
- Stats row: `format!("{}:{}", v.key, v.paints / 30)`, and the log text `paints a second, {} open: {}`.
- `test_support::stats` returns `(String, u64)`, and `test_support::reset` loses `gpu_paints`. Update their use in `own_browsers_in_x_planes_chromium`.
- Update the file's doc comment: it no longer mentions shared GPU textures.
- Leave `gpu_share.rs` alone: the app uses it.

- [ ] **Step 5: Run the tests**

Run TEST `browser_screens`. Expected: PASS.

- [ ] **Step 6: Check all three modes offline**

Run:
```bash
sh tools/browser-screens/paint_probe.sh soft "disable-gpu-compositing"
sh tools/browser-screens/paint_probe.sh card "gpu-rasterization-msaa-sample-count=0"
```
Expected: the numbers in the probe table.

- [ ] **Step 7: BUILD, INSTALL with backup name `2026-11-01-chromium-modes`, commit**

```bash
git add src/browser_screens/own_cef.rs src/lib.rs
git commit -m "feat(browser-screens): choose where Chromium composites, software by default"
```

### Task 5: Checkpoint C1, choose the mode in the sim

Needs the user's launch, twice. Do the two launches as one sitting if they agree.

- [ ] **Step 1: Software run**

FLIGHT `software` with `FBW_CHROMIUM_MODE=software;FBW_MAIN_THREAD_PROFILE=1`. The user flies R1 and R2 and closes X-Plane.

- [ ] **Step 2: Read the hook's lines first**

In `report.txt`:

| Log says | Meaning | Do |
|---|---|---|
| `Chromium is starting, ...` and `our switches applied` | Hook works | Go on |
| `X-Plane already called Chromium's start` | X-Plane starts it before our plugin | Step 3 |
| None of them, `not applied` | X-Plane reached Chromium another way | Step 3 |

Also record the `Chromium settings:` and `Chromium command line:` lines. They are the input for Step 6.

- [ ] **Step 3: If the hook never fired**

`Integrated` does not depend on the hook, so run Step 4 regardless. Then try the switch on X-Plane's own command line: FLIGHT with `X-Plane.exe --disable-gpu-compositing` by adding `-ArgumentList "--disable-gpu-compositing"` to `Start-Process` in `flight.ps1`. If `cefclient` memory falls to about 10 MB, CEF reads X-Plane's arguments, and the Steam launch options or a launcher can carry it. If not, Software is out; continue with Integrated.

- [ ] **Step 4: Intel run**

FLIGHT `integrated` with `FBW_CHROMIUM_MODE=integrated;FBW_MAIN_THREAD_PROFILE=1`. Same flying.

- [ ] **Step 5: Decide**

Compare `report.txt` from both runs and the 15:46 flight.

| Check | Software wins if | Otherwise |
|---|---|---|
| PFD and ND paints/s | At least 0.9x sim fps on R1 and R2 | Integrated |
| `cefclient` CPU | R2 fps not more than 1 below Integrated | Integrated |
| The user's eye | Text and lines look right | Integrated |

If Integrated wins: keep Software as the default for PCs with one GPU, and select Integrated when `displayGpu` is `"integrated"`, reading it with `crate::efb_settings::load(&root).get("xphfbw.displayGpu")` as `fps_cap` does. Paints then stay on the main thread at about 2 ms, because Task 7 needs Software, and R2 below becomes worth doing. If neither holds up, use Card with `force-gpu-mem-available-mb=512`, and tell the user the memory target is missed.

Set the winner in `chromium_mode()`. Add the runs' numbers to this document under "Where things stand". Commit:

```bash
git add src/browser_screens/own_cef.rs docs/superpowers/plans/2026-11-01-chromium-displays.md
git commit -m "feat(browser-screens): default Chromium mode from the 1 Nov flights"
```

- [ ] **Step 6: Try to make real pages run offline**

Real pages hang on a V8 stall in `own_browsers_in_x_planes_chromium`, and never hang in X-Plane. The two start Chromium differently, and Step 2's lines now show how. Put X-Plane's values into `test_support::start_chromium`'s `Settings`, and its extra switches into `FBW_CHROMIUM_SWITCHES`, then run `sh /e/A380/fbw-build/tmp/oc_run.sh`.

- Expected if it works: `SCREEN_DU_PFDL` shows paints above 0 in the last 12 s, and later tasks can be measured offline on real pages.
- If it still hangs: stop after two attempts and carry on measuring in the sim. This is a convenience, not a requirement.

---

## Phase C: Pixel work off the main thread

### Task 6: Send only what really changed

**Files:**
- Create: `src/browser_screens/paint_copy.rs`
- Modify: `src/browser_screens/mod.rs` (`#[cfg(any(windows, test))] mod paint_copy;`)
- Modify: `src/browser_screens/own_cef.rs` (`on_paint`)

**Interfaces:**
- Produces: `paint_copy::copy_changed(src: &[u8], dst: &mut [u8], stride: usize, rect: [u32; 4], max_rects: usize, out: &mut Vec<[u32; 4]>)`.

Chromium reports where it drew, not whether the pixels differ. A canvas redrawn with the same picture counts as damage. Comparing here costs about what the copy cost, and every unchanged row saves an upload, and when nothing changed, a mipmap rebuild and a device redraw too.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 16;
    const H: usize = 200;

    fn frame(fill: u8) -> Vec<u8> {
        vec![fill; W * H * 4]
    }

    fn set(frame: &mut [u8], x: usize, y: usize, v: u8) {
        frame[(y * W + x) * 4] = v;
    }

    #[test]
    fn an_identical_rectangle_reports_nothing_and_leaves_the_block_alone() {
        let (src, mut dst) = (frame(5), frame(5));
        let mut out = Vec::new();
        copy_changed(&src, &mut dst, W * 4, [0, 0, W as u32, H as u32], 32, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn one_changed_pixel_reports_one_pixel() {
        let (mut src, mut dst) = (frame(5), frame(5));
        set(&mut src, 3, 7, 9);
        let mut out = Vec::new();
        copy_changed(&src, &mut dst, W * 4, [0, 0, W as u32, H as u32], 32, &mut out);
        assert_eq!(out, vec![[3, 7, 1, 1]]);
        assert_eq!(dst, src);
    }

    #[test]
    fn close_rows_merge_and_far_rows_stay_apart() {
        let (mut src, mut dst) = (frame(5), frame(5));
        set(&mut src, 1, 10, 9);
        set(&mut src, 2, 12, 9);
        set(&mut src, 8, 100, 9);
        let mut out = Vec::new();
        copy_changed(&src, &mut dst, W * 4, [0, 0, W as u32, H as u32], 32, &mut out);
        assert_eq!(out, vec![[1, 10, 2, 3], [8, 100, 1, 1]]);
        assert_eq!(dst, src);
    }

    #[test]
    fn never_more_rectangles_than_allowed() {
        let (mut src, mut dst) = (frame(5), frame(5));
        for y in (0..H).step_by(40) {
            set(&mut src, 0, y, 9);
        }
        let mut out = Vec::new();
        copy_changed(&src, &mut dst, W * 4, [0, 0, W as u32, H as u32], 2, &mut out);
        assert!(out.len() <= 2);
        assert_eq!(dst, src);
        for y in (0..H).step_by(40) {
            assert!(out.iter().any(|r| r[0] == 0 && (y as u32) >= r[1] && (y as u32) < r[1] + r[3]));
        }
    }

    #[test]
    fn only_the_given_rectangle_is_looked_at() {
        let (mut src, mut dst) = (frame(5), frame(5));
        set(&mut src, 3, 7, 9);
        let mut out = Vec::new();
        copy_changed(&src, &mut dst, W * 4, [0, 20, W as u32, 10], 32, &mut out);
        assert!(out.is_empty());
        assert_eq!(dst, frame(5));
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run TEST `paint_copy`. Expected: `copy_changed` not found.

- [ ] **Step 3: Implement**

```rust
//! One paint's pixels into a screen's block, keeping only what really changed.

/// Runs of changed rows closer than this go up as one rectangle.
const MERGE_ROWS: u32 = 16;
const CHUNK: usize = 64;

fn first_diff(a: &[u8], b: &[u8]) -> Option<usize> {
    let mut at = 0;
    for (x, y) in a.chunks(CHUNK).zip(b.chunks(CHUNK)) {
        if x != y {
            return x.iter().zip(y).position(|(p, q)| p != q).map(|i| at + i);
        }
        at += x.len();
    }
    None
}

fn last_diff(a: &[u8], b: &[u8]) -> Option<usize> {
    let mut end = a.len();
    for (x, y) in a.rchunks(CHUNK).zip(b.rchunks(CHUNK)) {
        if x != y {
            return x.iter().zip(y).rposition(|(p, q)| p != q).map(|i| end - x.len() + i);
        }
        end -= x.len();
    }
    None
}

/// `rect` is x, y, w, h in pixels, inside the frame. `out` gets what changed.
pub fn copy_changed(src: &[u8], dst: &mut [u8], stride: usize, rect: [u32; 4], max_rects: usize, out: &mut Vec<[u32; 4]>) {
    let [x, y, w, h] = rect;
    let (from, len) = (x as usize * 4, w as usize * 4);
    let limit = out.len() + max_rects;
    // x0, y0, x1, y1
    let mut span: Option<[u32; 4]> = None;
    for row in y..y + h {
        let at = row as usize * stride + from;
        let (Some(a), Some(b)) = (src.get(at..at + len), dst.get_mut(at..at + len)) else { continue };
        let Some(first) = first_diff(a, b) else { continue };
        let last = last_diff(a, b).unwrap_or(first);
        let (p0, p1) = (first / 4, last / 4 + 1);
        b[p0 * 4..p1 * 4].copy_from_slice(&a[p0 * 4..p1 * 4]);
        let (x0, x1) = (x + p0 as u32, x + p1 as u32);
        span = Some(match span {
            Some(s) if row - s[3] < MERGE_ROWS || out.len() + 1 >= limit => [s[0].min(x0), s[1], s[2].max(x1), row + 1],
            Some(s) => {
                out.push([s[0], s[1], s[2] - s[0], s[3] - s[1]]);
                [x0, row, x1, row + 1]
            }
            None => [x0, row, x1, row + 1],
        });
    }
    if let Some(s) = span {
        if max_rects > 0 {
            out.push([s[0], s[1], s[2] - s[0], s[3] - s[1]]);
        }
    }
}
```

- [ ] **Step 4: Run the tests**

Run TEST `paint_copy`. Expected: PASS.

- [ ] **Step 5: Use it in `on_paint`**

Replace the copy loop:

```rust
let stride = width as usize * 4;
let rects: Vec<[u32; 4]> = list
    .iter()
    .filter_map(|r| {
        let (x0, y0) = (r.x.clamp(0, width), r.y.clamp(0, height));
        let (x1, y1) = ((r.x + r.width).clamp(0, width), (r.y + r.height).clamp(0, height));
        (x1 > x0 && y1 > y0).then_some([x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32])
    })
    .collect();
crate::perf::time("screens (browser paints)", || {
    h.writing.store(1, Ordering::Relaxed);
    let mut published = Vec::with_capacity(8);
    for rect in rects.iter().copied() {
        let left = MAX_DIRTY_RECTS.saturating_sub(published.len());
        super::paint_copy::copy_changed(src, dst, stride, rect, left, &mut published);
    }
    if h.frame.load(Ordering::Relaxed) == 0 || published.len() >= MAX_DIRTY_RECTS {
        published.clear();
        published.push([0, 0, width as u32, height as u32]);
    }
    if !published.is_empty() {
        h.publish(&published);
    }
    h.writing.store(0, Ordering::Relaxed);
});
```

The first paint always publishes the whole frame, because a page that starts out black would otherwise match the zeroed block and never reach the GPU. A full rectangle list also publishes the whole frame, as `publish` would drop the extra rectangles and leave stale pixels.

- [ ] **Step 6: Measure offline**

Run: `sh tools/browser-screens/paint_probe.sh soft "disable-gpu-compositing"`
Expected: ms per paint within 20% of before on the rotating page, which changes almost everything, so this is the worst case.

- [ ] **Step 7: Commit**

```bash
git add src/browser_screens/paint_copy.rs src/browser_screens/mod.rs src/browser_screens/own_cef.rs
git commit -m "perf(browser-screens): upload only pixels that really changed"
```

### Task 7: Copy paints on their own thread

**Precondition:** Task 5 chose Software. With GPU compositing the buffer is unmapped when `on_paint` returns, so this task does not apply; skip to Task 8.

**Files:**
- Create: `src/browser_screens/paint_thread.rs`
- Modify: `src/browser_screens/mod.rs` (`#[cfg(windows)] mod paint_thread;`)
- Modify: `src/browser_screens/own_cef.rs` (`View.block` becomes `View.slot`; `Browsers` owns the thread; `on_paint`)

**Interfaces:**
- Consumes: `paint_copy::copy_changed`.
- Produces: `PaintThread::start(blocks: Vec<ScreenBlock>) -> (PaintThread, Vec<Arc<PaintSlot>>)`, `PaintSlot::fits(&self, width: c_int, height: c_int) -> bool`, `PaintSlot::copy_now(&self, src: &[u8], rects: &[[u32; 4]])`, `PaintSlot::hand_over(&self, buffer: usize, rects: &[[u32; 4]])`, `PaintThread::stop(self)`, `read_failures() -> u64`. Env `FBW_PAINT_THREAD=0` turns it off.

**Why it is safe:** with software compositing `on_paint`'s buffer is one mapping per browser (measured: six browsers, six addresses, no faults). Chromium can replace that mapping when it reallocates, for example on a reload, and nothing tells us. So the thread reads through `ReadProcessMemory`, which fails instead of faulting on an unmapped page. A failed read marks the next paint as whole. A paint arriving during a read can tear one frame at worst: every row the next frame writes is in the next frame's damage, so it is copied again.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = 32;
    const H: u32 = 32;

    fn start(tag: &str) -> (PaintThread, Arc<PaintSlot>, ScreenBlock) {
        let block = ScreenBlock::create(tag, "SCREEN_DU_PFDL", W, H).unwrap();
        let view = ScreenBlock::open(tag, "SCREEN_DU_PFDL", W, H).unwrap();
        let (thread, mut slots) = PaintThread::start(vec![block]);
        (thread, slots.remove(0), view)
    }

    fn wait_for_frame(view: &ScreenBlock, frame: u64) {
        let until = Instant::now() + Duration::from_secs(5);
        while view.header().frame.load(Ordering::Acquire) < frame {
            assert!(Instant::now() < until, "no paint published");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn a_handed_over_paint_reaches_the_block() {
        let (thread, slot, view) = start(&format!("pt_a_{}", std::process::id()));
        let buffer = vec![7u8; (W * H * 4) as usize];
        slot.hand_over(buffer.as_ptr() as usize, &[[0, 0, W, H]]);
        wait_for_frame(&view, 1);
        assert_eq!(view.pixels()[..], buffer[..]);
        thread.stop();
    }

    #[test]
    fn a_repeat_of_the_same_pixels_publishes_nothing() {
        let (thread, slot, view) = start(&format!("pt_b_{}", std::process::id()));
        let buffer = vec![7u8; (W * H * 4) as usize];
        slot.hand_over(buffer.as_ptr() as usize, &[[0, 0, W, H]]);
        wait_for_frame(&view, 1);
        slot.hand_over(buffer.as_ptr() as usize, &[[0, 0, W, H]]);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(view.header().frame.load(Ordering::Acquire), 1);
        thread.stop();
    }

    #[test]
    fn a_buffer_that_is_gone_is_survived_and_the_next_paint_is_whole() {
        let (thread, slot, view) = start(&format!("pt_c_{}", std::process::id()));
        let before = read_failures();
        slot.hand_over(1 << 40, &[[0, 0, W, H]]);
        let until = Instant::now() + Duration::from_secs(5);
        while read_failures() == before {
            assert!(Instant::now() < until, "the bad read was not noticed");
            std::thread::sleep(Duration::from_millis(1));
        }
        let buffer = vec![9u8; (W * H * 4) as usize];
        slot.hand_over(buffer.as_ptr() as usize, &[[0, 0, 1, 1]]);
        wait_for_frame(&view, 1);
        assert_eq!(view.pixels()[..], buffer[..]);
        thread.stop();
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run TEST `paint_thread`. Expected: `PaintThread` not found.

- [ ] **Step 3: Implement**

```rust
//! Paints copied off X-Plane's main thread. See the plan's Task 7 for why reading
//! CEF's buffer here is safe: only with software compositing, and only by
//! `ReadProcessMemory`.

use std::ffi::{c_int, c_void};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use super::paint_copy::copy_changed;
use crate::xphfbw_bridge::{ScreenBlock, MAX_DIRTY_RECTS};

extern "system" {
    fn ReadProcessMemory(process: *mut c_void, base: *const c_void, buffer: *mut c_void, size: usize, read: *mut usize) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
}

static READ_FAILURES: AtomicU64 = AtomicU64::new(0);

pub fn read_failures() -> u64 {
    READ_FAILURES.load(Ordering::Relaxed)
}

#[derive(Default)]
struct Wake {
    due: Mutex<bool>,
    changed: Condvar,
}

impl Wake {
    fn notify(&self) {
        *self.due.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.changed.notify_one();
    }

    fn wait(&self) {
        let mut due = self.due.lock().unwrap_or_else(|e| e.into_inner());
        if !*due {
            due = self.changed.wait_timeout(due, Duration::from_millis(100)).unwrap_or_else(|e| e.into_inner()).0;
        }
        *due = false;
    }
}

#[derive(Default)]
struct Pending {
    buffer: usize,
    rects: Vec<[u32; 4]>,
    whole: bool,
}

pub struct PaintSlot {
    block: ScreenBlock,
    pending: Mutex<Pending>,
    wake: Arc<Wake>,
}

unsafe impl Send for PaintSlot {}
unsafe impl Sync for PaintSlot {}

impl PaintSlot {
    fn size(&self) -> (u32, u32) {
        let h = self.block.header();
        (h.width.load(Ordering::Relaxed), h.height.load(Ordering::Relaxed))
    }

    pub fn fits(&self, width: c_int, height: c_int) -> bool {
        self.size() == (width as u32, height as u32)
    }

    /// `src` is a whole frame.
    pub fn copy_now(&self, src: &[u8], rects: &[[u32; 4]]) {
        let (width, _) = self.size();
        self.copy_rows(src, 0, rects, width as usize * 4);
    }

    pub fn hand_over(&self, buffer: usize, rects: &[[u32; 4]]) {
        let mut p = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        p.buffer = buffer;
        p.rects.extend_from_slice(rects);
        drop(p);
        self.wake.notify();
    }

    /// `rows` holds frame rows from `first_row` on.
    fn copy_rows(&self, rows: &[u8], first_row: u32, rects: &[[u32; 4]], stride: usize) {
        let h = self.block.header();
        let (width, height) = self.size();
        let dst = &mut self.block.pixels()[first_row as usize * stride..];
        let mut published = Vec::with_capacity(8);
        h.writing.store(1, Ordering::Relaxed);
        for [x, y, w, rect_h] in rects.iter().copied() {
            let left = MAX_DIRTY_RECTS.saturating_sub(published.len());
            copy_changed(rows, dst, stride, [x, y - first_row, w, rect_h], left, &mut published);
        }
        published.iter_mut().for_each(|r| r[1] += first_row);
        if h.frame.load(Ordering::Relaxed) == 0 || published.len() >= MAX_DIRTY_RECTS {
            published.clear();
            published.push([0, 0, width, height]);
        }
        if !published.is_empty() {
            h.publish(&published);
        }
        h.writing.store(0, Ordering::Relaxed);
    }

    fn run_pending(&self, scratch: &mut Vec<u8>) {
        let (buffer, mut rects, whole) = {
            let mut p = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            if p.rects.is_empty() {
                return;
            }
            (p.buffer, std::mem::take(&mut p.rects), std::mem::take(&mut p.whole))
        };
        let (width, height) = self.size();
        if whole {
            rects = vec![[0, 0, width, height]];
        }
        let stride = width as usize * 4;
        let y0 = rects.iter().map(|r| r[1]).min().unwrap_or(0);
        let y1 = rects.iter().map(|r| r[1] + r[3]).max().unwrap_or(0).min(height);
        let bytes = (y1 - y0) as usize * stride;
        scratch.resize(bytes, 0);
        let mut read = 0usize;
        let ok = unsafe { ReadProcessMemory(GetCurrentProcess(), (buffer + y0 as usize * stride) as *const c_void, scratch.as_mut_ptr().cast(), bytes, &mut read) } != 0 && read == bytes;
        if !ok {
            READ_FAILURES.fetch_add(1, Ordering::Relaxed);
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).whole = true;
            return;
        }
        self.copy_rows(scratch, y0, &rects, stride);
    }
}

pub struct PaintThread {
    wake: Arc<Wake>,
    stop: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}

impl PaintThread {
    pub fn start(blocks: Vec<ScreenBlock>) -> (Self, Vec<Arc<PaintSlot>>) {
        let wake = Arc::new(Wake::default());
        let stop = Arc::new(AtomicBool::new(false));
        let slots: Vec<Arc<PaintSlot>> = blocks
            .into_iter()
            .map(|block| Arc::new(PaintSlot { block, pending: Mutex::new(Pending::default()), wake: wake.clone() }))
            .collect();
        let worker = {
            let (slots, wake, stop) = (slots.clone(), wake.clone(), stop.clone());
            std::thread::Builder::new()
                .name("fbw screen copies".into())
                .spawn(move || {
                    let mut scratch = Vec::new();
                    while !stop.load(Ordering::Relaxed) {
                        wake.wait();
                        for slot in &slots {
                            slot.run_pending(&mut scratch);
                        }
                    }
                })
                .expect("the thread starts")
        };
        (Self { wake, stop, worker }, slots)
    }

    pub fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        self.wake.notify();
        let _ = self.worker.join();
    }
}
```

Add test imports: `use std::time::Instant;` inside the tests module.

- [ ] **Step 4: Run the tests**

Run TEST `paint_thread`. Expected: PASS.

- [ ] **Step 5: Wire it into `own_cef.rs`**

- `new_view` takes `slot: Option<Arc<PaintSlot>>` instead of `block`. `View.block` becomes `View.slot`.
- `open_all`: first open every `ScreenBlock` and keep the indices of views that have one. Call `PaintThread::start(blocks)`, hand each view its slot. Add `paints: Option<PaintThread>` to `Browsers`.
- `Browsers::close`: at the very start, `if let Some(t) = self.paints.take() { t.stop(); }`. `close` takes `self`, so bind it `mut self`.
- `test_support::tga_like_dump` takes a `&ScreenBlock` the test opened itself; it is unaffected.
- `on_paint` becomes:

```rust
fn paints_on_their_thread() -> bool {
    static YES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *YES.get_or_init(|| {
        let yes = SWITCHES_APPLIED.load(Ordering::SeqCst) && chromium_switches().contains("disable-gpu-compositing") && std::env::var("FBW_PAINT_THREAD").as_deref() != Ok("0");
        super::log(&format!("paints copied {}", if yes { "on their own thread" } else { "on the main thread" }));
        yes
    })
}

unsafe extern "system" fn on_paint(handler: *mut RenderVt, browser: *mut Browser, kind: c_int, count: usize, rects: *const Rect, buffer: *const u8, width: c_int, height: c_int) {
    release_browser(browser);
    let v = &mut *view_of(handler);
    if kind != 0 {
        return;
    }
    v.paints += 1;
    let Some(slot) = v.slot.as_ref() else { return };
    if !slot.fits(width, height) {
        return;
    }
    let given = if rects.is_null() { &[][..] } else { std::slice::from_raw_parts(rects, count) };
    let whole = [Rect { x: 0, y: 0, width, height }];
    let list = if given.is_empty() || given.len() > MAX_DIRTY_RECTS { &whole[..] } else { given };
    let list: Vec<[u32; 4]> = list
        .iter()
        .filter_map(|r| {
            let (x0, y0) = (r.x.clamp(0, width), r.y.clamp(0, height));
            let (x1, y1) = ((r.x + r.width).clamp(0, width), (r.y + r.height).clamp(0, height));
            (x1 > x0 && y1 > y0).then_some([x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32])
        })
        .collect();
    crate::perf::time("screens (browser paints)", || {
        if paints_on_their_thread() {
            slot.hand_over(buffer as usize, &list);
        } else {
            slot.copy_now(std::slice::from_raw_parts(buffer, width as usize * height as usize * 4), &list);
        }
    });
}
```

The probe test's counters from Task 2 stay around the `perf::time` block, with the byte sum written for `[u32; 4]` rectangles. Add `read_failures()` to the 30-second stats line in `Browsers::tick`.

- [ ] **Step 6: Measure offline**

Run: `sh tools/browser-screens/paint_probe.sh soft "disable-gpu-compositing"`
Expected: ms per paint under 0.05, paints/s near 180.

Compare a frame: open `E:/A380/fbw-build/tmp/probe-1152x1536.bgra` with `bgra2png.py` from Task 13, or check it now by eye once that exists. The static text at the top must be sharp and whole.

- [ ] **Step 7: Commit**

```bash
git add src/browser_screens/paint_thread.rs src/browser_screens/mod.rs src/browser_screens/own_cef.rs
git commit -m "perf(browser-screens): copy paints on their own thread"
```

### Task 8: Uploads through a mapped buffer

**Files:**
- Create: `src/display/upload_ring.rs`
- Modify: `src/display/mod.rs` (`mod upload_ring;`)
- Modify: `src/display/gl.rs` (`Renderer`, `bridge_upload`)

**Interfaces:**
- Produces: `UploadRing::new(capacity: usize)`, `reserve(&mut self, bytes: usize) -> Option<usize>`, `fence(&mut self, fence: usize)`, `retire(&mut self, signalled: impl FnMut(usize) -> bool, delete: impl FnMut(usize))`. Env `FBW_UPLOAD_RING=0` turns it off.

Uploads run at 4.4 GB/s on the main thread, and jumped to 13 ms when the GPU was slow, which looks like the driver waiting on textures still in use. A persistently mapped buffer in cached memory takes the CPU copy at memcpy speed, and the GPU then does the transfer in order, without the CPU waiting.

**Gate:** check first. In C1's `symbolize.py` output, if `bridge_upload` is at least 1.5 ms per frame after Task 6, do this task. If row comparison already brought it under 1 ms, skip it and note the numbers here.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserves_in_order_and_wraps_past_the_end() {
        let mut r = UploadRing::new(100);
        assert_eq!(r.reserve(60), Some(0));
        r.fence(1);
        r.retire(|_| true, |_| {});
        assert_eq!(r.reserve(30), Some(60));
        assert_eq!(r.reserve(30), Some(0));
    }

    #[test]
    fn refuses_what_the_gpu_may_still_be_reading() {
        let mut r = UploadRing::new(100);
        assert_eq!(r.reserve(80), Some(0));
        r.fence(1);
        assert_eq!(r.reserve(30), None);
        r.retire(|_| false, |_| {});
        assert_eq!(r.reserve(30), None);
        let mut deleted = Vec::new();
        r.retire(|_| true, |f| deleted.push(f));
        assert_eq!(deleted, vec![1]);
        assert_eq!(r.reserve(30), Some(0));
    }

    #[test]
    fn something_bigger_than_the_ring_is_refused() {
        assert_eq!(UploadRing::new(100).reserve(101), None);
    }

    #[test]
    fn fences_retire_in_order() {
        let mut r = UploadRing::new(100);
        r.reserve(10);
        r.fence(1);
        r.reserve(10);
        r.fence(2);
        let mut deleted = Vec::new();
        r.retire(|f| f == 2, |f| deleted.push(f));
        assert!(deleted.is_empty(), "the older one is not done");
        r.retire(|_| true, |f| deleted.push(f));
        assert_eq!(deleted, vec![1, 2]);
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run TEST `upload_ring`. Expected: `UploadRing` not found.

- [ ] **Step 3: Implement the bookkeeping**

```rust
//! Which bytes of the mapped upload buffer are free. The GL calls are in `gl.rs`.

use std::collections::VecDeque;

pub struct UploadRing {
    capacity: usize,
    head: usize,
    used: usize,
    unfenced: usize,
    in_flight: VecDeque<(usize, usize)>,
}

impl UploadRing {
    pub fn new(capacity: usize) -> Self {
        Self { capacity, head: 0, used: 0, unfenced: 0, in_flight: VecDeque::new() }
    }

    /// An offset with room for `bytes`, or `None` while the GPU may still read it.
    pub fn reserve(&mut self, bytes: usize) -> Option<usize> {
        if bytes > self.capacity {
            return None;
        }
        let skipped = if self.head + bytes > self.capacity { self.capacity - self.head } else { 0 };
        if self.used + skipped + bytes > self.capacity {
            return None;
        }
        let offset = if skipped > 0 { 0 } else { self.head };
        self.head = offset + bytes;
        self.used += skipped + bytes;
        self.unfenced += skipped + bytes;
        Some(offset)
    }

    /// Everything reserved since the last fence is done when `fence` is.
    pub fn fence(&mut self, fence: usize) {
        self.in_flight.push_back((std::mem::take(&mut self.unfenced), fence));
    }

    pub fn retire(&mut self, mut signalled: impl FnMut(usize) -> bool, mut delete: impl FnMut(usize)) {
        while let Some(&(bytes, fence)) = self.in_flight.front() {
            if !signalled(fence) {
                break;
            }
            self.used -= bytes;
            self.in_flight.pop_front();
            delete(fence);
        }
    }
}
```

- [ ] **Step 4: Run the tests**

Run TEST `upload_ring`. Expected: PASS.

- [ ] **Step 5: The GL side**

In `gl.rs`, next to `NativeSync`, so a missing function means no ring rather than no displays (`Fns::load` fails outright on a missing function, and macOS has no `glBufferStorage`):

```rust
const GL_PIXEL_UNPACK_BUFFER: c_uint = 0x88EC;
const GL_SYNC_GPU_COMMANDS_COMPLETE: c_uint = 0x9117;
const GL_ALREADY_SIGNALED: c_uint = 0x911A;
const GL_CONDITION_SATISFIED: c_uint = 0x911C;
const RING_BYTES: usize = 64 << 20;
const RING_STORAGE_FLAGS: c_uint = 0x2C3;
const RING_MAP_FLAGS: c_uint = 0xC3;

struct UploadPath {
    ring: super::upload_ring::UploadRing,
    buffer: c_uint,
    mapped: *mut u8,
    fence_sync: unsafe extern "system" fn(c_uint, c_uint) -> *mut c_void,
    client_wait_sync: unsafe extern "system" fn(*mut c_void, c_uint, u64) -> c_uint,
    delete_sync: unsafe extern "system" fn(*mut c_void),
}

unsafe impl Send for UploadPath {}

impl UploadPath {
    unsafe fn load(gl: &Fns) -> Option<Self> {
        if std::env::var("FBW_UPLOAD_RING").as_deref() == Ok("0") {
            return None;
        }
        let dll = CString::new("opengl32.dll").ok()?;
        let module = LoadLibraryA(dll.as_ptr());
        if module.is_null() {
            return None;
        }
        let wgl = GetProcAddress(module, c"wglGetProcAddress".as_ptr());
        if wgl.is_null() {
            return None;
        }
        let wgl: unsafe extern "system" fn(*const c_char) -> *mut c_void = std::mem::transmute(wgl);
        let find = |name: &std::ffi::CStr| -> Option<*mut c_void> {
            let p = wgl(name.as_ptr());
            ((p as isize) < -1 || (p as isize) > 3).then_some(p)
        };
        let storage: unsafe extern "system" fn(c_uint, isize, *const c_void, c_uint) = std::mem::transmute(find(c"glBufferStorage")?);
        let map: unsafe extern "system" fn(c_uint, isize, isize, c_uint) -> *mut c_void = std::mem::transmute(find(c"glMapBufferRange")?);
        let fence_sync = std::mem::transmute(find(c"glFenceSync")?);
        let client_wait_sync = std::mem::transmute(find(c"glClientWaitSync")?);
        let delete_sync = std::mem::transmute(find(c"glDeleteSync")?);
        let mut buffer = 0;
        (gl.gen_buffers)(1, &mut buffer);
        (gl.bind_buffer)(GL_PIXEL_UNPACK_BUFFER, buffer);
        storage(GL_PIXEL_UNPACK_BUFFER, RING_BYTES as isize, std::ptr::null(), RING_STORAGE_FLAGS);
        let mapped = map(GL_PIXEL_UNPACK_BUFFER, 0, RING_BYTES as isize, RING_MAP_FLAGS) as *mut u8;
        (gl.bind_buffer)(GL_PIXEL_UNPACK_BUFFER, 0);
        if mapped.is_null() {
            crate::log("display: uploads stay on the old path, the buffer did not map");
            return None;
        }
        crate::log("display: uploads through a mapped buffer");
        Some(Self { ring: super::upload_ring::UploadRing::new(RING_BYTES), buffer, mapped, fence_sync, client_wait_sync, delete_sync })
    }
}
```

The flags are: read, write, persistent, coherent, client storage. Asking for read access keeps the memory cached on the drivers that matter.

Add `upload_path: Option<Option<UploadPath>>` to `Renderer`, initialised `None`.

In `bridge_upload`, replacing the rectangle loop:

```rust
let upload_started = std::time::Instant::now();
if self.upload_path.is_none() {
    self.upload_path = Some(UploadPath::load(&self.gl));
}
let gl = &self.gl;
let mut path = self.upload_path.as_mut().and_then(|p| p.as_mut());
if let Some(p) = path.as_mut() {
    let (wait, delete) = (p.client_wait_sync, p.delete_sync);
    p.ring.retire(
        |fence| matches!(wait(fence as *mut c_void, 0, 0), GL_ALREADY_SIGNALED | GL_CONDITION_SATISFIED),
        |fence| delete(fence as *mut c_void),
    );
}
let mut updated = false;
let mut ringed = false;
for &[x, y, w, h] in rects {
    if w == 0 || h == 0 || x + w > width || y + h > height {
        continue;
    }
    crate::perf::UPLOADED_BYTES.fetch_add(w as u64 * h as u64 * 4, std::sync::atomic::Ordering::Relaxed);
    let row = w as usize * 4;
    let stride = width as usize * 4;
    let offset = path.as_mut().and_then(|p| p.ring.reserve(row * h as usize));
    if let (Some(p), Some(offset)) = (path.as_mut(), offset) {
        for line in 0..h as usize {
            let from = (y as usize + line) * stride + x as usize * 4;
            std::ptr::copy_nonoverlapping(pixels[from..from + row].as_ptr(), p.mapped.add(offset + line * row), row);
        }
        (gl.bind_buffer)(GL_PIXEL_UNPACK_BUFFER, p.buffer);
        (gl.pixel_store)(GL_UNPACK_ROW_LENGTH, 0);
        (gl.tex_sub_image)(GL_TEXTURE_2D, 0, x as c_int, y as c_int, w as c_int, h as c_int, GL_BGRA, GL_UNSIGNED_INT_8_8_8_8_REV, offset as *const c_void);
        (gl.bind_buffer)(GL_PIXEL_UNPACK_BUFFER, 0);
        ringed = true;
    } else {
        (gl.pixel_store)(GL_UNPACK_ROW_LENGTH, width as c_int);
        let at = (y as usize * width as usize + x as usize) * 4;
        (gl.tex_sub_image)(GL_TEXTURE_2D, 0, x as c_int, y as c_int, w as c_int, h as c_int, GL_BGRA, GL_UNSIGNED_INT_8_8_8_8_REV, pixels[at..].as_ptr().cast());
    }
    updated = true;
}
if let (Some(p), true) = (path.as_mut(), ringed) {
    p.ring.fence((p.fence_sync)(GL_SYNC_GPU_COMMANDS_COMPLETE, 0) as usize);
}
(gl.pixel_store)(GL_UNPACK_ROW_LENGTH, 0);
crate::perf::note("of the draw callbacks: uploads", upload_started.elapsed());
```

Leave the mipmap code after it as it is. The binding is back at 0 before anything else runs: X-Plane and other plugins assume no unpack buffer. Read the existing `bridge_upload` for how the rectangle's byte offset is written and keep its variable names.

- [ ] **Step 6: BUILD**

Expected: compiles. There is no GL context in tests; this path is checked in the sim at Task 9.

- [ ] **Step 7: Commit**

```bash
git add src/display/upload_ring.rs src/display/mod.rs src/display/gl.rs
git commit -m "perf(display): upload screens through a persistently mapped buffer"
```

### Task 9: Stop native work nothing uses, then checkpoint C2

**Files:**
- Modify: `src/native_mode.rs`
- Modify: `src/lib.rs` (the block at `native_shadow::tick`)

**Interfaces:**
- Produces: `native_mode::any_native_work() -> bool`; `FBW_NATIVE_SHADOW=1` runs the comparisons for development.

With `nativeScreens` off, the log says `native shadow: shadow=[...] authoritative=[]` and `native systems host: shadow=[...] authoritative=[]`: these modules run every frame only to compare themselves with the pages.

- [ ] **Step 1: Write the failing test** (in `src/native_mode.rs`'s tests)

```rust
#[test]
fn native_work_runs_only_when_something_uses_it() {
    assert!(!work_wanted(&Modes::parse("", ""), false));
    assert!(work_wanted(&Modes::parse("", ""), true));
    assert!(work_wanted(&Modes::parse("all", ""), false));
    assert!(work_wanted(&Modes::parse("", "fms"), false));
    assert!(!work_wanted(&Modes::parse("", "off,fms"), false));
}
```

`"fms"` must be one of the names in `SYSTEMS`; `native_fms_shadow::applies` uses it.

- [ ] **Step 2: Run it to see it fail**

Run TEST `native_work_runs`. Expected: `work_wanted` not found.

- [ ] **Step 3: Implement**

```rust
pub fn work_wanted(modes: &Modes, comparisons_asked: bool) -> bool {
    comparisons_asked || (!modes.off && (modes.native_all || !modes.authoritative.is_empty()))
}

pub fn any_native_work() -> bool {
    static WORK: OnceLock<bool> = OnceLock::new();
    *WORK.get_or_init(|| work_wanted(&current(), std::env::var("FBW_NATIVE_SHADOW").is_ok_and(|v| v == "1")))
}
```

Import `OnceLock` if the file does not. In `src/lib.rs`:

```rust
if app_settings::native_mode::any_native_work() {
    crate::perf::lap("native: shadow");
    native_shadow::tick(&mut self.vars, self.time);
    crate::perf::lap("native: systems host");
    native_systems_host::tick(&mut self.vars, self.time, &h_events);
    crate::perf::lap("native: datalink");
    datalink_native::tick(&mut self.vars, self.time);
    h_events.extend(native_systems_host::take_h_events());
    crate::perf::lap("native: fms + mfd host");
    events.extend(native_fms_shadow::tick(&mut self.vars, self.time));
}
app_settings::native_mode::show_notices();
```

Before committing, confirm by reading each `tick` that it has no output a page reads when not authoritative. `datalink_native`'s mask is 0 in shadow, `native_fms_shadow` publishes only `if applies`, and `native_systems_host` queues outputs only for authoritative modules. If one does feed something, leave that one running and say so in the commit.

- [ ] **Step 4: Run the tests**

Run TEST `native`. Expected: PASS. Then the whole suite: TEST with no filter. Expected: only tests already failing before this task may fail; compare the list.

- [ ] **Step 5: Commit, BUILD, INSTALL with backup name `<date>-pixel-work`**

```bash
git add src/native_mode.rs src/lib.rs
git commit -m "perf: skip native modules when none of them drives anything"
```

- [ ] **Step 6: Checkpoint C2 (needs the user's launch)**

FLIGHT `pixel-work` with `FBW_MAIN_THREAD_PROFILE=1`. The user flies R1 and R2.

Expected, compared with C1's run in the same mode:
- `screens (browser paints)` under 0.1 ms if Task 7 applied, otherwise about 2 ms.
- `of the draw callbacks: uploads` under 1 ms.
- Uploaded MB per frame lower, with all four PFD and ND figures close together.
- `read_failures` flat after startup.

If the split between left and right displays stays uneven at high load, the frame's upload budget is starving later screens: raise `UPLOAD_BUDGET_BYTES` in `src/display/mod.rs` from 16 to 32 MB, as uploads are now cheap, and fly again.

If screens tear visibly or stop updating, set `FBW_PAINT_THREAD=0` or `FBW_UPLOAD_RING=0` to find which, and fix before going on.

---

## Phase D: Paint less

### Task 10: Find and stop needless repaints

**Files:**
- Create: `tools/browser-screens/paint_audit.mjs`
- Create: `src/page_paint_patches.rs`
- Modify: `src/lib.rs` (`mod page_paint_patches;`, `patches.extend(page_paint_patches::source_patches());` next to the other `extend` calls near line 320)

**Interfaces:**
- Consumes: `SourcePatch { path, find, replace, reason }` from `src/source_patch.rs`.
- Produces: `page_paint_patches::source_patches() -> Vec<SourcePatch>`.

Parked on a runway the two NDs resend about 8 MB a frame of the 12. Parked screens should send almost nothing. Task 6 stops identical pixels, but Chromium still rasterises them, which costs helper CPU on every core it touches.

- [ ] **Step 1: Write `paint_audit.mjs`**

```js
// paint_audit.mjs [port] [seconds]: per page, what painted and what asked for it.
const port = process.argv[2] ?? 9444;
const seconds = +(process.argv[3] ?? 5);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const version = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
const ws = new WebSocket(version.webSocketDebuggerUrl);
await new Promise((r) => { ws.onopen = r; });
const events = [];
let done;
const finished = new Promise((r) => { done = r; });
ws.onmessage = (ev) => {
  const m = JSON.parse(ev.data);
  if (m.method === 'Tracing.dataCollected') events.push(...m.params.value);
  if (m.method === 'Tracing.tracingComplete') done();
};
const categories = 'devtools.timeline,disabled-by-default-devtools.timeline,disabled-by-default-devtools.timeline.invalidationTracking';
ws.send(JSON.stringify({ id: 1, method: 'Tracing.start', params: { transferMode: 'ReportEvents', categories } }));
await sleep(seconds * 1000);
ws.send(JSON.stringify({ id: 2, method: 'Tracing.end' }));
await finished;
const pages = new Map();
for (const e of events) {
  const frames = e.args?.data?.frames;
  if (e.name === 'TracingStartedInBrowser' && frames) {
    for (const f of frames) if (f.url && !pages.has(f.processId)) pages.set(f.processId, f.url);
  }
}
const stats = new Map();
const of = (pid) => {
  if (!stats.has(pid)) stats.set(pid, { paints: 0, area: 0, raf: 0, script: 0, reasons: new Map(), calls: new Map() });
  return stats.get(pid);
};
const bump = (map, key, by = 1) => map.set(key, (map.get(key) ?? 0) + by);
for (const e of events) {
  const s = of(e.pid);
  const d = e.args?.data ?? e.args?.beginData ?? {};
  if (e.name === 'Paint') {
    s.paints += 1;
    const c = d.clip;
    if (c) s.area += Math.abs(c[4] - c[0]) * Math.abs(c[5] - c[1]);
  }
  if (e.name === 'FireAnimationFrame') s.raf += 1;
  if (e.name === 'FunctionCall') {
    s.script += (e.dur ?? 0) / 1000;
    if (d.functionName) bump(s.calls, `${d.functionName} @${(d.url ?? '').split('/').pop()}:${d.lineNumber}`, (e.dur ?? 0) / 1000);
  }
  if (/InvalidationTracking$/.test(e.name)) bump(s.reasons, `${e.name.replace('InvalidationTracking', '')} ${d.nodeName ?? ''} ${d.reason ?? d.invalidationSet ?? ''}`.trim());
}
const top = (map, n) => [...map.entries()].sort((a, b) => b[1] - a[1]).slice(0, n).map(([k, v]) => `${k} (${Math.round(v)})`).join('; ');
for (const [pid, s] of [...stats.entries()].sort((a, b) => b[1].area - a[1].area)) {
  if (!s.paints && !s.raf) continue;
  console.log(`${(pages.get(pid) ?? `pid ${pid}`).slice(-50)}: ${(s.paints / seconds).toFixed(1)} paints/s, ${(s.area / seconds / 1e6).toFixed(2)} Mpx/s, ${(s.raf / seconds).toFixed(1)} rAF/s, script ${(s.script / seconds).toFixed(1)} ms/s`);
  console.log(`  why: ${top(s.reasons, 6)}`);
  console.log(`  script: ${top(s.calls, 6)}`);
}
process.exit(0);
```

- [ ] **Step 2: Audit offline**

Run the offline bench with real pages in headless Chrome, so no window opens. `cefclient.exe` opens windows on the user's screen, so do not use it here. In the background:

Run: `BROWSER='/c/Program Files/Google/Chrome/Application/chrome.exe' sh tools/browser-screens/run.sh 120 60`

It serves the pages and opens them with DevTools on 9333. Wait for the pages to load, then:

Run: `node tools/browser-screens/paint_audit.mjs 9333 5 > E:/A380/fbw-build/tmp/paint-audit-offline.txt`

Expected: a table per page. Values in the bench are static, so anything painting steadily here is a needless repaint or a deliberate animation. This Chrome is newer than X-Plane's 117, so treat its numbers as a guide to what repaints, not how fast.

- [ ] **Step 3: Audit in the sim (needs the user's launch)**

FLIGHT `audit` with `FBW_CHROMIUM_SWITCHES=disable-gpu-compositing remote-debugging-port=9444 remote-allow-origins=*` (use the chosen mode's switches). While the user sits parked:

Run: `node tools/browser-screens/paint_audit.mjs 9444 5 > E:/A380/fbw-build/perf-runs/<run>/paint-audit-parked.txt`

Then again in flight, `paint-audit-flying.txt`.

- [ ] **Step 4: Fix each needless repaint with a patch**

Work down the table, biggest Mpx/s first. For each page:

| What the audit shows | Fix |
|---|---|
| A canvas redrawn every frame with the same inputs | In the patch, remember the last inputs and return early when unchanged |
| A value on screen changing in digits the display does not show | Round before the DOM write, to the displayed precision |
| An attribute or style written every frame with the same value | Compare before writing, in the page's subject or a patch |
| A CSS animation or blink that the real display has | Leave it. Check that it paints only its own box |
| Attitude or position noise when parked | Round in the page at the precision drawn |

The patch file:

```rust
//! Stops pages redrawing what has not changed. Each `find` must match exactly once
//! in the installed bundle: `dump_patches --check` proves it.

use crate::source_patch::SourcePatch;

const ND: &str = "/Pages/VCockpit/Instruments/A380X/ND/nd.js";

pub fn source_patches() -> Vec<SourcePatch> {
    vec![]
}
```

For each finding, add an entry using the exact text from the installed bundle:

```rust
SourcePatch {
    path: ND.to_string(),
    find: "<exact text from nd.js>".to_string(),
    replace: "<same code with the early return>".to_string(),
    reason: "<what stopped repainting>".to_string(),
}
```

Check each with:
```bash
cargo +stable-x86_64-pc-windows-gnu run --release --features js --bin dump_patches -- --check
```
Expected: every patch matches. The app route needs `XPHFBW.exe` rebuilt to get patches, so note that in the commit if the app route is still in use.

A patch must not freeze a screen. At C3, for each patched page, the user changes what it shows (heading, range, mode, a system page) and confirms it follows. List what to try in the message before that flight.

- [ ] **Step 5: Commit**

```bash
git add tools/browser-screens/paint_audit.mjs src/page_paint_patches.rs src/lib.rs
git commit -m "perf(displays): stop pages repainting what has not changed"
```

Target: parked at R1, ND under 0.5 MB and PFD under 1 MB per frame, and helper CPU down.

### Task 11: One Chromium frame per sim frame

**Files:**
- Modify: `src/browser_screens/own_cef.rs` (`Host`, `open_all`, `Browsers`)
- Modify: `src/browser_screens/mod.rs` (`begin_frames`)
- Modify: `src/lib.rs` (flight loop callback, near line 3752)
- Modify: `src/display/mod.rs` (`UPLOAD_MIN_INTERVAL` use)

**Interfaces:**
- Produces: `browser_screens::begin_frames()`, env `FBW_PACED_SCREENS=0` falls back to timers.

Timers drift against the sim. Some sim frames get two paints of a screen, and the first is wasted. Others get none, which reads as stutter. CEF's external BeginFrame lets the sim ask for exactly one, which is the user's 1x with no governor guessing.

**Gate:** count first. Add the count of paints per sim frame from Task 2's stats. If PFD and ND already hold within 10% of sim fps and C2 shows no stutter, skip this task and record the numbers.

- [ ] **Step 1: Write the failing test** (in `own_cef.rs`)

```rust
#[test]
fn only_the_big_screens_are_paced() {
    for key in ["SCREEN_DU_PFDL", "SCREEN_DU_PFDR", "SCREEN_DU_NDL", "SCREEN_DU_NDR", "SCREEN_DU_EWD", "SCREEN_DU_SD"] {
        assert!(paced(key));
    }
    for key in ["SCREEN_DU_MFD", "FCU", "SCREEN_EFB", "@19"] {
        assert!(!paced(key));
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run TEST `only_the_big_screens_are_paced`. Expected: `paced` not found.

- [ ] **Step 3: Implement**

```rust
fn big_screen(key: &str) -> bool {
    matches!(key, "SCREEN_DU_PFDL" | "SCREEN_DU_PFDR" | "SCREEN_DU_NDL" | "SCREEN_DU_NDR" | "SCREEN_DU_EWD" | "SCREEN_DU_SD")
}

fn paced(key: &str) -> bool {
    big_screen(key) && std::env::var("FBW_PACED_SCREENS").as_deref() != Ok("0")
}
```

Use `big_screen` inside `rate_for` as well, so the list exists once. `paced` reads the environment, so cache its result in `View` when it is built instead of calling it per frame. In `Host`, replace `external_begin_frame: Slot` with:

```rust
send_external_begin_frame: unsafe extern "system" fn(*mut Host),
```

In `open_all`: `external_begin_frame_enabled: c_int::from(paced(&w.key))`. In `Browsers`, add `frames_outside: u32`, and:

```rust
const OUTSIDE_EVERY: u32 = 6;

impl Browsers {
    /// Once per sim frame, paused or not: paced pages run only when asked.
    pub fn begin_frames(&mut self) {
        let outside = super::VIEW_OUTSIDE.load(Ordering::Relaxed);
        self.frames_outside = if outside { self.frames_outside.wrapping_add(1) } else { 0 };
        if outside && self.frames_outside % OUTSIDE_EVERY != 0 {
            return;
        }
        for v in &self.views {
            unsafe {
                if !paced(&(**v).key) {
                    continue;
                }
                if let Some(host) = host_of((**v).browser) {
                    ((*host).send_external_begin_frame)(host);
                    release_host(host);
                }
            }
        }
    }
}
```

In `mod.rs`:

```rust
pub fn begin_frames() {
    if !own_route() {
        return;
    }
    if let Ok(mut s) = state().lock() {
        if let Some(b) = s.browsers.as_mut() {
            b.begin_frames();
        }
    }
}
```

Add `pub fn begin_frames(&mut self) {}` to `Browsers` in the non-Windows stand-in `own_cef` module in `mod.rs`.

In the flight loop callback in `src/lib.rs`, call `browser_screens::begin_frames()` before the `plugin.replay.active` and `plugin.paused()` early returns, under `#[cfg(feature = "js")]`. Otherwise the pages freeze in pause and replay.

In `refresh_bridge`, a Chromium screen need not wait out `UPLOAD_MIN_INTERVAL` on this route: the pages are paced already, and 28 ms would cap uploads at 35 Hz whatever the sim does:

```rust
#[cfg(feature = "js")]
let min_interval = if crate::browser_screens::enabled() && crate::browser_screens::own_route() { std::time::Duration::ZERO } else { UPLOAD_MIN_INTERVAL };
#[cfg(not(feature = "js"))]
let min_interval = UPLOAD_MIN_INTERVAL;
```

and use `min_interval` in `due`. `own_route()` alone is true on Windows even with the app in use, so check `enabled()` too.

- [ ] **Step 4: Prove it offline**

Extend `paint_cost_probe`: call `browsers.begin_frames()` once per 28 ms loop.

Run: `sh tools/browser-screens/paint_probe.sh paced "disable-gpu-compositing"`
Expected: 6 x 35.7 = about 214 paints/s or fewer, matching the calls, and the same with `FBW_PACED_SCREENS=0` at the timer rate. If paints stop, external begin frames do not work on this CEF with this compositing mode: revert the task and record it under "Already tried".

- [ ] **Step 5: Run the tests and commit**

Run TEST `browser_screens`. Expected: PASS.

```bash
git add src/browser_screens/own_cef.rs src/browser_screens/mod.rs src/lib.rs src/display/mod.rs
git commit -m "feat(browser-screens): one paint per sim frame for the big screens"
```

- [ ] **Step 6: Checkpoint C3 (needs the user's launch)**

BUILD, INSTALL. FLIGHT `paint-less` with `FBW_MAIN_THREAD_PROFILE=1`, R1 and R2.

Expected: all four PFD and ND paint figures within 10% of sim fps. Upload figures match Task 10's targets. Screens keep moving in pause and replay. Ask the user to look: smoother or the same, never worse.

---

## Phase E: Plugin tick

### Task 12: Work down the real hot spots

**Files:** decided by the data. Record them in `docs/analysis/plugin-hotspots-2026-11.md` (create).

**Target:** plugin total without screens at 3.5 ms or less on R1.

- [ ] **Step 1: Name the hot spots**

From the latest profile flight:

Run: `python3 tools/browser-screens/symbolize.py E:/A380/fbw-build/perf-runs/<run>/Log.txt E:/A380/fbw-build/perf-runs/<run>/win.xpl`

Expected: functions with the module they were running in. This also explains the 10.7% of `ntdll` with our plugin on the stack.

- [ ] **Step 2: Write down the table**

In `docs/analysis/plugin-hotspots-2026-11.md`: function, ms per frame (share x frame ms), leaf module, cause, fix.

- [ ] **Step 3: Fix from the top, one commit each, while an item is at least 0.2 ms**

| Leaf or symptom | Likely cause | Fix |
|---|---|---|
| `ntdll` under a tick, heap functions | Allocation every frame: `format!`, `Vec`, `String` keys | Keep buffers in the struct; pre-built names; resolved ids |
| `ntdll` wait under a tick | A lock held by another thread, such as a server thread reading the session | Shorter hold, or per-frame snapshot |
| Hash functions under `prim.rs` `Names` | `HashMap<String, _>` lookup for each variable each frame | Resolve to `VariableIdentifier` once, as `xphfbw_host` does |
| `XPLM_64.dll` under `xplm.find` | Dataref looked up by name every frame | Cache the handle in the struct, as `FlightDeckView` does |
| `XPLM_64.dll` under many `get_f` | Reading datarefs that did not change or are not used | Read less often, or only when something uses them |
| Our own code in `deep` | A subsystem stepping faster than it needs | Step it at its own rate, if the result is unchanged; prove with its golden tests |
| `nvoglv64` under draw callbacks | GL state set per screen | Share setup across screens |

For `deep` and anything with golden digests, the existing tests are the proof: `TEST deep`. Any digest change means the behaviour moved: undo that fix.

To find per-frame name lookups:
```bash
grep -n 'xplm.find(\|\.find("sim/' src/*.rs | grep -v 'get_or_insert\|fn new\|test'
```
`browser_screens::note_view` does two `find`s a frame: cache them first.

- [ ] **Step 4: Checkpoint C4 (needs the user's launch)**

BUILD, INSTALL. FLIGHT `tick` with `FBW_MAIN_THREAD_PROFILE=1`, R1 and R2.

Expected: plugin total at 6 ms or less on R1. Record the numbers in the hot spots file. Commit it:

```bash
git add docs/analysis/plugin-hotspots-2026-11.md
git commit -m "docs: what the plugin's frame costs after the tick pass"
```

---

## Phase F: Resolution

### Task 13: Find where the screens go soft, and tune it

**Files:**
- Create: `tools/browser-screens/bgra2png.py`
- Modify: `src/app_settings.rs` (`display_prefilter_extra`)
- Modify: `src/display/mod.rs:795`

**Interfaces:**
- Produces: `displayPrefilterExtra` in `xphfbw.json`, read live, for tuning in flight. Removed in Step 6.

**What is known:** frames from a test page are equally sharp in both compositing modes. The 31 Oct prefilter (`view_scale.rs`, four taps at level L-0.5 in `SHARPEN_FRAGMENT`) was installed the day before these flights and had never been flown. Its note says "softer text; tune the -0.5 if the user finds it soft". So the prefilter is the first suspect, not Chromium. Note also the Intel look: Chromium skips MSAA on paths on the UHD 770 and uses it on the RTX, so arcs differ slightly between modes.

- [ ] **Step 1: Write `bgra2png.py`**

```python
#!/usr/bin/env python3
"""python bgra2png.py <in.bgra> <width> <height> <out.png> [scale]"""
import struct
import sys
import zlib
from pathlib import Path


def chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))


def main(source, w, h, out, scale=1):
    px = Path(source).read_bytes()
    rows = []
    for y in range(h):
        row = bytearray()
        for x in range(w):
            o = (y * w + x) * 4
            row += bytes((px[o + 2], px[o + 1], px[o])) * scale
        for _ in range(scale):
            rows.append(b"\0" + bytes(row))
    head = struct.pack(">IIBBBBB", w * scale, h * scale, 8, 2, 0, 0, 0)
    Path(out).write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", head) + chunk(b"IDAT", zlib.compress(b"".join(rows))) + chunk(b"IEND", b""))


if __name__ == "__main__":
    a = sys.argv
    main(a[1], int(a[2]), int(a[3]), a[4], int(a[5]) if len(a) > 5 else 1)
```

- [ ] **Step 2: Add the live setting**

In `src/app_settings.rs`, beside `display_sharpen`:

```rust
pub display_prefilter_extra: f64,
```

default `0.`, and in parsing `display_prefilter_extra: as_f64(map, "displayPrefilterExtra", defaults.display_prefilter_extra).clamp(0.0, 9.0),`. At `src/display/mod.rs:795`:

```rust
let extra = crate::app_settings::current().display_prefilter_extra as f32;
renderer.prefilter_lod = (self.view_scale.lod(self.screens[index].def.id) - extra).max(0.);
```

`current()` is read per screen per frame there; if it allocates, read it once per frame in `refresh` and keep the value in `Displays`.

- [ ] **Step 3: Checkpoint C5, compare the three stages in flight (needs the user's launch)**

During one flight, parked with the PFD in view:

1. Raw page frame. Run with `FBW_DUMP_TAG` from the `XPHFBW bridge attached (<tag>)` line in `Log.txt`, and `FBW_DUMP_DIR`:
   ```bash
   FBW_DUMP_TAG=<tag> FBW_DUMP_DIR='E:\A380\fbw-build\perf-runs\<run>\frames' cargo +stable-x86_64-pc-windows-gnu test --release --features js --lib dump_screens_of_a_running_session -- --ignored --nocapture
   python3 tools/browser-screens/bgra2png.py <run>/frames/SCREEN_DU_PFDL_1152x1536.bgra 1152 1536 <run>/frames/pfd.png
   ```
2. Device texture: `display::grab_device` writes `DEVICE_<id>_<w>x<h>.png` when `DEVICE_GRABS` is armed; read how it is armed at `src/display/mod.rs:2102` and arm it the same way.
3. What the monitor shows: the user's screenshot of the same screen, cropped.

Read the images. Decide where sharpness goes:

| Page PNG | Device PNG | Monitor | Cause | Next |
|---|---|---|---|---|
| Soft | Soft | Soft | Chromium raster | Step 5 |
| Sharp | Soft | Soft | Prefilter or sharpen shader | Step 4 |
| Sharp | Sharp | Soft | X-Plane's sampling | Report to the user: only the zoom or the screen's texture size changes this |

- [ ] **Step 4: Tune the prefilter by eye**

While the user watches the PFD, edit `displayPrefilterExtra` in `Output/preferences/xphfbw.json`; the settings poll picks it up within 2 s. Try `0`, `0.5`, `1`, `9` (off), and `displaySharpen` at 0.6 and 1. The user picks. Also have them pan slowly, because less filtering brings the shimmer back that this filter was added for.

- [ ] **Step 5: If Chromium's raster is soft**

Compare with the app's look: the old route ran on the Intel without MSAA on paths. Fly `FBW_CHROMIUM_SWITCHES` with `gpu-rasterization-msaa-sample-count=0` added to the chosen mode, and with `FBW_SCREEN_SUPERSAMPLE=2` for one flight to see whether more pixels even help. Show the user both. The higher setting costs bytes everywhere, so it needs a clear visible win.

- [ ] **Step 6: Fix the choice and remove the tuning setting**

Put the chosen value in the shader or as a const in `view_scale.rs`, and remove `displayPrefilterExtra`. A `view_scale` test pins the value:

```rust
#[test]
fn the_prefilter_keeps_the_chosen_detail() {
    assert_eq!(prefilter_lod(4.), 2.0);
}
```

Adjust it to whatever Step 4 settled on. Commit:

```bash
git add src/display/view_scale.rs src/display/gl.rs src/display/mod.rs src/app_settings.rs tools/browser-screens/bgra2png.py
git commit -m "fix(display): prefilter tuned by eye in flight"
```

---

## Phase G: X-Plane's own share

### Task 14: Report what is left (checkpoint C6)

No code. Each row needs the user's launch; do only the ones they want. Same R1 and R2 each time.

- [ ] **Step 1: What the screens cost in total.** FLIGHT `no-screens` with `FBW_SCREENS=off`. Frame time with minus without is everything the displays cost, including what no timer sees.
- [ ] **Step 2: The user's motion vector layer.** The user disables it in Plugin Admin for one flight. It shows at 8.7% of the main thread (3.5 ms). It also pins the texture pager's scale floor at 1.0, per its own log (`scale floor: at 1.0 the pager can no longer reduce texture resolution AT ALL`), so when the card is full X-Plane spills rather than lowering textures. Report only.
- [ ] **Step 3: Other plugins.** SASL 2.5%, FlyWithLua 1%, plus XJet, simpleOPT, AviTab, Terrain radar, Gizmo. Disable the optional ones for one flight. Report only.
- [ ] **Step 4: X-Plane's settings.** Reflections, shadows, world objects, AI traffic. One flight with the user's choice of a lower setting. Report only.
- [ ] **Step 5: Write it up.** `docs/analysis/xplane-share-2026-11.md`: ms for each, and what is left. If the A380's own objects dominate `X-Plane.exe`, that is its own plan: triangle and draw-call work on the OBJs, under the user's rules in the 12 Oct perf notes (edit OBJs directly, no reconversion, the 19 Oct decimation was reverted).

---

## Phase H: Ports

### Task 15: Linux and Mac stay green

**Files:**
- Create: `tools/linux/plugin_check.sh`

- [ ] **Step 1: Write the check**

```bash
#!/bin/bash
# Mac compile check on Windows, then the library's tests on Linux under WSL.
cd /e/A380/fbw-int/plugin || exit 1
FAKE='E:\A380\fbw-build\tmp\fakecc\fake.exe'
CARGO_TARGET_DIR=/e/A380/fbw-build/target-mac TMP='E:\A380\fbw-build\tmp' TEMP='E:\A380\fbw-build\tmp' \
  CC_aarch64_apple_darwin="$FAKE" CXX_aarch64_apple_darwin="$FAKE" AR_aarch64_apple_darwin="$FAKE" \
  cargo +stable-x86_64-pc-windows-gnu check --target aarch64-apple-darwin --lib --features js > /e/A380/fbw-build/tmp/mac.log 2>&1
echo "mac check exit $?"
grep -aE "^error" -A6 /e/A380/fbw-build/tmp/mac.log | cut -c1-140 | head -30
cat > /e/A380/fbw-build/tmp/wsl_check.sh <<'EOF'
mkdir -p ~/fbw-src/plugin ~/fbw-src/fbw-aircraft/fbw-a380x/src ~/fbw-src/fbw-aircraft/fbw-common/src ~/fbw-build/logs
rsync -a --delete --exclude target --exclude .git --exclude dist --exclude glass --exclude node_modules --exclude app/ui --exclude '*.dll' --exclude '*.xpl' /mnt/e/A380/fbw-int/plugin/ ~/fbw-src/plugin/ || exit 10
rsync -a --delete --exclude target /mnt/e/A380/fbw-int/fbw-aircraft/fbw-a380x/src/wasm/ ~/fbw-src/fbw-aircraft/fbw-a380x/src/wasm/ || exit 11
rsync -a --delete --exclude target /mnt/e/A380/fbw-int/fbw-aircraft/fbw-common/src/wasm/ ~/fbw-src/fbw-aircraft/fbw-common/src/wasm/ || exit 12
cd ~/fbw-src/plugin || exit 13
export CARGO_TARGET_DIR=~/fbw-build/target
cargo build --release --features js --lib > ~/fbw-build/logs/lin_build.log 2>&1
echo "linux build exit $?"
RUST_MIN_STACK=8388608 timeout 500 cargo test --release --features js --lib > ~/fbw-build/logs/lin_full.log 2>&1
echo "linux test exit $?"
grep -aE "^test result|^error" ~/fbw-build/logs/lin_full.log | cut -c1-140
grep -aE "^test .*FAILED" ~/fbw-build/logs/lin_full.log | cut -c1-110 | head -30
EOF
wsl.exe -d Ubuntu-24.04 -- bash -lc 'tr -d "\r" < /mnt/e/A380/fbw-build/tmp/wsl_check.sh > /tmp/wsl_check.sh; bash /tmp/wsl_check.sh' 2>&1 | tr -d '\000' | cut -c1-160
```

- [ ] **Step 2: Run it**

Run: `bash tools/linux/plugin_check.sh`
Expected on 1 Nov's tree: Mac check exit 0, Linux build exit 0, 3,653 pass and 2 fail.

- [ ] **Step 3: Settle the two Linux failures**

`deep::electrical_crate_parity::the_recorded_golden_digests_are_this_plugins`: the digests were recorded on Windows and libm differs in the last bits. Mark the test `#[cfg(windows)]`, saying so in the commit.

`deep::pneumatic_ducts::network::tests::closing_all_cross_bleed_valves_stops_a_non_running_engine_from_pressurising`: Linux settles at 148 kPa against a floor of 150 kPa. Print `pack_supply_pressure_pa[0]` every 100 steps on both systems. If the two runs track within 2%, it is libm again, and the floor is arbitrary: lower it to 140 kPa, keeping what the test means ("pack 1 still fed"). If they diverge, that is a real numerical sensitivity in the model: report it, do not paper over it.

- [ ] **Step 4: Commit**

```bash
git add tools/linux/plugin_check.sh src/deep/electrical_crate_parity.rs src/deep/pneumatic_ducts/network.rs
git commit -m "chore(port): one check for Mac and Linux; the last two Linux test failures"
```

### Task 16: Run the port check after each phase

- [ ] **Step 1:** After Phases B, C, D and E, run `bash tools/linux/plugin_check.sh`. Expected: no new failures. Every new Windows call must have a `#[cfg(windows)]`, and `own_cef`'s stand-in module in `browser_screens/mod.rs` must grow anything new that shared code calls, such as `Browsers::begin_frames`.
- [ ] **Step 2:** What Linux and Mac get from this plan: page patches (Task 10), the upload ring on Linux (macOS has no `glBufferStorage`, so it falls back). They stay on X-Plane's browser devices, at about 30 Hz, until someone has a machine to test on. Own browsers on Linux would need `cef_types_linux.h`'s structs and a different API hash. **Blocked:** nobody has run the plugin in X-Plane on either system. Packaging `lin.xpl` and `mac.xpl` waits for a tester.

---

## Phase I: Ship

### Task 17: Make it the normal route

Only after the user has flown it and said so.

**Files:**
- Modify: `src/app_settings.rs` (default of `browser_displays`)
- Modify: `docs/browser-screens.md`
- Modify: release staging per the release notes

- [ ] **Step 1:** Rewrite `docs/browser-screens.md` to describe what runs: own browsers, chosen Chromium mode, the copy thread, the ring, the env switches, the flight kit.
- [ ] **Step 2:** Change `browser_displays`'s default to `true`, and its EFB label to say what it does without "experimental". Keep the setting, so the app remains the fallback for one release.
- [ ] **Step 3:** Ask the user whether XPHFBW and the systems server leave the package now, or after a release with the fallback. Do not remove them unasked.
- [ ] **Step 4:** Release only on the user's word, following the release notes: exact install minus licensing exclusions, no names or personal folders in anything published, and check the sync traps. The user pushes and publishes.
- [ ] **Step 5:** Commit.

```bash
git add src/app_settings.rs docs/browser-screens.md
git commit -m "feat(browser-screens): screens in X-Plane's Chromium are the default"
```

---

## Held in reserve

Not scheduled. Each has a trigger that says when it becomes worth doing.

### R1. Lower page resolution when the eye cannot use it

**Trigger:** after Phase C, uploads or helper CPU still limit the frame, or PFD and ND cannot hold 1x on R2.

At the seat one monitor pixel covers about 2.2 device texels (`view_scale::minification`), so a 1.5x page holds about 3.3 page texels per monitor pixel, and a 1x page still has 1.5x more than the monitor shows. Every cost falls by 55%: raster, copy, upload. `ViewScale` already knows the minification per screen. Switch `View.scale` between 1 and 1.5 with hysteresis (1x above 1.8, 1.5x below 1.5), through `notify_screen_info_changed` and `was_resized`.

Hard parts: `ScreenHeader.width/height` change while the block keeps its capacity; `draw_bridge` needs a logical page size separate from the texture's; `prefilter_lod` must drop by log2(1.5) at 1x; the paint thread must flush before a size change. The user judges quality when zooming in and out.

### R2. Chromium on its own thread

**Trigger:** Task 3's stacks show Chromium's pump, excluding paint copying, costing at least 2 ms a frame after Phase C.

CEF's `multi_threaded_message_loop` moves everything to its own UI thread, off X-Plane's main thread. The hook can edit a copy of `settings`, and the delay slot for `cef_do_message_loop_work` can point at the same return-0 stub as `app_outliving_this_dll`. The danger is X-Plane's own browser handlers then running on a foreign thread: our boot device goes through them. The alternative is for the plugin to call `cef_initialize` itself, with the settings Task 2 logs, no boot device, and the hook answering X-Plane's later call with 1. Other aircraft's browser devices in the same session become the risk, and CEF must then be shut down at process exit. Spike with a backup in hand; expect crashes on the first tries.

### R3. Helpers on efficiency cores

**Trigger:** the i9's main thread is slower with helpers running: `FBW_SCREENS=off` shows a bigger gain than the plugin's timers explain.

From the 30-second stats path, on a background thread: snapshot processes, find `cefclient.exe` children of X-Plane, call `SetPriorityClass(BELOW_NORMAL_PRIORITY_CLASS)`, then try `SetProcessInformation(ProcessPowerThrottling, EXECUTION_SPEED)`. A/B one flight each: keep only if sim fps rises and paints stay at 0.9x.

### R4. Hide browsers nobody sees

**Trigger:** Task 5 keeps a GPU mode, so idle browsers still hold tiles on a card. With Software this gains nothing.

`was_hidden(1)` for screens X-Plane has not drawn in 2 s (the draw counts in `display: gpu (screen callbacks)` already show it) from an allow list without logic in the page: IFE, RTPI, BAT. Pages with logic stay awake.

### R5. Opaque pages

**Trigger:** raster in Software mode limits paint rate.

`background_color: 0xFF000000` for screens with nothing under them, so Chromium skips blending. The ND keeps transparency if a plugin underlay is drawn beneath it on this route; check that terrain and weather radar still show.

## What each step should give

Estimates, to be replaced with measurements as checkpoints finish.

| Step | Main thread | Graphics memory |
|---|---|---|
| Chromium off the RTX (Tasks 4, 5) | Paints 3.7 to about 2.2 ms; stalls of 10+ ms gone | 1.2 GB back; spill down |
| Row comparison (Task 6) | Upload bytes down 30-70% parked, less in flight | |
| Copy thread (Task 7) | Paints about 2.2 to 0.05 ms | |
| Upload ring (Task 8) | Uploads 2.9 to under 1 ms | 64 MB of system memory, none on the card |
| Native shadows off (Task 9; terrain probe already done) | 0.3-0.9 ms | |
| Page patches (Task 10) | Fewer bytes, helper CPU down | |
| Hot spots (Task 12) | 1-2 ms | |
| **Total** | **Plugin about 11 to about 5-6 ms** | **About 1.2 GB** |
