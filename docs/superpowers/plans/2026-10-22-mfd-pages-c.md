# MFD page trees, group C (mfd_pages_c)

Goal: pure functions from a page model to a `mfd_tree::Node` tree that reproduces the DOM FlyByWire's TSX renders for the page body (the contents of `div.mfd-page-container`), translated from FBW's own sources under `fbw-a380x/src/systems/instruments/src/MFD/pages/FMS/*` and the UiWidgets they use.

Crate: `crates/mfd_pages_c`, path dependency on `crates/mfd_tree`. Shell, header, footer and shared widgets belong to `mfd_pages_a`; `w.rs` here holds minimal local copies (button, icon button, input field, dropdown, top tab navigator, confirmation dialog, conditional component, svg leaves) so the crate stands alone and is easy to swap.

Order, by value and by whether a v15 capture exists:

1. DATA: STATUS, WAYPOINT, NAVAID, AIRPORT (captures 0293-0314)
2. POSITION: IRS, MONITOR, NAVAIDS (captures 0231-0237)
3. SEC INDEX (captures 0243-0287)
4. WIND (captures 0209-0220), the largest page
5. No capture: HOLD, DIR TO, AIRWAYS, VERT REV, DEPARTURE, ARRIVAL

Proof: a model per capture is built in `tests/` / `examples/golden.rs` (raw values from the captured text, documented per model), written to `E:/fbw-build/mfd-pages-c/<capture>.json`, then compared with `E:/fbw-build/mfd-pages-c/cmp.py`, a thin wrapper over `tools/pagetree/compare.py` that picks the page-container whose subtree matches best (the DOM holds several `mfd-page-container` nodes per MFD, so `cls=` alone selects the hidden first one).

Rules kept: no FSComponent/Subject/EventBus code is translated, only FBW's TSX render output and formatting helpers; no explanatory comments; do not commit.

Status 22 Oct (mfdC30): POSITION MONITOR/NAVAIDS/IRS (position.rs), SEC INDEX (sec_index.rs) and WIND (wind.rs) done, 33 captures at 0 differences (0231, 0233, 0235, 0237, 0243-0287 SEC, 0147-0160 and 0209-0220 WIND). Captures 0040, 0044, 0065, 0072 (DIR TO, HOLD, AIRWAYS) show the "FMS PAGE NOT AVAIL" dialog, built as page_not_avail.rs, 4 captures 0 differences. Helpers: fmt.rs coordinate_to_string / position_text / pad_end, x.rs buttons on mfd_pages_a widgets. Proof: examples/golden_c2.rs -> E:/fbw-build/mfd-pages-c/c2, run_c2.py.
Left: VERT REV (captures 0096-0145, tabs RTA/SPD/CMS/ALT/STEP ALTs, MfdFmsFplnVertRev.tsx 1526 lines), HOLD, DIR TO, AIRWAYS, DEPARTURE, ARRIVAL real pages (no capture shows them), GNSS (FBW has no page, button disabled).

Status 22 Oct (mfdD31): VERT REV done (vert_rev.rs, all five tabs RTA/SPD/CMS/ALT/STEP ALTs, alt_window helper), 18 captures 0096-0145 at 0 differences; proof examples/golden_vr.rs -> c2, run_c2.py now reports 55/55 captures at 0 differences. RadioButtonGroup in radio.rs (input type/name/checked/disabled and label `for` carried in svgattr, which compare.py ignores). Entry formatters in entry.rs translated from DataEntryFormats.tsx with JS Number()/toFixed() semantics: SpeedKnots, AltitudeOrFlightLevel, Altitude, FlightLevel, TimeHHMMSS, Latitude (FBW has no longitude format), ILS/VOR-DME/ADF frequency, hold heading/inbound course/dist/time, text/airport entries. HOLD, DIR TO, AIRWAYS, DEPARTURE, ARRIVAL bodies in fpln_rev.rs (they render as fragments, so each returns the body siblings as Vec<Node>), unit tests only (no capture). FBW's DIR TO buttons pass "ERASE<br />DIR TO*" as a plain string label, so the DOM shows it as literal text; reproduced as is.
Left: GNSS (no FBW page); HOLD/DIR TO/AIRWAYS/DEP/ARR need captures of the real pages to prove 0 differences.
