# FMS completion and flying-realism audit (20 Oct 2026)

Scope: the A380's FMS as it runs in the X-Plane port (FlyByWire's shared FMGC in
`fbw-a32nx/src/systems/fmgc`, configured by `A380AircraftConfig.ts`, the A380 FMC in
`fbw-a380x/src/systems/instruments/src/MFD/FMC`, and the MFD pages), and the 22
flying-experience items the user listed. Status is from reading the code; nothing in
this document was re-verified in the sim unless it says so.

Legend: **done** = implemented and believed correct, **partial** = implemented with known
gaps, **missing** = not implemented, **verify** = implemented but never checked in X-Plane.

## Part A - FMS

### A1. How the FMS is assembled

- One FMC instance (`FlightManagementComputer.ts`, master only). The real aircraft has
  three FMCs (A, B, C) with master/slave/standby reversion; FlyByWire stubs this
  (`FmcService.ts:99`, `FlightManagementComputer.ts:310`).
- Shared FMGC with the A380 configuration: VNAV descent mode NORMAL, latched descent off,
  A380 drag polars and engine tables (`A380AircraftConfig.ts`).
- Our port adds three runtime patches to `mfd.js` (`plugin/src/source_patch.rs`): flight
  plan backup/restore, a guard on VNAV's climb-thrust table lookup, and the runway-spawn
  phase check. Everything else is FlyByWire's code.

### A2. LNAV (lateral guidance)

| Area | Status | Notes |
|---|---|---|
| ARINC 424 legs AF CA CD CF CI CR DF FA FC FD FM HA/HF/HM IF PI RF TF VM | done | `guidance/lnav/legs` |
| VI, VR legs | partial | built as CI/CR (course, not heading); `GeometryFactory.ts:304,312` |
| VA, VD, VM naming | partial | `FlightPlanLegNaming.ts` TODOs |
| Transitions: fixed radius, path capture, course capture, direct-to, hold entry | done | |
| DME arc transitions | partial | approximated (`TransitionPicker.ts:173-178`, `DmeArcTransition.ts` HAX) |
| Holds | partial | IMM EXIT during teardrop/parallel entry, HF/HA entry, PPOS hold not supported (`HX.ts:44-447`, `FplnRevisionsMenu.tsx:110`) |
| Lateral OFFSET | missing | no offset in flight plan or guidance; MFD shows "(N/A) OFFSET" |
| FM position (GPIRS/IRS/radio blend), ANP vs RNP, NAV ACCUR DOWNGRAD | missing | LNAV uses the true aircraft position (`LnavDriver.ts:125,617`, `GuidanceController.ts:291`) |
| RNP / RNP AR display messages to ND | missing | `FlightManagementComputer.ts:1753` |
| VM leg wind correction | partial | `VM.ts:21-67` |
| FLS (FMS landing system, F-LOC / F-G/S beams for non-ILS approaches) | missing | no FLS anywhere in the FMGC |
| GLS approaches | missing | `landingsystem.ts:15` |
| LOC back-course approaches | missing | filtered out (`MfdFmsFplnArr.tsx:141`) |

### A3. VNAV (vertical guidance and predictions)

| Area | Status | Notes |
|---|---|---|
| Climb, cruise, descent, approach path builders; TOD; tactical descent; constraints; step climbs | done | `guidance/vnav` |
| Climb speed limit (SPD LIM 250/FL100) in the speed profile | partial | `SpeedProfile.ts:204` |
| Descent path intercept | missing | "we never compute an intercept" (`VnavDriver.ts:227`): capture when early/late on path is approximate |
| Takeoff and go-around profiles | missing | `VerticalProfileManager.ts:305` |
| Cruise acceleration step reaching past T/D | missing | logged as not implemented (`CruisePathBuilder.ts:132`) |
| Fuel flow model | wrong for A380 | A320 polynomial times 2.8 (`EngineModel.ts:106`); our port's engines are Trent 900 / GP7200 physics models, so predicted fuel and times won't match the engines |
| Predictions use per-leg wind | partial | current wind only (`Predictions.ts:842`, `VnavDriver.ts:582`) |
| Cost-index speeds | partial | A320-style, "needs rework" (`fmgc.ts:331`) |
| Time constraints / RTA | missing | only the FM message text exists |
| Engine-out / drift-down pseudo-waypoint | missing | `FlightManagementComputer.ts:1690` |
| Speed-constraint deceleration | partial | "big hack until VNAV can do this" (`FmcAircraftInterface.ts:1103,1671-1690`) |

### A4. A380 FMC functions

| Area | Status | Notes |
|---|---|---|
| Three FMCs with sync and reversion | missing | single FMC |
| SEC flight plan predictions (fuel, time) | missing | returns null (`FlightManagementComputer.ts:614,703-765`) |
| Alternate fuel estimate | missing | `fmgc.ts:471` |
| Performance factor | missing | `fmgc.ts:511` |
| Landing CG | partial | `FmcAircraftInterface.ts:1385` |
| Company route / alternate route request | missing | `MfdFmsInit.tsx:88-90` |

### A5. MFD pages

Added on 20 Oct: ACTIVE/WIND and the F-PLN WIND revision (FBW #10929), DATA/NAVAID (#9763),
DATA/WAYPOINT (#9786), ATCCOM REQUEST (#9913).

Pages that still do not exist (greyed in the menus): POSITION/REPORT, GNSS and TIME (no layout
reference);
DATA/ROUTE, PRINTER; F-PLN revisions OFFSET (FBW #9393 stores the offset but never flies it)
and CMS; ATCCOM REPORT-MODIFY, EMER.

Pages with gaps: F-PLN (SEC/TMPY predictions, RNP and time columns, DIR TO ETA, PPOS hold,
runway-only approaches), INIT (CPNY/ALTN route, disabled buttons), FUEL&LOAD (SEC
predictions), PERF (speed-limit labels and predictions), POSITION/MONITOR (FMS2, radio
position, GNSS2, GPS deselect), POSITION/NAVAIDS (deselect), DATA/STATUS (MODIFY), SEC INDEX.

### A6. Plan to finish the FMS

Each phase ends with scripted test flights in X-Plane. The plugin logs LNAV XTK/TAE, the
VNAV deviation, and predicted against actual fuel and time.

1. **F0 - Baseline flights (first).** Fly one short route, one long route with steps, and
   approaches (ILS, RNAV(GNSS), an RF-leg RNP, and a hold). Record what LNAV and VNAV
   actually do in our port before changing anything.
2. **F1 - Model correctness.**
   - A380 fuel flow from our own Trent/GP engine model, replacing the A320 polynomial.
   - A380 cost-index speed tables.
   - Climb speed limit.
   - Descent path intercept.
   - Takeoff and go-around profiles.
   - Cruise step past T/D.
   - Per-leg wind in predictions.
   - Proper speed-constraint deceleration.
3. **F2 - LNAV completeness.**
   - VI and VR as true heading legs.
   - Hold edge cases and PPOS hold.
   - A proper DME arc transition.
   - Lateral OFFSET across the flight plan, guidance, ND and MFD.
   - FM position blending (GPIRS/IRS/radio) with ANP, RNP and NAV ACCUR messages.
4. **F3 - Approaches.**
   - FLS: F-LOC and F-G/S beams computed by the FMS, PFD and ND display, and the AP modes.
   - GLS and LOC back course.
5. **F4 - FMC functions.**
   - SEC predictions, alternate fuel, performance factor.
   - RTA and time constraints.
   - The drift-down pseudo-waypoint and engine-out predictions.
6. **F5 - MFD pages.**
   - WIND (climb, cruise and descent winds; feeds F1 predictions).
   - POSITION/GNSS, TIME and REPORT.
   - DATA/WAYPOINT, NAVAID and ROUTE (pilot-stored).
   - F-PLN OFFSET and WIND revisions.
   - INIT CPNY RTE (SimBrief route uplink) and ALTN RTE.
   - PERF labels; FUEL&LOAD SEC predictions; the remaining disabled buttons.
7. **F6 - Optional, large.**
   - Three FMCs with sync and reversion.
   - ATCCOM/CPDLC (needs a network such as Hoppie).
   - PRINTER.

## Part B - The 22 flying-experience items

| # | Item | Status | What exists | Plan |
|---|---|---|---|---|
| 1 | Ground handling (friction, tiller curve, breakout, stop rock) | partial | FBW steering conditions, our handling module, ground bounce fix | Measure turn radius, tiller/pedal authority and breakout against Airbus data; tune; add the pitch rock when stopping |
| 2 | Brake fade and carbon brake heat | partial | deep `gear_structure/brakes.rs` brake temperatures | Verify the heat build-up in taxi against the BTMS; add friction fade with temperature |
| 3 | Spool lag, idle asymmetry, flight-idle spool-up | partial | our physics engine spools genuinely | Verify acceleration times (idle to TOGA about 8 s); add per-engine variation and the flight-idle/approach-idle difference |
| 4 | Airbus-correct roll and pitch rates, protections | verify | FlyByWire's real PRIM laws | Measure rate per stick deflection and the protection onset in test flights; fix where X-Plane's response differs |
| 5 | Gear and flap vibration (camera and sound) | missing | buffet removed 14 Oct, shake cut 80% | A subtle camera vibration model by configuration and speed, plus sound |
| 6 | Runway roughness and touchdown feel | partial | ground bounce fix, sound triggers | Oleo firmness tuning, spin-up chirp, de-rotation moment, runway roughness from surface type |
| 7 | Crosswind behaviour on the ground | missing | X-Plane flight model only | Tune weathervaning (fin and gear side force), decrab and drift; test against crosswind limits |
| 8 | Ground effect and flare float | missing | X-Plane flight model only | Tune the acf ground effect for the A380 span; verify flare float distance |
| 9 | Wake turbulence from AI traffic | missing | nothing in our plugin | Check X-Plane 12's own wake model first; add our own encounters only if needed |
| 10 | Graded turbulence (chop, moderate, CAT) | partial | buffet/turbulence shake | Graded intensity from weather, with camera motion |
| 11 | Full ADIRS alignment by latitude, degraded nav | verify | FlyByWire ADIRS with our inertial physics (vertical drift fixed today) | Verify alignment times by latitude and degraded nav when misaligned |
| 12 | Engine start realism (hung, hot, no-light, auto-abort) | partial | FADEC start sequence; hung start via a starter-clutch failure | Add hot and no-light starts with the auto-abort logic, as random and scheduled failures |
| 13 | APU (start, EGT, bleed, generator) | done/verify | FlyByWire's Rust APU and deep APU | Verify timings |
| 14 | Fuel system (transfers, trim tank, fuel temperature) | done/partial | FlyByWire plus our fuel network and transfer | Verify CG trim logic; add fuel temperature |
| 15 | Pressurisation | done/verify | FlyByWire's A380 CPCS | Verify cabin rates and the FMS landing elevation |
| 16 | Hydraulics | done | FlyByWire Rust plus the reservoir fix | Verify |
| 17 | Electrical (transfers, display flicker, emergency config, RAT) | partial | FlyByWire Rust plus deep electrical, RAT | Add display flicker and blanking on bus transfers |
| 18 | Brakes, autobrake, BTV, RTO | done/verify | FlyByWire Rust autobrake and BTV | Verify deceleration rates and BTV exit targeting |
| 19 | Weather radar and PWS | partial | our WXR | Verify gain and tilt; add PWS alerts |
| 20 | TCAS RAs | verify | FlyByWire TCAS computer reading X-Plane traffic | Verify RA geometry and aurals with AI traffic |
| 21 | EGPWS modes 1-7 | partial | fbw-common Rust EGPWS module, wired into the A380 systems | Check which modes and callouts it covers; add the missing ones |
| 22 | ROW/ROP | partial/verify | flight warning system references | Verify the alerts on landing |

Suggested order: frame-rate work first, since the ND re-upload costs every flight. Then
feel (1, 3, 4, 6, 7, 8), then F0/F1 of the FMS, then the systems verification pass
(11-18), then surveillance (19-22), then the rest of the FMS phases.
