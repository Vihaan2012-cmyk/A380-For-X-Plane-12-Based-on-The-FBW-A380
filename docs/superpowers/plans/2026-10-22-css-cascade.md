# CSS cascade engine (crates/css_cascade)

Goal: give the native MFD/OIT page trees (Node in crates/mfd_tree, dom2tree JSON) the computed styles the native painter reads from `data-cs`, derived from FlyByWire's own stylesheets instead of a Chromium getComputedStyle dump.

## Inputs
- mfd.css / oit.css as shipped in the aircraft html_ui (SCSS compiled by FBW's bundler), plus the two `<style>` blocks the instrument host injects (`*{min-height:0;box-sizing:border-box}`, `.hidden{display:none!important}`, `html{color:#fff}`).
- A small user-agent sheet written for this crate (Chromium defaults for the tags the pages use).
- Element tree: tag, id, classes, attributes, inline `style`, parent/child order.

## Output
Per element a `BTreeMap<&'static str, String>` with the 75 properties of `PROPS` (the 64 of data-cs, 4 corner radii, 3 per-side border styles, plus background-image, background-size, box-shadow, font-style), values in Chromium `getComputedStyle` serialisation (`rgb(r, g, b)`, `12px`, `normal`, quoted family names). `to_data_cs` writes the sparse or full `prop:value;...` string the DomDisplay parser reads.

## Stages
1. Tokenise + parse stylesheets: rules, comma selector lists, declarations with `!important`, `@media` (min/max width/height evaluated against the viewport), `@layer`/`@supports` flattened, `@font-face`/`@keyframes`/`@import` skipped, rules with pseudo-elements dropped.
2. Selectors: type, universal, id, class (with escapes), attribute operators, combinators ` ` `>` `+` `~`, `:not :is :where :first-child :last-child :only-child :nth-child :nth-last-child :first-of-type :last-of-type :nth-of-type :empty :root :checked :disabled :enabled :read-only`; dynamic pseudo-classes (`:hover :focus :active :focus-within :focus-visible :visited :placeholder-shown`) never match. Rules are indexed by the rightmost compound (id, class, tag, universal).
3. Cascade per element: candidate rules, sorted by (important, origin, specificity, order); inline style on top. `var()` substituted from the element's custom properties (inherited), shorthands expanded to longhands, `inherit/initial/unset/revert` handled.
4. Computed values: parent-first walk. font-size (keywords, em, %, rem) first, then lengths against the element font-size, colours to rgb/rgba, line-height and letter-spacing to px, border widths snapped (0 when style none), blockification of flex/grid items and out-of-flow boxes, `min-width:auto` rule, `overflow` pair serialisation, `gap`/`grid-column` serialisation, 2D transforms folded to `matrix()`.
5. Proof harness `examples/prove.rs`: parses a v15 capture (full document including head styles and inline style attributes), ignores its data-cs, cascades, compares with the captured data-cs per property.

## Layout-resolved properties
Chromium reports USED values for width, height, top/left/right/bottom of positioned boxes, auto margins, percentage margins/padding, and grid tracks. This crate outputs the AUTHORED value (`auto`, `50%`, `100px`, `1fr 2fr`) so the layout engine computes used sizes. Comparison against data-cs skips width, height, top, left, right, bottom, and counts margin/padding/grid-template/flex-basis only when the authored value is a plain px length; others are reported separately as "layout-resolved".

## Scoreboard
Per property and per capture match %, over the 125 mfd and 143 oit captures. Mismatches are listed grouped by (property, expected, got) so the UA sheet and value rules can be fixed from data.

## Not covered
Pseudo-elements (::before/::after), animations/transitions, `@import`, container queries, `calc()` with mixed percentage (px-only calc is evaluated), `color-mix`, `env()`, JS-set CSSOM changes that are not in the style attribute.

## Results (v15, 125 mfd + 143 oit captures, data-cs ignored, 75 props computed, 64 compared)
Run: `CARGO_TARGET_DIR=E:/fbw-build/target cargo run --release --example prove -- <captures> <html_ui> <report.txt> [filter]`; full report E:/fbw-build/css-cascade/report.txt.
- 99.934% of 10,560,820 compared values match (mfd 99.915%, oit 99.970%); 864 layout-resolved values skipped; width/height/top/left/right/bottom excluded.
- 100% on 46 of 64 compared properties. Below 100%: white-space 98.69, display 98.70 (both one svg `text.AnsuFailedText` rule whose source is not in the shipped CSS), margin-bottom 99.72, margin-left 99.88, transform 99.61 (svg `transform` attributes on some `g`), grid-template-rows 99.74 (auto rows are used values), padding-* 99.98-99.99, color 99.98, overflow 99.98, line-height/gap/min-width/align-items 99.99.
- Capture facts learned: MFD captures were recorded at device scale 1.5 (border widths floor to device pixels: 3px -> 2.66667px, 1px -> 0.666667px), OIT at 1.0; `Cascade::dpr` selects it. OIT viewport is 1333x1000. `min-width:auto` resolves to `0px` when the element or an ancestor is display:none.
- Remaining mismatch classes: form-control UA defaults (radio margins, select line-height, option padding/gap, button padding variants), disabled/`input` colour (rgb(84,84,84)), `overflow:clip` on inputs.

## Differences from data-cs
data-cs carries Chromium used values for width/height/top/left/right/bottom, auto and percentage margins/padding, and grid track lists. The cascade outputs authored values (`auto`, `50%`, `1fr 2fr`); the layout engine resolves them. Everything else is serialised like getComputedStyle.

## Left
Adapter from `mfd_tree::Node` (tag, id, cls, inline, value) to `Doc` (attributes `checked`, `disabled`, `type` need to be carried by Node for `:checked`/attribute selectors); wiring `Style::to_data_cs` into the native page builders; sparse output form; `::before/::after` content boxes; svg presentation attributes beyond display/visibility/opacity/font/transform; form-control UA sheet refinement.
