# Deliberate differences from FlyByWire, and FBW behaviour believed wrong

One row per item. "Real behaviour" cites an FCOM section id or a public document; numbers are
paraphrased, no FCOM wording. Status: **fixed** (we differ from FBW on purpose), **ported faithfully,
fix pending** (we copy FBW and the item is open), **our bug, fixed** (a port mistake, not an FBW issue).
Commit column: changes are uncommitted until the user commits; the file is given.

## Flight model and performance

| Area | FBW behaviour | Real behaviour + source | Our change | Status | Commit / file |
|---|---|---|---|---|---|
| VMCL / VMCL-2 (VLS floor, flap retraction speeds) | `VMCL` 123 kt, `VMCL2` 147 kt | 120 kt / 145 kt (FCOM LIM-13, minimum control speeds) | constants set to 120 / 145 | fixed | `crates/fms_perf/src/constants.rs` |
| CONF FULL flap angle in the .acf | FBW FULL is 33 deg; the converted .acf's last detent was 32 | slats 23 / flaps 33 deg (FCOM LIM-13 flap table) | release tuning sets `acf/_flap1_dn/5`, `_flap2_dn/5` to 33; flap ratio table FULL = 33 | fixed | `tools/acf_patch.py`, `src/handling/physics.rs` |
| Body-gear steering in the .acf | FBW BWS reaches 15 deg; .acf `_gear/1,2/_steerdeg_lospeed` was 8 | BWS table to 15 deg inner (FCOM DSC-32-10-40) | release tuning sets both to 15 | fixed | `tools/acf_patch.py` |
| Stall-arc speeds in the .acf | n/a (FBW PFD does not use them) | Vs1g from FBW VLS / 1.23 at MLW: about 110 kt FULL, 150 kt clean | `acf/_Vso_kts` 110, `acf/_Vs_kts` 150 (display only) | fixed | `tools/acf_patch.py` |
| Climb performance (golden EGLL-KJFK) | TOC 114.8 NM / 16 min / 7.2 t at 425 t | estimated 200+ NM, 25-30 min, 12-15 t for a heavy A380 (public pilot data, estimated) | none yet | ported faithfully, fix pending | `docs/superpowers/plans/2026-10-22-native-fms-vnav.md` |

## Ground handling (steering, brakes)

| Area | FBW behaviour | Real behaviour + source | Our change | Status | Commit / file |
|---|---|---|---|---|---|
| Brake pivot turn | brake force per side only, all brakes of a side alike | NWS above 20 deg: body gear brakes and aft wing-gear axle brakes released (FCOM PRO-NOR-SOP-100, 180 deg turn) | X-Plane per-side brake ratio scaled to 2/8 (forward wing axle of FBW's 4 wing + 4 body brakes) above 20 deg NWS, parking brake excluded | fixed (X-Plane side) | `src/handling/physics.rs` `pivot_brake_share`, `src/handling.rs` |
| Alternate NWS rate | single nose actuator at 15 deg/s whatever the source | 10 deg/s on the alternate (LEHGS/accumulator) supply, 15 normal (FCOM DSC-32-10-40) | tyre angle to X-Plane follows FBW's at 10 deg/s while the green pressure switch is off; deep nose model also 10 deg/s on green loss | fixed | `src/handling/physics.rs` `nose_steer_follow`, `src/deep/gear_structure/steering.rs`, `mod.rs` |
| NWS hydraulic source | nose steering actuator on the YELLOW circuit (a380 hydraulic/mod.rs:2467-2473) | normal NWS on GREEN, alternate on the steering LEHGS + NWS accumulator (FCOM DSC-32-10-40) | fork: NWS actuator fed by GREEN while the green pressure switch is on, else by a steering LEHGS (all engines running) + NWS accumulator; volumes go to green or the accumulator; accumulator size GENERIC | fixed | fork `fbw-a380x/src/wasm/systems/a380_systems/src/hydraulic/mod.rs`, `alternate_supply.rs` (`NoseWheelSteeringAlternateSupply`) |
| Heading control function | `heading_control_steering.rs`, below 50 kt, +/-3 deg | below 50 kt, max +/-3 deg, off above 50 kt (FCOM DSC-32-10-40) | none needed | matches | - |
| Brake accumulators | one yellow alternate-brake accumulator, 1 gal (A320-derived comment, mod.rs:2002-2004) | four brake accumulators: forward WLG, aft WLG, left BLG, right BLG (FCOM DSC-32-10-30-30) | fork: four isolated alternate brake circuits, each with its own accumulator (sizes GENERIC, FBW's old 1 gal / 1000 psi each); WLG fed by GREEN-side LEHGS/ARV, BLG by YELLOW-side; brake LEHGS runs with any engine running (GENERIC, no electrical dependency); `HYD_BRAKE_ALTN_LEFT/RIGHT_PRESS` now body gear, `HYD_BRAKE_ALTN_ACC_PRESS` the lower BLG accumulator | fixed | fork `fbw-a380x/src/wasm/systems/a380_systems/src/hydraulic/mod.rs`, `alternate_supply.rs` (`A380AlternateBraking`) |
| Alternate refill valves | not modelled | brake ARVs close above 65 kt, steering ARV above 30 kt, stay closed for the flight (FCOM DSC-32-10-30-10-10, DSC-32-10-40) | fork: brake ARVs open once an engine runs, steering ARV once all run; latched closed above 65 / 30 kt until engines are shut down on ground (reset condition GENERIC); fluid through a closed ARV goes to the LEHGS, not the aircraft circuit | fixed | fork `fbw-a380x/src/wasm/systems/a380_systems/src/hydraulic/mod.rs`, `alternate_supply.rs` (`AlternateRefillValve`) |
| Parking brake gear | audited: park brake set max demand on the single alternate circuit, which braked all four gears (wing + body) | body gear brakes only; wing gear added above about 50 % N1 (FCOM DSC-32-10-30-30) | fork: park brake demands body gear only; wing gear gets pedal demand, or full when any engine corrected N1 > 50 %; force factor weights alternate pressure 1/4 per WLG circuit, 1/2 BLG. X-Plane hold: see the parking brake hold row | fixed | fork `fbw-a380x/src/wasm/systems/a380_systems/src/hydraulic/mod.rs`, `alternate_supply.rs` |
| Normal brake supply | normal braking puts all 16 brakes (wing and body gear) on one GREEN `NORM` circuit; body gear reverts to alternate only when green is lost | WLG normal on GREEN, BLG normal on YELLOW; left BLG, right BLG and WLG revert to alternate independently (FCOM DSC-32-10-30-10) | fork: second normal circuit `NORM_BLG` on YELLOW for the body gear (new vars `HYD_BRAKE_NORM_BLG_LEFT/RIGHT_PRESS`, volumes to yellow); WLG and BLG normal availability judged separately (FBW's 1305/2176 psi hysteresis each); autobrake arming needs both; force factor weights normal WLG and BLG 1/2 each | fixed | fork `fbw-a380x/src/wasm/systems/a380_systems/src/hydraulic/mod.rs` |
| Parking brake pressure | park brake limited to 2103 psi (2538 with pedals), same limit on wing gear | about 3600 psi on BLG with accumulators full (FCOM DSC-32-10-30-30-60); WLG added above 50 % N1 at a 1700 psi limit (DSC-32-10-30-30-50) | fork: BLG park limit 3600 psi, WLG park limit 1700 psi; pedal-only alternate limits unchanged (2538, 1160 with A-SKID off) | fixed | fork `fbw-a380x/src/wasm/systems/a380_systems/src/hydraulic/mod.rs` |
| Parking brake hold in X-Plane | n/a (plugin) | parking brake force comes from BLG accumulator pressure (FCOM DSC-32-10-30-30-60) | X-Plane `parkbrake` = lower of `HYD_BRAKE_ALTN_LEFT/RIGHT_PRESS` / 3600 psi while the lever is on, else 0; drained accumulators no longer hold. Wing gear above 50 % N1 still arrives through the force factors | fixed | `src/handling.rs`, `src/handling/physics.rs` `park_brake_hold` |
| ACCUS REINFLATE | no pb or var (only the ECAM procedure line) | pb on the PARK BRK panel runs the BLG LEHGS with engines off (FCOM DSC-32-10-30-10, DSC-32-10-30-30-60) | none: with engines off the BLG accumulators refill only once an engine runs (LEHGS + ARV); a yellow electric pump does not refill them (plugin test `the_body_gear_brake_accumulators_do_not_refill_from_yellow_with_engines_off`) | not modelled (no FBW var) | `src/start_state.rs` |
| Towing | `BypassPin` from the pushback tug frees the NWS; plugin stops overriding steering while a tug is connected | TOW switch lets the NLG castor (FCOM DSC-32-10-40) | none | matches | `src/handling.rs` |

## Native FMS: lateral (LNAV)

From `docs/superpowers/plans/2026-10-22-native-fms-lnav.md`; all ported faithfully, fix pending.

| Area | FBW behaviour | Real behaviour + source | Our change | Status | Commit / file |
|---|---|---|---|---|---|
| Minimum bank | GeometryFactory passes `SegmentType.Departure` to every leg, so min bank is always 10 deg | enroute legs should allow 5 deg (FBW's own `minBank` intent) | adapter sets `enroute = false` everywhere (parity) | ported faithfully, fix pending | `crates/fms_lnav` |
| LNAV discrete word | LnavDriver reads FMGC 1 discrete word 2 for FMGC 2 | per-FMGC word | none | ported faithfully, fix pending | `crates/fms_lnav` |
| CA leg termination | origin altitude 0, 2000 ft/min, 175 kt constants | aircraft climb performance from the real airport elevation | none | ported faithfully, fix pending | `crates/fms_lnav` |
| FA leg cross track | measured from fix to termination even after an inbound transition moved the path start | XTK from the flown path | none | ported faithfully, fix pending | `crates/fms_lnav` |
| CI leg start | throws when the inbound guidable is not computed yet | ordering should not raise | none | ported faithfully, fix pending | `crates/fms_lnav` |
| Course capture | track change rounded to 0.1 deg before building the arc | no rounding error | none | ported faithfully, fix pending | `crates/fms_lnav` |
| Transition picker | HX logs a copy-paste DF message; FM/VM -> some legs and RF -> TF/CF return no transition | every legal sequence connected | none | ported faithfully, fix pending | `crates/fms_lnav` |

## Native FMS: vertical (VNAV)

From `docs/superpowers/plans/2026-10-22-native-fms-vnav.md`; all ported faithfully, fix pending.

| Area | FBW behaviour | Real behaviour + source | Our change | Status | Commit / file |
|---|---|---|---|---|---|
| Managed econ speeds | cruise / climb Mach / descent Mach are constants (310 kt, M0.84, M0.85); CostIndex CAS uses A320 weights | CI- and weight-based econ speeds | none | ported faithfully, fix pending | `crates/fms_vnav` |
| CI law | climb uses (CI/999)^2, descent CI/999 linear, different bounds | consistent law | none | ported faithfully, fix pending | `crates/fms_vnav` |
| MMRC search | below-tropopause delta/theta at every altitude | tropopause-aware atmosphere | none | ported faithfully, fix pending | `crates/fms_vnav` |
| Descent speed limit | `underAltitude` used without a null check | no limit -> no target | crate skips the target when there is no limit | ported faithfully, fix pending | `crates/fms_vnav` |
| Climb winds | A380 sorts climb winds descending but the forecast treats entry 0 as the lowest | lowest entry used near the ground | none | ported faithfully, fix pending | `crates/fms_vnav` |
| Geometric path | `lastTarget` is a const null, so the previous segment's reason is never carried | carried over | none | ported faithfully, fix pending | `crates/fms_vnav` |

## Native FWS

From `docs/superpowers/plans/2026-10-22-native-fws.md` and `2026-10-22-native-fws-data.md`; all ported faithfully, fix pending.

| Area | FBW behaviour | Real behaviour + source | Our change | Status | Commit / file |
|---|---|---|---|---|---|
| T.O CONFIG in phase 2 | `FwsFlightPhases.toConfigTest` never assigned; only the 120 s timer brings up the T.O memo | the T.O CONFIG pushbutton brings up the memo | none | ported faithfully, fix pending | `crates/fws` |
| Engine 4 TLA | `eng4TLAFullPwr` uses `eng3TLA` | engine 4's own TLA | none | ported faithfully, fix pending | `crates/fws` |
| AIR DATA F/O on 3 | `airDataFoOn3` tests the captain's knob position; knob read after the phase logic | F/O knob position | none | ported faithfully, fix pending | `crates/fws` |
| NO SMOKING memo | `healthInjector` forces the memo var true every update | follows the switch | none | ported faithfully, fix pending | `crates/fws` |
| Initial flight phase | phase starts at 1, subscription only at init; spawning in the air leaves `FWC_FLIGHT_PHASE` 0 until the next transition | correct phase from the first update | none | ported faithfully, fix pending | `crates/fws` |
| Phase inputs | radio height, N1, fire button and engine-running read from the previous update | current update | none | ported faithfully, fix pending | `crates/fws` |
| T.O CONFIG pulse | `toConfigPulseNode` written twice per update | one edge detection | none | ported faithfully, fix pending | `crates/fws` |
| Fault suppression timing | suppressing alert's confirm time measured from the suppressed alert's activation | from the suppressing alert's own activation | none | ported faithfully, fix pending | `crates/fws` |
| SFAIL ordering | presented-failures list sorted in place while computing the SD page | order unaffected by the SD | none | ported faithfully, fix pending | `crates/fws` |
| FMC power | FMC A/B/C fault uses DC ESS / DC 2 / DC 1 (not checked against the real bus allocation) | real bus allocation (not yet sourced) | none | ported faithfully, fix pending | `crates/fws` |
| FWS data | 7 `notActiveWhenItemActive` ids point at no alert; 990900011 lists 15 show-entries for 12 items | consistent tables | pinned in tests | ported faithfully, fix pending | `crates/fws_data` |
| Engine start sequencing | FBW `fadec.rs::next_state`: starter on IGN START + master, fuel at idle gate; no abort, retry, dry crank, manual start or relight logic; ENGMANSTARTn never read | FCOM DSC-70-80-30-20/-30, DSC-70-80-40, LIM-70: auto aborts (hot/hung/no light-up/EGT/stall/N1 jam/valve/starter time/starter fail), 2 retries after 30 s dry crank, no abort in flight or above 58.4 % HP, manual start only EGT-protected, quick relight, windmill above 260 kt | deep `start_sequencer.rs` per `EngineData`; drives deep start valve + ignition, publishes `A32NX_ENG_n_START_*` | added; physics fuel/starter not yet driven by it | `src/deep/engine_accessories/start_sequencer.rs` |

## Port mistakes fixed

| Area | FBW behaviour | Real behaviour + source | Our change | Status | Commit / file |
|---|---|---|---|---|---|
| Copilot approach checklist trigger | FBW A380 `FmgcFlightPhase`: Approach = 5, GoAround = 6 (`fbw-a380x/src/systems/shared/src/flightphase.ts`) | - | `FMGC_APPROACH` 6 -> 5 (it fired on go-around, not approach) | our bug, fixed | `src/copilot.rs` |
