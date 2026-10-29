# Ground handling vs FCOM (22 Oct)

Sources: FCOM section ids are cited, numbers paraphrased only. Public: Airbus A380
Aircraft Characteristics (AC) - Airport and Maintenance Planning. GENERIC = not
from FBW, the FCOM or a public document.

Who does what in X-Plane: FlyByWire's `a380_systems` (run by the plugin) computes
the NWS/BWS angles and brake force; `src/handling.rs:766-772` writes them into
X-Plane's tyre steer datarefs with `override_wheel_steer`; X-Plane's own tyre model
does rolling, friction and cornering. The deep gear model
(`src/deep/gear_structure/`) runs alongside for faults, brake heat and ECAM.

## Behaviour table

| Behaviour | FCOM / public | Now | Gap | Fix |
|---|---|---|---|---|
| Pedal NWS authority | 6 deg max via pedals (DSC-32-10-40) | FBW pedal map, 6 deg cap, speed limiter (a380 hydraulic/mod.rs:4387-4390, 4296-4299) | Without a tiller axis the plugin sent pedal ratio 0 and pedals only via the handwheel path (`handling/aspects.rs` post_tick), so above 40-100 kt the nose had no steering at all | Done: pedal ratio always passed; see twist row |
| Handwheel NWS | 70 deg max (DSC-32-10-40) | FBW tiller curve to 74 deg, demand capped 70, full to 40 kt, zero by 100 kt (mod.rs:4392-4401) | none | - |
| Twist-grip / no-tiller users | (no FCOM item) | Yaw axis drove the full handwheel range at every speed: 50 % twist at 30 kt gave ~15 deg NWS | Twitchy on the take-off roll, no pedal authority at speed | Done: `yaw_tiller_share` (aspects.rs) - yaw also drives the handwheel at full share to 20 kt, fading to zero at 30 kt (GENERIC blend speeds, set to the taxi limits); pedal 6 deg authority stays at all speeds. Users with a tiller axis or the realistic-tiller setting are unchanged |
| NWS rate | 15 deg/s normal, 10 deg/s alternate (DSC-32-10-40) | FBW actuator 15 deg/s (mod.rs:1803); deep model was 30 deg/s GENERIC | Deep model wrong; no alternate-rate input in deep model | Done: deep nose 15 deg/s (`steering.rs`). Left: 10 deg/s ALTN needs a hydraulic-source input |
| Heading control (HCF) | below 50 kt, max +/-3 deg when handwheel released (DSC-32-10-40) | FBW `heading_control_steering.rs` | not audited | check in sim |
| NWS with engines off | not available (DSC-32-10-40) | FBW: needs green/LEHGS pressure + oil pressure gate (mod.rs:4686-4689) | none seen | - |
| Towing | TOW sw frees the NLG to castor (DSC-32-10-40, DSC-32-20-180 memo) | FBW disconnect + `extra_backend/pushback.rs`; deep `disc_mechanism_fail` | not audited | - |
| BWS engage | NWS > 20 deg and GS < 30 kt; rate 4.5 deg/s; yellow hyd (DSC-32-10-40) | FBW: same table (mod.rs:4208-4239); deep model was "30 % of NWS, fading to 15 m/s", GENERIC, same angle both sides | Deep model wrong | Done: deep `body_steering_angle_deg(nose, gs, left_side)` = FBW table (0 to 20 deg NWS, 15 deg inner / 11 deg outer at 70 deg, off from 30 kt), body rate 4.5 deg/s |
| BWS in X-Plane geometry | 15 deg | acf `_gear/1,2/_steerdeg_lospeed` = 8 (backup acf 2026-10-21) | acf lists 8 deg, plugin writes up to 15 deg through the override | Not edited. Check in sim that X-Plane honours 15 deg under override; if clamped, raise acf to 15 |
| 180 deg turn width | ~55 m at 70 deg NWS, 60 m runway U-turn at 5 kt, pivot braking; not with BWS fault (PRO-NOR-SOP-100) | acf nose z -99.15 ft, body z +5.45 ft -> 31.9 m nose-to-body wheelbase; nose radius at 70 deg = 31.9 / sin 70 = 33.9 m | No unit test (X-Plane tyre model decides the track) | In-sim check below |
| Brake pivot | NWS > 20 deg: body brakes and aft wing-gear brakes released (PRO-NOR-SOP-100 note) | FBW brake force is per side only | Not modelled in X-Plane | Left: needs per-gear brake split in `handling/physics.rs` |
| Autobrake | LO 2.0, 2 2.5, 3 3.0, HI 3.5 m/s2, RTO max; RTO needs > 72 kt spoilers, not below 40 kt (DSC-32-10-30-20) | FBW autobrakes.rs | none seen | - |
| ARV / accumulators | brake ARVs close > 65 kt; accumulators hold PARK BRK 12 h; NWS ARV closes > 30 kt (DSC-32-10-30-20, DSC-32-10-40) | FBW brake accumulator 1 gal (mod.rs:1786-1792); deep `ParkingBrakeAccumulator` | not audited | - |
| Brake temperature: hot | BRAKES HOT above 300 C (DSC-32-20-70 WHEEL SD) | FBW BrakeAssembly temp drives SD/ECAM | - | - |
| Brake cooling | FCOM cooling table (PRO-SUP-95-50): at SAT 15 C about 85 min from 600 to 300 C | deep stack cooled in 42 min (GENERIC area 1.2 m2) | 2x too fast | Done: area 0.6 m2 (GENERIC, calibrated), test asserts 85 +/- 17 min |
| Brake heat per energy | dT = E / (m c) | deep stack 100 kg (GENERIC) x 1650 J/kg K | stack mass not public | Test pins dT = E/(m c); a normal landing (~9 MJ per brake) gives ~55 K above start |
| Anti-skid | max manual braking with A-SKID; off in park brake / emergency (DSC-32-10-30-20) | FBW + deep slip-based release | - | - |
| Tyre pressure/heat | ~15.5 bar main (GENERIC, AC doc) | `physics/tyre_model.rs` Gay-Lussac + soak | - | - |
| Rolling resistance / breakaway | taxi thrust guidance only: U-turn preset 20 % THR outer engine, +15-25 % at MTOW; turn pad 10-15 % (PRO-NOR-SOP-100) | X-Plane `_tire_rol` (acf), no plugin term | Unknown until flown | In-sim check below, then tune acf rolling coefficient (list before editing) |
| Taxi on 2 engines | allowed in symmetric pairs, more thrust per engine, outer pair for tight turns (PRO-SUP-95-20) | FADEC idle thrust | Unknown until flown | In-sim check below |
| Crosswind on ground | HCF compensates crosswind below 50 kt; U-turn: start into wind (PRO-NOR-SOP-100) | FBW HCF + X-Plane | - | in sim |

## Files changed

- `src/handling/aspects.rs`: `yaw_tiller_share`, pedal ratio always written, reads `GPS GROUND SPEED` (kt); tests.
- `src/deep/gear_structure/steering.rs`: FCOM rates, BWS law per side; tests.
- `src/deep/gear_structure/mod.rs`: per-side BWS command (2 lines).
- `src/deep/gear_structure/live.rs`: leak test runs 3 s (nose must pass 20 deg before BWS moves).
- `src/deep/gear_structure/brakes.rs`: `SURFACE_AREA_M2` 0.6; cooling and energy tests.

## In-sim checklist (user flies; do not automate)

1. Twist grip, KJFK or YSSY, taxi at 10-15 kt: half twist should give a gentle turn, full twist a tight one; at 25 kt the same twist should be clearly softer; on the take-off roll twist should give small corrections only (6 deg pedal authority) and the rudder above ~60 kt.
2. 180 deg turn on a 60 m runway: line up 20 deg to the edge, 5 kt, full tiller/twist when the nose gear is ~1.5 m from the edge, outer engine ~20 % THR, inside brake. Pass: turn completes without leaving 60 m and with no nose tyre scrub at 5 kt. WHEEL SD should show BWS moving once NWS > 20 deg.
3. Idle creep: 4 engines idle, park brake off, light weight, level taxiway: note the speed after 60 s. Repeat on 2 engines (1+4). Expected: 4-engine idle creeps (crew brakes to hold taxi speed); 2-engine needs a little thrust.
4. Breakaway: MTOW, from standstill, raise thrust slowly; note N1/THR % at first movement. FCOM U-turn guidance suggests ~20-25 % THR on one outer engine is enough to keep the aircraft moving in a full-lock turn.
5. Brake temps after a normal landing (autobrake LO or 2, idle reverse) and taxi in: hottest brake on WHEEL SD; expected well under 300 C (no BRAKES HOT). After 15 min at the gate, note temp again for the cooling rate.
6. Pedal users: confirm nothing changed with a tiller axis bound.
