# Native EFB (efb27)

Date: 22 October 2026. Scope: replace the Chromium-drawn EFB (`app/ui/index.html`, served by `app/src/web.rs`, bridge in `app/src/*` and `src/efb.rs`/`src/panel.rs`) with page trees built in Rust from EFB state and painted by the generic `DomDisplay` of `E:/fbw-int/a380-displays` (the way the MFD and OIT ports work). Prototype: `crates/efb_pages` (Home and Settings, DOM-identical to the live EFB).

## 1. What the EFB is, measured

Source: one file, 11,607 lines, 1.55 MB, of which 598 KB is the inlined background JPEG (line 773), 171 KB the inlined `window.__DOCS__` checklist data (line 1157) and ~71 KB one diagram line (5727). The code that matters:

| Part | Lines | Notes |
|---|---|---|
| CSS, static | 765 (lines 7-772) | ~700 rule blocks, 264 `var(--x)` uses |
| CSS, injected by JS | 17 `<style>` injections | `home-rem`, `lr-*`, diagrams, docs, OJE etc. |
| HTML skeleton | 380 | 22 tab panels, rail, lock screen, footer, toast |
| JS main script | 4,568 (1158-5725) | bridge, Settings, Study, Failures, Breakers, Wear, Components, MEL, Payload, Throttle, Ground, Diagnostics, Charts, Flight Plan, Home, Documents, Performance |
| JS diagrams + feature script | 5,880 (5726-11605) | 258 top-level functions: airflow/bleed/vent, ATC, cargo, checklists (cl*), copilot (fo*), ECAM-style diagrams (ex*, elec*, eng*, fctl*, fuel*, hyd*, ns*, sl*), failure scheduler, FAP, ground commands (gc*, gm*, lp*), landing report, markdown docs (md*), operator/livery (op*), performance calculators (pc*), pushback (pb*), SimBrief progress (sb*) |

Interactive surface: 216 `addEventListener`, 69 timers (25 `setInterval`), 37 `innerHTML`, 11 inline `<svg>` templates (the diagrams build SVG strings), 24 scroll uses, 6 range, 10 select, 7 localStorage keys, 1 clipboard write, 1 `window.open`.

### Pages (what the user sees)

A 12-entry left rail (Home, Flight, Documents, Ground, Cabin, Systems, Components, Wear & Tear, Maintenance, Breakers, Failures, Settings) over 22 `tab-panel` sections (home, study, failures, breakers, maintenance, components, wear, flightplan, ground, pushback, fap, charts, loadsheet, cargo, atc, performance, checklists, throttle, settings, status, support, diagnostics); a lock screen overlay (`#efb-lock`: photo background, clock, route, "TAP TO UNLOCK"); top bar with UTC/local clock and chrono; footer; toast; body-wide text-size zoom (`efb-zoom`, 0.9 to 1.5) and brightness (`filter: brightness`).

Approximate JS lines per area (function ranges, both scripts): Documents + checklist + markdown + copilot ~1,180; Performance calculators ~740; Study/ECAM FUEL/WHEEL/topology ~1,180; ECAM-style system diagrams (ex, elec, eng, fctl, fuel, hyd, air, ns, sl) ~2,000; Ground services + gm/gc/lp/lr + pushback ~1,350; Failures (+ picker, scheduler, diagrams) ~830; Components/Wear/Maintenance ~520; Payload and fuel + cargo ~650; Settings + common ~690 (Settings itself ~270 plus a 121-key data table); Breakers ~180; FAP ~285; ATC ~180; Home ~240; Flight plan ~110; Charts ~75; Throttle ~80; Diagnostics ~80.

### Bridge calls and data sources

- App bridge (`window.xphfbw` CEF router, or HTTP on the app's port): `getSettings`, `setSettings(values)` (12 call sites; keys incl. `xphfbw.docPins`, `xphfbw.myDocuments`, `xphfbw.atcNetwork`, `xphfbw.gseOperator`, `LIGHT_PRESET_NAMES`, `xphfbw.userKeys`), `action(name)` (restart-all/displays/systems, run-updater, report, open-logs, open-dumps, discord, licenses, installed-version, `resetAircraftState`, `resetAirframe`), `status()` polled every 1 s. Preview fallback is localStorage.
- Plugin HTTP on 127.0.0.1:8390 (`src/panel.rs`): `/study/pages|failures|breakers|components|maintenance|mel|wear|scheduled|charts|amdb`, `POST /study/action` (42 call sites), `/vars?names=` (17 polls; ARINC-packed values decoded by `unpackArinc`), `/efb/state`, `/efb/livery`, `/efb/notices?after=`, `/efb/landings`, `/efb/local_procedures`, `/gse/state`, `/throttle/state`, `/perf/cal`, `/oit/export`, `/atc/online`, `/wx/metar/<icao>`, `/diag/*`, `/keyboard/efb` (text-focus capture of X-Plane keys).
- App web server (`app/src/web.rs`): `/api/settings`, `/api/status`, `/api/action/*`, `/api/runways`, `/api/nearest`, `/api/charts`, `/api/documents`, `/api/document?path=` (Documents folder, user markdown), `/api/open?url=`.
- External from the page: SimBrief OFP (`simbrief.com/api/xml.fetcher.php`), GitHub releases (update check), chart images and PDFs (FAA d-TPP, user charts as `<img>` / `<iframe>`).
- Static data in the page: `__DOCS__` checklists (normal, sensed, non-sensed, deferred, limitations, inop, perf), `SETTINGS_PAGE` (121 settings in 8 categories) and `DEFAULTS`, changelog text, diagram geometry.

### CSS features used (static block, counts of occurrences)

flex 243, grid 85 (`minmax()` 21, `repeat()` 8, `auto-fill` 3, `fr` 53), border-radius 80, `var()` 264, `clamp()` 9, `max()` 21, `calc()` 4, transform 59 (rotate/translate/scale), overflow 49 (`overflow-y:auto` scrollers 19), box-shadow 12, filter 13, backdrop-filter 10, linear-gradient 7, radial-gradient 2, repeating-linear-gradient 1, transition 7, text-transform 47, letter-spacing 59, text-overflow 3, `:hover` 15, `:focus` 3, `:checked` 5, `::before` 3, `::after` 8, `::-webkit-scrollbar` 6, `position:fixed` 9, `sticky` 1, `@media` 4, `@font-face` (web fonts), SVG fill/stroke/stroke-dasharray/vector-effect. Keyframes and animation: none in the static block; diagram scripts inject a few (`afmove`, `engdash`, `engpulse`, `engspark`).

## 2. Design

### 2.1 Pipeline

```
EFB state (Rust)  ->  efb_pages::<page>(&EfbModel) -> mfd_tree::Node (dom2tree JSON)
                      style table (generated from the live CSS, tag.class keys)
                      -> a380-displays "efbx" (DomConfig: 1430x1000, topic a380.efb.dom)
                      -> layout engine (core/src/layout.cpp) -> DomDisplay painter -> texture
input (mouse/touch/keys from X-Plane window or cockpit tablet) -> Rust hit-test -> EfbModel mutation -> next tree
```

- **State** lives in one Rust `EfbState` (settings map, tab/sub-tab, per-list filters and scroll offsets, doc ticks and pins, last OFP, polled study/vars data). It is in-process: the plugin already owns settings (`src/efb.rs`, `app_settings.rs`), the study pages (`src/study/**`), the `/vars` store, landings, notices, charts. The HTTP endpoints are replaced by direct calls; only SimBrief, METAR/ATC and chart downloads touch the network, all through the plugin's existing curl/HTTP helpers.
- **Trees**: one builder per area, `fn page(&EfbModel) -> Node`, same shape as the DOM the JS produced (same tags and class names), so the CSS-derived style table applies unchanged and `tools/pagetree/compare.py` proves each page against the live EFB. Builders are pure functions of the model (testable offline, golden-comparable).
- **Style table**: generated, not hand-written. `tools/layout_lab/make_style_table.py` already derives `tag.class` keyed computed styles from `data-cs` DOM dumps; for the EFB a generator step loads the EFB in headless Edge (`E:/fbw-build/efb-native/capture.mjs`, below) with every tab and state class combination exercised, and keeps properties identical across occurrences. Prototype result: 173 keys from 4 captures (Home, Home with OFP, Settings x2). Properties missing from the capture list today (added to `capture.mjs` when needed): `text-overflow`, `text-shadow`, `outline`, `text-decoration-line` is present, `-webkit-text-security`. The background image (600 KB data URL) is moved out of the table into the class background table (`ClassBackground`), as OIT does.
- **Where it differs from the DOM by design**: native controls (`select`, `input[type=range]`, `checkbox`, `input[type=number]`) are drawn as styled `div`s the builder emits (text of the selected option, thumb position as `style` width percent, check mark as a class `on`), because `DomDisplay` paints `input` value text only. The compare script then needs a normaliser for these nodes; this affects Settings (about 1,000 of its 2,000 nodes are controls) and is the one place the DOM-identity claim stops.

### 2.2 Input

- Hit testing: add `data` attributes to `mfd_tree::Node` (additive, `skip_serializing_if` empty): `act` (action id string, e.g. `set:xphfbw.nadp=NADP1`, `tab:settings`, `doc:tick:n:3:2`), plus optional `scroll` and `focus`. `DomDisplay` gets `hit(x, y) -> act` (top-most node with `data-act`, honouring clip and scroll offsets) exposed through the c_api; Rust maps the string to a model mutation. Prototype note: `Node` has no attribute map, so the prototype keeps `id`, `cls`, `value` only (all the compare script looks at) and the plan adds `data` in `mfd_tree` first.
- Pointer: the EFB mesh in the cockpit and the pop-up EFB window already deliver XPLM clicks and wheel events to CEF (`app/src/views.rs` `send_mouse_*`); the same events go to the native EFB instead: down/up -> tap (a 6 px dead-zone separates tap from drag), drag on a scroll container -> scroll with inertia, wheel -> scroll.
- Scroll containers: `layout.cpp` has no scroll offsets (NOTES/layout.md: "No scroll offsets"). Add: per container `scroll-y` (px) carried as `data-st` on the node (the MFD live snapshot already reserves `data-st`/`data-sl`), content laid out unclipped, painter translates children and clips to the padding box, a thin native scrollbar painted on overlay. Content height from layout gives the clamp. Long lists (Failures 707 rows, MEL, Documents, Settings 121 rows) are windowed in Rust: the builder emits only rows intersecting the viewport plus a spacer `div` of the skipped height, which keeps each frame's tree under about 1,500 nodes.
- Text entry: focus is native state (`EfbState.focus = Some(field_id)`, caret index, selection). The builder emits the field as a div with the text and a caret `span` (blink on the frame clock). Keys: the plugin's existing `/keyboard/efb` capture (grab X-Plane keys while a field is focused) becomes a direct call; key events from XPLM (`Char`, backspace, arrows, enter, tab, ctrl+a/c/v) edit the string. IME and RTL are out of scope. Clipboard copy ("Copy setting name") uses the plugin's OS clipboard call.
- Keyboard-free flows: dropdowns (`select`) open a Rust-built overlay list (absolute `div` at the anchor's rect, found from the layout box that the c_api returns for an `id`), closed on outside tap, as the MFD dropdowns already do.

### 2.3 Network

- SimBrief OFP: plugin `fetch_simbrief(user_id)` (curl, as `charts.rs` does for the FAA index), result parsed into the typed OFP struct (today `lastOfp` is the raw JSON); the same call feeds Home, Flight Plan, Performance, Cargo, Payload.
- METAR/TAF: plugin already serves `/wx/metar/<icao>`; call the Rust function directly, cache 5 minutes. The METAR colouring (`metarText`, 25 lines) becomes a builder helper.
- ATC online (`/atc/online`) and GitHub update check: same direct calls, polling on the EFB frame clock instead of `setInterval`.
- Charts: lists and image bytes from the plugin (`charts.rs`, AMDB `/study/charts`); images decode with Skia (`SkImage::MakeFromEncoded`) into the texture cache and draw as a scaled, pannable image (drag-pan and wheel-zoom natively, no `overflow:auto`). PDF charts (FAA d-TPP are PDF; the page used an `<iframe>`) are the risk: Skia in our build has no PDF reader. Options, in order: ask the FAA/Navigraph endpoints for PNG renditions where available (`image_night_url` already exists for AMDB); otherwise add a PDF rasteriser (pdfium binaries or `pdfium-render`) in the plugin and cache first-page PNGs on disk. Decision needed before Phase 5.

### 2.4 CSS/DOM features `DomDisplay` lacks for the EFB, and how to cover them

Already present (domdisplay.cpp / layout.cpp): block, inline, inline-block, flex (all of it), basic grid (px/auto tracks, `span`), absolute and relative positioning, margins/padding/borders, border-radius (per corner), `linear-gradient` backgrounds, `background-image` with class table, opacity, overflow hidden clip, z-index stacking (css_stacking), letter-spacing, text-transform, kerning-aware text with LCD option, SVG polygon/line/rect/circle/path/polyline leaves.

| Feature (EFB use) | Status | Plan |
|---|---|---|
| Scroll containers (19 `overflow-y:auto`, `.column`, `.study-main`, `.st-list`, `.page-main`) | missing | `data-st` offsets, clip + translate, native scrollbar, windowed lists (2.2). Required for every page but the lock screen |
| `grid-template-columns` with `fr`, `minmax()`, `repeat()`, `auto-fill` (85 grid uses, 53 `fr`) | missing (NOTES: "No fr") | extend `layout.cpp` track sizing: `fr` distribution, `minmax(0,1fr)`, `repeat(n, ..)`; `auto-fill` resolved in the Rust builder to a fixed column count from the known container width |
| `clamp()`, `max()`, `min()`, `calc()` (34) | missing | resolve in the style-table generator at the target viewport (computed styles are already used values), so no engine support needed for properties from `data-cs`; the 4 viewport-dependent ones become builder-computed inline styles |
| `box-shadow` (12) | missing | painter: blurred rounded-rect shadow via `SkMaskFilter::MakeBlur` before the background; parse the first shadow |
| `filter: brightness()` on body, `backdrop-filter: blur()` (10: panels, lock) | missing | brightness: one `SkColorFilter` on the root layer. Backdrop blur: `SkImageFilters::Blur` over a saved-layer copy of what is behind the box (Skia `saveLayer` with backdrop); fall back to a flat translucent fill on CPU-only machines |
| `radial-gradient`, `repeating-linear-gradient` (body background) | missing (only linear) | painter: `SkGradientShader::MakeRadial`; the repeating stripe is a 1.8 percent white tint, replace by the baked background image |
| `transform` (59: rotate/translate/scale, checkbox tick, kebab, rail icons) | missing | painter: apply `translate/rotate/scale` (2D matrix) around the transform-origin when painting a node and its subtree; layout is unaffected, as in CSS |
| `transition`, `animation`, `@keyframes` (7 + 4 injected) | missing | not needed for a static frame; drive the few visible ones (lock-screen slide out, toast fade, SD flow dashes, engine pulse) from builder state: the builder emits the end-state class plus a `phase` number, advanced by the frame clock |
| `text-overflow: ellipsis` (3 + `home-rem-t`) | missing | line breaking already measures text; add ellipsis truncation for `white-space:nowrap; overflow:hidden; text-overflow:ellipsis` boxes in the painter |
| `text-shadow` (lock clock), `text-decoration` underline (LICENSES link, link buttons) | missing | painter: shadow pass; underline from font metrics |
| `position: fixed`, `inset` (lock screen, toast, menu, sleep overlay) | missing | treat as absolute against the root box (the EFB is a single full-viewport document) |
| `::before` / `::after` (11: checkbox tick, bars) | missing | builder emits explicit `span` children (the DOM equivalent) |
| `:hover` (15), `:focus` (3), `:checked` (5), `:disabled` | not applicable | hover = pressed/hover state class set by Rust from pointer position (`.is-hover`), `:checked`/`:focus` = classes; style table keys carry them (`button.home-rem-row.is-hover`) |
| `-webkit-scrollbar`, `scrollbar-color` | n/a | native scrollbar overlay |
| `<select>`, `<input type=range|number|checkbox|text>` | `input` value text only | builder-substituted divs (2.1); caret and selection painted by the builder-emitted `span`s |
| `<img src=...>` charts, logos | class background table only | `img` leaf: `data-src` id resolved by the plugin to a decoded `SkImage` in a cache; object-fit contain/cover in the painter |
| SVG with `viewBox`, `<g>`, `<text>`, `<path>` with arcs, markers, `stroke-dasharray`, `vector-effect`, `<linearGradient>` (the ~2,000 lines of system diagrams, icons, seat map, runway maps) | partial (`svg.cpp` 446 lines: polygon, line, rect, circle, path, polyline; NOTES/layout.md lists viewBox/g/text as unpainted in the first cut) | extend `svg.cpp`: viewBox mapping, nested `g` with transform, `text` (text-anchor, font-size), dasharray with an offset phase from the builder, round caps/joins, `fill-opacity`. Diagrams are built as `Node.shapes` in Rust from the same geometry tables |
| Web fonts (`@font-face`: Manrope, Segoe UI Variable, mono) | text agent owns `declare_font_weights` | add the EFB faces to the font set (OIT does the same for Inter); check synthetic bold vs real weights |
| `canvas` | not used (2 text matches only) | none |
| `iframe` (PDF charts) | missing | see 2.3 |
| `<details>`, `contenteditable`, drag and drop | not used | none |

### 2.5 Crate layout (target)

`crates/efb_pages` grows one module per area (home, settings, flight, docs, study, failures, ...) plus `style/` (generator glue), `state` (EfbState, actions), `input` (hit mapping, text editing). `mfd_tree` gains the `data` map and a `scroll` hint. A new display `efbx` in a380-displays (a `DomConfig` like `oitx`: size 1430x1000 css, topic `a380.efb.dom`, class backgrounds, `css_stacking`, LCD text off since it is a bright tablet) and a c_api `a380d_hit` / `a380d_box`.

## 3. Phases

1. **Foundation (1 wave, 3 agents).** `mfd_tree` `data` map; `efbx` display + hit test + scroll offsets; style-table generator run over all 22 tabs (capture script extended with a tab-by-tab state driver); lock screen, rail and top bar trees.
2. **Engine gaps (1-2 waves, 3 agents).** `fr`/`minmax`/`repeat` grid, box-shadow, transform, text-overflow, radial gradient, fixed positioning, backdrop blur, filter. Scored on goldens captured from the live EFB (`capture.mjs` PNG vs `efbx` PNG, tools/score.py).
3. **Pages by area (4-5 waves, up to 10 agents).** Each agent owns a module and its reference capture: Settings (done in prototype, add controls substitution), Home (done), Flight plan + Charts + Throttle + Diagnostics + Status + Support, Documents (+ markdown, checklist runner, copilot panel), Study + Components + Wear + Maintenance + Breakers + Failures, Payload & fuel + Cargo + Performance calculators, Ground + Pushback + FAP + ATC, ECAM-style diagrams.
4. **Input, text entry, network (1-2 waves, 3 agents).** Actions, dropdown overlays, focus, keyboard capture, SimBrief/METAR/ATC/update/charts, settings persistence through `app_settings`.
5. **Switch-over (1 wave).** All-display golden tour with the EFB native, perf budget (target: tree build + layout under 2 ms for a 1,500-node page, repaint only on change), cockpit tablet and pop-up window wiring, remove the EFB browser from `views.rs` (`spawn_efb_view`), remove `app/ui` from the install, and the CEF app web server routes that nothing else uses.

## 4. Size of the full port

- Rust builders: the 10,450 JS lines of logic and DOM building port to about 13,000 to 15,000 lines of Rust (the prototype is 830 lines of Rust including tests for Home + Settings, which are about 520 JS lines (home 240, settings 270), i.e. about 1.5 lines of Rust per JS line including serde models and tests), plus data resources: the settings table (23 KB JSON, already extracted), the docs checklist data (171 KB, moves to a resource file), the changelog.
- C++ in a380-displays: about 2,500 lines (scroll, grid `fr`, shadow/transform/filter, svg viewBox/text, hit test, efbx display).
- Style table: generated, about 1,500 keys at full coverage (173 keys for the four captured Home/Settings states); injected-CSS blocks are covered by the capture.
- Effort at the 10-agent, 30-minute-wave pace used for the displays: about 12 to 16 waves, dominated by phase 3. The two heaviest single items are the SVG diagrams (about 2,000 JS lines of geometry plus svg.cpp work) and PDF charts (decision pending).
- Risks: the DOM-identity check does not hold for form controls (normaliser needed); windowing long lists changes scroll maths; PDF charts; text-field editing edge cases; the EFB brightness and zoom are CSS (`filter`, `zoom`) and need explicit painter support (zoom = a root scale in the style table).

## 5. Prototype status (crates/efb_pages)

- Builds Home (with and without an OFP, reminders, status, landings, maintenance dispatch, pinned documents) and Settings (all 121 settings from the extracted table; categories, search filter, compact view, badges DEFAULT/USER/GLOBAL/LIVERY/SESSION, `show` visibility rules, seg/select/range/number/text/switch/callout controls with values) from an `EfbModel`.
- Reference DOM: `E:/fbw-build/efb-native/capture.mjs` serves `app/ui` from a local node server (with `/api/settings` and `/api/status` stubs, `?fixture=1` so no plugin is needed), drives headless Edge over DevTools (port 9444, own profile; no XPHFBW and no X-Plane involved), dismisses the lock screen, optionally runs a state script, and writes `<name>.dom.html` with a `data-cs` computed-style attribute on every element, plus a PNG. Large DOM strings are fetched in 400 KB slices (a single 8 MB `Runtime.evaluate` result never returns).
- Results with `python tools/pagetree/compare.py`: Home 94 nodes 0 differences; Home with flight plan and two landings 188 nodes 0 differences; Settings (default, Aircraft category) 2,000 nodes 0 differences; Settings with changed values, Audio category, livery and session badges 2,000 nodes 0 differences. `cargo test` in the crate: 6 tests pass. Reproduce with `E:/fbw-build/efb-native/compare_all.sh`.
- Not done: no render through `DomDisplay` (an `efbx` display is outside the files this task may touch), no native-control substitution, no data map for hit testing; the compare script ignores attributes other than `id`, `class`, `value` and text, so `data-*`, `type`, `checked`, `placeholder` and inline `style` are not proven.

## 6. Status 22 Oct, wave efbp42

- References: `capture.mjs` now serves `/fonts/*` from the FBW EFB font folder (`EFB_FONTS` overrides), stubs every other `/api/*` with `{}` (`/api/documents` with an empty tree, `EFB_DOCS` overrides), logs loaded faces and anything stubbed or 404. New refs in `E:/fbw-build/efb-native/ref2` (Manrope 400/500/600/700 load; `ref/` untouched). `compare_ref2.sh` (set `CAPTURE=1` to re-capture) builds `efb_dump` and compares all of them.
- `index.html` changes under us (other agents): the settings table was re-extracted from the live page (121 settings, 4 new in category 4, three descriptions reworded); the Home capture without OFP is occasionally taken mid-render (26 nodes) and is stable on re-capture.
- Results (`tools/pagetree/compare.py`, ref2): Home 94 / 0, Home with OFP 188 / 0, Settings 2,052 / 0, Settings Audio 2,052 / 0, Flight Plan empty 4 / 0, Flight Plan with OFP (navlog of 3 fixes, 2 alternates, lbs/kgs) 181 / 0, Payload & Fuel (fixture: no loadsheet page) 3 / 0.
- `flight.rs`: `flight_plan_content` (the `#fp-content` tree), JS `Number()`/`toLocaleString`/`??` semantics (`js_number`, `js_num`, `js_hhmm`), `simbrief_load` (the loadsheet/`loadSimbrief` body, lbs to kg 0.45359237, defaults 84/20 kg from the page) and the "Loaded: ..." message.
- `payload.rs`: only the no-page states. The real loadsheet page comes from the plugin (`/study/pages` kind `loadsheet`), which `app/ui/test/study-fixture.json` does not contain; a fixture page has to be captured from a running plugin before the loadsheet (gauges, decks, holds, CG, fields) can be compared.
- Backgrounds: `tools/efb_bg/jpeg2png.py` converts `app/ui/bg/*.jpg` and the inlined `window.__BG__` (identical to `gate`) to PNG + `manifest.json` (default out `E:/fbw-build/efb-native/bg`). `efb_pages::bg` loads that folder as `data:image/png` URLs (what efbx decodes today) and falls back to `bg/<name>.png`; `page_bg` / `lock_bg` emit `div.efb-bg` / `div.lock-bg` with the `background-image` the JS sets (both use the inlined image).
- Not started: Ground services (438 nodes; `gmSection` alone is ~390 JS lines, plus `gmPlan`, `gmList`, `lpSection`), Failures, Checklists, Documents.

### Failures scroll model (for the port)

The list is windowed in Rust: rows have a fixed height `h` taken from the reference capture per row class (group header vs failure row); the builder gets the container's `scroll_top` and viewport height `vh` (from the efbx scroll topic), emits `div.spacer` of `first * h`, then rows `first .. min(n, first + ceil(vh / h) + 2)`, then a trailing spacer for the rest, so the content height (and the clamp efbx applies to `data-st`) equals the full list. Filters and group collapse change `n` and the row list before windowing; `scroll_top` is clamped to `max(0, n * h - vh)` after a filter change. Drag and wheel only move `scroll_top`; a new tree is built when the first visible row index changes, otherwise only the efbx offset changes (no re-layout). Compare against the live DOM by building the unwindowed tree (`vh = inf`).
