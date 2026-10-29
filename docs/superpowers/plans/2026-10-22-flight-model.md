# Flight model audit against FCOM and public data (22 Oct)

Scope: the X-Plane 12 A380 (.acf aerodynamics + Rust engine/FADEC code). FBW's flight-control laws run as ported code; X-Plane flies the .acf.
FCOM references are section ids only; values are paraphrased numbers. Values marked GENERIC are not from FBW or a public source.
Measured = read from data in hand. Estimated = public figure or rule of thumb, not measured here.

## 1. Speeds and limits

| Item | FCOM / public | Ours | Gap | Fix | Where |
|---|---|---|---|---|---|
| VMO / MMO | 340 kt / M0.89 (LIM-13) | fms_perf `VMO 340`, `MMO 0.89`; acf `_Vne_kts 340`, `_Mmo 0.89` | none | - | - |
| VFE per lever 1 / 1+F / 2 / 3 / FULL | 263 / 222 / 220 / 196 / 182 kt (LIM-13) | fms_perf constants identical; acf `_Vfe1_kts 263`, `_Vfem_kts 182` | none | - | - |
| Flap angle CONF FULL | slats 23, flaps 33 deg (LIM-13 flap table) | acf `_flap1_dn/5` and `_flap2_dn/5` = 32 | 1 deg less flap in FULL | edit below (A1) | acf |
| VMCL / VMCL-2 | 120 / 145 kt (LIM-13) | `crates/fms_perf/src/constants.rs` VMCL 123, VMCL2 147 (FBW upstream values) | VLS floor 3 kt (2 kt) high at light weight | owner of fms_perf: set 120 / 145, or keep FBW parity and note it | code (not this brief's files) |
| VLS factors | 1.15 Vs1g takeoff, 1.18 CONF 1 after ARS, 1.23 others, floored by VMCL (DSC-22-27 operating speeds) | `speeds.rs` uses the same factors | none | - | - |
| Vs1g CONF FULL at 386 t, CG 36 % | public approach speed about 138 kt at MLW (Airbus AC doc) | FBW table VLS about 135.5 kt -> Vs1g about 110 kt | within 3 kt | none | - |
| Vs1g clean at 386 t | - | FBW VLS conf0 about 185 kt -> Vs1g about 150 kt | - | X-Plane must stall near this; test card T1 | acf (verify) |
| acf stall-arc speeds | display only | `_Vso_kts 115`, `_Vs_kts 171` | arcs not used by FBW PFD; clean arc 21 kt high | edit below (A2), cosmetic | acf |
| Max altitude with flaps out | 20 000 ft (LIM-12) | not enforced (FBW FWS alert covers it) | - | - | - |
| Gear VLE / VLO | 250 kt / M0.55 (LIM-13) | FBW values | none | - | - |
| Max tyre ground speed | 204 kt (LIM-13) | tyre model in src/physics/tyre*.rs (not re-audited) | - | test card T7 | - |
| Tail-strike pitch | about 12 deg with main gear compressed (DSC-31-60-20-30, HUD) | geometry not measurable offline | unknown | test card T5 | acf (verify) |

## 2. Engines and performance

| Item | FCOM / public | Ours | Gap | Fix | Where |
|---|---|---|---|---|---|
| FCOM cruise / climb / descent tables | none in this FCOM: PER-IFT-* only describes the in-flight performance application (inputs, speed laws: turbulence 260/300/M0.85, ECON) | - | cannot check N1 / fuel per FL against FCOM numbers | use the in-sim test cards and public figures below | - |
| Max takeoff thrust per engine | Trent 900 family 70 000 to about 80 000 lbf by rating (EASA engine TCDS) | acf `_engn/0/_thrust_max_limit 80213` lbf; plugin applies the selected rating | acf sits at the top rating; plugin scales | none here | - |
| TO N1 design point | FBW 83.83 % displayed | memory of 18 and 21 Oct fixes | fixed earlier | - | code |
| X-Plane jet design points | - | `_jet_N1_takeoff 91`, `_jet_N1_climb 82`, `_jet_N1_hicrz 80`, `_jet_SFC_*` 0.42-0.44 | X-Plane uses these for fuel flow; plugin owns displayed FF (fadec.rs) | leave; check FF in T3 | acf |
| MTOW | 575 t heaviest WV (EASA TCDS A.110) | acf `_m_max 1124355` lb = 510 t | X-Plane weight UI caps at 510 t | edit below (A3), after checking the plugin weight setter does not already override | acf |
| OEW | about 277 t typical (Airbus AC doc, estimated per cabin) | acf `_m_empty 661403` lb = 300 t | 23 t heavy if the loadsheet adds cabin items on top | check loadsheet first; optional edit (A4) | acf |
| Fuel capacity | about 320 000 L, about 254 t at 0.785 kg/L (Airbus AC doc) | acf `_m_fuel_max_tot` 259.8 t | 2 % high | none | - |
| Climb to FL360 at 425 t (VNAV golden EGLL-KJFK) | real: see section 4 | prediction TOC 114.8 NM / 1078 s / 7245 kg | 18 min to FL360 looks quick for 425 t (estimated: 22-28 min typical for a heavy A380 with 250 kt below FL100) | T3 compares X-Plane against the prediction | - |

## 3. Handling

| Item | FCOM / public | Ours | Gap | Fix | Where |
|---|---|---|---|---|---|
| Roll rate, normal law full stick | FBW law, max commanded roll rate about 15 deg/s (FBW ported law) | ported | none expected; X-Plane aileron/spoiler authority must reach it | T4 | acf (verify) |
| Approach pitch CONF FULL at VAPP | about +2 to +3 deg (estimated, public pilot reports) | unknown | - | T5 | acf |
| Liftoff pitch / rotation | about 3 deg/s rotation, liftoff about 8-10 deg, margin to the 12 deg strike line (estimated) | unknown | - | T5 | acf |
| Engine-out VMCG / VMCA | FBW tables VMCG 116 kt, VMCA 114 kt at sea level; FCOM: flight-test values, not displayed | X-Plane rudder authority unknown | - | T6 | acf (verify) |
| Ground effect / flare | - | X-Plane default from wing geometry | - | T5 notes float | acf |

## 4. Real climbs (ADS-B)

OpenSky historical tracks need a login now (`/flights/*` returned 403); only `tracks/all?time=0` (live) works anonymously. Types from hexdb.io. Two live A380 tracks were found on 22 Oct; only one was a departure.

| Flight | Measured | Notes |
|---|---|---|
| SIA424, 9V-SKP, WSSS departure | 5 000 ft at 244 s / 14.7 NM; 10 000 ft at 378 s / 25.4 NM; FL150 at 613 s / 47.5 NM; FL200 at 795 s / 68.7 NM (level-off, ATC); FL210 at 892 s / 80.7 NM | track distance from first airborne point; weight unknown; fuel not in ADS-B |
| UAE11M, A6-EUM | arrival segment only | not used |

Compared with our golden (FL360 at 114.8 NM / 1078 s): SIA424 needed 795 s to FL200 including a level-off, so the golden's average 2 000 ft/min to FL360 is at the quick end. Not conclusive with one flight. Next step: rerun the live script at the OMDB/WSSS/EGLL evening A380 bank, or use a logged-in OpenSky account. Script: scratchpad `fm33/climb.py` (not shipped).

## 5. FADEC architecture (DSC-70-20-10, DSC-70-90)

FCOM: one FADEC per engine, each one EEC with channels A/B, one controlling; the controlling channel changes at each start; supply from the gearbox alternator above 12.5 % HP rotor, from the aircraft network below 10.5 % or with the alternator failed, through one EIPM per two engines.

| Item | Before | After (this change) | Left |
|---|---|---|---|
| Four independent FADECs | yes: `deep::engine_accessories` keeps one `Eec` per engine; fadec.rs keeps per-engine state | unchanged | - |
| Channel failover | yes, A then B | preferred channel honoured, failover either way | - |
| Channel swaps each start | no (always A) | `Eec::step_power` toggles on each new starter engagement | - |
| Alternator vs aircraft supply | not modelled | `PowerSource::{Alternator, Aircraft, Unpowered}`, on at 12.5 %, off below 10.5 % | - |
| Unpowered FADEC | n/a | no controlling channel, and `EEC_NO_VALID_CHANNEL` (FADEC FAULT) stays off while unpowered, so cold-and-dark gives no false caution | - |
| Aircraft supply source | - | engine's own AC bus (`Truth::ac_bus_volts[eng]`) | real wiring: channel A from AC ESS, channel B from AC 2 / AC 3 by engine pair (DSC-70-90); EIPM pairing; 15 min at power-up and 10 min FADEC GND PWR timers |
| Per-engine FADEC failures | channel A/B faults and 10 sensor faults per engine (registry ids) | unchanged | alternator failure is an input (`alternator_fault`) fed 0.0: a registry id must be appended at the END of the registry so existing ids do not shift |

Files: `src/deep/engine_accessories/eec.rs` (CRLF kept), `src/deep/engine_accessories/live.rs`. `src/fadec.rs` and `src/engine_commands.rs` needed no change: fadec.rs is FBW's per-engine engine/thrust-limit port and does not model EEC hardware.

## 6. Proposed .acf edits (not applied)

| Id | Field | Old | New | Reason |
|---|---|---|---|---|
| A1 | `P acf/_flap1_dn/5` | 32.000000000 | 33.000000000 | CONF FULL flap 33 deg (LIM-13) |
| A1 | `P acf/_flap2_dn/5` | 32.000000000 | 33.000000000 | same, second flap set |
| A2 | `P acf/_Vso_kts` | 115.000000000 | 110.000000000 | Vs1g FULL at MLW from FBW VLS / 1.23 (arc only) |
| A2 | `P acf/_Vs_kts` | 171.000000000 | 150.000000000 | Vs1g clean at MLW from FBW VLS conf0 / 1.23 (arc only) |
| A3 | `P acf/_m_max` | 1124355.000000000 | 1267658.000000000 | 575 t, heaviest weight variant (EASA TCDS A.110); only if the plugin does not already raise the limit |
| A4 | `P acf/_m_empty` | 661403.000000000 | (decide after loadsheet check) | public OEW about 277 t vs 300 t here |

A1 changes CONF FULL lift and drag: re-check T5 approach pitch and VAPP thrust after it.

## 7. In-sim test card (user; do not automate)

T1 Stall / protections, 386 t, CG 36 %, FL100, CONF 0 then FULL: idle, hold altitude in normal law. Note IAS at alpha prot (amber/black tape), alpha floor (A.FLOOR), and the lowest speed held. Expect alpha prot a little under VLS (clean about 185 kt, FULL about 135 kt) and no stall break. Then direct law: note stall IAS; expect about 150 kt clean, about 110 kt FULL.
T2 VMO / MMO: FL350 M0.85 cruise, push to M0.89 and 340 kt below FL270: overspeed at M0.89 + about 0.006, high-speed protection pitches up, AP drops at M0.93 / VMO + 15 kt.
T3 Climb and cruise balance, 425 t, ISA: takeoff CONF 1+F, managed climb 250/300/M0.85 to FL360. Record time, distance, fuel to FL360 against the golden (1078 s / 114.8 NM / 7245 kg). At FL360 M0.85 level: record N1, FF per engine, gross weight. Estimated target: about 2.9-3.2 t/h per engine at 400 t (public averages, estimated).
T4 Roll: FL100, 250 kt, full lateral stick: roll rate about 15 deg/s and bank holds at 33 deg on release, 67 deg max held.
T5 Takeoff and landing: rotate at VR about 3 deg/s; note liftoff pitch and the closest approach to the 12 deg strike line (HUD symbol). Approach CONF FULL at VAPP on a 3 deg path: note pitch (expect +2 to +3), N1, and float in the flare.
T6 Engine out: 500 t, CONF 1+F, fail ENG 1 at VMCG + 5 kt on the runway: centreline held with rudder? Fail ENG 1 at V2 in the air: wings level with less than 5 deg bank possible.
T7 FADEC: cold and dark, then aircraft power: no FADEC FAULT caution. Start ENG 1, shut down, start again, then fail channel B, then channel A: one of the two failures alone gives only the channel fault (control hands over), both give FADEC FAULT. The controlling channel itself is not published yet (unit-tested only).
