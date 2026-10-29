# MFD native assembly (crates/mfd_native)

Goal: the native MFD (a380-displays `mfdx` / `DomDisplay`) is fed a document generated from the Rust page builders and css_cascade instead of Chromium's DOM snapshot.

## What exists
- `mfd_tree::Node` gained `attrs` (`BTreeMap`, serialised only when non-empty, so compare.py output is unchanged), builder methods `attr(name, v)` / `attr_if(name, on)` for `type`, `checked`, `disabled`, `placeholder`, and `Deserialize` (also reads dom2tree JSON; its `cs` key is ignored).
- `crates/mfd_native` (own `[workspace]`):
  - `Native::load(html_ui, dpr)`: UA + custom-element sheets, host sheet, `/Pages/VCockpit/Instruments/A380X/MFD/mfd.css` (the sheets prove.rs uses); dpr 1.5 like the captures.
  - `Native::document(&body) -> String`: adapter Node -> `css_cascade::Doc` (html root with `animationsEnabled`, id/class/style/value/attrs/svgattr, svg `shapes` rebuilt as nested child elements by `_d`), cascade, then the exact form DomDisplay parses: `<body ...>` subtree, each element and svg shape with a full `data-cs` (75 props), `style` attributes, input `value`, no `<html>` wrapper (dom.cpp starts at `<body>`).
  - `mfd_screen(left, right)`: body > vcockpit-panel > a380x-mfd > `#MFD_CONTENT` with the host/MFD.tsx inline styles (absolute inset 0, 100% box, flex row height 1024, left 768 px + margin-right 110 px, right 768 px).
  - `splice_left(screen, part)`: puts a builder output into the left MFD (whole `MFD_LEFT_PARENT_DIV`, `mfd-navigator-container`, or the `mfd-page-container` that is a direct child of the navigator).
  - `examples/assemble.rs <html_ui> <jobs.txt> <out_dir>`: per job builds the screen and writes `<name>.stripped.dom.html` (captured structure, data-cs replaced by the cascade) or `<name>.native.dom.html` (left page from a builder JSON). 15-50 ms per full screen (release).
- Tests: `cargo test -p mfd_tree -p mfd_native` 4/4.

## Proof (E:/fbw-build/mfd-native, `python tools/run_proof.py`)
12 v15 captures (F-PLN x3 from mfd-pages-a, PERF x2 / FUEL&LOAD / INIT from mfd-pages-b, WIND / POSITION MONITOR / SEC INDEX from mfd-pages-c/c2, ATCCOM CONNECT / D-ATIS from mfd-pages-d). Rendered with `E:/fbw-build/a380-displays/gpu152/a380disp_render.exe mfdx - <state.json> <png>` and `MFDX_DOM=<doc>`; strict = tools/compare.py rule. Right MFD comes from the capture structure in both b1 and b2 (no builder for its page state); `left` columns crop the left half.

| capture | a vs golden | b1 stripped vs a | b2 native vs a | b2 vs a (left half) | b2 vs a tol1px |
|---|---|---|---|---|---|
| 0018 F-PLN | 99.37 | 90.57 | 71.75 | 55.94 | 78.19 |
| 0030 F-PLN revisions YSSY | 98.37 | 93.68 | 44.84 | 28.37 | 47.85 |
| 0087 F-PLN NEW DEST | 99.15 | 91.24 | 71.69 | 56.39 | 78.49 |
| 0171 PERF | 98.96 | 73.00 | 73.00 | 59.82 | 78.28 |
| 0181 PERF CLB | 99.35 | 77.36 | 77.36 | 65.96 | 82.38 |
| 0203 FUEL&LOAD | 99.75 | 92.03 | 92.03 | 94.20 | 94.85 |
| 0224 INIT | 99.71 | 94.60 | 94.29 | 97.36 | 95.98 |
| 0209 WIND | 99.56 | 92.36 | 80.28 | 70.41 | 86.80 |
| 0231 POSITION MONITOR | 99.78 | 93.66 | 91.91 | 93.69 | 93.97 |
| 0243 SEC INDEX SEC 1 | 99.60 | 84.14 | 84.14 | 79.71 | 85.99 |
| 0318 ATCCOM CONNECT | 99.56 | 92.30 | 92.30 | 95.33 | 94.72 |
| 0331 ATCCOM D-ATIS | 97.43 | 90.59 | 90.59 | 91.07 | 92.75 |
| mean | 99.22 | 88.80 | 80.35 | 74.02 | 84.19 |

Reading it:
- b1 isolates "cascade instead of data-cs" on identical structure: 88.8. The remaining gap is layout, not style: css_cascade matches data-cs on 99.93% of compared values, but data-cs also carries Chromium's USED width/height/top/left/right/bottom (and resolved % margins, grid tracks). The cascade emits authored values (`auto`, `50%`, `1fr`), so DomDisplay's layout engine now resolves them itself.
- DomDisplay does resolve them (core/src/layout.cpp `parse_len`: px, `%` against the containing block, `auto` -> shrink-to-fit / stretch / min-max-content; absolute insets; flex). What it lacks shows up in the low captures: PERF (grid `auto`/`fr` tracks, row tracks; NOTES/layout.md says no `fr`), SEC INDEX, and outer `svg` sizes (width/height come from the svg `width`/`height` attributes, which the cascade does not map to CSS width/height; 21 svgs on INIT carry px in data-cs and `auto` from the cascade; triangle/chevron svgs collapse).
- b2 = b1 where the builder tree equals the capture (PERF, FUEL&LOAD, INIT, SEC INDEX, ATCCOM: identical numbers). F-PLN (pages-a) and WIND (c2) lose more: their builder trees differ from the capture in structure/inline styles (0030: 625 vs 632 elements, 269 vs 284 inline styles; pages-a's `display_unit` had no left-div inline style, now kept from `mfd_screen`). The page agents' tree compare does not score inline styles, so those gaps were not visible to them.

## Remaining steps to replace the DOM snapshot in the plugin
1. Layout gaps in DomDisplay (a380-displays owner): grid `fr` and auto row tracks (PERF), map outer `<svg width/height>` attributes to used size (or have mfd_native emit `width:<attr>px;height:<attr>px` for outer svg whose computed width is `auto`, which is the SVG2 presentation-attribute rule), then re-run b1 until it is >= 98 on the 12 captures; b1 is the measure of "cascade + layout instead of Chromium".
2. Builder fidelity: DONE for pages-a, pages-b, WIND and POSITION MONITOR/IRS (see "Builder fidelity" below); b2 now equals b1 on all 12 proof captures. Still open: oit_pages (40/143 clean with `--styles`, 909 differences).
3. Right MFD: build it from its own ShellModel/page model (same builders, side FO), not from the capture.
4. Plugin wiring (plugin owner): replace the `MFD_DOM_BATCH` / `a380.mfd.dom` producer (app/js/msfs/mfd-dom.js snapshot) with `Native::document(&mfd_screen(left, right))` from the native FMS models; keep the bus topic and string format so DomDisplay is unchanged. Load the cascade once (mfd.css parse) and re-cascade only on model change (15-50 ms per full screen in release; a sparse/incremental cascade or caching per subtree would bring it under a frame).
5. Interaction state (dropdown open, scroll offsets, focus) from native state into the models; scroll offsets need `data-st/data-sl` (NOTES/mfd-live.md).

## Builder fidelity (`compare.py --styles`)
- `tools/pagetree/compare.py --styles` also compares inline style declarations (split on `;`, property lower-cased, value whitespace-collapsed, sorted), outer svg attributes and svg shapes (attribute names lower-cased as the HTML parser does; `_display`, which dom2tree takes from data-cs, is skipped). Without the flag the output is unchanged.
- mfd_pages_a: 19/19 captures 0 differences with `--styles` (left MFD div), including 0030 F-PLN revisions. Changes: left div and wrapper inline styles, the four overlay svgs (inline `display:none`, second SelfTest progress rects, six EngineeringTestMode texts), duplicate-names dialog (column widths, double-arrow icons, flex wrappers), Button (`visibility: inherit` on the container, `undefined` when no buttonStyle, menu `display: block` when opened, upward menu `top: -(33*n+6)px`), InputField (unit `visibility: inherit`, text container `min-width: round(digits*18)px`, caret `display`), footer message colours as rgb(), erase-temporary line display, revisions menu position 195/183. Model fields added: `revisions_menu_positioned`, `tmpy_line`; capture_to_model.py fills them and the footer message colour. The 33/6 px menu row/padding is GENERIC (fitted to the 7-item F-PLN INFO capture).
- mfd_pages_c wind.rs: all 11 WIND captures (0147-0160, 0209-0220) 0 differences with `--styles` (`E:/fbw-build/mfd-pages-c/run_c2.py --styles`); golden_c2.rs sets `return_button_visible` for the 01xx captures and `descent_entries_loaded` for 0160/0220. Cruise rows count as loaded once a cruise waypoint is selected.
- mfd_pages_c position.rs: 0231 MONITOR and 0237 IRS 0 differences. The b2 < b1 gap on 0231 was the tree, not layout: FBW passes `containerStyle='"width:130px;'` (stray quote) to the monitor WPT field, so Chromium drops the width; the builder applied 130 px. Now emits the same invalid declaration, plus InputField `min-width` (RNP 72, WPT 126, SET HDG 90) and the footer message background (`footer_background`, model field). 0233/0235 NAVAIDS still differ (22).
- mfd_pages_b: 16/16 captures 0 differences with `--styles` (structure 0). Changes: tab-bar svg width `20.784609690826528` (JS number text), CONFIRM/T.O SPDs and APPR star labels `justify-content: space-between`, CRZ FL `justify-content: center`, trip wind `flex-start`. InputField builds its `alignTextSub` before `onAfterRender` defaults `alignText`, so an omitted alignText renders `center`, and a subscribable alignText is read once at render (trip wind is disabled at render, so always `flex-start` until focused). Models 0187/0189 need `crz_pred: step` (run_perf.py updated).
- oit_pages widgets: button containers `visibility: inherit`, InputField unit `visibility` and caret `display`, dropdown menu elements `text-align: center`: 8302 -> 909 differences, 0 -> 40 clean captures (`E:/fbw-build/styles43/run_oit.py`). Left: InputField `min-width` per field (needs max digits per format, 668), DropdownMenu menu `display: block` in 199 places, AOC form inline styles (~40), chart viewer `img.oefb-sheet` transform (8).
- mfd_pages_d 22/22 clean.

| capture | b1 stripped vs a | b2 native vs a (before) | b2 native vs a (now) | b2 left half (now) |
|---|---|---|---|---|
| 0018 F-PLN | 90.57 | 71.75 | 90.57 | 91.00 |
| 0030 F-PLN revisions YSSY | 93.68 | 44.84 | 93.68 | 95.51 |
| 0087 F-PLN NEW DEST | 91.24 | 71.69 | 91.24 | 92.32 |
| 0209 WIND | 92.36 | 80.28 | 92.36 | 95.06 |
| 0224 INIT | 94.60 | 94.29 | 94.60 | 97.91 |
| 0231 POSITION MONITOR | 93.66 | 91.91 | 93.66 | 97.23 |
| mean (12) | 88.80 | 80.35 | 88.80 | 87.93 |
