# Native GPWS (crates/gpws)

Goal: FlyByWire's A380X ground proximity logic from the SystemsHost hidden view as a plain Rust crate, no simulator dependency, terrain height never needed.

## What FBW actually runs in the A380X

- `systems-host/Misc/LegacyGpws.ts` (971 lines): modes 1, 3, 4, 5 (mode 2 is written but disabled upstream), minimums / 100 above, radio-altitude auto callouts, RETARD, windshear, ARINC discrete words `A32NX_EGPWS_ALERT_{1,2}_DISCRETE_WORD_{1,2}`, `A32NX_GPWS_Warning_Active`, `A32NX_GPWS_GS_Warning_Active`.
- `systems-host/Misc/LegacySoundManager.ts` (150): periodic aural scheduling, `PLAY_INSTRUMENT_SOUND`.
- `systems-host/Misc/EfisTawsBridge.ts` (604): `A32NX_TERR_{1,2}_FAILED`, `A32NX_GPWS_{1,2}_FAILED`, `A32NX_WXR_{1,2}_FAILED`, terrain-on-ND / VD gating, ND / VD availability. The terrain look-ahead (TERRAIN AHEAD, OBSTACLE AHEAD) is not in FBW's A380X JS or in the A380X Rust systems: the A380X `EnhancedGroundProximityWarningComputer` (`enhanced_gpwc/mod.rs`) only forwards position data to the SimBridge terrain renderer, which the plugin already replaces with `terronnd`. `surveillance/egpws/runtime.rs` is the A32NX-only computer and is not wired into `a380_systems`, so it is out of scope.
- `shared/src/AutoCallOuts.ts`: call-out pin flags.

## Crate layout

`crates/gpws`: `arinc` (word encode/decode), `throttle` (UpdateThrottler), `sound` (LegacySoundManager), `callouts` (altitude and RETARD state machines), `gpws` (LegacyGpws), `bridge` (EfisTawsBridge failure and display gating), `vars` (L-var name to input adapter, so the plugin or the golden comparator can feed raw variables).

Inputs: ADR1 baro altitude, RA1/RA2, on-ground, flight phase, MDA/DH, flaps handle, landing conf 3, gear extension, airspeed, vertical speed, plane altitude, G/S validity and deviation, GPWS switches, AP, TLA x4, F-factor, call-out pins. Terrain height is not an input to any A380X GPWS mode (mode 2 uses radio altitude rate only).
Outputs: the two alert L-vars, four discrete words, aural enqueue/dequeue events for the FWS sound manager, `PLAY_INSTRUMENT_SOUND` names.

## Proof

1. Unit tests per mode taken from the TS thresholds.
2. Offline oracle `E:/fbw-build/gpws-oracle`: `oracle.mjs` bundles FBW's real `LegacyGpws.ts`, `LegacySoundManager.ts`, `arinc429.ts`, `flightphase.ts`, `AutoCallOuts.ts` with esbuild against independent stubs for the SDK / MSFS globals, runs scripted scenarios (approach, sink-rate, too-low gear/flaps, glideslope, take-off altitude loss, windshear, autocallouts down to RETARD, go-around) at a 50 ms frame, records inputs and every output per frame as JSON goldens. The Rust test `tests/oracle.rs` replays the inputs and compares every output and event.
3. `EfisTawsBridge` has no offline harness (it pulls the whole bus); covered by unit tests derived line by line.

Run: `cargo test --manifest-path crates/gpws/Cargo.toml` (CARGO_TARGET_DIR=E:/fbw-build/target); regenerate goldens with `node E:/fbw-build/gpws-oracle/oracle.mjs`.

## TAWS terrain look-ahead (`taws.rs`, added 22 Oct)

The real A380 has T2CAS / TAWS look-ahead that FBW does not model. `taws::Taws` adds it as a pure-logic module, independent of `Gpws`. Envelopes are our own parameterisation of the publicly described behaviour of Honeywell / ACSS TAWS (TSO-C151 / DO-367 classes of alert: forward looking terrain avoidance, premature descent alert, terrain clearance floor, obstacle awareness); no manufacturer text or data is used. All numbers are in `taws::params` and are tuning values, not certified figures.

Interfaces
- `TerrainSampler::elevation_ft(lat, lon) -> Option<f64>` (MSL feet; a closure also works). The plugin backs it with `XPLMProbeTerrainXYZ`.
- `ObstacleSource::obstacles_near(lat, lon, radius_nm, &mut Vec<Obstacle>)`, optional; `None` disables obstacle alerts and `outputs.obstacle_data_available` is false. X-Plane exposes no obstacle database through the SDK, so OBSTACLE AHEAD is wired but has no data source yet.
- `TawsInputs`: position, geometric altitude, optional radio altitude, ground speed, track, track rate, vertical speed, on-ground, landing configuration (gear down and landing flaps, from the existing `Gpws` flap logic), glideslope tracking, TERR OFF, system off, failed, and a slice of nearby `Runway`s (threshold, true heading, length, elevation, from navdata).
- `Taws::update(delta_ms, &inputs, &dyn TerrainSampler, Option<&dyn ObstacleSource>)` runs a cycle every 500 ms. `outputs()` gives `flta` / `obstacle` (`AlertLevel::{None, Caution, Warning}`), `premature_descent`, `clearance_floor`, `pull_up_light`, `caution_light`, `threat` (nearest hazard lat/lon and distance), and `nd` flags: `terrain_popup`, `terrain_caution` / `terrain_warning` (amber / red terrain overlay), `obstacle_caution` / `obstacle_warning`, `too_low_terrain`, `pull_up`, `terr_off`, `terrain_unavailable`.
- Aural: `desired_sound()` returns the sound and repeat period; `apply_to(&mut SoundManager)` swaps the periodic sound. Priority: TERRAIN AHEAD PULL UP, OBSTACLE AHEAD PULL UP, TERRAIN AHEAD, OBSTACLE AHEAD, TOO LOW TERRAIN. New names in `sound.rs`: `aural_terrain_ahead` (7 s), `aural_terrain_ahead_pull_up` (2 s), `aural_obstacle_ahead` (7 s), `aural_obstacle_ahead_pull_up` (2 s); TOO LOW TERRAIN reuses `aural_too_low_terrain` (3 s). Sound assets for the four new names still have to exist in the pack.

Forward looking terrain avoidance
- The path is projected from the current track at ground speed, stepping 0.2 nm (1 to 3 s) up to the caution time, curving with the track rate (clamped to 3 deg/s). Three rays per step (centre, left, right); the cone starts 0.2 nm wide and widens at 4 deg half angle, plus up to 8 deg more on the side of a turn (2.5 deg per deg/s).
- Look-ahead time scales with ground speed (distance = gs x time, capped at 12 nm): caution 60 s, warning 30 s; inside 15 nm of a runway (terminal) 45 s and 20 s.
- Flight path: the predicted altitude is `alt + vs_eff * t`, where descent is taken fully and climb is credited at 50 percent. In landing configuration on final the descent credit is scaled by the desensitisation factor below, because the descent is expected to end at the runway.
- Required clearance below the predicted path depends on the terrain point's distance to the nearest runway segment: 100 ft at 1 nm, 500 ft at 8 nm, 700 ft at 15 nm and beyond (linear between). Warning clearance is 30 percent of caution clearance. Caution if terrain + clearance exceeds the predicted altitude within the caution time; warning if terrain + warning clearance does within the warning time.
- Alerts are held 2 s (caution) / 3 s (warning) after the condition clears to avoid chatter.

Runway handling and inhibition
- Aligned approach: runway ahead within 15 nm, lateral offset inside 0.3 nm + 12 deg cone, track within 30 deg of the runway heading.
- Final-approach inhibit: aligned, landing configuration, 3.0 nm or closer: terrain and obstacle alerts off. Between 3.0 and 7.5 nm, clearance and descent credit scale linearly from 0 to 1.
- Over-runway inhibit: from 0.3 nm before the threshold to 1 nm past the far end, within 0.5 nm of the centreline and below 2500 ft above runway elevation.
- Everything off for TERR OFF, system off, failure, on ground, or ground speed under 40 kt. If every sample returns `None`, nothing alerts and `terrain_unavailable` is set.

Premature descent alert: aligned approach, 0.5 to 5 nm from the threshold, height above the runway between 30 ft and `d x 6076 x tan(1.7 deg)`, radio altitude under 1000 ft if available, not climbing above 500 fpm, not tracking the glideslope. Aural TOO LOW TERRAIN.

Terrain clearance floor: radio altitude below a floor that rises linearly from 100 ft at 1 nm to 700 ft at 15 nm from the nearest runway; not evaluated inside 1 nm or beyond 15 nm, and not over the runway. Aural TOO LOW TERRAIN.

Obstacles: point obstacles in a corridor 0.3 nm + 4 deg cone along the current track, caution clearance 300 ft, warning clearance 100 ft, same times and inhibits as terrain.

Cost: at most 200 terrain probes per 500 ms cycle (about 100 at 500 kt, 45 at 150 kt). The plugin can call `update` every frame; the module throttles itself.

Tests (`tests/taws.rs`, synthetic terrain, 100 ms simulation steps)
- flat terrain at FL350: silent for 5 minutes; ridge ahead: caution at about 4 nm and pull-up warning at about 2 nm at 250 kt, both aurals played, and the 500 kt envelope is several nm longer; ridge 1100 ft below and ridge beside the track: silent; marginal 500 ft clearance: caution only.
- rising slope descending at 600 fpm: caution 35+ s and warning 12+ s before impact; turning into a ridge that straight flight misses: alert only when turning.
- hill under a 3 degree path on a landing-configuration final: silent, while the same hill alerts without runway data and landing configuration; over the runway: silent.
- PDA: fires 400 ft above the runway at 4 nm, silent on the glideslope, on a 3 degree path and in a go-around; TCF: fires at 150 ft RA 8 nm out, silent at 600 ft and within 1 nm.
- TERR OFF and on-ground inhibit and the `terr_off` flag; alerts and the periodic aural clear after a climb; missing terrain data reported; obstacles alert only with a database attached; probe count bounded.

## Left

Plugin hook (not done here): feed `Gpws::update` each frame from the var store and write back the outputs; route `PLAY_INSTRUMENT_SOUND` names and the `enqueueSound` events into the native sound path. The VD path waypoint extension (160 nm tail) of the bridge is not translated.

TAWS plugin hook (not done): implement `TerrainSampler` with `XPLMProbeTerrainXYZ` (create one probe, `XPLMWorldToLocal`, probe, convert the hit back to elevation; return `None` when the probe misses); build `TawsInputs` from GPS position and geometric altitude, ground speed, track and track rate, the `Gpws` landing configuration, the TERR OFF button, `bridge` failure flags and navdata runways within 20 nm; call `Taws::update`, call `apply_to` on the sound manager that plays the GPWS aurals, and publish the `nd` flags and `pull_up_light` / `caution_light` to the ND terrain overlay and the glareshield lamps (auto terrain display on alert via `terrain_popup`). Obstacle data has no source in X-Plane; an `ObstacleSource` can later read a bundled obstacle database. Sound assets for the four new aural names are needed. ARINC discrete word bit assignments for the new alerts are not defined.
