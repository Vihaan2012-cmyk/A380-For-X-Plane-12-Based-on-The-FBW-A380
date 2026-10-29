# Native FMS performance and speeds: crates/fms_perf

Date: 22 October 2026. Agent perf26. Standalone crate (own `[workspace]`, no dependencies, no X-Plane): `cargo test --manifest-path crates/fms_perf/Cargo.toml`.

## Sources (FlyByWire GPL, translated)

- `fbw-a380x/src/systems/shared/src/OperatingSpeeds.tsx`: lookup tables, `A380OperatingSpeeds`, `A380SpeedsUtils`.
- `fbw-a380x/src/systems/shared/src/PerformanceConstants.ts`, `OperatingAltitudes.tsx`, `flightphase.ts`, `autopilot.ts` (`VerticalMode`).
- `fbw-a380x/.../MFD/FMC/FmcAircraftInterface.ts`: `updatePerfSpeeds`, `updateManagedSpeed`, `getAppManagedSpeed`, `getHoldingSpeed`, `getManagedTargets`, `getNavModeSpeedConstraint`, `toSpeedsChecks`, `getToSpeedsTooLow`.
- `fbw-a380x/.../MFD/FMC/fmgc.ts` (`FmgcDataService`): gross weight, ECON climb/cruise/descent speeds, speed limits.
- `fbw-a380x/.../MFD/FMC/FlightManagementComputer.ts`: landing/takeoff weight, REC MAX / OPT / EO MAX FL.
- `fbw-a380x/.../MFD/pages/FMS/MfdFmsPerf.tsx` and `pages/common/DataEntryFormats.tsx`: PERF field limits (V1/VR/V2 90..Vmo, FLEX 0..99, CI 0..999), FLEX encoding.
- `fbw-a32nx/src/systems/fmgc/src/flightphase/{FlightPhaseManager,Phase}.ts`: flight phase manager.
- `fbw-a32nx/src/systems/fmgc/src/guidance/vnav/CostIndex.ts`, `descent/SpeedMargin.ts`.

No `*.spec.ts` exists in FBW for any of these; the tests are written from the TypeScript behaviour and the oracle.

## Modules

- `table.rs`: clamped multilinear lookup (independent of msfs-sdk `LerpLookupTable`).
- `constants.rs`: Vmo/Mmo, Vfe, Vmcl, weight limits.
- `isa.rs`: ISA pressure, Mach/CAS conversions.
- `speeds.rs`: `OperatingSpeeds::compute`, `SpeedsUtils` (wind, VMCA/VMCG, Vs1g, GS mini).
- `perf.rs`: `PerfInputs`, `PerfComputer::update_perf_speeds` (en-route and approach speeds, F/S/GD/VLS/VS/VMAX/VFEN/VAPP).
- `managed.rs`: managed speed per phase, ECON climb/cruise/descent, holding speed, speed margins.
- `takeoff.rs`: V speed validation, too-low check, FLEX temperature encoding, discrete word 3 bits.
- `weights.rs`: gross/landing/takeoff weight, REC MAX altitude, OPT/EO MAX FL.
- `cost_index.rs`: CostIndex.ts economy formulas that do not need the engine model.
- `phase.rs`: flight phase manager.
- `outputs.rs`: LVar names and units produced.
- `tests/oracle.rs`: comparator against `E:/fbw-build/fms-oracle/*.json` (SPEEDS_* LVars at the first FMS update) and `E:/golden-tour/flight-v4` captures (phase, flap handle, weight and CG words).

## Outputs for src/prim.rs

Listed in `outputs.rs` (`PERF_LVARS`) and the final report.

## Boundaries

fms2 owns flight plan and performance data storage (`crates/fms`), lnav26 `fms_lnav`, vnav26 `fms_vnav`. This crate takes plain values as inputs (V speeds, CI, speed limits, weights) and returns values; it never reads their types.
