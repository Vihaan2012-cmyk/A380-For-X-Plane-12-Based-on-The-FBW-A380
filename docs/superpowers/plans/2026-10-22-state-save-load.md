# Full aircraft state save and load

Date: 2026-10-22. Status: design + plan; the format/slot crate `crates/state_snapshot` is built and tested; nothing is wired into the plugin yet.

Goal: save the complete aircraft state at any moment and load it later so the flight resumes exactly where it was: X-Plane's situation (position, attitude, velocities, time), every FlyByWire and plugin system, fuel, payload, failures/damage/wear and their timers (an engine fire keeps burning), FMS flight plans and performance data, ECAM state, cabin, doors, GSE, EFB/OIT state and display settings.

User decisions (22 Oct):

- Autosave is a user choice on the EFB: interval off / 2 / 5 / 10 / 15 / 30 min (default 5), rolling slots 1-10 (default 3), save at each phase change on/off (default on), save on quit on/off (default on).
- Phase-change saves: pushback, take-off, top of descent, landing, arrival at the gate.
- Load restores everything, including X-Plane's situation, with the sim paused until the systems settle.
- UI is an EFB page only: list, name/rename, delete, load, save now. No X-Plane menu items or key commands.
- Failures, damage, wear and timers resume exactly as saved.

## 1. What persists today (audit)

| Store | File / owner | What | Notes |
|---|---|---|---|
| Airframe | `src/persistence.rs` -> `Output/preferences/fbw_a380x_airframe.json` | airframe/APU hours, engine wear (`physics::damage::EngineWear` x4), armed failure ids + magnitudes, MEL deferrals + tech log, random-failure config + RNG, `wear::WearStore`, `components::Direct`, walkaround items | temp + rename, named mutex, saved every 120 s + on exit (`lib.rs` ~3507). Version 1, mismatch = new airframe. This is the long-lived airframe, not a flight snapshot; keep it separate. |
| Reset | `src/aircraft_reset.rs` | request flag (state / new airframe), `backup()` | reuse `backup()` idea only. |
| Start state | `src/start_state.rs` | classifies hangar/apron/taxi/runway/climb/cruise/approach from X-Plane's placement; reads MSFS `.flt` cockpit L:vars | load must force the start state the snapshot was taken in instead of re-detecting. |
| Variables | `src/lib.rs` `Vars` (~430) | every FBW variable: `slots[SIMULATOR/NAMED]`, `names`, `ids`; `snapshot_kinds() -> Vec<(bool, String, f64)>` already exists; writes go through `SimulatorReaderWriter::write` (invariant-checked) | simple flat map: the crate's `vars` adapter takes this iterator directly. |
| Systems | `remote::Systems` (`src/remote/mod.rs`) | FBW `Simulation<A380>` in the plugin (`Local`) or in `fbw_a380_systems_server.exe` (`Remote`), sharing one variable block (`wire.rs`) | `Simulation<T>` and `A380` are both `#[derive(Clone)]`; no serde. Internal state lives in struct fields (list in section 4). |
| Deep layer | `src/deep/plugin.rs` `DeepLayer` -> `deep::live::Deep { areas: Vec<Box<dyn Area>> }` | 18 areas (electrical, hydraulics, fire_ice, apu, engine_accessories, ...), engine/APU fire state, leaks, thermal zones | `Area` trait has `tick/publish/derived_failures`, no save/load. |
| Failures | `src/failures.rs` | `armed_ids()`, `active_magnitudes()`, `set_active`, `set_magnitude`, `restore_magnitudes` | already round-tripped by persistence. |
| Random / scripted failures | `src/random_failures.rs`, `src/scripted_failures.rs` | `snapshot()` / `restore()` on both | scripted triggers already serialisable. |
| MEL | `src/mel.rs` | `snapshot/restore`, `tech_log/restore_tech_log` | |
| Damage, tyres, breakers | `src/physics/damage.rs` (`snapshot()`), `src/physics/tyre.rs`, `src/breakers.rs` (`snapshot(vars)`), `physics::electrical::CircuitProtection` | wear yes; tyre temperatures/pressures, breaker I^2t heat and trip latches: no save | |
| Walkaround | `src/walkaround.rs` `snapshot()` | | |
| Published numbers | `src/published.rs` `snapshot_numbers/restore_numbers` | | |
| App settings | `src/app_settings.rs` (`xphfbw.*` keys) | live-watched settings file | settings are preferences, not flight state: not in the snapshot. |
| MSFS stored data | `src/js/msfs/mod.rs` (`GetStoredData`/`SetStoredData` JSON) | flyPad / EFB settings, SimBrief ids, Hoppie code | preferences + credentials: never in the snapshot. |
| State dump / replay | `src/state_dump.rs`, `src/replay.rs` | every variable every N frames (diagnostics), 4 Hz tape | proves the variable snapshot is cheap; not restorable. |
| FMS | JS (QuickJS/CEF) today; native `crates/fms` coming | FBW `FlightPlan.serialize()` / `FlightPlan.fromSerializedFlightPlan()` exist in FBW's flight plan code (used to sync plans between instruments) | native `crates/fms/src/plan.rs` types have no serde derives yet. |
| X-Plane situation | XPLM | `XPLMSaveDataFile` / `XPLMLoadDataFile` / `XPLMPlaceUserAtLocation` are **not** bound; `Xplm::xplm_symbol(name)` (`src/xp.rs`) loads any XPLM symbol on demand | see section 3. |

Conclusion: today only the airframe (long-term wear/failures/MEL) persists. Nothing saves a flight. Most per-module state is plain Rust fields with no serde.

## 2. Snapshot format (built: `crates/state_snapshot`)

- One file per slot: `<X-Plane>/Output/preferences/a380_states/<id>.a380state` (folder name GENERIC).
- gzip (level 6) of one JSON document:
  ```
  { "format": "a380-xp12-state", "version": 1, "created_unix": ..., "name": "...",
    "reason": "manual" | "autosave" | "quit" | {"phase": "pushback"|"takeoff"|"top_of_descent"|"landing"|"gate"},
    "aircraft": { "name", "plugin_version", "livery", "engine_variant" },
    "summary": { latitude, longitude, altitude_ft, ias_kt, on_ground, flight_phase, origin, destination, flight_number, sim_zulu_s },
    "sections": { "<name>": { "v": <section version>, "data": <any JSON> } } }
  ```
- Two version levels: file `version` (envelope; `migrate::MIGRATIONS[i]` upgrades i+1 -> i+2 on the raw JSON before typing) and per-section `v` (each owner upgrades its own data in `Section::load(version, data)`). A section newer than the reader is reported (`Restored::NewerThanReader`) and skipped, never half-applied; a file newer than the reader is refused.
- f64 exact: `serde_json` `float_roundtrip`; NaN/inf stored as `"nan"`/`"inf"`/`"-inf"` (ARINC sentinels and unset values survive).
- `Section` trait (object-safe): `name()`, `version()`, `save() -> Value`, `load(version, &Value) -> Result`. `Snapshot::put`, `put_raw`, `restore`, `restore_all` (one failing section never blocks the rest; results are logged per section).
- Size estimate: ~30k variables x ~40 bytes ~ 1.2 MB JSON, ~250-400 kB gzipped (GENERIC estimate). Encoding + write happen on a worker thread (copy values on the flight loop, like `state_dump.rs`).

### Crash-safe write

`codec::write_atomic`: write `.<file>.<pid>.<seq>.tmp` in the same folder, `sync_all`, `rename` over the target (atomic replace on one volume, Windows and POSIX), temp removed on failure. A crash leaves the previous slot intact; `SlotManager::sweep_temp_files` deletes stray temps at start.

### Slots (built)

`SlotManager::with_settings(dir, AutosaveSettings)`:

| Kind | Id | Rule |
|---|---|---|
| Manual (EFB "Save now") | `manual-<unix>-<slug>` | never overwrites; name up to 64 chars, control chars removed |
| Autosave | `autosave-1..N` | N = EFB "rolling slots" (1-10, default 3); writes an empty slot first, then overwrites the oldest |
| Quit | `quit` | one slot, if "save on quit" is on |
| Phase | `phase-pushback`, `phase-takeoff`, `phase-tod`, `phase-landing`, `phase-gate` | one slot per event, latest flight, if "phase saves" is on |

`list()` (newest first, corrupt files listed with `error` so the EFB can offer delete), `load(id)`, `delete(id)`, `rename(id, name)`, `save_if_allowed()` (returns `None` when the settings turned that kind off), `set_settings()` (shrinking the slot count deletes surplus autosaves). Ids are validated (`[a-z0-9_-]`), so the EFB can never address a path outside the folder.

### Autosave settings (built: `slots::AutosaveSettings`)

`{ interval_min: 0|2|5|10|15|30, rolling_slots: 1..=10, phase_saves: bool, quit_save: bool }`, defaults `5, 3, true, true`; `normalized()` snaps hand-edited values to the nearest EFB choice. Stored with the EFB's own settings (preferences), not in snapshots. `phase::AutosaveClock::from_settings` counts only running sim time (paused / loading frames do not count).

### Phase triggers (built: `phase::PhaseWatch`)

Input per frame: `A32NX_FMGC_FLIGHT_PHASE`, on-ground, ground speed, pushback active (GSE tug / X-Plane pushback), any engine running, parking brake.

- Pushback: pushback rising edge.
- Take-off: FMGC phase enters TAKEOFF on the ground.
- Top of descent: CRUISE -> DESCENT.
- Landing: on-ground rising edge after being airborne.
- Gate: after a flight, on ground, all engines off, parking brake set, ground speed < 1 kt (GENERIC).

Each fires once per flight; "gate" re-arms all. After a load, `prime()` seeds the watch with the loaded state so loading never immediately overwrites a slot.

### Never saved

Credentials and personal ids: anything whose key contains password, secret, token, api key, client id, credential, cookie, logon, auth code, refresh, SimBrief user, Navigraph, Hoppie, email (`secrets::scrub` runs on every section, and the vars adapter refuses such names). Also not saved: app/EFB preference files, sound pack paths, window positions, user folder paths (release-privacy rule). The snapshot only holds aircraft state.

## 3. X-Plane situation

Two options were weighed:

1. `XPLMSaveDataFile(xplm_DataFile_Situation = 1, path)` / `XPLMLoadDataFile(1, path)` (XPLMUtilities). Loads weather/time/position the X-Plane way, but loading a situation reloads the user aircraft, which unloads and restarts this plugin mid-load. It would need a "pending restore" marker file read in `XPluginEnable`/`confirm_start_state` and a second boot path.
2. **Chosen:** dataref restore, no aircraft reload. Save the values below; on load: pause, `XPLMPlaceUserAtLocation(lat, lon, elev_msl_m, heading_true, speed_m_s)` (XPLMPlanes, XPLM400) so X-Plane loads the scenery, wait for `sim/graphics/scenery/async_scenery_load_in_progress == 0` (GENERIC timeout 120 s), then write the exact state.

Also write the `.sit` next to each snapshot (`<id>.sit`, `XPLMSaveDataFile`) as a debug companion and for weather; not loaded by default.

Saved (section `xp_situation`, owner: new `src/state_save.rs`):
`sim/flightmodel/position/latitude`, `longitude`, `elevation`; `local_x/y/z` (re-derived via `XPLMWorldToLocal` on load); `q[4]`, `psi`, `theta`, `phi`; `local_vx/vy/vz`; `P/Q/R` (and `Prad/Qrad/Rrad`); `sim/time/zulu_time_sec`, `sim/time/local_date_days`; `sim/flightmodel/weight/m_fuel[]`, `m_fixed`, `m_stations[]`; `sim/flightmodel2/gear/deploy_ratio[]`, `sim/flightmodel/controls/flaprqst`, `flap_handle`, trims; engine `N1/N2/EGT/ITT/FF`, igniter/starter, mixture/throttle ratios; `sim/cockpit2/controls/parking_brake_ratio`; tyre/brake temperatures where X-Plane owns them; X-Plane failure datarefs this plugin writes (`xp_effects`). Position/attitude/velocities are written again after settling (`Stage::ReapplySituation`) because the systems' settle frames must not move the aircraft.

Pause: commands `sim/operation/pause_on` / `pause_off` (via `XPLMFindCommand` + `XPLMCommandOnce`, already bound). While paused, X-Plane's `frame_rate_period` is 0, so settle frames tick the systems with a synthetic delta (GENERIC 1/30 s) and the X-Plane inputs held at the restored values.

## 4. Systems: what is restored how

### 4a. Through variables (no code in the owner)

Section `vars` = `Vars::snapshot_kinds()` through `state_snapshot::vars::save`. On load, `vars::load(data, &Filter)` returns the named (`A32NX_...`) values to write with `SimulatorReaderWriter::write`; simulator variables are skipped by default (X-Plane feeds them from the restored situation). Covers every cockpit switch/knob/pushbutton position, selector, payload/boarding variables, FCU targets, EFIS settings, display brightness potentiometers, light switches, FBW latch variables that FBW reads back each tick, ECAM recall/clear/status state that lives in variables, published flags other modules read.

Skip list (`Filter`): momentary event variables (`*_PRESSED`, `*_PUSHED`, `_EVENT`, key-event counters), `A32NX_COLD_AND_DARK_SPAWN`, start-state variables, the plugin's own diagnostics and live-log variables. The list is built once from a cold-and-dark vs. cruise diff of `vars` and kept in `src/state_save.rs`.

A variable write alone does not move FBW internal state: FBW elements read inputs from variables but keep physics in fields. Those are 4b.

### 4b. FBW Rust systems with internal state (fields, not variables)

Verified in `fbw-aircraft` (FBW's crates in this fork):

| FBW element | Field(s) | Why it matters |
|---|---|---|
| `electrical::battery::Battery` | `charge`, `temperature`, RC polarization state | battery state of charge / temperature |
| `hydraulic::Reservoir` | `current_level`, `air_pressure` | fluid lost to leaks |
| `hydraulic::Accumulator` (incl. brake accumulators in `BrakeCircuit`) | `gas_volume`, `fluid_volume`, `gas_pressure`, `current_gas_init_precharge` | pressure on a cold start, brake accumulator |
| `hydraulic::HydraulicCircuit` | section fluid volumes / pressures | pressure while pumps run |
| `hydraulic::flap_slat::FlapSlatAssembly`, `TrimmableHorizontalStabilizerAssembly`, `linear_actuator::LinearActuator` (surfaces, gear, doors) | positions, speeds | flaps/slats/THS/gear/doors would snap back to the start-state position |
| `navigation::adirs` IR | `remaining_align_duration`, `remaining_attitude_initialisation_duration`, `quick_realign_remaining_available_time`, `alignment_failed` | IRs must be aligned in flight |
| `apu::AuxiliaryPowerUnit` (turbine state machine, EGT, N, flap) | `Box<dyn Turbine>` state | running APU stays running |
| `air_conditioning::cabin_air::CabinAirSimulation` | cabin zone air (pressure, temperature), filtered exterior pressure / flow | cabin altitude and temperatures |
| Pressurisation controllers / outflow valves, pneumatic valves and duct pressures | positions, controller integrators, timers | seconds to settle; settling handles most |
| Fire bottles, fire detection loops | discharged / squib state | a fired bottle stays empty |
| Any FBW `Failure`-driven latches | latched flags | resumed by failures + settle |

Approach, in order of preference:

1. **Rebuild with the saved start state.** `remote::Systems::rebuild(StartState, vars)` with the snapshot's FBW start state (e.g. Cruise / Approach / Taxi), so FBW's own `InitContext::start_state()` paths initialise IRs aligned, hydraulics/batteries charged, engines running, cabin pressurised. Then variables (4a), then settle. This alone gets most of the way.
2. **Minimal FBW patch: optional per-element persistence through the existing visitor.** One change in `systems/src/simulation/mod.rs`: add to `SimulationElement`
   `fn persist(&mut self, _p: &mut dyn Persist) {}` (default no-op) and a `Persist` trait with `fn f64(&mut self, key: &str, v: &mut f64)` / `bool` / `opt_f64` that either reads into or writes from the value. A `SnapshotVisitor` walks the tree with the existing `accept` visitor; each element's keys are prefixed by visit index + `type_name::<T>()`, so a changed tree is detected (mismatched keys are skipped and logged). `Simulation<T>` gets `pub fn save_internal(&mut self) -> Vec<(String, f64)>` and `pub fn load_internal(&mut self, &[(String, f64)])`. Then one small `persist` impl per element in the table (about 12 impls, uom quantities converted to SI f64). No serde dependency in FBW's crate.
3. **Remote systems:** add two wire messages to `src/remote/wire.rs` / `server.rs` / `client.rs`: `SaveInternal` (server replies with the key/value list in the message area or a side file) and `LoadInternal(path)`. Until then, in-process (`FBW_SYSTEMS_IN_PROCESS=1`) only, or variable restore + start state for remote.
4. **In-session quick restore** (same session, no file): `Simulation<A380>` is `Clone`; a `Local` session could keep the clone in memory. Not needed for the EFB feature; noted only.

### 4c. Plugin modules (serialise themselves)

Each gets a `state_snapshot::Section` impl (or `snapshot()/restore()` pair it already has, wrapped in `src/state_save.rs`). Priority order:

1. Failures: `failures::armed_ids/active_magnitudes` + `set_active/restore_magnitudes`; `random_failures` `snapshot/restore` (RNG state included so the next random failure is the same); `scripted_failures` `snapshot/restore` (pending triggers); `mel` (deferred + tech log). Already serialisable.
2. Damage and timers: `physics::damage::Damage` (time-at-temperature integrators, exceedances), `physics::tyre::Tyres` (pressure, temperature, leak, fuse plugs), `wear` + `components`, `breakers::Breakers` + `physics::electrical::CircuitProtection` (I^2t heat, trip latches), `physics::bays::Bays` temperatures, `engine_detach`, `separation`, `gear_collapse`, `brake_fire_fx`, `hail_dents_fx`, `windshield_cracks_fx`.
3. Deep layer: add to `deep::live::Area` `fn save_state(&self) -> Option<serde_json::Value> { None }` and `fn load_state(&mut self, _: &serde_json::Value) -> Result<(), String> { Ok(()) }`; `Deep` saves `{area name: value}`. First areas: `fire_ice` (fire loops, engine/APU fire burn state: the fire keeps burning), `apu`, `engine_accessories`, `hydraulics` (leaks, fluid), `electrical`, `thermal_zones`, `fuel`, `oxygen`, `gear_structure`. Deep failure magnitudes (`derived`) are recomputed from area state, not saved.
4. Engines: `fadec::Fadec` (FADEC state machine, start sequence, oil quantity/temperature), `physics/engine/*` free-engine spool/oil/hot-section state, `engine_commands`, `throttle` (detent calibration excluded: preference).
5. Fuel: `fuel::Fuel` tank quantities and temperatures, valve/pump states, transfer logic phase; X-Plane tanks follow it.
6. `oxygen::Oxygen` (quantity: fixes the gap noted in `persistence.rs`), `physics::hydraulics`, `adirs_physics` (strapdown IRS error state and drift), `physics::air`.
7. Doors, slides, cowl doors, walkaround, GSE (`gse::Gse`: vehicles present, positions, jetway docked, pushback state), `cabin_crew`, `cabin_mood`, `cabin_option`, `seat_belt_signs`, `fap`, `interphone`, `copilot`, `hud`, `radios`, `surveillance`, `weight_balance`, `landing_report` (current flight's events), `maintenance_log` entries for this flight.
8. EFB / OIT: `efb::Efb` page state that is flight state (loaded OFP, performance calculations, ground-ops progress, failure page state, checklist progress), OIT flight folder; display settings (`display` per-screen brightness, native page state `native_page_state.rs`, ND range/mode — mostly in variables already).
9. ECAM: FWS state in `crates/fws` (active warnings, cleared/recalled, procedure line progress, inhibited/acknowledged sets, master caution/warning latches) needs its own `save/load`; ECL checklist progress in `crates/fws_ecl`. Until then, alerts re-raise from restored system state after settle, which is acceptable for a first version.

### 4d. FMS

- JS FMS today: the FMS host gets two calls through the JS bridge: `fbwSaveFms()` returning `{ plans: { active, temporary, secondary[] } as FBW's serialized flight plan, activeLegIndex, fmgc data (ZFW, ZFWCG, block, cost index, cruise FL, flex/derate, V-speeds, transition altitudes, winds, perf page entries), uplink state }` and `fbwLoadFms(json)` using FBW's `FlightPlan.fromSerializedFlightPlan` per plan, then re-sequencing to the saved active leg. Navdata cycle and airport/runway idents are stored; if the cycle differs, the load reports it and still tries.
- Native `crates/fms`: add `serde` derives to `plan.rs` types (`LegDefinition`, `Leg`, `Segment`, constraints, `CruiseStep`, `PendingAirways`) and `service::Fms` state; section `fms` v1. Owned by the native-FMS agents, listed here as a dependency.
- LNAV/VNAV predictions (`fms_lnav`, `fms_vnav`, `fms_perf`) are recomputed, not saved.

## 5. Load order (built: `restore::ORDER` + `restore::Sequence`)

1. `PauseSim` (pause command; autosave clock and phase watch stop).
2. `PlaceAircraft` (`XPLMPlaceUserAtLocation`), 3. `WaitForScenery` (poll, timeout), 4. `SetSituation` (exact position/attitude/velocities/time, weights, fuel).
5. `RebuildSystems` (saved FBW start state; deep layer rebuilt cold; FADEC/fuel told the start state, as `confirm_start_state` does).
6. `WriteVariables` (4a), 7. `RestoreModules` (4c), 8. `RestoreDeepAreas`, 9. `RestoreFbwInternals` (4b patch), 10. `RestoreFailures` (after everything, so arming a failure acts on restored state; random-failure RNG last).
11. `RestoreFms`, 12. `RestoreUi` (EFB/OIT/display pages).
13. `Settle`: systems tick paused with synthetic delta; done when at least 60 frames and 2 s of system time have passed and the restored variables have been quiet for 30 frames, or after 900 frames (all GENERIC). The result (`converged`) is logged as `state load: settled in N frames (converged|gave up)`.
14. `ReapplySituation`, 15. `Resume` (unpause only if the snapshot was not paused), `PhaseWatch::prime`, autosave clock reset.

Loads are allowed on the ground and in the air; the EFB asks for confirmation ("current flight will be replaced"). A load never writes `fbw_a380x_airframe.json` until the first normal save tick after resume, and the airframe's long-term hours are not rolled back (they come from the airframe file, not the snapshot).

## 6. EFB page

New EFB page (FBW-style, own layout) "Saved states" under the existing EFB shell:

- List: name, kind (manual / autosave / quit / phase label), date, origin-destination, altitude/speed/phase summary, size; corrupt entries shown with "Delete".
- Actions: Save now (with a name field), Load (confirm), Rename, Delete (confirm).
- Settings block: Autosave interval (off / 2 / 5 / 10 / 15 / 30 min), Rolling autosave slots (1-10), Save at phase changes (on/off), Save on quit (on/off). Stored with the EFB's settings, defaults 5 min / 3 / on / on.
- Status line while loading ("Loading... settling systems") driven by `Sequence::stage()`.
- No X-Plane menu entries and no key commands.

## 7. Hooks needed (not done here: shared files)

| File : function | Change |
|---|---|
| `Cargo.toml` (root) | `state_snapshot = { path = "crates/state_snapshot" }` |
| `src/lib.rs` `mod` list | `mod state_save;` (new file, owns the plugin-side glue) |
| `src/lib.rs` `struct Plugin` | field `state_save: state_save::StateSave` (holds `SlotManager`, `AutosaveClock`, `PhaseWatch`, optional `restore::Sequence` + loaded `Snapshot`, writer thread handle) |
| `src/lib.rs` `Plugin::new` (~1640, next to `Persistence::new`) | `StateSave::new(xplane_root, efb_settings)`; `sweep_temp_files()` |
| `src/lib.rs` `Plugin::tick` (~2117), right after `self.persistence.tick(real_delta)` (~2407) | `self.state_save.tick(self, real_delta)`: feed `PhaseWatch` (FMGC phase id already in `scripted_phase_id`), on event / clock fire build a snapshot and hand it to the writer thread; if a load is pending, run `Sequence::step` before the systems tick and skip the normal X-Plane input read while paused |
| `src/lib.rs` `Plugin::tick`, before `self.simulation.tick(...)` | while `Stage::Settle`: tick `simulation`, `deep`, `fadec`, `fuel` with the synthetic delta even though X-Plane is paused |
| `src/lib.rs` `XPluginStop`/disable (~3507, next to `persistence.save()`) | `state_save.save_on_quit(...)` if `quit_save`, synchronous (no thread) |
| `src/lib.rs` `Vars` | none: `snapshot_kinds()` and `SimulatorReaderWriter::write` are enough |
| `src/remote/mod.rs` `Systems` | `save_internal()` / `load_internal()` forwarding (Local: call `Simulation`; Remote: wire messages) |
| `src/remote/wire.rs`, `server.rs`, `client.rs` | `SaveInternal` / `LoadInternal` messages |
| `src/deep/live.rs` `trait Area` + `Deep` | `save_state` / `load_state` defaults; `Deep::save_areas()` / `load_areas()` |
| `src/xp.rs` | bind `XPLMPlaceUserAtLocation`, `XPLMSaveDataFile`, `XPLMLoadDataFile` through `Xplm::xplm_symbol` |
| `src/efb.rs` | routes `states/list`, `states/save`, `states/load`, `states/rename`, `states/delete`, `states/settings` to `state_save` request queue (same request/apply pattern as `random_failures::request_config`) |
| `src/start_state.rs` `detect` / `Plugin::confirm_start_state` | accept a forced start state from a pending load |
| each module in 4c | a `Section` impl or `snapshot()/restore()` pair |
| `fbw-aircraft` `systems/src/simulation/mod.rs` + ~12 elements | `persist` default method + impls (4b.2) |
| JS FMS host | `fbwSaveFms` / `fbwLoadFms` (4d) |
| `crates/fms` | serde derives on plan types |

### 7a. Exact hook lines for `src/lib.rs`, root `Cargo.toml` and `src/panel.rs` (state35, 22 Oct)

`src/state_save.rs` is written and tested but is not compiled into the plugin until these lines are applied. They were applied to a shadow copy (`E:/fbw-build/state35/shadow`), which built and passed 7 tests.

1. Root `Cargo.toml`, `[dependencies]`, after `fms = { path = "crates/fms" }`:
   ```toml
   state_snapshot = { path = "crates/state_snapshot" }
   ```
   (Turns on `serde_json`'s `float_roundtrip` feature for the whole plugin through feature unification: float parsing becomes exact, and a little slower.)
2. `src/lib.rs` module list, before `mod persistence;`:
   ```rust
   mod state_save;
   ```
3. `src/lib.rs` `struct Plugin`, after `persistence: persistence::Persistence,`:
   ```rust
   state_save: state_save::StateSave,
   ```
4. `src/lib.rs` `Plugin::new`, the struct literal, after `scripted_phase_id,`:
   ```rust
   state_save: state_save::StateSave::new(&crate::xp::system_path().unwrap_or_else(|| ".".into())),
   ```
5. `src/lib.rs` `fn tick(&mut self, delta: f64) {`, first line of the body (drives a pending load; while it settles, the whole tick runs with the synthetic 1/30 s delta):
   ```rust
   let delta = state_save::before_systems(self).unwrap_or(delta);
   ```
6. `src/lib.rs` `Plugin::tick`, right after `self.persistence.tick(real_delta);`:
   ```rust
   state_save::tick(self, real_delta);
   ```
7. `src/lib.rs` plugin stop/disable, right before `if let Err(e) = plugin.persistence.save() {`:
   ```rust
   state_save::on_exit(plugin);
   ```
8. `src/panel.rs` `handle`, before `if let Some((status, text)) = crate::oit_files::route(method, path, query, &body) {` (the EFB's HTTP routes live in `panel.rs`, not `efb.rs`):
   ```rust
   if let Some((status, text)) = crate::state_save::route(method, path, &body) {
       return reply(&mut stream, status, "application/json", text.as_bytes());
   }
   ```

Routes: `GET /states/list` (states, settings, load status), `POST /states/save {name}`, `/states/load {id}`, `/states/rename {id,name}`, `/states/delete {id}`, `/states/settings {interval_min,rolling_slots,phase_saves,quit_save}`. Settings are stored in `Output/preferences/a380_state_settings.json` (file name GENERIC); snapshots and their `.sit` companions are stored in `Output/preferences/a380_states/`.

Done in state35 (no hooks needed): `src/xp.rs` `place_user_at_location`, `save_data_file`/`load_data_file` (+ `DATA_FILE_SITUATION`), `set_d`, `world_to_local`; `deep::live::Area::save_state/load_state` defaults + `Deep::save_areas/load_areas` + `DeepLayer` forwarders; `fire_ice` save/load (zone combustion temperature and burning flag, agent concentration, engine and APU bottles, squib latches, wing and nacelle ice, rain film); EFB "Saves" tab in `app/ui/index.html`.

Load stages wired in `state_save.rs`: PauseSim, PlaceAircraft, WaitForScenery (`async_scenery_load_in_progress`), SetSituation, RebuildSystems (`remote::Systems::rebuild` with the saved FBW start state; deep layer, FADEC and fuel are not re-told the start state yet), WriteVariables, RestoreModules (engine wear, component wear, direct component settings, walkaround), RestoreDeepAreas, RestoreFailures (armed ids + magnitudes, scripted triggers, MEL + tech log, random-failure config + RNG), Settle, ReapplySituation, Resume (only if the save was not paused). RestoreFbwInternals, RestoreFms and RestoreUi do nothing yet.

## 8. Tasks

1. [done] `crates/state_snapshot`: format, sections, codec (gzip, atomic write), migrations, slots + autosave settings, phase watch, autosave clock, vars adapter, secrets scrub, load sequence. 29 tests.
2. [code done, hooks pending (7a)] `src/state_save.rs` + lib.rs hooks: save path only (vars + failures + plugin modules already serialisable + situation). Test: emulator crate round trip (save, perturb, load, `vars::differences` == empty on named inputs).
3. [code done, untested in sim] Situation load with pause, place, scenery wait, settle, reapply. Test in sim on the ground, then cruise.
4. [done except apu (apu31)] Deep `Area` save/load: fire_ice, hydraulics (reservoir fluid, accumulator gas, node pressures and air, fluid temperatures), fuel (tank mass/temperature, jettison/LP/APU feed valves, filter state), engine_accessories (EEC preferred channel, alternator/power source, start request, SOV/start valve, starter heat/disintegration), oxygen (crew/therapeutic bottles, pax generators), gear_structure (brake heat/wear/fire, parking accumulator, leg retraction, wing fatigue), cabin (slides, cargo door, water, waste). Electrical (battery charge/temperature, network breakers, RAT) and breakers (417 units: trip state, heat, lockout, drift/weld) keep std-only `saved*/restore*` in their shared files; the JSON conversion is the plugin-only `Persisted` wrapper in `deep/live.rs` (`deep_electrical` has no serde).
5. FBW `persist` patch (battery, hydraulics, ADIRS, surfaces, APU, cabin air) + remote wire messages.
6. [done] Section `plugin_systems`: FADEC internals (`fadec::SavedFadec`), fuel network (`fuel::SavedFuel`, applied after the start-state set-up re-runs), oxygen bottle, tyres (temperature, leak, tread, fuse plug, burst), connected GSE services (re-requested when they differ). `RebuildSystems` now writes `START_STATE`, rebuilds FADEC and the deep layer, restarts fuel and spawns engines at idle/stopped. Damage engine wear is in `airframe_condition`; structure separation comes back with the failures; doors are variables.
7. [JS done, native pending] Section `fms`: the JS FMS's plans (`BaseFlightPlan::serialize`, every plan index) are copied every 10 s (GENERIC) through `js_worker::request_eval`; `RestoreFms` rebuilds them with `FlightPlan.fromSerializedFlightPlan` + `FlightPlanManager.set`. Native `crates/fms`: needs `Fms::serialize()/restore()` (other agent) and a `fms` section v2.
8. [done, hooks pending (7a)] EFB page + settings.
9. FWS/ECL section: not done. `crates/fws` is not run by the plugin (ECAM is FBW's JS FWS); alerts re-raise after settle. When the native FWS runs, it needs `Fws::save() -> FwsState` / `Fws::load(&FwsState)` covering `Core::{alerts (active, cleared, recalled, inhibited, acknowledged, procedure line ticks), phases, to_config_* memories, startup_completed, status_normal}` and `fws_ecl` checklist ticks.
10. Acceptance: save at cruise with an engine fire armed and a hydraulic leak, quit, restart at an airport, load: same position +-1 m, same attitude, IRs NAV, fire still burning with the same bottle state, same fluid level, FMS on the same active leg, ECAM showing the same alerts after settle.

All numeric limits in this plan not quoted from FBW or X-Plane documentation are GENERIC: settle 60 frames / 2 s / 30 quiet / 900 max, scenery timeout 120 s, synthetic delta 1/30 s, gate ground speed 1 kt, name 64 chars, gzip level 6, size estimate.
