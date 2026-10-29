# Native FMS Phase 4: VNAV and predictions (crates/fms_vnav)

Standalone crate, no X-Plane dependency, no dependency on `crates/fms` or `crates/fms_lnav` (own input types; an adapter fills them later).
Test with `cargo test --manifest-path crates/fms_vnav/Cargo.toml` (CARGO_TARGET_DIR=E:/fbw-build/target).

Source: `fbw-aircraft/fbw-a32nx/src/systems/fmgc/src/guidance/vnav` (FlyByWire's A380X uses the same code; the A380 data is `flightplanning/A380AircraftConfig.ts`).
No `*.spec.ts` exists for vnav in fbw-aircraft, so there is nothing to translate there; the Rust tests are new.

## Tasks

1. Atmosphere and speed conversions (`common.ts`) - done: `src/atmosphere.rs`.
2. A380 flight model and engine data (`A380AircraftConfig.ts`) - done: `src/config.rs`.
3. Engine model (`EngineModel.ts`) and flight model (`FlightModel.ts`) - done: `src/engine.rs`, `src/flight_model.rs`.
4. Step predictions (`Predictions.ts`, all 10 step functions) - done: `src/predictions.rs`.
5. Climb strategies (`ClimbStrategy.ts`) - done: `src/strategy.rs`.
6. Profile checkpoints, interpolation, computation parameters (`BaseGeometryProfile.ts`, `NavGeometryProfile.ts` types, `VerticalProfileComputationParameters.ts`) - done: `src/profile.rs`.
7. MCDU speed profile (`SpeedProfile.ts` McduSpeedProfile) - done: `src/speed_profile.rs`.
8. Takeoff path (`TakeoffPathBuilder.ts`) - done: `src/takeoff.rs`.
9. Climb path with altitude and speed constraints (`ClimbPathBuilder.ts`) - done: `src/climb.rs`.
10. Cruise path without steps (`CruisePathBuilder.ts`) - done: `src/cruise.rs`.
11. Comparator against `E:/fbw-build/fms-oracle` - done for what the goldens record: `tests/oracle.rs`.

## Not yet translated

- `VerticalProfileManager.ts` beyond `computeTacticalMcduPath` before the climb phase (present-position start, the tactical descent branch, `computeDescentPath`, ND/expedite/selected profiles), `VnavDriver.ts`.
- `TacticalDescentPathBuilder`, descent guidance laws (`DescentGuidance`, `LatchedDescentGuidance`, `TodGuidance`, `AircraftToProfileRelation`, `SpeedMargin`).
- Winds: `ConstantWindProfile` (selected-mode profile; not needed by the managed path).
- Fuel and EFOB pages, FM messages, EFIS symbols, flight phase manager (the rest of Phase 4).

## Translated in the 22 Oct wave 2

- `src/managed.rs`: `FmgcDataService.getManagedClimbSpeed/Mach`, `getManagedCruiseSpeed/Mach`, `getManagedDescentSpeed/Mach` (A380X `MFD/FMC/fmgc.ts`), and all of `CostIndex.ts` (specific range, Mmrc, econ Mach, climb and descent CAS).
- `src/descent.rs`: `DescentStrategy.ts` (`IdleDescentStrategy`), `TemporaryCheckpointSequence.ts`, `IdlePathBuilder.ts`, `GeometricPathPlanner.ts`, `GeometricPathBuilder.ts`, `DescentPathBuilder.ts` (managed descent path, `evaluateAltitudeConstraint`, `isAltitudeConstraintMet`).
- `profile.rs`: `AltitudeDescriptor`, `AltitudeConstraint`, and `DescentAltitudeConstraint` now carries the constraint and the leg index.

## Comparator findings

- All four goldens record `--:--` / `---` / `---.-` for time, speed, altitude and EFOB: the golden state has no ZFW/FOB, so `canComputeProfile()` is false and FBW draws no predictions. `tests/oracle.rs` checks that `ComputationParameters::can_compute_profile` is false for every recorded snapshot and that no time cell holds a value. A real numeric comparison against the 1 ft / 0.5 kt / 1 s / 10 kg tolerances needs a new scenario that enters ZFW and block fuel; the oracle then also needs managed speeds (see above).
- The golden `totalFlightPlanDistance` is 3580.45 NM (MFD DEST cell 3580), the plan legs' cumulative distance is 3572.5 NM to KJFK; that difference is LNAV/FAF handling, not vnav.

## FBW quirks kept on purpose

- `verticalSpeedStep` and `verticalSpeedDistanceStep` pass the accel factor mode enum (0 or 1) as the accel factor to `getThrustFromConstantPathAngle`.
- `VerticalSpeedStrategy.predictToAltitude` passes `config.gearExtended` as the speedbrake flag.
- `climb_thrust_corrected_n1` returns the top-row values above the last altitude row (FBW divides by zero there).

## Predictions golden (predictions-egll-kjfk)

Scenario `tools/fms_oracle/scenarios/predictions-egll-kjfk.json`: canned EGLL-KJFK uplink, ZFW 330 t, ZFWCG 38.5 %, block fuel 95 t, CI 30 (all through `fps.setPerformanceData`, the call the MFD input fields make), CRZ FL360 through `fmc.trySetCruiseFl`, F-PLN page open, then a dump of `vnavDriver.profileManager.mcduProfile` (checkpoints and waypoint predictions) published as the bus event `oracle_vnav_dump` (recorded in `events`), once before and once after climb/descent/cruise winds. Recorded twice into scratch dirs, byte-identical (`same.py` 1/1). No change to `fms_oracle.rs` or the snapshot script.

Comparator (`tests/predictions.rs`, asserts 0.01 NM / 1 s / 1 ft / 0.5 kt / 10 kg on every checkpoint from the thrust reduction point to the cruise end, in the golden's own distance frame; `FMS_VNAV_DUMP=1` prints both checkpoint lists):

- Takeoff, climb and cruise (checkpoints 0 to 29, liftoff to the cruise end at 3541.2 NM): distance, time, altitude, speed and fuel identical (0.0000 NM, 0.000 s, 0.000 ft, 0.000 kt, 0.000 kg).
- Cause of the earlier 18 ft climb altitude and 12.5 kg cruise fuel differences (22 Oct, vnav31): `TakeoffPathBuilder.buildPathToThrustReductionAltitude` reads the TOGA N1 from `L:A32NX_AUTOTHRUST_THRUST_LIMIT_TOGA`, which the headless oracle never sets, so FBW predicts the liftoff-to-thrust-reduction step with N1 0: the step comes out backwards (-4.73 NM, -105 s) with no fuel burned. The crate used 90 % and started the climb 600 lb lighter, which moved every 1500 ft step 0.01 to 0.06 NM earlier (the 18 ft at matched distance) and carried into the cruise burn. With `takeoff_toga_n1 = 0` in the comparator the whole path is bit for bit equal, and the takeoff checkpoints land at -4.73 NM like the golden, so no present-position shift glue is involved in preflight (`computeTacticalMcduPath` only adds a PresentPosition checkpoint from the climb phase on).
- Idle descent from the recorded `IdlePathEnd` back to the TOD: distance, time, altitude and fuel identical (0.0 s, 0.0 ft, 0.0 kg) over all 22 checkpoints.
- The 8.6 t climb fuel seen earlier was a 470 t aircraft (350 t ZFW plus 120 t fuel); at 330 t ZFW plus 95 t FBW itself burns 7.2 t to FL360 and the crate matches it. The climb is not low relative to the golden.
- The TOC checkpoint speed differs by 20 kt only because the comparator matched the duplicate-distance point after the TOC; the TOC point itself is 290.0 kt in both.
- Winds do not reach the recorded predictions because the scenario wrote the wind vectors as `Float64Array([x, y])`; FBW's `WindVector` is `{ direction (radians), magnitude (kt) }` (`flightplanning/data/wind.ts`). With `direction`/`magnitude` undefined, `isWindVectorComplete` is false, `WindUtils.dot` returns 0 and every tailwind is 0; the 5e-5 s changes in the WIND dump are noise. Fixed scenario: `tools/fms_oracle/scenarios/predictions-egll-kjfk-winds.json` (same steps, vectors as `{ direction: deg * PI / 180, magnitude }`). Not recorded yet; record with `FMS_ORACLE_ONLY=predictions-egll-kjfk-winds FMS_ORACLE_OUT=<scratch dir>` and `cargo test --release --features js --lib fms_oracle::tests::record_fms_oracle -- --ignored`, twice, then copy to `E:/fbw-build/fms-oracle`. `fps.setClimbWindEntry` through `FlightPlanService` leaves `climbWindEntries` empty on the A380X active plan (draft winds, `DRAFT_ON_WIND_EDIT`); the scenario sets the arrays with `setPerformanceData` instead.
- Takeoff checkpoints: the golden TRA/ACC at -4.73 NM, -105 s are the N1 0 artifact above, not a shift behind the present position.
- Per-leg F-PLN predictions and the TOD are now compared (vnav37 section below); decel and landing still need `ApproachPathBuilder` (golden decel 3651.6 NM, total 3659.5 NM).

## 22 Oct, vnav37: winds golden, WindProfile, F-PLN waypoint predictions

- Winds golden recorded: `predictions-egll-kjfk-winds.json` (release `--features js` build, two runs into `E:/fbw-build/vnav37/rec1` and `rec2`, `same.py` 1/1 identical, copied to `E:/fbw-build/fms-oracle`). Climb and descent winds reach the predictions (TOC 110.04 -> 111.32 NM, TOD 3541.20 -> 3544.77 NM). The cruise wind entry from `fps.addCruiseWindEntry(3, ...)` does not: every waypoint after the TOC is shifted by the same -9.5 s as the TOC, so the cruise is flown in zero wind (draft cruise winds, like the climb winds through `setClimbWindEntry`). The snapshot serialises the climb/descent wind entries as `null`, so the comparator takes them from the scenario values.
- `src/wind.rs`: `WindProfile.ts` (climb, cruise, descent forecasts, trip wind, measurement blending), `WindUtils.ts`, `FlightPlanTrackProfile.ts` (including its leg-index cache), `LegWinds` and `BaseFlightPlan.propagateWindsAt` + `sortWindEntriesByAltitude`, `scaleWindVector`. Implements the crate's `Winds` trait. The measurement (`WindObserver`) is an `Option<WindEntry>` filled by the adapter.
- `src/waypoints.rs`: `NavGeometryProfile.computePredictionsAtWaypoints` (interpolate at each leg's `cumulativeDistanceWithTransitions`, `casOrMach`, EFOB, alt error, speed-constraint check).
- Crate fix found by the waypoint comparison: `CruisePathBuilder`'s level acceleration checkpoint now keeps the previous checkpoint's Mach (`TemporaryCheckpointSequence.addCheckpointFromStep` copies `lastCheckpoint.mach`); the crate had written the cruise Mach, which only showed in the Mach interpolated at the cruise waypoints.
- `tests/predictions.rs` compares all 13 recorded F-PLN waypoint predictions (time, EFOB, speed or Mach, altitude) and the TOD in the plan's distance frame, for both goldens. The comparator builds takeoff, climb and cruise with the crate, the idle descent with the crate from the recorded idle path end, and appends the recorded approach (Decel to Landing) because `ApproachPathBuilder` is not translated yet. Plan distances: `cumulativeDistanceWithTransitions` is the running sum of `distanceWithTransitions`, plus the direct `cumulativeDistance` jump across a discontinuity (gives 3659.475 NM at KJFK, the recorded total). Origin/destination altitudes are the airport locations (EGLL 77.25 ft, KJFK 12.625 ft), not `departureElevation` (0 in the oracle): with 0 ft the climb wind scaling put the climb 0.002 to 0.007 NM off.
- Results, no wind: legs 0 to 12 time 0.000 s, EFOB 0.000 kg, speed 0.000 kt / Mach 0.00000, altitude 0.000 ft; TOD 3541.197 NM equal. Winds: the same, all zero to 0.001; TOD 3544.769 NM equal (idle descent with the descent winds exact). Leg 14 (KJFK), decel (3651.588 / 3651.744 NM) and landing (3659.475 NM) are the recorded approach values, not yet computed.
- Still not translated: `ConstraintReader.ts`, the rest of `NavGeometryProfile` (cruise step list, `ignoreCruiseStep`), `VerticalProfileManager.ts`, `CruiseToDescentCoordinator.ts` (the comparator still takes the cruise length from the golden), `ApproachPathBuilder.ts`, `TacticalDescentPathBuilder.ts`, cruise steps, `targetSpeed` on deceleration checkpoints.

## 22 Oct, vnav46: whole profile native (approach, coordinator, constraint reader, cruise steps)

- `src/approach.rs`: `ApproachPathBuilder.ts` (landing, final segment to 1000 ft AGL, FAF deceleration, altitude constraints with the bisection on the deceleration distance, speedbrakes retry, TOO STEEP PATH, speed constraints, flap checkpoints, DECEL), `FlapConfigurationProfile`, `AircraftConfigurationRegister.setFromSpeed`, `BisectionMethod.ts`. `Checkpoint` now carries `target_speed` (set by `addDecelerationCheckpointFromStep`: FULL 148, 3/2 = F speed, 1 = S speed, DECEL = O speed). Too-steep legs are returned as leg indices (FBW sets `leg.calculated.endsInTooSteepPath`).
- `src/coordinator.rs`: `CruiseToDescentCoordinator.ts` (destination EFOB/time estimate kept between computations, 4 iterations to 100 lb / 1 s, flight plan too short via `ProfileInterceptCalculator.ts` and `MathUtils.intersect`).
- `src/constraints.rs`: `ConstraintReader.ts` over a plain leg list (`ConstraintPlan`): climb/descent altitude and speed constraints, cruise steps, total distance, final descent angle, FAF distance, final altitude (MAP, runway threshold + 50, airport + 50, 50), `ignoreCruiseStep` (`Profile.ignored_cruise_step_waypoints` reports the legs back to the adapter).
- `src/cruise.rs`: cruise steps (`compute_cruise_path_with_steps`, step climb with climb thrust, step descent with V/S -1000 like `computeTacticalMcduPath`, steps ending within 50 NM of the T/D ignored), `getFinalCruiseAltitude`.
- `src/manager.rs`: `VerticalProfileManager.computeTacticalMcduPath` for the preflight/takeoff phases (takeoff, climb, coordinator, `finalizeProfile` sort).
- Comparator (`tests/predictions.rs`): nothing taken from the golden any more except the scenario inputs. The whole profile is computed natively and all 64 checkpoints are compared by index (reason, distance, time, altitude, speed, fuel) plus all 13 F-PLN waypoints. No wind: worst 0.0000 NM, 0.000 s, 0.001 ft, 0.000 kt, 0.001 kg; TOD 3541.197, decel 3651.588, landing 3659.475 NM. Winds: worst 0.0000 NM, 0.000 s, 0.001 ft, 0.000 kt, 0.000 kg; TOD 3544.769, decel 3651.744. The destination estimate is state FBW keeps between recomputations: the oracle computed the no-wind profile twice before the NOWIND dump and once more after the winds (one pass gives 0.012 s / 0.06 kg no wind, 0.08 s / 0.4 kg winds). The old climb/cruise-only and idle-descent-only comparisons, which borrowed the cruise length and the idle path end from the golden, were removed (the whole-profile test covers them).
- `tests/coordinator.rs`: constraint reader, approach checkpoints and target speeds, step climb flown and a late step ignored, whole-profile ordering, bisection.

### Why the cruise wind entry does not reach FBW's predictions

`FpmConfigs.A380.DRAFT_ON_WIND_EDIT` is true. `FlightPlanService.addCruiseWindEntry` calls `BaseFlightPlan.addCruiseWindEntry(atIndex, entry)` with `checkDraftConfig = true`, so `prepareCruiseWindDraftModification` clones the cruise winds into `draftCruiseWindEntries` and the new entry goes into the draft only (`hasDraft` is true, only the version is bumped). `propagateWindsAt` and the VNAV `WindProfile` read `leg.cruiseWindEntries`, which stay empty until `FlightPlan.insertDraftWindEntries()` runs: the MFD WIND page INSERT button (`MfdFmsWindPage.confirmFlightPlanDraftWinds(true)`) or `prepareDestructiveModification` before a lateral revision. The scenario's `if (fps.hasTemporary) await fps.temporaryInsert()` does nothing (no temporary plan). Fix for the next recording: add `fps.active.insertDraftWindEntries();` after `addCruiseWindEntry` (scenario file not touched by vnav46). The climb/descent winds reached the predictions only because the scenario sets them with `setPerformanceData`, bypassing the draft.

FBW quirks found and kept: `CruisePathBuilder.computeCruisePath` passes `sequence.lastCheckpoint.distanceFromStart` as the fuel on board to `levelAccelerationStep` (the crate had used the fuel; only matters when the cruise target is more than 1 kt above the TOC speed, not in the goldens); the step descent passes `managedCruiseSpeed` as the Mach; `insertDraftWindEntries` returns without inserting anything when all four draft kinds exist (the guard was probably meant for none existing).

## Deliberate differences (FBW bugs fixed behind a flag)

`WindFixes::FBW` reproduces FBW and is what the comparator uses (it matches the goldens to 0.001); `WindFixes::ALL` turns on every fix below. `compute_predictions_at_waypoints(.., fix_between)` takes the alt-error fix.

- `climb_lowest_entry`: the A380 stores climb winds highest first (`SORT_CLIMB_WIND_DESCENDING`), `WindProfile.getClimbWindForecast` takes entry 0 as the lowest. Fixed: the lowest-altitude entry is used, so below the highest entry the lower climb entries are interpolated as entered. Confirmed by the winds golden (FBW mode reproduces it exactly).
- `scale_magnitude_only`: `scaleWindVector` multiplies the direction (radians) by the scale as well as the magnitude, so below the lowest entry the wind direction turns towards north (290 deg at 30000 ft becomes 56 deg at 5800 ft), turning a headwind into a tailwind on the westbound EGLL departure. The FBW comment says the scaling is meant to keep the direction. Fixed: only the magnitude is scaled.
- `shortest_arc_direction`: `WindUtils.interpolateWindVector` interpolates directions linearly in radians, so 350 deg and 010 deg average to 180 deg. Fixed: shortest arc.
- `fix_between` (`computeAltError`): for a BETWEEN constraint below altitude 2 FBW returns `predicted - altitude1` (the upper bound). Fixed: `predicted - altitude2`.
- Not translated, logged only: `ConstantWindProfile.getDescentWindForecast` drops the `return` on the interpolated branch, so above the lowest descent entry it always returns zero wind.

## Suspected FBW errors (kept as is)

- `getManagedCruiseSpeed`, `getManagedClimbSpeedMach`, `getManagedDescentSpeedMach` return constants (310 kt, M0.84, M0.85); the code comments say the econ computation is "not mature". `CostIndex.costIndexToClimbCas/DescentCas` clamp to 270..340 kt with A320 weight ranges (50..77 t), so they cannot be used for the A380 (kept for reference, not used by the managed getters).
- `getManagedClimbSpeed` uses `(ci/999)^2` while `getManagedDescentSpeed` uses `ci/999` linear, with different bounds (290..330 versus 288..300): inconsistent between climb and descent.
- `CostIndex.naiveFindMmrc` evaluates `Common.getDelta(altitude)` and theta with the below-tropopause formulas at any altitude.
- `buildAccelerationTargets` dereferences `descentSpeedLimit.underAltitude` without a null check (the crate skips the limit target when there is none).
- A380 climb winds: `FpmConfigs.A380.SORT_CLIMB_WIND_DESCENDING` is true, so `climbWindEntries[0]` is the highest entry, but `WindProfile.getClimbWindForecast` treats `climbWindEntries[0]` as the lowest: below the highest entry it scales that entry linearly down to zero at the origin and never uses the lower climb entries. (Descent winds use the last entry as the lowest, which matches the descending order.) Now fixed behind `WindFixes::climb_lowest_entry`, see above.
- `GeometricPathBuilder.executeGeometricSegments` declares `lastTarget` as a `const null` and passes it into every segment, so the speed-limit/constraint reason of the previous segment is never carried over.
- The A380X golden climb (TOC at 114.8 NM, 16 min, 7.2 t for a 425 t aircraft) is far steeper and cheaper than a real A380 at that weight (roughly 200+ NM, 25-30 min, 12-15 t): to check against the real aircraft before trusting the engine tables for the climb.

## 22 Oct, vnav76: VnavDriver, descent guidance, ND/expedite profiles, winds2 and in-flight goldens

- `src/driver.rs`: `VnavDriver.ts` (recompute, update, reset, shouldUpdateDescentProfile/numberOrNanChanged, didLegsChange, descent speed guidance writing `SPEEDS_MANAGED_PFD`, hold speed, findNextSpeedChange, getPerfCrzToPrediction, invalidateFlightPlanProfile, leg TAS/GS predictions, shouldShowLatDiscontinuityAhead). Holds the profile manager state (mcdu, descent, ND, expedite profiles). SimVar reads come in `RecomputeInputs`/`UpdateInputs`, writes go to `DriverOutputs`.
- `src/guidance.rs`: `AircraftToDescentProfileRelation`, `SpeedMargin`, `TodGuidance` (popups as `TodPopup` events), `DescentGuidance` (path capture states), `LatchedDescentGuidance` (A380 config uses the non-latched one). L:var writes kept in `GuidanceOutputs`.
- `src/nd.rs`: `computeTacticalNdProfile` (Nav vs Selected profile, expedite/MCDU/ND speed profiles, climb/descent strategy for the FCU mode), intercept with the guidance profile, level-segment and next-descent PWPs, expedite climb profile, shouldShowTooSteepPathAhead. `src/modes.rs`: FCU mode numbers and `FcuModeObserver`. `speed_profile.rs`: `SpeedProfile` trait, `ExpediteSpeedProfile`, `NdSpeedProfile`. `ConstantWindProfile` is not translated: callers pass the winds for selected-mode profiles.
- Goldens: `predictions-egll-kjfk-winds2.json` and `predictions-egll-kjfk-inflight.json` copied to `E:/fbw-build/fms-oracle` from ONE successful run each (`E:/fbw-build/vnav61/keep-*`); the second runs failed to compile in other agents' in-progress files (`crates/fms`, `crates/fms_lnav`), so `same.py` was not run. Re-record twice with `bash E:/fbw-build/vnav61/rec.sh` when the lib compiles.
- winds2 (cruise wind confirmed with `insertDraftWindEntries`): cruise winds now reach FBW's predictions (TOD 3544.77 -> 3548.32 NM). Comparator injects the leg-3 cruise entry (snapshot does not serialise it): all 64 checkpoints and 13 waypoints exact (0.0000 NM, 0.000 s, 0.001 ft, 0.000 kt, 0.001 kg), with and without wind.
- in-flight: PREFLIGHT guidance descent profile exact (34 checkpoints, 0.0000 NM / 0.000 s / 0.001 ft); DESCENT-phase tactical MCDU profile (DES below path, V/S -1000) exact (4 checkpoints, 0.0 everywhere) once the parameters carry the oracle's ISA deviation (+59.4, ambient temperature left at 15 C). CRUISE dump not compared: its guidance profile uses approach speeds and cruise geometry the oracle did not snapshot.
- `VnavDriver` on the preflight golden: two recomputes give all 64 recorded checkpoints exactly; relation, ND profile and PERF CRZ T/D consistent.

Suspected FBW error found (kept): `FlightPlanTrackProfile`/`WindProfile` searches the leg list from leg 0, so after sequencing it picks flown legs whose stale `cumulativeDistanceWithTransitions` cover the new frame; in the in-flight golden the descent from leg 12 (track 080) used CPT's track 272, turning a 46 kt tailwind into a headwind. The comparator reproduces it with the snapshot's stale distances.
