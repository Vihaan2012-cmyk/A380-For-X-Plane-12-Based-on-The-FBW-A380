# New systems audit (9 user-chosen features)

Audit only, no code changed. Scope: `src/`, `src/deep/*`, `src/physics/*`, `crates/*`, FBW `a380_systems` and A380X instruments (read-only). FCOM used for behaviour checks only; section ids cited, no wording copied.

Effort: S = under 1 agent-day, M = 1-3, L = 3-6, XL = 6+. Sorted by effort, smallest first.

| # | Feature | Effort | Main blocker |
|---|---|---|---|
| 7 | RMP -> X-Plane COM audio | S | X-Plane has only COM1/COM2 audio |
| 4 | Water/waste over the flight | S-M | nothing feeds demand or calls `service()` |
| 8 | Tyres, TPIS, burst | M | SD WHEEL shows a fixed PSI; wear is failure-only |
| 5 | Crew O2 + pax masks visible | M (+ art) | two oxygen models; no cabin mask OBJ |
| 1 | Full CMS + post-flight report | M | no class, no legs, not persisted, not wired natively |
| 6 | Icing | M-L | two ice models; deep ice drag not applied; no ice-detected alerts |
| 9 | Start states on native systems | L | deep layer ignores the start state; no turnaround state |
| 3 | EHA/EBHA per actuator | L | EHA power is "any AC bus"; rudder/spoiler 5 EBHA missing |
| 2 | Engine start, one path, Trent + GP | XL | no auto-abort/retry/manual start; GP is a re-skinned Trent |

---

## 7. RMP controls X-Plane COM audio (S)

FCOM: DSC-23-20 (RMP, ACP keys), DSC-23-10-30-10 (VHF/HF).

| Exists | Where |
|---|---|
| COM1/2 freq, standby, volume, receive, transmitter -> X-Plane | `src/radios.rs:1013-1026` (`audio_volume_com1/2`, `audio_selection_com1/2`, `audio_com_selection` 6/7) |
| COM3 held internally only | `src/radios.rs` table: "X-Plane has no COM3" |
| Native RMP + AMU logic incl. VHF1-3, HF1-2, INT/CAB/PA/NAV RX keys + volume vars | `crates/rmp/src/panel.rs:252-269`, `host.rs` |
| `crates/rmp` is not called from `src/` yet | no `rmp::` use in `src/` |

| Gap | Plan |
|---|---|
| `crates/rmp` not wired (still the JS RMP) | Hook crate into the tick; keep var names `radios.rs` already reads |
| VHF3 has no X-Plane audio | Route VHF3 RX to whichever X-Plane COM is not used by VHF1/2 only when user opts in; else keep data-only (ACARS default) |
| HF1/2 have no receiver | Keep HF RX volume/selection state for the panel; no X-Plane audio (X-Plane has no HF). Document. |
| Per-seat ACP (capt/F/O/3rd) vs one X-Plane mixer | Use the seat the user occupies (flight_deck_view) as the source of volumes |
| NAV/ADF ident volumes come from the RMP NAV key | Map `NAV_VOL_RX` to `audio_volume_nav*`/`adf*` |

Effort: S (half a day wiring + tests on the var map).

## 4. Cabin water/waste quantities with consequences (S-M)

FCOM: DSC-38-10 (potable water, waste, toilet system), DSC-38-20.

| Exists | Where |
|---|---|
| Potable tank, ullage pressurisation, heaters, drain masts, leak fault | `src/deep/cabin/water.rs` (capacity GENERIC 800 L) |
| 3 waste zones, level sensors, flush valves, vacuum generator; lavs INOP at 95% full | `src/deep/cabin/waste.rs:147-179` |
| `CabinLive::service()` (water full, waste empty, door) | `src/deep/cabin/live.rs:363` |
| GSE water truck + lav truck vehicles | `src/gse.rs:303-304` |

| Gap | Plan |
|---|---|
| `water_demand_l_s` is never fed (stays 0) | Demand = pax count (payload) x per-pax rate x phase factor; galley peaks after TO + before TOD |
| `flush_commanded` never set | Poisson flush events per zone from pax count and flight time |
| `service()` is never called | GSE water/lav trucks call partial service (rate per minute while connected), aircraft reset calls full |
| `lav_inoperative` / `tank_full` not consumed | Publish to CIDS/FAP (`crates/cids`, `fap_pages`), ECAM if FBW has a LAV alert, cabin sounds |
| Quantities not persisted | Add to `persistence.rs` airframe state |
| Water quantity not shown on FAP/door page | Feed FAP water/waste page |

Effort: S-M (1-1.5 days). GENERIC per-pax rates must be marked.

## 8. Tyres: 22 wheels, TPIS, wear, burst (M)

FCOM: DSC-32-10-60 (TPIS), DSC-32-20-70 (WHEEL SD TPIS/BTMS), DSC-32-50 (WHEEL TIRE PRESS LO).

| Exists | Where |
|---|---|
| 22 wheels (16 braked + 6 nose/unbraked) | `src/physics/tyre_model.rs:94-121` |
| Per-wheel pressure (gas law), temp (brake soak + flex), leak, tread, fuse plug | `src/physics/tyre.rs`, outputs `TYRE_PRESSURE_PA:n`, `TYRE_TEMPERATURE_C:n`, `TYRE_TREAD_MM:n` |
| WHEEL TIRE PRESS LO trigger | `src/deep/ecam/fbw/ata32.rs:269`, test in `fbw_tests.rs:688` |
| Burst -> X-Plane per leg (5 `rel_tire`), ids 32_101..104 | `src/physics/damage.rs:1173-1180` |
| FBW SD WHEEL draws a fixed "220" PSI | `fbw-a380x/.../SD/Pages/Wheel/elements/Wheel.tsx` |

| Gap | Plan |
|---|---|
| SD WHEEL shows constant 220 | Native SD wheel page reads `TYRE_PRESSURE_PA:n`, 5 PSI steps, colours per DSC-32-20-70 |
| Tread only wears under a failure | Normal wear: touchdown spin-up scrub, braking energy, turning scrub on body gear |
| Burst is per leg, not per wheel | Per-wheel burst flag; per-leg X-Plane failure only when wheels on that leg exceed a share; extra drag/pull per burst wheel via plug forces |
| No slow natural pressure loss / servicing | Daily loss rate (GENERIC), nitrogen service via GSE/EFB ground page |
| Not persisted | Tread/pressure into `persistence.rs` |
| TPIS power/BCS dependency | TPIS invalid when BCS 2 / DC ESS lost (DSC-32-60) |

Effort: M (2 days, excluding the native SD page owner's work).

## 5. Crew oxygen depletion + pax masks in the cabin (M, plus art)

FCOM: DSC-35-20-10/20 (crew system, bottle, SD indication), DSC-35-30-10/20 (cabin system, auto/manual deploy), DSC-35-20-40 / 35-30-30 (alerts).

| Exists | Where |
|---|---|
| Model A: crew bottle (ideal gas, diluter demand), pax generators, mask deploy at cabin alt, `service()` | `src/oxygen.rs` (called from `lib.rs:2108` on reset only) |
| Model B: deep crew cylinders, regulators, pax generators, PSU doors, therapeutic O2, faults | `src/deep/oxygen/*` |
| Pax mask auto deploy flag | `OXYGEN_PAX_MASKS_DEPLOYED` (A) and `OXYGEN_MASKS_DEPLOYED` + `DEEP_OXY_PAX_MASKS_DEPLOYED` (B) |

**Duplication:** A and B share the same constants (3260 L x2 cylinders, 1850 psi, 14000 ft, 25% low) and both compute crew/pax O2. Two writers of mask state.

| Gap | Plan |
|---|---|
| Two models | Keep deep (richer, faulted); reduce `src/oxygen.rs` to the X-Plane hypoxia/felt-altitude bridge reading deep outputs |
| Bottle size is an A320 figure scaled | Use the A380 size from DSC-35-20-10 |
| Deep crew masks always stowed (`masks_donned: [false; 4]`, `deep/oxygen/live.rs:242`) | Source donning from `OXYGEN CREW MASK ON` / cockpit mask stowage box command |
| No servicing path in flight ops | GSE "oxygen" service + EFB ground services call `OxygenLive::service()`; generators replaced only by maintenance |
| No manual pax mask deploy pb | Wire overhead pb (FBW behaviour XML) to the deploy command |
| Masks not visible in cabin | New cabin OBJ: dropped mask clusters per PSU, `ANIM_hide` on deploy var (hide full range first, see OBJ lone-show rule), main + upper deck. Needs modelling. |
| Crew O2 low pressure ECAM / SD DOOR value | Check FWS reads deep pressure; feed native SD DOOR page |

Effort: M code (1.5-2 days) + cabin mask OBJ art (1-2 days, Blender).

## 1. Full CMS with fault history and post-flight report (M)

FCOM: DSC-46-20-10-40 (NSS AVNCS apps, eLogbook), DSC-46-20-20-120 (Technical Logbook pages). FCOM covers the crew view only; CMS internals are AMM, not available.

| Exists | Where |
|---|---|
| 64-slot event log: failure armed/cleared, derived system faults, C/B trip/reset, MEL defer/release, with ATA, phase, sim time | `src/maintenance_log.rs` -> `A32NX_DEEP_MAINT_LOG_k_*` |
| Per-ATA active flags | `A32NX_DEEP_MAINT_ATA_nn_ACTIVE` |
| OIT AVNCS renderers: Current/Post flight report (dedupe by fault key), ATA filter, system status by chapter, deferred items, remote C/B | `crates/oit_pages/src/avncs.rs:341-520` |
| Tech log persisted | `persistence.rs:80` (`tech_log`) |

| Gap | Plan |
|---|---|
| No fault class (1/2/3) | Class per failure from registry severity (ECAM-visible = 1, status/maint only = 2, other = 3); store with each event |
| Time is sim seconds, no UTC date / flight leg | Stamp zulu date+time and leg id (leg opens at first engine start, closes at last engine shutdown) |
| Log not persisted, 64 slots | Ring buffer of N legs (e.g. last 10) in airframe persistence |
| No producer for `ReportModel` in native OIT | Build `MaintContent::Report` from the log in the native OIT host; post-flight = closed leg |
| No message text per fault | Message = registry title + ATA chapter name; correlate ECAM alert to failure id |
| No "print" / auto print at shutdown (DSC-46-20-10-40 mentions auto print of reports) | Print to OIT file folder (`oit_files.rs`) as text |
| No intermittent/repeat counting | Count re-occurrences per leg |

Effort: M (2-3 days).

## 6. Icing from X-Plane weather (M-L)

FCOM: DSC-30-10-10 (overview, anti-ice control, ice detection), DSC-30-10-20, DSC-30-10-30 (alerts), DSC-30-10-40.

| Exists | Where |
|---|---|
| LWC/MVD from X-Plane cloud type + SAT | `src/deep/integration/weather_model.rs:109`, `weather_truth.rs` |
| Accretion per surface (2 wings, 4 nacelles, windshield), Messinger-style | `src/deep/fire_ice/icing.rs`, `integration/fire_ice_adapter.rs` |
| Wing/nacelle hot-bleed anti-ice, probe/window heat, faults | `src/deep/fire_ice/anti_ice.rs` |
| 2 ice detectors (`DEEP_ICE_DETECTOR_n`) | `src/deep/sensors/ice_detector.rs`, `sensors/live.rs:1014` |
| Anti-ice pb read from FBW vars; pushed to X-Plane `ice_*` datarefs | `deep/plugin.rs:957,998`; `lights.rs:463-468`, `engine_commands.rs:526` |
| Ice crystal icing, volcanic ash | `src/deep/environment/*` |

**Duplication:** X-Plane's own airframe icing runs in parallel with deep icing (X-Plane only gets the switch states). Two answers for "how much ice".

| Gap | Plan |
|---|---|
| Deep ice drag not applied (`lib.rs:797-804`, W124) | Publish `cd_increase_fraction`; apply via `xp_consequences` plug force; per-side for asymmetry |
| X-Plane accretes its own ice too | Pick one authority: write deep ice ratio into `frm_ice`/per-surface ice datarefs each tick so X-Plane lift/stall effects match deep; verify X-Plane does not add its own on top |
| No severe-icing level from detectors | Add rate-based severe flag |
| Ice-detected / severe-ice cautions and the ice-not-detected memo absent (not in FBW `fws_data`, not in deep registry) | Add deep ECAM triggers per DSC-30-10-30 conditions (detector + engine/wing anti-ice pb state) |
| Wing anti-ice ground 30 s limit (DSC-30-10-10) | Add ground timer in `anti_ice.rs` |
| Performance effects | Engine anti-ice bleed penalty already in FADEC limits; add ice-induced stall margin (alpha prot / VLS) only if FBW PRIM has an input, else drag only |
| Engine fan ice shedding / vibration | Nacelle ice -> vibration pickup (`sensors/engine_sensors.rs`) |

Effort: M-L (3-4 days).

## 9. Start states fully native (L)

FCOM: n/a (procedures, PRO-NOR-SOP). FBW: `systems/simulation/mod.rs` start states.

| Exists | Where |
|---|---|
| Detection of Hangar/Apron/Taxi/Runway/Climb/Cruise/Approach/Final; file/EFB override; `coldGateStart` | `src/start_state.rs:74-352`, `efb.rs:906,1437` |
| Re-spawn: walkaround, engines idle/stopped, gear lever, cockpit controls from FBW flight files, display brightness | `src/lib.rs:2000-2063` |
| Engine physics idle/stopped spawn | `engine_commands.rs:446-452` |
| Reset services oxygen only | `lib.rs:2108` |

| Gap | Plan |
|---|---|
| `DeepLayer::new` takes no start state (`deep/plugin.rs:693`) | Add `DeepLayer::seed(state)`: APU, engine accessories (oil/fuel temps, starter cool), hydraulics fluid temp, thermal zones, brakes, tyres, fuel temp, ice cleared, pneumatic ducts warm |
| No "turnaround" state | Define as Apron + APU running + GPU + doors/jetway open + water/waste partially used + O2/tyres as left; select in EFB |
| "Ready to taxi" = Taxi via FBW flight file only | Verify each deep area seeds a running-engine thermal state; doors armed, slides armed (`cabin/doors_slides.rs`) |
| Cold & dark: FBW apron state, battery off | Check deep thermal soak to OAT, cabin temp to OAT, IRS off |
| In-air states: deep systems start at ground defaults | Seed cruise thermal states, cabin pressure, fuel temps, oil; tyres cold; ice per current cloud |
| No test per state | Offline harness test per state: no alerts, no NaN, no phantom faults after 60 s |

Effort: L (3-5 days; touches every deep area's constructor).

## 3. Per-surface EHA/EBHA on 2H/2E (L)

FCOM: DSC-27-10-10 (actuators per surface, EHA/EBHA description), DSC-24 (EHA AC/DC busbars).

| Exists | Where |
|---|---|
| Per-actuator PCU servo model, modes Active/Damping/Standby, EHA/EBHA motor-pump, jam/runaway/supply loss/transducer faults | `src/deep/flight_controls/actuator.rs` |
| Allocation per actuator to PRIM/SEC + G/Y/EHA, priority arbitration | `src/deep/flight_controls/allocation.rs:186-275` |
| Backup control (BPS + BCM) | `src/deep/flight_controls/backup.rs` |
| Deep surface position overrides FBW's actuator output into X-Plane | `src/deep/integration/flight_control_surfaces.rs`, `deep/plugin.rs:1288` |
| FBW: EHA backup inhibit is a placeholder | `a380_systems/src/hydraulic/mod.rs:1585` |

| Gap | Plan |
|---|---|
| EHA power = "any AC bus live" (`flight_controls/live.rs:962`) | Use deep electrical EHA AC/DC busbars (normal AC 3/DC 2, emergency AC ESS/DC ESS) per actuator |
| Rudder actuators have no electric backup (alloc G/Y only) | Rudder = 4 EBHAs; add EHA fallback per actuator |
| Spoiler EBHA only panel 6 (`live.rs:1219`) | Add panel 5 (yellow side, AC 3 / AC ESS) |
| EHA reservoir refill on ground, EHA overheat/duty limits | Add to `ElectricMotorPump` |
| Degraded control not visible to crew | Per-actuator status vars for native SD F/CTL page; ECAM F/CTL actuator alerts if in FBW `fws_data` |
| Surface lost (both actuators off) -> law reconfig | Feed into PRIM inputs (`prim.rs`) so law degrades from actuator state, not only computer failures |
| Arbitration is GENERIC (first healthy wins) | Keep, mark GENERIC |

Effort: L (3-5 days).

## 2. Engine start, auto + manual, one code path for Trent 900 + GP7200 (XL)

FCOM: DSC-70-80-30-20 (auto start, abort list, 3 attempts, in-flight no abort, quick relight, windmill), DSC-70-80-30-30 (manual start: only EGT overlimit auto-abort), DSC-70-80-40 (dry/wet crank), DSC-70-20-20 (protections).

| Exists | Where |
|---|---|
| FBW engine state machine Off/On/Starting/Restarting/Shutting | `src/fadec.rs:632` (`next_state`) |
| Physical engine (Trent 3-spool gas path), starter torque, hung/hot starts emerge from physics | `src/physics/engine/*` |
| One protection: TGT > ground limit below 50% HP latches `start_aborted`; cleared by master off | `physics/engine/mod.rs:646-657` |
| Starter valve stuck failure, ignition fault | `engine_commands.rs:110-119` |
| Deep accessories: ATS + sprag clutch, start air valve, duty cycle, EEC channels | `src/deep/engine_accessories/starting/*`, `eec.rs` |
| Ratings incl. GP7270/7277, GP EGT display patches | `src/aircraft_options.rs:57-120` |

| Gap | Plan |
|---|---|
| No auto-start supervisor | One `StartSequencer` in the FADEC path: start valve, igniters, FMV timing per DSC-70-80-30-20 |
| Missing protections: hung, no light-up, stall, N1 not turning, start valve abnormal, starter time exceed, starter disconnect | Detectors on physics outputs; per-type thresholds |
| No 3-attempt retry with dry crank; no abort above 58.4% N2-equivalent | Auto abort -> crank -> retry x2 (not for starter fail / N1 jam) |
| No in-flight "monitor only" behaviour, no windmill/starter-assisted choice, no quick relight | Add to sequencer |
| No manual start: `A32NX_ENGMANSTARTn_TOGGLE` (FBW cockpit XML) is never read | Read the pb; manual = valve on pb+IGN START, fuel+ign on master, only EGT overlimit auto-abort |
| No dry/wet crank sequencing | CRANK selector + MAN START handled by sequencer |
| ECAM start-fault alerts | Wire FBW `fws_data` engine start-fault alerts from sequencer outputs |

**Duplication / per-type findings:**

| Finding | Where |
|---|---|
| GP7200 has no engine model: physics is Trent-only (`params.rs` constants, 3 spools); GP EGT is the Trent EGT linearly rescaled | `physics/engine/params.rs`, `aircraft_options.rs:120,272` |
| Limits split: Trent limits in Rust (`damage::trent900`), GP limits only as JS patch strings (1002/970) | `physics/damage.rs:36-81`, `aircraft_options.rs:101-111` |
| Two starter models | `physics/engine/starter.rs` (drives spool) and `deep/engine_accessories/starting/turbine.rs` (ATS with clutch, runs in parallel) |
| Two start valve models | `engine_commands.rs:110` (`starter_valve`, failure 80_000+i) and `deep/engine_accessories/starting/air_valve.rs` |
| Three engine controllers | FBW FADEC port (`fadec.rs`), ported FADEC computer in `engine_commands.rs`, deep `eec.rs` (channel selection only) |

Plan: `EngineType { Trent900, Gp7200 }` + one `EngineData` struct (spool count, design speeds, limits, start thresholds, starter cutoff, idle) loaded per type; `damage`, `physics::engine`, sequencer and display patches all read it. GP7200 needs a 2-spool param set (TCDS/public data, GENERIC where missing). Merge the starter/valve pairs: deep ATS/valve become the only ones and feed torque/flow to the physics spool.

Effort: XL (6-9 days: sequencer + manual 2-3, de-dup 1-2, GP 2-spool params + tests 3-4).

**Status 22 Oct (eng54):** done: `physics/engine/engine_data.rs` (`EngineType` + `EngineData`: limits, start schedule, idle per type; Trent from `damage::trent900`/`params`, GP from TCDS IM.E.026 + FCOM LIM-70) read by physics start limiter, `damage` start checks, `aircraft_options` EGT display/patches; `deep/engine_accessories/start_sequencer.rs` (auto start + aborts/dry crank/3 attempts, in-flight monitor-only, starter-assisted vs windmill, manual start via `ENGMANSTARTn_TOGGLE`, dry/wet crank, quick + auto relight) drives the deep start valve and ignition, 14 timeline tests x both types. Open: physics fuel valve/starter still follow FBW `fadec.rs` + `engine_commands.rs` (sequencer outputs published, not consumed); two starter/valve models not merged; stall input unwired; windmill hyd-pump disconnect and ECAM start-fault alerts not wired; GP still a rescaled Trent gas path.
