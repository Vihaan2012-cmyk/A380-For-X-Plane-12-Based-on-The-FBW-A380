# Native OANS and BTV (oans26)

Date: 22 October 2026. Scope: FlyByWire's OANC (fbw-common `instruments/src/OANC`, `oans/`, `shared/src/publishers/OansBtv`) and BrakeToVacate (fbw-a380x `systems-host/PseudoPRIM/BrakeToVacate.ts`) as the std-only crate `crates/oans`. The AMDB server (`src/oans`) is untouched.

## What the native ND needs from this crate

`a380-displays/displays/nd/oans.cpp` reads, per frame:

| Input | Kind | Producer in this crate |
|---|---|---|
| `oans_display_airport` | bus string, ICAO of the map | `control_panel::ControlPanel` (ARPT SEL, DISPLAY AIRPORT, auto-load, reset unload) |
| `oans_not_avail` | bus bool, "NOT AVAIL" flag | `oanc::oans_available`/`oans_failed` (`!available`) |
| `oansSelectedLandingRunway` | bus string `ICAORWY`, drives runway-end highlight and the cyan arrow | `btv::Oans` (selection + BrakeToVacate) |
| `fmsDepartureRunway`, `fmsLandingRunway`, `fmsOrigin` | bus strings from the FMS (input, not produced here) | consumed by `label_style`, `Oanc` |
| `L:A32NX_EFIS_{L,R}_OANS_RANGE` | zoom index | input |
| `L:A32NX_ARPT_NAV_POS_LOST` (ND `map_available`) | bool | `oanc::ppos_not_available` |
| positionString (bottom box) | string | `oanc::compute_position` |
| `a380.mfd.model.oans_panel` | control panel state | `ControlPanel` fields + `state_json()` (page drawing left to the host) |

Other topics/vars this crate produces (rest of the ND/PFD/FWS and golden `nd-*.state.json`): `ndBtvMessage`, `ropsDetectedAirport`, `ropsDetectedRunway`, `ropsDetectedRunwayLda`, `oansSelectedExit`, `oansExitCoordinates`, `ndRwyAheadQfu`, `oans_remove_btv_data`, `oans_show_set_plan_mode`, `L:A32NX_OANS_BTV_REMAINING_DIST_TO_EXIT`, `..._TO_RWY_END`, `L:A32NX_OANS_BTV_REQ_STOPPING_DISTANCE`, `L:A32NX_OANS_RWY_LENGTH`, `L:A32NX_OANS_RWY_BEARING`, `L:A32NX_BTV_APPR_DIFFERENT_RUNWAY`, `L:A32NX_OANS_WORD_1`, `L:A32NX_OANS_AVAILABLE`, `L:A32NX_OANS_FAILED`, `L:A32NX_ARPT_NAV_POS_LOST`. All ARINC words are written as `f32 bits + ssm * 2^32`.

## Steps

1. Crate skeleton (own Cargo.toml, std + serde_json only): geo, arinc, map_utils, projection, labels, btv, selection, oanc.
2. Translate FBW logic, file by file, keeping FBW's constants and branch order (sources and line ranges in the report).
3. Status: steps 1 to 4 done for BTV/selection (6 scenarios match FBW's JS); labels, flags, position and label generation are unit-tested only. Unit tests in `crates/oans/tests` (math, ARINC encoding, BTV state machine, selection rules, label filter, flags), plus golden-state checks against `E:/golden-tour/*/captures/*/nd-*.state.json` (runway length/bearing words, selected runway, LDA) when the folder exists.
4. Oracle in `crates/oans/oracle` (own Cargo.toml: rquickjs + oxc, pattern `src/js/msfs/fms_offline.rs`): FBW's TypeScript (OansMapProjection, OancMapUtils, OancLabelFilter, BrakeToVacate) is transpiled and run in QuickJS with framework shims, scenarios (airport loaded, runway selected, exit selected, aircraft rolling) recorded as JSON goldens in `E:/fbw-build/oans-oracle`; `cargo test` in the crate compares native output with the goldens.
5. Report the plugin hooks needed (bus/var routing in the native hosts), not edited here.

6. oans31: `control_panel` (OansControlPanel.tsx + OancControlPanelUtils.ts: MAP DATA entity list/selection, cross/flag/centre buttons, BTV fallback stopping distance, ARPT SEL search modes and sort, ORIGIN/DEST/ALTN buttons, DISPLAY AIRPORT, auto-load nearest < 20 NM on ground and destination < 50 NM in flight with the 10 min manual hold, 60 s performance-mode hide, STATUS database strings, `L:A32NX_OANS_AVAILABLE`/`FAILED`, `oans_not_avail`), `markers` (OancMarkerManager.ts, turf length/along reproduced in `turf`), `advisory` (RWY AHEAD from OansBrakeToVacateSelection.updateRwyAheadAdvisory, own lineOffset/booleanContains/booleanDisjoint in `turf`, `L:A32NX_OANS_WORD_1` + `ndRwyAheadQfu` via `Oans::update_rwy_ahead_advisory`), `navdb::OansNavDb` (= `btv::NavDb` + `control_panel::AmdbSource`). Oracle now loads real turf 7.2 from node_modules and records `rwy_ahead`, `post_landing_exit` (DRY/WET/STOP lines, all colours) and `markers`; control panel is unit-tested (`tests/control_panel.rs`), its TSX is not transpilable without the framework.

Plugin hooks (not edited): host implements `OansNavDb` (AMDB server airport list + per-type map data, FMS navdata runways/nearby airports); per ND side a `ControlPanel` driven at 0.5 Hz (`on_real_time`) with FWC phase, sim time, ppos words, FMS origin/dest/altn/ldg runway, rops topics, reset panel, `nd_show_oans`, `oans_symbols_for_feature_ids`; feed `oans_display_airport` back to both panels; MFD click handlers call the panel methods; OANC calls `Oans::update_rwy_ahead_advisory` once per position update with the runway-element layer polygons and wall-clock ms, and routes `oans_add/remove_*_at_feature` to `MarkerManager`.

## Out of scope / left

Canvas rendering of layers (the native ND paints from AMDB), drawing the control panel pages, DMS lat/long strings on ARPT SEL, LDG SHIFT values (FBW does not use them), turf-based union/simplify of AMDB layers.
