# Native FMS Phase 3: LNAV geometry

Crate `crates/fms_lnav` (standalone, std plus serde_json, no X-Plane dependency; `cargo test --manifest-path crates/fms_lnav/Cargo.toml`). `crates/fms` is not touched; the plan crate feeds `Plan` / `LegInput` through one adapter point (`input.rs`).

## Source map (fbw-a32nx/src/systems/fmgc/src/guidance)
- geodesy: own `geo.rs` (great-circle, small-circle intersections), not a translation of msfs-geo/MathUtils.
- legs: `lnav/legs/{IF,TF,DF,CF,RF,XF,Leg}.ts` -> `geometry.rs` (legs section)
- transitions: `lnav/transitions/{FixedRadiusTransition,PathCaptureTransition}.ts`, `utilss/CourseChange.ts`, `TransitionPicker.ts` -> `geometry.rs` (transitions section)
- path vectors, sequencing, distances, guidance selection, RAD: `Geometry.ts`, `lnav/CommonGeometry.ts`, `lnav/PathVector.ts` -> `geometry.rs`, `common.rs`
- LNAV driver laws (capture condition, forced turn, sequencing area): `lnav/LnavDriver.ts`, `lnav/HpathLaw.ts` -> `driver.rs`

## Steps
- [x] geo.rs, common.rs, input.rs
- [x] legs IF/TF/DF/CF/RF
- [x] FixedRadius (with revert) + PathCapture + picker for XF->XF
- [x] Geometry: multi-pass compute, path vectors, completeLegPathLengths, shouldSequenceLeg, guidance
- [x] Comparator against `E:/fbw-build/fms-oracle/uplink-egll-kjfk.json` (ACTIVE vectors in the 82 EFIS vector events)
- [x] TransitionPicker.ts full table -> `picker.rs` (all 19 leg kinds, 6 transition kinds, same-waypoint rule for TF/DF -> FC/FD)
- [x] CI leg (CI.ts: Geo.legIntercept, flipped/after-fix correction, null leg) and CourseCaptureTransition (CourseCaptureTransition.ts, bank from maxBank(tas, false), forced-turn guidance)
- [x] Comparator `tests/flight.rs`: guidance (XTK, TAE, phi command), sequencing and EFIS ACTIVE vectors along `flight-egll-kjfk-sid` / `-star`
- [x] FA/FC/FM legs (FA.ts, FC.ts, FM.ts; FA start altitude is passed in by the adapter, not read from the inbound leg's altitude descriptor)
- [x] CA leg (CA.ts: start moves to PPOS once active unless the inbound IF is a runway, termination estimated at 2000 ft/min and 175 kt from altitude 0) and CR leg (CR.ts: intercept with the navaid radial, null beyond 5000 nm)
- [x] Phi limit per active guidable (Geometry.ts isGuidableCapturingPath: roll limit 1 for CA/CI/CR/VM legs and course capture, else roll limit 2); CA after a course capture stays the active guidable
- [x] `place_bearing_intersection` returns the intersection along the first bearing first, as msfs-geo does (CF start estimate, CI/CR intercepts)
- [ ] VM/FD/AF/CD/PI/HX legs and DirectToFix, DmeArc, HoldEntry transitions (picker already selects them; geometry skips slots for kinds not built yet). VM needs a HEADING law in `LateralPath`.
- [ ] GeometryFactory rules not yet in the adapter: XI-IF-XF null IF (previous leg i-2), EOSID, missed-approach transition limit, course reversals (PI)
- [ ] Hold geometry, pseudo waypoints, nav-capture / roll law wiring into the `fly` scenario op

## Oracle flight ops
- `fly` step in `src/js/msfs/fms_oracle.rs`: `{ "fly": { "start": "plan" | {lat, lon, track}, "kt": 250, "seconds": 900, "until_leg": 8, "sample_ms": 1000 } }`. Writes PLANE/GPS position, GPS GROUND TRUE TRACK/SPEED, AIRSPEED TRUE, heading and the three ADIRS IR position/track/speed/heading words each 50 ms step, rolls the aircraft with the FG law (`phi = phi_command + K2 * (K1 * xte + gs * clamp(tae, 45))`, 5 deg/s, limit L:A32NX_FG_PHI_LIMIT) and adds a `trace` array (position, track, gs, tas, FBW active leg index, FG xte/tae/phi) to the golden only when a scenario flies, so existing goldens are byte identical.
- Scenarios: `flight-egll-kjfk-sid` (first EGLL runway and SID, to the SID end), `flight-egll-kjfk-star` (STAR + approach from the first STAR IF).

## Oracle fidelity fixes
- The oracle VarStore ignores units. CommonGeometry.ts `arcGuidance` reads `GPS GROUND SPEED` in metres per second, so the old recordings fed knots as m/s and FBW's arc phi command came out 35 deg instead of 10.5 deg (the "CI -> CF turn mismatch" in the old notes). `Recording::get_var` in fms_oracle.rs now converts `GPS GROUND SPEED`, `GROUND VELOCITY` and `AIRSPEED TRUE` to m/s when asked for that unit. The `flight-*` goldens predate this; use the `flight2-*` ones.
- GuidanceController.ts recomputes the geometry only on a plan change or every 5 s (GEOMETRY_RECOMPUTATION_TIMER), and Geometry.recomputeWithParameters does one pass. The CI intercept, the CF start and the CI -> CF fixed radius turn feed each other, so recomputing every sample drifted the turn ~0.1 nm. The comparator now recomputes on leg change or after 5 s, like FBW.

## Suspected FBW errors (ported faithfully, not fixed)
- GeometryFactory.ts passes `SegmentType.Departure` to every leg it builds, so `minBank(segment)` (CommonGeometry.ts:120) never sees `Enroute` and the minimum bank is always 10 deg, never 5 deg. The crate adapter therefore sets `enroute = false` for every leg.
- LnavDriver.ts:63 `fmgc2DiscreteWord2Var` reads `L:A32NX_FMGC_1_DISCRETE_WORD_2` (FMGC 1 word used for FMGC 2).
- CA.ts `recomputeEstimatedTermination` takes `originAltitude = 0` (constant), 2000 ft/min and 175 kt, so a CA leg at a high airport is too long and ignores the aircraft's real climb performance.
- FA.ts `getGuidanceParameters` measures cross track from the line fix -> termination even when an inbound transition moved the path start, so XTK jumps by the offset.
- CI.ts `getPathStartPoint` throws when the inbound guidable is not computed yet; a geometry update ordering problem becomes an exception.
- CourseCaptureTransition.ts rounds the track change to 0.1 deg before building the arc, leaving up to 0.05 deg of course error at the final turning point.
- TransitionPicker.ts: `fromHX` logs "Illegal sequence DFLeg" (copy and paste), `fromFM` -> CF/FA/FM and `fromVM` -> FA/FM return null without a log although no transition leaves the legs unconnected, `fromRF` -> TF/CF returns null so an RF leg followed by an XF leg has no transition and relies on the leg start being the RF end.

## Comparator results, lnav36 (tests/flight.rs; new goldens in `E:/fbw-build/lnav36/out`, scenarios `tools/fms_oracle/scenarios/flight2-*.json`)
- `flight2-egll-kjfk-sid` (units fixed): 350 guidance samples compared; mean XTK difference 0.0017 nm; over tolerance xte/tae/phi/phi-limit 1/0/1/1 (one RAD sample at t=84 s and one transition switch at t=86 s, both at the CI -> CF turn, one sample apart); EFIS ACTIVE vectors equal at 350/350 samples.
- `flight2-egll-kjfk-arr-hold` (STAR SIE..CAMRN with a pilot hold at BOTON, 220 kt, 420 s): FBW sequences into the HM leg; we compare the TF leg before it (410 samples, all within tolerance except the first sample after setActiveLegIndex and the last 5 samples where FBW's RAD rolls into the hold entry). EFIS vectors 0/410 because the HM leg and HoldEntry transition are not built yet.
- `flight2-egll-kjfk-arr-app` (STAR SIE..CAMRN, 220 kt, 720 s; only reaches leg 5, the STAR is long) and `flight2-egll-kjfk-app-missed` (ILS 04R from the leg after BEBKE, 160 kt, 360 s, into the missed approach): guidance agrees except the first sample after setActiveLegIndex (718 and 358 samples). EFIS 0/718 and 0/358 for two reasons still open: (a) the adapter does not mark missed-approach legs, so our ACTIVE list runs on into CA/VI/CF/HM while FBW stops at the runway (needs `missed_approach` from the plan's firstMissedApproachLegIndex); (b) the first TF -> TF fixed radius turn is drawn with FBW's pre-flight TAS (160 kt minimum) until FBW's 5 s timer fires, while the comparator's first recompute uses the flown TAS. Seeding the comparator with a TAS 0 recompute fixes (b) for the arrival but breaks 6 SID samples, so FBW's timer phase needs recording in the trace first.
- The old `flight-egll-kjfk-star` scenario made the approach IF the active leg, so FBW never sequenced and the trace stays on one leg; the `flight2-*-arr-*` scenarios activate the leg after the first IF.

## Comparator results, lnav28 (goldens `E:/fbw-build/fms-oracle/flight-egll-kjfk-{sid,star}.json`)
- SID (EGLL 09L BPK5K, IF/CF/CI/CF/CF/TF/TF/TF): 346 guidance samples compared at 1 s; mean XTK difference 0.004 nm; XTK within 0.05 nm at 334, TAE within 1 deg at 323, phi command within 1 deg at 318 samples; leg sequencing agrees to within one sample at every leg; EFIS ACTIVE vectors equal (2e-3 deg) at 191/346 samples.
- Open: the two CI -> CF / CF -> CF turns at t=80-100 s. FBW switches to the transition ~1 s later than we do on leg 2 (CI) and flies a tighter arc on leg 3 (phi 35 against our 10.5), so FBW's transition radius or turn-start differs; trying TAS 160 for the geometry made it worse, so the cause is not the minimum predicted TAS. Suspect FBW's per-leg predictedTas/predictedGs, which the trace now records (`pred` per sample) and `Geometry::set_predictions` can feed, but they were null in the offline run.
- STAR/approach flight: the first 4 elements are supported; all guidance samples agree except the first sample after the oracle's setActiveLegIndex (FBW reports xte -2993 nm for one sample). EFIS vectors are not comparable until the remaining leg types exist (our vector list is truncated at the first unsupported leg).
- Determinism: two independent recordings of `flight-egll-kjfk-sid` are byte identical (`tools/fms_oracle/same.py`: 1/1 identical).
