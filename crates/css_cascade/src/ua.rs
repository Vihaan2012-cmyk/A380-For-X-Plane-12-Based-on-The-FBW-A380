pub const UA_SHEET: &str = r#"
html, address, blockquote, body, center, dialog, div, figure, figcaption, footer, form, header, hr, legend, listing, main, p, plaintext, pre, search, xmp, article, aside, h1, h2, h3, h4, h5, h6, hgroup, nav, section, dir, dd, dl, dt, menu, ol, ul, details, summary, fieldset, optgroup, option { display: block; }
head, link, meta, script, style, title, base, area, datalist, noembed, noframes, param, rp, template, source, track { display: none; }
[hidden] { display: none; }
li { display: list-item; }
table { display: table; box-sizing: border-box; border-spacing: 2px; border-collapse: separate; text-indent: initial; }
caption { display: table-caption; text-align: center; }
colgroup { display: table-column-group; }
col { display: table-column; }
thead { display: table-header-group; vertical-align: middle; }
tbody { display: table-row-group; vertical-align: middle; }
tfoot { display: table-footer-group; vertical-align: middle; }
tr { display: table-row; vertical-align: inherit; }
td, th { display: table-cell; vertical-align: inherit; padding: 1px; }
th { font-weight: bold; text-align: center; }
img { display: inline; }
svg:not(:root) { overflow: hidden; }
body { margin: 8px; }
p { margin-block: 1em; }
h1 { font-size: 2em; margin-block: 0.67em; font-weight: bold; }
h2 { font-size: 1.5em; margin-block: 0.83em; font-weight: bold; }
h3 { font-size: 1.17em; margin-block: 1em; font-weight: bold; }
h4 { margin-block: 1.33em; font-weight: bold; }
h5 { font-size: 0.83em; margin-block: 1.67em; font-weight: bold; }
h6 { font-size: 0.67em; margin-block: 2.33em; font-weight: bold; }
ul, ol, menu { margin-block: 1em; padding-inline-start: 40px; }
ul ul, ol ul, ul ol, ol ol { margin-block: 0; }
b, strong { font-weight: bolder; }
i, em, cite, var, address, dfn { font-style: italic; }
u, ins { text-decoration: underline; }
s, strike, del { text-decoration: line-through; }
small { font-size: smaller; }
big { font-size: larger; }
sub, sup { font-size: smaller; }
code, kbd, samp, pre, tt { font-family: monospace; }
pre { white-space: pre; }
a:-webkit-any-link { text-decoration: underline; }
fieldset { margin-inline: 2px; padding-block: 0.35em 0.625em; padding-inline: 0.75em; border: 2px groove rgb(192, 192, 192); min-inline-size: min-content; }
button, input, select, textarea { text-rendering: auto; color: fieldtext; letter-spacing: normal; word-spacing: normal; line-height: normal; text-transform: none; text-indent: 0; text-shadow: none; display: inline-block; text-align: start; margin: 0; font: 13.3333px Arial; }
input, textarea, select, button { color: rgb(0, 0, 0); }
input { padding: 1px 2px; border: 2px inset rgb(118, 118, 118); background-color: rgb(255, 255, 255); }
input[type="button" i], input[type="submit" i], input[type="reset" i], input[type="file" i]::file-selector-button, button { align-items: flex-start; text-align: center; cursor: default; color: rgb(0, 0, 0); background-color: rgb(239, 239, 239); box-sizing: border-box; padding: 1px 6px; border: 2px outset rgb(118, 118, 118); }
input[type="checkbox" i], input[type="radio" i] { margin: 3px 3px 3px 4px; padding: initial; background-color: initial; border: initial; box-sizing: border-box; }
input[type="range" i] { margin: 2px; padding: initial; border: initial; background-color: initial; }
input[type="color" i] { padding: 1px 2px; border: 1px solid rgb(118, 118, 118); background-color: rgb(239, 239, 239); width: 50px; height: 27px; }
textarea { font-family: monospace; font-size: 13.3333px; white-space: pre-wrap; padding: 2px; border: 1px solid rgb(118, 118, 118); background-color: rgb(255, 255, 255); }
select { display: inline-block; box-sizing: border-box; align-items: center; white-space: pre; border: 1px solid rgb(118, 118, 118); border-radius: 0px; background-color: rgb(255, 255, 255); }
select:-internal-list-box { overflow: auto; }
img, canvas, video { overflow: clip; }
canvas { display: inline; }
video { object-fit: contain; }
iframe { border: 2px inset; }
hr { margin-block: 0.5em; margin-inline: auto; border-style: inset; border-width: 1px; color: gray; overflow: hidden; }
svg, svg:not(:root) { display: inline; }
dialog:not([open]) { display: none; }
"#;

pub const HOST_SHEET: &str = r#"
html { color: #fff; }
*, *::before, *::after { min-height: 0; box-sizing: border-box; }
::-webkit-scrollbar { display: none; }
html, body { overflow: hidden; }
.hidden { display: none !important; }
"#;

pub const CUSTOM_ELEMENTS: &str = "vcockpit-panel, a380x-mfd, a380x-oit { display: block; }";
