# Fuel jettison and trim-tank CG management (crate `crates/fuel_mgmt`)

Goal: give the A380 a proper jettison controller (target weight, ARM/ACTIVE logic, inhibits, memos and alerts) and a complete automatic CG controller for the trim tank (aft and forward transfers, landing preparation, manual transfers, faults), as pure logic the plugin can drive.

Basis: public descriptions of the A380 fuel system, FlyByWire's published source, and the plugin's own code. No FCOM text was read for this work. Everything below that is not traceable to FlyByWire source or to the physical layout is marked GENERIC (plausible, unsourced, tunable in `JettisonConfig` / `TrimConfig`).

## What exists today

### Plugin fuel model

- `src/fuel_transfer.rs` (`LegacyFuel`, port of FBW `LegacyFuel.ts`): feed-tank and inner/mid transfer triggers, trim-to-feed forward transfer when a feed tank runs low (triggers 1..4, 34), and CG transfer triggers 43/44 against `calculate_cg_target(weight)` (FBW's FCOM-regression polynomial, start at target, stop at target - 1.0). It only ever moves trim fuel forward. There is no aft transfer into the trim tank, no landing-preparation emptying, no manual transfer, no inhibits other than "on ground".
- `src/fuel.rs` + `src/fuel_network.rs`: FBW's `flight_model.cfg` fuel network. `Jettison` struct (FUEL-001) reads a `FUEL JETTISON SWITCH` var, opens `JettisonNozzleValveLeft/Right` instantly, and drains the wing tanks proportionally through one lumped orifice. No target weight, no stop, no ARM/ACTIVE, no inhibit, no source ordering, nothing for the SD or ECAM except "valve open".
- `src/deep/fuel/` (deep model, 5,000 lines): `jettison.rs` (valve transit time, stuck valve, nozzle blockage, orifice flow against head and pump rise), `cg_transfer.rs` (named valve/pump health, achieved-rate model, `TransferFaultDetector`, wing-bending relief, outer-tank retention, wing-balance cross-feed decision), `live.rs` (`tick_jettison`, publishes `FUEL_JETTISON_VALVE_POSITION:n`, `FUEL_JETTISON_FLOW_KG_S:n`, `FUEL_JETTISON_L/R_VALVE_FAULT`, `FUEL_JETTISON_VALVE_NOT_CLOSED:n`). This is plant physics and fault physics. It has no controller above it: `truth.controls.jettison_armed/jettison_valve_selected` are plain switches.
- `src/deep/ecam/fbw/ata28.rs`: ECAM entries already exist for FUEL JETTISON (status memo, driven by nozzle flow > 0), JETTISON FAULT (valve failed closed) and JETTISON VLV NOT CLOSED. There is no COMPLETED memo and no trim-transfer alert.
- `src/weight_balance.rs`, `src/mass_balance.rs`: gross weight and CG % MAC sources.

### FlyByWire A380X (read-only reference)

- Rust `a380_systems/src/fuel`: `cpiom_f` (refuel targets, `trim_tank_targets.toml` for the ground trim-fuel table by ZFW/ZFWCG), `fuel_measuring.rs` (tank centroids, FQMS CG % MAC from tank moments, `A32NX_FQMS_CENTER_OF_GRAVITY_MAC`), `fuel_quantity_data_concentrator.rs`. No jettison logic, no in-flight trim logic.
- Transfers are done by the MSFS fuel system through `LegacyFuel.ts` triggers (the code ported to `fuel_transfer.rs`).
- Jettison is explicitly unfinished upstream: `FuelPage.tsx` has `isJettisonActive = false; // TODO` (nozzle valve drawn amber when open without "active"), `ata28.ts` has the message titles `FUEL JETTISON`, `FUEL JETTISON COMPLETED`, `FUEL JETTISON FAULT`, `FUEL JETTISON VLV NOT CLOSED` (ids 281800052..55) and `FUEL JETTISON (WIP)` procedure with no items, and `MfdFmsFuelLoad.tsx` has a `jettisonGrossWeight` field on the FUEL&LOAD page that is never populated.
- Network topology (flight_model.cfg): jettison nozzle valves 57/58 are fed from `AftGalleryJunction1`; mid and inner tank aft pumps and the trim tank pumps (via `TrimLineIsolationValveAft`) feed that gallery; outer tank pumps feed the forward gallery. Trim tank inlet valves 43 and 59, trim line isolation valves 44 (fwd) and 45 (aft), trim pumps 19/20.

## Gaps against the real aircraft

Confidence tag in brackets: [pub] publicly described, [FBW] visible in FBW data, [GEN] generic/assumed.

### Jettison

| Item | State before | Gap |
| --- | --- | --- |
| Target weight (jettison gross weight entered on the MFD FUEL&LOAD page, default maximum landing weight) [pub, FBW UI field] | none | Stop automatically at the target; completion memo |
| ARM then ACTIVE selection [GEN: overhead panel with guarded ARM and ACTIVE pushbuttons] | one switch | Two-stage logic, latch, deselect resets |
| Two jettison nozzles, one per wing, fed from the aft gallery [FBW] | lumped orifice | Per-side flow, one nozzle failed gives half rate |
| Sources: inner, mid, trim via aft gallery; outer last; feed tanks never jettisoned [FBW topology, GEN ordering] | proportional over wing tanks incl. feed-adjacent | Source weighting, outer after the main tanks |
| Jettison rate [GEN: ~2,500 kg/min total, public figures range 1,000..3,300 kg/min] | 500 gal/h reference | Rate cap per side, taper as sources empty, valve ramp |
| Minimum fuel remaining [GEN: 10 t] | none | Stop at floor or when sources are exhausted ("completed short") |
| CG protection [GEN] | none | Draw trim fuel faster when CG is aft of target, spare it when forward |
| Wing balance [GEN] | none | Throttle the lighter wing's nozzle on imbalance |
| Inhibits [GEN: on ground, flaps/slats beyond CONF 1, loss of power, nothing to shed] | none | Inhibit before start; stop and latch while active; re-select needed |
| Auto trim transfer inhibited while jettisoning [GEN] | none | `inhibits_auto_trim` output |
| ECAM FUEL JETTISON memo, COMPLETED memo, FAULT, VLV NOT CLOSED [FBW titles] | memo from flow, FAULT, VLV NOT CLOSED partly | Drive from controller state; add COMPLETED |
| SD FUEL page: JETTISON label amber when valve open but jettison not active [FBW FuelPage.tsx] | upstream TODO | Publish a real "jettison active" var |

### Trim tank / automatic CG control

| Item | State before | Gap |
| --- | --- | --- |
| CG target as a function of gross weight [FBW regression, FCOM-derived] | `calculate_cg_target` | New crate has its own GENERIC table, plugin may inject the FBW function through `TrimConfig::target_fn` |
| Aft transfer into the trim tank in climb/cruise when CG is forward of the target band [pub concept: trim tank is used to hold the CG aft in cruise for trim drag] | missing | Inner (then mid) tanks to trim tank via inlet valves and aft isolation valve |
| Forward transfer when CG is aft of target [FBW] | trigger 43/44, flat | Hysteresis band, rate taper, lateral weighting to the emptiest feed tank |
| Landing preparation: trim tank emptied forward in descent/approach [pub concept: trim fuel is moved forward before landing] | partially (only when a feed tank is low) | Phase-driven emptying; no aft transfer in descent |
| Low feed tank forces forward transfer [FBW] | yes | Kept, now independent of CG |
| Manual forward/aft transfer with guards [GEN] | missing | Forward until empty or forward CG limit; aft until target |
| Inhibits: ground, refuelling, jettison, no power, CG data invalid, AUTO off [GEN] | ground only | Complete set, each reported |
| Pump/valve faults: one trim pump = half rate, both = no forward transfer, inlet valve failed = half aft rate, aft isolation valve failed = no transfer [FBW naming] | named faults exist in deep model, no consequence on sequencing | Rate model and alerts (degraded, fault after confirm) |
| CG out of limits alert [GEN limits 28..43.5 % MAC] | none | `alert_cg_out_of_limits` |

### Not modelled (needs a real source before adding)

- Exact jettison pump count (the brief asked about two jettison pumps per wing; none is documented publicly, the crate models availability as 0..2 pumps per side).
- Official jettison rate, minimum jettison fuel, the real inhibit set, ARM/ACTIVE pushbutton wording and the exact FQMS trim/CG algorithm beyond the public concept. Anything found should replace the GENERIC constants.

## Crate

`crates/fuel_mgmt`, standalone (`[workspace]` in its own Cargo.toml, no dependencies).

- `model`: `Tank` (the 11 tanks, index = plugin tank number minus one: LeftOuter, Feed1, LeftMid, LeftInner, Feed2, Feed3, RightInner, RightMid, Feed4, RightOuter, Trim), `Kg` array, capacities from `flight_model.cfg` (about 323,000 L), `CgModel` (tank arms in % MAC from FBW tank centroids, `cg_after` moment arithmetic), `apply_delta`.
- `jettison::JettisonController::update(&JettisonInputs, dt) -> JettisonOutput`.
  - Inputs: ARM and ACTIVE pushbuttons, optional target gross weight (default 391,000 kg), gross weight, tank quantities in kg, on ground, flap config, power, per-valve failed-closed/stuck-open, pumps available per side (0..2), CG and CG target.
  - Output: state (Off, Armed, Active, Completed, Stopped(reason)), inhibit, valve commands and positions (5 s travel), per-side flow, per-tank draw in kg/s, pump commands, lamps (ARM, ACTIVE, FAULT), memos (JETTISON, COMPLETED), alerts (FAULT, VLV NOT CLOSED after confirm), `inhibits_auto_trim`, `completed_short`.
  - Stops early by the valve closing distance so the final gross weight lands within a few hundred kg of the target.
- `trim::TrimController::update(&TrimInputs, dt) -> TrimOutput`.
  - Inputs: tank quantities, gross weight, CG % MAC (None = invalid), phase, refuel active, jettison active, AUTO pb, trim pump switches, manual transfer request, faults, power.
  - Output: mode (Idle, Forward, Aft) and reason, inhibit, CG target, trim pump / inlet valve / isolation valve commands, rate, signed per-tank flows (sum zero), alerts (auto transfer fault, degraded, CG out of limits), memo.
  - Hysteresis band around the target: forward starts above target, stops at target - 0.7 % MAC; aft starts below target - 1.7, stops at target - 0.5; rate tapers near the stop; aft only in climb/cruise; descent/approach empties the trim tank forward.
- Tests (`tests/behaviour.rs`, 27): jettison stops at target and closes valves, never touches feed tanks, rate and valve ramp, ARM/ACTIVE logic, ground/flap/power inhibits, stop on flaps and re-select, one nozzle failed (half rate, alert, still reaches a lower target), stuck-open valve alert, minimum fuel / exhausted sources, CG-biased trim draw, wing-imbalance throttling, trim aft transfer reaches the band and stops, forward transfer pulls CG into the band, CG held during a fuel burn with automatic forward transfers, descent emptying, no aft in descent, every trim inhibit, mass conservation, pump and valve fault rates and alerts, low feed forward transfer, manual transfers, CG limit alert.

## Plugin hook needed (not done)

1. Add `crates/fuel_mgmt` as a path dependency of the plugin. Per-frame: build `TrimInputs` from the fuel network quantities (gallons to kg at the plugin's density), `A32NX_FQMS_CENTER_OF_GRAVITY_MAC` (ARINC SSM invalid maps to `None`), flight phase, refuel flag and the circuit states of valves 43/59/44/45 and pumps 19/20; build `JettisonInputs` from the new pushbutton vars, the MFD jettison gross weight, WOW, flap config and valve health.
2. Replace the CG-related part of `LegacyFuel::run` (triggers 43/44 and the low-feed trim triggers) with the trim output: set the same trigger/valve states from `TrimOutput` (forward: trim pumps on, aft isolation open, aft transfer valves of the feed tanks per flow; aft: inlet valves open, inner aft pumps on). Keep the FBW triggers for inner/mid/outer feeding.
3. Replace `Jettison` in `fuel.rs` by `JettisonController`: `valve_cmd` to `JettisonNozzleValveLeft/Right` through the deep model's `JettisonValve`, and let the deep nozzle flow (not `flow_kg_s` from the crate) remove the fuel once that model owns the plant, feeding actual valve positions back. If the deep model is not live, use `JettisonOutput::delta_kg` directly.
4. New vars: `FUEL_JETTISON_ARM_PB`, `FUEL_JETTISON_ACTIVE_PB`, `FUEL_JETTISON_TARGET_GW_KG` (written by the MFD FUEL&LOAD page), outputs `FUEL_JETTISON_ACTIVE`, `FUEL_JETTISON_COMPLETED`, `FUEL_JETTISON_FAULT`, `FUEL_TRIM_XFR_MODE`, `FUEL_TRIM_XFR_FAULT`, `FUEL_TRIM_XFR_DEGRADED`, `FUEL_CG_TARGET_MAC`.
5. ECAM (`deep/ecam/fbw/ata28.rs`): drive FUEL JETTISON from `FUEL_JETTISON_ACTIVE`, add FUEL JETTISON COMPLETED (281800053), drive FAULT and VLV NOT CLOSED from the controller alerts. FBW `FuelPage.tsx` `isJettisonActive` should read `FUEL_JETTISON_ACTIVE`.
6. EFB failure picker: the jettison and trim faults already map to the deep model valves and pumps; pass them in as `valve_failed_closed`, `valve_stuck_open`, `pump_failed`, `inlet_valve_failed`, `iso_aft_failed`.

## Open points

- CG arms come from FBW's tank centroid positions and a 12.36 m MAC; the sensitivity (about 0.5 % MAC per tonne moved to the trim tank at 500 t) looks high, so check against the loadsheet CG before trusting absolute CG numbers. Relative behaviour and logic are unaffected.
- All GENERIC constants are in `JettisonConfig` and `TrimConfig`; replace them as real sources are found.
