# Native Glass: a cockpit-only engine to replace Chromium

Everything in this plan is new code. Nothing comes from earlier attempts (the
QuickJS cockpit, its JS DOM, the tessellating GL path, the vector PFD). The fixed
inputs are FlyByWire's instrument bundles, built unchanged against xp-kit (our
own clean-room avionics library), and the API surface those bundles call
(SimVar, Coherent, the MSFS instrument lifecycle, a DOM subset). This plan gives
that surface a fresh implementation.

## 1. Measured baseline (15 Oct 2026)

Harness, all 24 views loaded, Taxi state.

| What | Today (Chromium in XPHFBW) |
|---|---|
| Processes | 30 |
| Private memory, all XPHFBW processes | 1,830 MB (3,065 MB working set) |
| Live JS heap after GC, all views | 105 MB |
| JS heap including garbage | 149 MB used / 195 MB reserved |
| JS CPU | 1,010 ms per second (one core); the systems host alone 528 ms/s |
| DOM elements | 15,416 (ND 3,044 each, MFD 1,607, EWD 1,562, EFB 1,007, PFD 833) |
| GPU, per screen repaint | ~2 ms (compositing + readback) |
| GPU, 7 screens at 20 Hz | 13-30 % of the RTX 4060 |
| X-Plane main thread | ~2.2 ms per frame copying ~11 MB of pictures |
| In the sim | GPU frame time 21 to 39 ms, fps 43 to 23, when the PFDs came alive |

Live JS heap per view (MB):

| View | MB | View | MB |
|---|---|---|---|
| MFD | 14.1 | systems host | 11.8 |
| EWD | 7.4 | EFB | 5.6 (loaded twice; one copy is redundant) |
| extras host | 5.4 | ND | 5.2 each |
| ISIS | 4.6 | SD | 4.4 |
| PFD | 4.3 each | SDv2 | 4.1 |
| BAT | 3.3 | RTPI | 3.1 |
| OIT | 2.8 each | RMP | 2.2 each |
| FCU | 2.0 | Clock | 1.1 |
| SND | 0.9 | IFE | 0.8 |

The smallest views show that the per-view runtime floor is about 0.8 MB.

Direct2D probe on this PC (new code, scratch):
- a D3D11 device on the RTX: 49 MB private, 33 MB working set (NVIDIA's
  driver);
- Direct2D and DirectWrite: about 1 MB more;
- drawing six 768x1024 screens: 5 MB more;
- CPU per screen frame drawn the slowest way (400 lines and 60 texts, no
  caching, text laid out on every call): 0.65 ms.

## 2. Targets

| | Target |
|---|---|
| Engine process memory (XPHFBW) | **under 100 MB private** |
| Renderer memory (in X-Plane's process) | under 60 MB (mostly the driver's fixed cost) |
| Total | under 160 MB, against 1,830 MB today (over 11x less) |
| X-Plane main thread | under 0.1 ms per frame for all screens, against ~2.2 ms |
| GPU per screen update | under 0.3 ms, against ~2 ms |
| Screen updates | every screen at X-Plane's frame rate, same frame data on both sides |
| Startup to first frame | under 2 s, against CEF's several seconds |
| Fidelity | 99.5 %+ matching pixels against Chromium on every recorded state |
| Install | about 200 MB smaller once CEF is gone |

## 3. Architecture

```
fbw_a380_systems.xpl (in X-Plane)                XPHFBW.exe (the engine, no GPU)
 flight loop: systems tick                         3 worker threads, one V8 isolate each
   -> var snapshot (new, flat, indexed) --------->   tick: FBW bundles (unchanged)
                                                         runtime: SimVar, Coherent,
                                                         lifecycle, timers, fetch (new)
                                                         DOM (new, Rust)
                                                         CSS + layout (new, Rust)
   render thread: Direct2D + DirectWrite  <--------   scene ops (retained, per frame)
     retained scene per screen                        events between views
     only dirty screens, dirty regions
     -> shared texture per screen (RTX)
 avionics draw callback (main thread):
   copy the dirty region of the newest texture,
   then brightness / dimming (new shader)
```

### Where each part runs, and why

**The engine runs in XPHFBW, its own process, with no GPU at all.** JavaScript
and the DOM are the parts most likely to misbehave; in their own process a fault
cannot take X-Plane down. With no GPU device the process saves the driver's
~50 MB, which is what makes under 100 MB reachable.

**The renderer runs in the plugin, on its own thread.**
- The driver's ~50 MB is paid once, in the process that already talks to the
  RTX.
- The textures are shared inside one process.
- The main thread only copies.
- Direct2D is mature, and a lost device is recreated without touching X-Plane.

**One fixed data path from the engine to the renderer: scene operations, not
pixels.** The engine resolves everything (CSS cascade, layout, transforms,
text runs) and sends drawing primitives only. The renderer never sees CSS or
the DOM.

## 4. The engine (XPHFBW, all new)

### 4.1 JavaScript: V8 without Blink

`rusty_v8`, three isolates on three threads, grouped so no bundle is loaded
twice across isolates:

| Thread | Views | Why together |
|---|---|---|
| logic | systems host, extras host | no drawing; half of today's JS CPU |
| flight | PFD x2, ND x2, EWD, SD, SDv2, ISIS, SND | share bundles and update every frame |
| panels | MFD, OIT x2, FCU, RMP x3, BAT, RTPI, clock, IFE, EFB | interaction-driven |

Memory levers, all inside V8:
- **One compile per bundle.** Contexts in one isolate share compiled code for
  the same script, so the second PFD, ND, OIT and RMP cost only their data.
- **Shared library code.** The FBW bundles are rebuilt with the shared code
  (xp-kit and FBW's common libraries) split into common chunks, compiled once
  per isolate instead of once per view. This is a build change only; the
  instrument sources stay the same.
- **Small young generation** (semi-space 1 MB), pointer compression,
  `--optimize-for-size`, bytecode flushing for cold code, and an old-generation
  cap per isolate with a near-limit callback that logs and collects.
- **No browser overhead per view** (no frames, no layout objects, no paint
  data, no per-process renderer state).

Expected V8 total: the 105 MB live today, minus the redundant EFB copy (5.6),
the per-view runtime floor (about 0.8 x 23), duplicate bundle code and
duplicate shared library code. **Target: 65-75 MB.**

### 4.2 Runtime: the API the bundles call (new)

- **SimVar get/set.** Var names are interned to integer ids on first use per
  call site. Each tick reads one snapshot, the same frame for every view, so
  the PFDs and NDs can no longer be a frame apart. Writes are queued to the
  plugin.
- **Coherent:** call/on/trigger and the views' event bus between contexts, with
  isolates linked by lock-free queues, delivered at the next tick (as
  Chromium's IPC does today).
- **Instrument lifecycle:** connectedCallback, Update and the frame loop, paced
  by the plugin's frame counter, never by painting.
- **Timers, promises, fetch (to the plugin), storage.**
- **One copy of shared responses** (airport map data for both NDs, navdata
  answers).

### 4.3 DOM (new, Rust)

Only what the bundles use: elements and text nodes, the attributes, classList,
style (inline declarations), textContent, querySelector(All),
insertBefore/append/remove, a few measurements (getBoundingClientRect, getBBox,
getComputedTextLength) and events for the interactive views.
- Nodes are compact structs in an arena per view; JS holds small handles.
- Every write marks its node dirty in a per-view journal, so nothing watches
  for changes and nothing walks the tree looking for them.
- **xp-kit fast path:** xp-kit's `Kit.h` creates every FSComponent element
  (`xp-kit/src/components/Kit.ts:209`) and binds live values with
  `value.sub(v => setAttr(...))`. Because xp-kit is ours, it gets a backend that
  talks to the engine's DOM directly, with typed setters for the hot bindings
  (transform, class, visibility, text) instead of strings through the generic
  DOM path. The FBW code itself still sees a normal DOM.
- Budget: 15,000 elements at under 200 bytes each, under 5 MB.

### 4.4 CSS (new, Rust)

- Each view's stylesheets are parsed once into rules (the views carry 5-900
  rules each).
- Selector matching runs only for nodes whose class, id, attributes or parent
  changed, with results memoised by (tag, classes, inline style, parent
  style), so 3,000 ND elements resolve to a few hundred distinct styles.
- Only the properties that draw or lay out are resolved:
  - fill, stroke, width, dash, cap, join;
  - opacity, visibility, display;
  - font family, size, weight;
  - text anchor, baseline, letter spacing, paint-order;
  - transform;
  - box model, flex and position for HTML.
- Keyframe animations (the blinks) become native timelines evaluated by the
  renderer, so a blink costs nothing in JS or CSS.

### 4.5 Layout (new, Rust)

- SVG needs none: geometry comes straight from attributes.
- HTML boxes (ND, EWD and SD panels, MFD, OIT, EFB): block, absolute and flex
  layout with taffy; text measured with DirectWrite metrics shipped from the
  font files, so the engine stays GPU-free.
- Only dirty subtrees are laid out again.

### 4.6 Scene operations (the engine's only output)

Per view and per tick, a batch ending in a commit, so the renderer never shows
half a frame:
- create and remove nodes, and order them;
- geometry: path verbs and points (SVG path data parsed in the engine), rect,
  line, circle, polyline/polygon;
- paint: fill, stroke, dashes, caps, joins, opacity, visibility;
- 2x3 transform;
- text run: string, font, size, anchor and baseline already resolved, outline;
- clip: rectangle or path;
- image, canvas image, gradient;
- animation timeline.

Carried in one shared-memory ring per engine thread. In flight this is a few
kilobytes per frame, against 11 MB of pixels today.

## 5. The renderer (plugin, all new)

- **Device:** D3D11 on the adapter X-Plane renders with (the GL context's
  `GL_DEVICE_LUID_EXT` matched against DXGI), single-threaded and with the
  driver's internal threading off. Then Direct2D on it, and DirectWrite with
  FBW's fonts loaded from html_ui in memory.
- **Retained scene per screen,** shared caches across screens:
  - geometry realizations keyed by geometry hash, so identical symbols on the
    two PFDs and two NDs are made once;
  - text layouts keyed by (string, font, size);
  - brushes keyed by colour.
- **Layers.** A subtree whose only change for a few frames is its transform
  (tapes, attitude ladder, compass rose, ND symbols) becomes a cached layer and
  is drawn as one item under its transform. Small and complex subtrees use a
  command list; large ones a bitmap.
- **Damage only.** Each commit's changed nodes give a dirty rectangle per
  screen; the renderer redraws only that region of the screen's persistent
  texture (one texture per screen, about 3 MB at 768x1024), and screens with
  no changes are not touched.
- **Hand-off.** One shared texture per screen with a keyed mutex. The main
  thread's device callback:
  - takes the mutex with zero wait;
  - copies just the dirty region into the device's texture on the GPU;
  - releases;
  - draws the existing frame again if the renderer is busy;
  - never blocks.
- **Brightness and dimming:** a new small shader applied in the same callback
  (per-unit brightness, dimming regions, breaker-dark).
- **Timing:** D3D11 timestamp queries per screen, main-thread microseconds
  per callback, logged every 10 s.

## 6. Phases and gates

Each phase ends with a gate measured against section 2. Until the last phase,
every view can still run in Chromium; during migration, Chromium views and
engine views live in the same XPHFBW process, so events between them are local
calls.

1. **Renderer to an X-Plane screen.**
   - Plugin render thread, Direct2D test card (shapes, dashes, clips,
     translucency, gradient, Ecam text with outline) on the SD.
   - Gate: correct in X-Plane (one-time readback), main thread under 50 µs,
     GPU under 0.3 ms, renderer memory under 60 MB. Needs one X-Plane launch.
2. **Engine core, headless.**
   - V8 isolates on threads, the runtime, the DOM; the EWD and the systems host
     run with no drawing.
   - Gate: the EWD's DOM and attribute values match Chromium's every frame for
     the same recorded var stream (differential test against today's XPHFBW
     as the oracle); unknown APIs logged by name; memory and JS CPU per view
     logged and within budget.
3. **Style, text and drawing.**
   - CSS, SVG geometry, text and blinks into scene operations; the renderer
     draws them.
   - Gate: the EWD at 99.5 %+ pixels against Chromium on its recorded states.
4. **First screen in the cockpit: EWD.**
   - Per-screen switch in the EFB (`Screen renderer`) with automatic fallback
     to Chromium and the reason logged.
   - Gate: before and after in the sim at the same spot (fps, GPU and CPU time
     per frame, main-thread time, renderer GPU time).
5. **PFD, ND, SD, SDv2, ISIS, SND.**
   - Same gates. ND canvases (airport map) through a Canvas 2D subset drawn by
     the renderer.
6. **Panels: FCU, RMP, BAT, RTPI, clock, IFE.**
   - Clicks hit-tested on the engine's own geometry.
7. **MFD and OIT.**
   - Flex layout, focus, text input, the KCCU, scrolling.
8. **EFB.**
   - Our own page, moved to the engine's subset (its controls already avoid
     native form popups).
9. **Chromium removed.**
   - CEF leaves the build and the install. Final measurement against section 2,
     then a release with notes.

## 7. How fidelity is proven

- **Differential runs.** The same recorded var stream drives today's XPHFBW
  (as the reference only) and the engine. Their DOMs are compared node by node
  each frame, and the rendered frames pixel by pixel. A mismatch names the
  node and attribute, or the region.
- **Recorded states** covering every page and mode (PFD in each flight phase,
  ND in ROSE/ARC/PLAN and each range, every SD page, ECAM procedures, MFD
  pages), extended whenever a difference is found in the sim.
- **In the sim:** the renderer's one-time readback per screen, and the per-screen
  fallback with its logged reason.

## 8. Risks

- **The bundles call something not implemented.** Logged by name in phase 2;
  that view stays on Chromium until it is added.
- **Text differs from Chromium.** Chromium on Windows also rasterises with
  DirectWrite; the remaining differences (metrics, hinting) are caught by the
  goldens.
- **JS CPU on one thread is too much.** Three isolates spread today's one
  core; the logic thread can be split further.
- **Memory over budget.** Per-view heap telemetry from phase 2 shows the view
  and the cause.
- **The GL interop is missing on a PC.** The renderer maps its texture and the
  plugin uploads it (slower, still no Chromium).
- **Driver reset.** The renderer recreates its device and scene; the screens
  hold their last frame meanwhile.
