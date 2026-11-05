# Cockpit screens on macOS: why they are black, and a porting plan

Found on a real Mac install (2026-11-05): beta installer `tools/beta/mac.sh`,
release 1.0.5 + `main` at `cc77493`, X-Plane 12.4.3-r2, macOS 27.0.1, Apple M4 Pro.

## Summary

On a Mac every DU (PFD, ND, EWD, SD, MFD, ...) is solid black even with all AC
buses powered and the brightness pots at 1.0. Nothing is broken in the systems,
power logic or GL drawing. **No display route works on macOS today:**

| Route | Where it runs | Why it does not work on this Mac |
|---|---|---|
| XPHFBW.exe (`app/`) | Windows default | CEF 152 + D3D11 shared textures + Win32; Windows-only |
| Browser devices (`browser_screens`, default on non-Windows) | `XPLMAvionicsSetURL` / `XPLMAvionicsRefresh` / `XPLMAvionicsInjectScript` | **Not exported by X-Plane 12.4.3** (checked with `nm` on `XPLM.framework`; it only has `XPLMCreateAvionicsEx` and the draw/hook calls). `enabled()` falls back. |
| Own CEF (`browser_screens/own_cef.rs`) | Windows only (`#[cfg(windows)]`, `libcef.dll`) | Not built for macOS |
| Built-in QuickJS runtime (`src/js/msfs`) | The fallback that actually runs | Cannot run the shipped pages (below) |

### Why the fallback draws nothing

`src/js/msfs/instrument.js` `importPage()` only runs MSFS-style
`<script import-script="...">` tags and `<script type="text/html" id>` templates.
The shipped pages (release zip and `fbw/xplane/xplane/build.mjs` output alike)
are the xp-ui format (`xp-ui/spec/DATA-SPEC.md` 7.1):

```html
<template id="A380X_PFD">...</template>
<script src="pfd.js"></script>
```

So no gauge script runs, `registerInstrument`/`defineInstrument` is never
called, and the renderer clears each screen to black. The log shows it:

```
js: VCockpit05 A380X/PFD/pfd.html?Index=1&duID=0: loaded in 2 ms, 2.3 MB
js: instruments            <- empty: no instrument identified itself
```

Red herrings ruled out: the `GL_INVALID_OPERATION` lines after the atlas/VBO
uploads come from `glBindVertexArray(0)` (core name) in the legacy 2.1 context
X-Plane gives plugins on a Mac. A standalone replay of the renderer's calls on
the same driver ("2.1 Metal - 91.7") showed that is the only failing call, and
the GL_ALPHA8 atlas, VBO, 4x MSAA framebuffer and blit all work. Harmless, but
`gl.bind_vertex_array` could be skipped on a legacy context to quiet the log.

## Porting options (cheapest first)

### A. Own CEF on macOS (recommended)

X-Plane for Mac already runs Chromium in-process:
`X-Plane.app/Contents/Frameworks/Chromium Embedded Framework.framework`,
**CEF 117.2.5 / Chromium 117.0.5938.152**. That is the CEF generation
`own_cef.rs` was written for (117.1.4). Measured on the Mac:

- `cef_api_hash(0)` = `35b5f3657fe9f203a7852235b85b5f2a7501d59e` (platform),
  `cef_api_hash(1)` = `133bd59744d82162591d93434cb6ff146717a3f7` (universal).
  `API_HASH_WINDOWS` differs, as platform hashes always do; the hand-written
  structs need checking against the 117.2.5 macOS headers, not just the hash.

Work:
1. Build `own_cef` for macOS: `dlopen(..., RTLD_NOLOAD)` / `dlsym` on the
   framework instead of `GetModuleHandleA`/`GetProcAddress`; a per-platform
   hash constant; the macOS `cef_window_info_t` layout for windowless rendering
   (`parent_view`, `windowless_rendering_enabled`).
2. Un-gate `paint_copy` / `paint_thread` (they have no Windows calls) and feed
   `OnPaint` BGRA buffers to the existing GL upload path
   (`display::gl::Renderer::bridge_upload`, already plain GL 2.1).
3. Keep `server.rs` (loopback `coui://` server) as is; it is not
   Windows-gated.
4. In `browser_screens::enabled()`, take the own-CEF route on macOS when the
   framework is loaded and its hash matches, before the
   "no browser devices" fallback.

Risk: CEF calls must run on the thread that pumps X-Plane's CEF; on macOS that
is the AppKit main thread, which is X-Plane's flight-loop thread. Confirm
before relying on it.

### B. Teach the QuickJS runtime the xp-ui page format

Make `importPage()` also run `<script src>` and clone `<template id>`, and load
`xplane-js-data.js` (the xp-ui page runtime the browser route injects) instead
of / alongside the MSFS shims. This needs no Chromium, but the pages were built
for a real browser: expect missing DOM/CSS features (the log already lists
unsupported `@media`, `:host`, `font-feature-settings`), and the whole
instrument stack running on QuickJS on the frame thread. Larger and riskier
than A.

### C. Wait for X-Plane's browser-device API

The plugin's own message says "12.4 or later needed", but 12.4.3 does not
export it. Worth asking Laminar which build adds `XPLMAvionicsSetURL`. Needs no
work here, but nothing is shown until it ships.

## Two Mac fixes found on the way

1. **HFS paths** (fixed, `src/lib.rs` `XPluginStart`): `remember_main_thread()`
   cached `XPLMGetSystemPath()` before `use_native_paths()` ran, so the whole
   session saw `Macintosh HD:Users:...`. Effects: "navdata: ... is not an
   X-Plane folder", "the FMS's flight plans could not be read", and
   `xphfbw.json` (e.g. `displaySafeMode`) never read. `use_native_paths()` now
   runs first.
2. **Misaligned Mach-O string table** (not fixed in the build): the arm64
   slice `cargo` produces with the macOS 27 toolchain (ld-27106.1) has
   `LC_SYMTAB.stroff` 4 bytes off an 8-byte boundary (0x014AF8C4), and
   dyld refuses it ("mis-aligned LINKEDIT string pool"). `strip -x` keeps the
   misalignment. Worked around after linking by inserting 4 zero bytes before
   the string pool, bumping `stroff` and `__LINKEDIT.filesize`, then `lipo` and
   an ad-hoc `codesign`. `mac.sh` should do this (or find the linker flag that
   avoids it) or every Mac install produces a plugin X-Plane cannot load.
