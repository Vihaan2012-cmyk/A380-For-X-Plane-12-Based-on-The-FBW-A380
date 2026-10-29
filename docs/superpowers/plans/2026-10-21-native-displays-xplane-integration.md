# Native displays in X-Plane: integration plan

Starts once every display holds 98% strict on its goldens and the performance pass is done. Revised 22 Oct: rendering is Ganesh GL from Chromium 152's own Skia (a380-displays NOTES/skia.md); all displays except the old mfd are >= 98 on v12/flight4; mfdx (98.19 on v15) paints the MFD from a live DOM snapshot (NOTES/mfd-live.md); one frame of every display costs about 8.5 ms on the GPU.

**Goal:** the cockpit screens are drawn by the native C++/Skia displays in `E:/fbw-int/a380-displays`, not by Chromium. During the switch-over FlyByWire's JavaScript logic keeps running in XPHFBW without painting.

**Architecture:**
- The display library is linked into the existing Rust plugin through a small C ABI. It is not a second X-Plane plugin.
- The Rust plugin already owns:
  - the variable store;
  - every bus and generic-data message between views (`xphfbw_host.rs` `route_view_trigger`);
  - the AMDB web server;
  - the screen textures and their upload budget (`display/mod.rs`).
- The native renderer becomes one more texture source next to the XPHFBW bridge.

## Global constraints

- Skia is our build of Chromium 152's Skia (commit 0873ec1, `GR_AA_TESSELLATOR_MAX_VERB_COUNT=100`, Ganesh GL, MSVC `/MD`; args in a380-displays `tools/skia/args.gn`, Linux `args.linux.gn`). The plugin links no other Skia (it only uses tiny-skia, which is pure Rust).
- The native renderer owns its own GL context on a dedicated render thread (WGL on Windows, EGL on Linux, CGL on Mac later). It never touches X-Plane's GL context or state; frames reach X-Plane as CPU buffers through the existing upload path. A direct GL-texture route can follow once the CPU-buffer route is proven.
- No Windows headers outside `core/src/platform_win.cpp`.
- No explanatory comment paragraphs.
- Never launch X-Plane without asking the user first.
- `CARGO_TARGET_DIR=E:/fbw-build/target`.

## Task 1: C ABI in a380-displays

**Files:**
- Create: `core/include/a380/c_api.h`
- Create: `core/src/c_api.cpp`
- Create: `tools/c_api_test.cpp`

**API:**
- `a380d_display* a380d_create(const char* id, const char* variant)` and `void a380d_destroy(a380d_display*)`.
- `a380d_inputs* a380d_inputs_new(void)` and `void a380d_inputs_free(a380d_inputs*)`.
- Setters on an `a380d_inputs*`:
  - `a380d_set_var(name, unit, double)`;
  - `a380d_set_string(name, utf8)`;
  - `a380d_set_bus(topic, json)` and `a380d_set_generic(key, json)`;
  - `a380d_set_fetched(url, body, len)`;
  - `a380d_set_now(ms)` and `a380d_set_dpr(dpr)`.
- `void a380d_size(a380d_display*, int* css_w, int* css_h)`.
- `int a380d_draw(a380d_display*, const a380d_inputs*, uint8_t* rgba, int w, int h, int stride)`: renders on the library's GPU context (CPU fallback when no GL is available) and reads back premultiplied RGBA8888. Returns 1 when the picture changed since the last draw.
- `int a380d_gpu_init(void)`: creates the render-thread GL context; all `a380d_*` calls after it must come from that same thread. Returns 0 when only CPU is available.

**Test:** `c_api_test` renders a v7 state through the C ABI. The result must be byte-identical to `a380disp_render` for nd, mfd, pfd, ewd and sdv2.

## Task 2: Rust side, build and link

**Files:**
- Modify: `build.rs` (cmake the static `a380_displays` library under `E:/fbw-build/a380-displays/plugin-<profile>`, then link `skia.lib`).
- Create: `src/native_displays/mod.rs` (FFI declarations and a safe wrapper).
- Modify: `Cargo.toml` (feature `native-displays`, off by default).

**Test:** a unit test draws the ND from a v7 state.json through the wrapper, and the PNG matches the golden at the same score as `a380disp_render`.

## Task 3: Live inputs

**Files:**
- Create: `src/native_displays/inputs.rs`
- Modify: `src/xphfbw_host.rs` (tap `GENERIC_DATA`, including `xp-ui.bus` batches split per topic, exactly as `tools/golden-tour/rec.js` does).

**Each frame, per native screen:**
- Push only the variables that screen reads. Collect the read set once per display by recording `Inputs::var` lookups, then refresh only those.
- Push only the bus and generic-data topics that changed since the last frame.

**Test:** replay a golden's recorded writes into the live store. The live `a380d_inputs` must equal `Inputs::from_state_file` for that capture: the same lookups give the same values.

## Task 4: Texture source

**Files:**
- Modify: `src/display/mod.rs` (`draw_screen`: a native route next to `draw_bridge_screen`).
- Create: `src/native_displays/screen.rs`

**Behaviour:**
- `FBW_NATIVE_SCREENS=nd,pfd,...` (later an EFB setting) picks the screens.
- A native screen renders into a CPU buffer at the screen's pixel size, at most every 33 ms, and only when inputs changed.
- The upload goes through the existing dirty-rectangle and upload-budget path.
- XPHFBW stops painting that view (it keeps running its JavaScript).

**Test:** unit tests for the route choice. An in-sim check of each screen is made only when the user agrees to launch X-Plane.

## Task 5: Input events

**Files:**
- Modify: `src/native_displays/screen.rs`
- C ABI addition: `a380d_pointer(display, x, y, kind)` and `a380d_key(display, code, text)`.

**Behaviour:**
- MFD, OIT and RMP touch and KCCU keys reach the native display.
- Presentation state stays native: open page, dropdown, cursor, typed text.
- Actions go to FlyByWire's logic as the same Coherent calls and events the Chromium page sent today. They are taken from the recorded `calls`/`writes` in the goldens.

**Test:** each MFD tour step from `tools/golden-tour/tour.mjs` is replayed as pointer events. Every resulting picture must match the golden of that step.

## Task 6: Switch-over and Chromium paint removal

**Behaviour:**
- Every screen with a native display at 98%+ is native by default.
- XPHFBW runs with painting disabled for those views.
- Frame time is measured: CPU per screen and upload bytes. The comparison against the Chromium path is logged as `native displays:` lines.

## Task 7: Linux and Mac

**Behaviour:**
- Build the library and plugin with our Skia M152 build for that platform (Linux status in a380-displays NOTES/linux.md; Linux text is FreeType and needs a glyph-mask plan before it can match the Windows goldens).
- Fonts load from exact-case paths.

**Test:** `c_api_test` byte-identical on each platform for the five reference states.
