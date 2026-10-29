# Native FMS Phase 2: crates/fms, flight plan and uplink

Date: 22 October 2026. Spec: `docs/superpowers/specs/2026-10-21-native-fms-mfd-design.md`. Oracle: `E:/fbw-build/fms-oracle/*.json` (navdata JUN13JUL10/24).

## File map

- `crates/fms/Cargo.toml`: workspace member, no X-Plane dependency (serde, serde_json only).
- `crates/fms/src/geo.rs`: `Coordinates`, great-circle bearing and distance.
- `crates/fms/src/navdb.rs`: `Fix`, `Runway`, `Procedure`, `Airway`, `trait NavDatabase`, `MemoryNavDb` (fixtures).
- `crates/fms/src/plan.rs`: legs, elements, segments, `FlightPlan` (FlightPlan.ts, BaseFlightPlan.ts, FlightPlanLeg.ts, segments/*.ts).
- `crates/fms/src/service.rs`: `Fms` (FlightPlanService.ts, FlightPlanManager.ts): active, temporary, uplink and secondary plans; `Command`.
- `crates/fms/src/uplink.rs`: `Ofp`, route chunks, uplink (SimBriefUplinkAdapter.ts).
- `crates/fms/tests/oracle.rs`: comparator; replays `tools/fms_oracle/scenarios/*.json` steps and compares with the goldens.
- `crates/fms/tests/fixtures/navdb.json`: the fixes and airways the goldens use, taken from the goldens' `def.waypoint` records.
- `src/fms_navdb.rs`: `NavDatabase` over `src/navdata::NavData` (its public facility-call API only).

## Command enum (`service::Command`)

`NewCityPair`, `SetOriginRunway`, `SetDestinationRunway`, `SetDeparture`, `SetArrival`, `SetApproach`, `NextWaypoint`, `InsertWaypointBefore`, `DeleteElementAt`, `InsertDiscontinuityAfter`, `SetPilotAltitudeConstraint`, `SetPilotSpeedConstraint`, `AddOrUpdateCruiseStep`, `TemporaryInsert`, `TemporaryDelete`, `SecondaryCopyFromActive`, `ActiveAndSecondarySwap`, `Uplink`, `UplinkInsert`, `DirectToLeg`, `DirectToWaypoint`, `NewDest`, `EnableAltn`, `Sequence`.

## Data types

- `Fix`: database id, ident, ICAO code, location, section codes, airport extras (transition altitude and level).
- `Leg`: definition (`LegDefinition`), ident, annotation, effective type, flags, constraint type, pilot constraints, cruise step.
- `Element`: `Leg(Leg)` or `Disco`.
- `Segment`: kind (12 kinds in FlightPlan order), elements, strung.
- `FlightPlan`: segments, active leg index, airports, runways, procedures, alternate plan, flags, performance data.

## Comparator

For each snapshot it compares plan by plan: origin, destination, active leg index, leg count, then per leg: disco, ident, type, flags, waypoint descriptor, constraint type, altitude and speed constraint, annotation, procedure ident, overfly, position (1e-6 deg), database id, true track (0.01 deg). Leg distances (`calc.*`) belong to Phase 3 geometry and are not compared yet.

## Phase 2b: revisions, alternate, holds, winds, saving (22 October)

- `crates/fms/src/revisions.rs`: `sequence()` and `stringMissedApproach`, DIR TO (`directToLeg`, `directToWaypoint`, `cleanUpAfterDiscontinuity`), `newDest`, `enableAltn`, `deleteAlternateFlightPlan`, origin/destination default performance data (thrust reduction, acceleration and engine-out acceleration at elevation + 1500 ft, FBW's `CONFIG_*` defaults), the engine-out departure segment, and saving.
- The alternate plan's origin now follows `setDestinationAirport`. A deleted alternate is an empty plan, as in FlyByWire.
- `TemporaryInsert` clears the pending flag on a DIR TO turning point and moves it to the aircraft position (`Fms::aircraft`).
- `crates/fms/src/hold.rs`: `HoldData`, `HoldUtils.parseHoldFromProcedureLeg` (set on every procedure leg), `addOrEditManualHold`, `revertHoldToComputed`.
- `crates/fms/src/winds.rs`: climb and descent wind entries (A380 FPM config: 5 climb levels sorted descending, 5 descent, 4 cruise per leg), GRND snapping within 400 ft, PERF APPR wind from a GRND descent entry, alternate wind, cruise winds on legs. Draft winds and cruise-wind propagation are not translated yet.
- `crates/fms/src/waypoint_entry.rs`: `WaypointEntryUtils` (place, runway, lat/lon, PBX, PBD, PD formats; `parsePlace` through `NavDatabase` with a de-duplication callback). Creating the new waypoint is the caller's job (`WaypointEntry` result).
- Engine-out departure: `NavDatabase::engine_out_departure` (default `None`) feeds `FlightPlan::engine_out_departure`. The X-Plane adapter does not provide EOSIDs yet, and `isEngineOutBranch` needs a `ProcedureLeg` field the adapter would have to fill.
- Saving: `FlightPlan::serialize_json` / `deserialize_json` write `{ "format": 1, "plan": ... }` (serde). The `available*` lists, queued operations and pending airways are not saved; `deserialize_json(json, Some(db))` reloads the lists.
- Oracle scenarios added: `dirto-egll-kjfk` (DIR TO a leg, an in-plan waypoint and an off-plan waypoint), `sequence-egll-kjfk` (three sequences, to the MAP, missed approach strung), `altn-egll-kjfk` (set alternate, ENABLE ALTN, alternate after enable, NEW DEST). The comparator now also checks each leg's hold.
