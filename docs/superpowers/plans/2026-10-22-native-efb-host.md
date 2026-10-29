# Native EFB host (efbhost70)

`src/efb_native/` runs the EFB without Chromium. Each change rebuilds the page tree (shell + `efb_pages` builders), `doc.rs` styles it with css_cascade (EFB stylesheet `efb.css`, extracted from `app/ui/index.html` by `style::extract_css`, plus `native.css` for the keyboard, dropdowns and dialogs), turns `::before/::after` rules into real `efbn-before/after` children, and writes the `data-cs` document `host.rs` publishes on `a380.efb.dom`; scroll offsets go on `a380.efb.scroll` (no relayout).

## Hooks (not done here: shared files)

1. `Cargo.toml`: `mfd_tree`, `css_cascade`, `efb_pages` path dependencies. `src/lib.rs`: `#[cfg(feature = "js")] mod efb_native;` (host part needs `native-displays`).
2. Flight loop: `efb_native::host::tick()` once per frame when the EFB screen is native.
3. `src/display/mod.rs`: EFB screen pointer events -> `efb_native::host::pointer(x, y, kind, button)`; key callbacks -> `efb_native::host::key(ch, vkey, flags)` before X-Plane gets them (both ch and vkey are needed: `%` and LEFT share 0x25).
4. a380-displays: `int a380d_boxes(display, float* xywh, int cap)` returning the laid-out border box of every element in document order (body first, svg shapes and children of replaced elements excluded, before scroll offsets), and the host calls `efb_native::host::set_boxes` after each draw. Hit testing, range sliders, dropdown anchors and scroll extents use these boxes; until this exists, taps do nothing in the sim (all logic is unit-tested with synthetic boxes).
5. Background: set `Efb::bg_url` to an asset path the efbx asset dir serves (efb_pages `bg` PNGs), not a data URL (a 2 MB string per publish).

## Input model

Tap = down/up within 6 px (GENERIC) on an element with `data-act`. Drag on a scroll container (computed `overflow-y: auto|scroll`; containers without an id get `efbn-s-<tab>-<n>`) scrolls; release flings with exponential decay (tau 325 ms, GENERIC); wheel 48 px per click (GENERIC). Text fields: `focus:<field>` opens the on-screen keyboard and takes X-Plane's keyboard (`display::set_efb_text_focus`); Enter/Esc/Done commit and release. Caret is drawn into the field value.

## Remaining areas

Each area is a builder in `efb_pages` (read-only here) plus, in `engine.rs`, a `panel()` arm, a decorate pass adding `data-act`, an `activate()` arm and a `poll()` source:

- Documents/Checklists: `docs_page` + `DocsState` (already in `Efb::docs_state`); acts `doc:tick:<list>:<i>`, `doc:open:<id>`, pins.
- Failures: `failures_content` with `FailuresView.scroll_top/viewport` from `Efb::scrolls` (windowed rows); data `crate::study::web::failures_json`, arm via `POST /study/action`.
- Ground/Pushback: `ground_content`; data `/gse/state`, writes via `/study/action`.
- Payload & Fuel/Cargo/Performance: loadsheet page from `/study/pages`; Charts: lists from `charts.rs`, images as `img` assets, PDFs via `pdf::PdfRenderer`.
- Study/Components/Wear/Maintenance/Breakers: `crate::study::web::*_json`; Status/Diagnostics: `diag_route`; Throttle: `throttle::state_json`; ATC/FAP/Support.

## PDFium

- Install: the installer downloads the pdfium-binaries release for the platform (`pdfium.dll` / `libpdfium.so` / `libpdfium.dylib`, Apache-2.0/BSD) into the plugin folder, checksum pinned in the installer manifest.
- Runtime: load by name with the plugin's existing dynamic-library loader; `FPDF_InitLibrary`, `FPDF_LoadMemDocument`, `FPDF_GetPageCount`, `FPDF_LoadPage`, `FPDFBitmap_Create` (BGRA), `FPDF_RenderPageBitmap` with `FPDF_ANNOT`, swap to RGBA, close page/document. Implements `pdf::PdfRenderer`; `NoPdf` is the stub when the library is missing.
- Use: render on a worker thread, cache PNGs per (file, page, size) on disk, show as an image asset; pan by drag, zoom by wheel.

## Known gaps

css_cascade has no `filter`, `backdrop-filter`, `text-overflow`, `background-position`, `pointer-events`, `inset` in its property list, so those do not reach the document; `select` boxes are drawn as text inputs showing the chosen label; dates use UTC (`toLocaleString` is not reproduced); app actions (restart, updater, logs) need the app and only report that.
