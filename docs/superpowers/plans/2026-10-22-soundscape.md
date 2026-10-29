# Cockpit and systems soundscape

Goal: every A380 system that makes a real noise has a sound driven by the native system state. Numbers marked GENERIC are our own choices, not from FlyByWire or a public source. The mapping lives in `crates/soundscape` (pure, no deps, 33 tests); wiring it into the plugin is the next step (section 5).

## 1. What plays today

Three sound paths, all on the main thread:

| Path | Code | Source audio | What drives it |
|---|---|---|---|
| FlyByWire Wwise | `src/sound/mod.rs` (`Sound::update` :627, `play_pcm` :1202), `triggers.rs`, `rtpc.rs`, `voices.rs`, `anim_triggers.rs` | `plugins/fbw_a380_systems/sound/*.PCK` + `sound.xml`, from the user's own FBW MSFS package (`find_sound_dir` :157) | `sound.xml` `SimVarSounds` stepped from L:vars (`TriggerState::step`); RTPC curves fed by `rtpc.rs`; `AvionicSounds` by name (`play_instrument_sound` :113); clicks from `sound_triggers.txt` |
| x_A380 FMOD pack | `src/sound_pack.rs` (`SoundPack::new` :826, `update` :1021, `update_extra` :990) | `fmod/*.bank` (FTSiM+ demo), `fmod_sets/{trent900,gp7200}` | `fbw/snd/*` datarefs plus renamed bank parameters (`BANK_PARAMETERS` :232); replaces the Wwise events in `REPLACED` :11 |
| Plugin-made | `src/hail_sound.rs` (synthesised loop :17, `update` :87) | generated PCM | `ENV_HAIL_SHOWER` x precipitation, TAS |
| Voice pack | `src/sound/voice_pack.rs` | `sound/voices/*.wav` (A380X_SoundMod) | callouts/FO phrases by name |

Gain groups: `src/sound_levels.rs` (`SOUND_LEVEL_*`), binaural for FMOD events: `src/binaural.rs`.

### Per system (FBW `sound.xml` lines in `fbw-a380x/.../FlyByWire_A380X/common/sound/sound.xml`)

| System | Today | Trigger | Gap |
|---|---|---|---|
| Electric hyd pumps (4 EMP) | Wwise `epump1..4` sound.xml:547 | `HYD_{YA,YB,GA,GB}_EPUMP_RPM` >= 3500 | Wwise only; pack has a single `fbw/snd/hyd_pump` flag from green/yellow > 1500 psi (`sound_pack.rs:994`), no per-pump |
| Engine-driven pumps (8 EDP) | none | - | no sound; masked by engines, exterior only |
| PTU | not fitted on A380 | - | none needed |
| Fuel pumps (21) | pack `fbw/snd/fuel_pumps` (`sound_pack.rs:992`) | any `FUELSYSTEM PUMP ACTIVE:1..21` | on/off only, no count |
| Avionics ventilation | Wwise `AEVC380` sound.xml:215 | `ELEC_AC_1_BUS_IS_POWERED` | ignores the vent fans' own state, no spin-up |
| Cabin/recirc fans | Wwise `frontcabfan` sound.xml:255 | AC 1 | no fan switch |
| Packs, flight deck air | Wwise `cockpitpacks380` :261, pack `fbw/snd/packs_on` (`sound_pack.rs:1067`) | pack 1/2 FCV flow | pack replaces Wwise; flag only |
| Packs, exterior ACM | Wwise `a380pack1ext..4ext` :275, `pack1extshut` :283 | pack flow | - |
| Bleed / start valves | Wwise `startcontactors(dc)` sound.xml:143 | `ELEC_CONTACTOR_10KA_AND_5KA_IS_CLOSED` | no bleed/crossbleed valve clunks |
| APU | pack (`apu*` in `REPLACED`), Wwise `apu*` / `apuflapopen` :132 | `APU_N_RAW`, APU flap | covered |
| Electrical contactors | Wwise `contactor` sound.xml:219 (990XG1..), `acbus1..4` :231, `bat1` :371, `TR380` :208, `ac1` :402 | contactor/bus L:vars | Wwise only; a contactor that chatters clicks every flip |
| Gear / gear doors | pack `380gup`, `380gmain`, `noselock380`, `fbw/snd/gear_doors_opening/closing` (`sound_pack.rs:1000`) | handle, door positions | per-leg locks only through the pack's own logic |
| Cargo doors | none | - | no sound |
| Brakes | Wwise `brake_max_braking` is the ROP aural, not a brake noise | - | no squeal; A380 has no brake fans (FCOM text has no brake-fan entry) |
| Flaps / slats | pack `flaps380`, `flaps380stop`, `slats380stop`, `fbw/snd/flaps_moving` (`sound_pack.rs` Aurals) | flap/slat % | covered |
| Spoilers | pack `Spoiler0/50/100` (lever), Wwise none for surfaces | handle | no surface hydraulic noise |
| Wipers | Wwise `wiper_slow/fast/park L/R` sound.xml:907 | `CIRCUIT POWER SETTING:141` | X-Plane circuit index, not the native wiper state |
| Rain | Wwise `rain_sound` :1078 | `AMBIENT PRECIP RATE` >= 0.1 | Wwise only |
| Hail | `hail_sound.rs` | `ENV_HAIL_SHOWER` | covered |
| Cockpit door | Wwise `cockpitdooropen/closed` :355 | `A32NX_COCKPIT_DOOR_LOCKED` | the L:var is not written anywhere in `src/` (grep), so likely silent |
| Seats | Wwise `seatmotor*` :951 | `SEAT_*_MOVE_*` (derived RTPC) | Wwise only |
| Oxygen mask test | none | - | no sound |
| CVR test | Wwise `cvr_test` :1069 | `A32NX_RCDR_TEST` + ground control | `RCDR_TEST` not written in `src/` (grep) |
| ECAM chimes, GPWS, TCAS, callouts | Wwise + pack + voice pack | FWS/GPWS/TCAS | covered; positional warnings in `2026-10-22-taws-aurals-obstacles.md` |

## 2. Licence status of each source

| Source | Where | Licence | Ship? |
|---|---|---|---|
| FBW `sound.xml` | `fbw-aircraft` repo | GPL-3.0 (FBW source) | yes, as trigger reference |
| FBW Wwise banks (`*.PCK`) | user's own MSFS package only; not in the GitHub repo | not stated; FBW README covers code (GPL) and 3D assets (CC BY-NC) only | no (`package_release.py` excludes `*.pck`) |
| x_A380 FMOD pack (FTSiM+ demo) | `fmod/`, `fmod_sets/` | third-party, free download | no; installer asks for the user's zip |
| A380X_SoundMod voice pack | `sound/voices/` | no licence text in the repo | no |
| `hail_sound.rs` loop | generated in code | ours (GPL with the plugin) | yes |

Rule: no audio copied from paid products, and nothing from the two third-party packs goes into a release.

## 3. Sound table (what `crates/soundscape` does)

Behaviour: L = loop (Start looped, Set on change > 0.01 gain / 0.005 pitch, Stop with fade), S = one-shot. Location names the FBW `sound.xml` node used for the same sound (`Location::node`), which `sound_nodes.txt` already positions.

| Sound | Trigger (Snapshot field -> native variable) | Behaviour | Gain / pitch | Location |
|---|---|---|---|---|
| Contactor (per index) | `contactors` <- `ELEC_CONTACTOR_*_IS_CLOSED` | S once per settled change: 30 ms hold, 0.25 s gap, change inside the gap announced at the gap's end, chatter that ends where it began is silent (GENERIC) | 0.5; open pitch 0.94 (GENERIC) | AvionicsBay (`SOUND_SPEAKER_OHVD`, as FBW) |
| AC bus up (1-4) | `ac_buses` <- `ELEC_AC_n_BUS_IS_POWERED` | S on power-up, 1 s gap | 0.6 | Overhead |
| TR hum | any AC and `dc_ess` | L | 0.2 | Overhead |
| EMP (GA, GB, YA, YB) | `emp_rpm` <- `HYD_*_EPUMP_RPM` | L on >= 3500 rpm (FBW), off < 3000 | gain 0.25 + 0.5 x rpm/7600, pitch rpm/7600 in 0.5-1.15 (GENERIC nominal) | MainGearBay (`WHEELS_L`, as FBW) |
| EDP (1A..4B) | `edp_pressurising` <- not low-press and not disconnected | L | 0.15 | Engine n |
| Fuel pumps | `fuel_pumps` <- `FUELSYSTEM PUMP ACTIVE:1..21` | one L while any runs | 0.1 + 0.3 x sqrt(running/21) | WingRoot |
| Avionics vent | `avionics_vent` and any AC | L with fan spool (1.5 s up, 4 s down, GENERIC) | 0.35 x speed, pitch 0.5-1 | AvionicsBay |
| Recirc fans | `recirc_fans` <- `OVHD_VENT_CAB_FANS_PB_IS_ON` and AC | L, same spool | 0.25 x speed | CockpitDoor (`SOUND_FWD_GALLEY`, as FBW) |
| Flight deck air | sum of `pack_flow_kg_s` | L on > 0.001 kg/s (FBW), off < 0.0005, 3 s fade (FBW) | 0.45 x flow / 2 kg/s (GENERIC nominal) | FlightDeckAir |
| Pack ACM (1, 2) | each pack flow | L on > 0.005 (FBW), off < 0.003 | 0.6 x flow, pitch 0.8-1 | PackBay L/R |
| Bleed valves | `bleed_valves` <- engine/APU/crossbleed open | S per change, 0.5 s gap | 0.4, close pitch 0.9 | WingRoot |
| Start valves | `start_valves` | S per change | as above | Overhead |
| APU start / run / shutdown | `apu_n` <- `APU_N_RAW` | S start when N rises through 1 %; L while N >= 1 %; S shutdown when N falls through 94 % after running; no start sound if already running at load | gain 0.1 + 0.5 N, pitch 0.25-1 | ApuTail (`FX_EXHAUST_APU`) |
| APU flap | `apu_flap` % | L while travelling | 0.3 | ApuTail |
| Gear motion | mean of 5 legs | L while moving | 0.7 | MainGearBay |
| Gear up/down lock (5 legs) | each leg 0..1 | S on reaching 0 or 1 from transit; hysteresis 0.005/0.02 and 0.98/0.995 (GENERIC) | 0.8 | Nose/MainGearBay |
| Gear doors (3) | `GEAR_DOOR_{CENTER,LEFT,RIGHT}_POSITION` | L while moving | 0.5 | Nose/MainGearBay |
| Cargo doors (3) | `CABIN_CARGO_DOOR_PERCENT:n` / 100 | L while moving, S latch at either end | 0.4 / 0.5 | CargoDoor n |
| Cockpit door | `cockpit_door_locked` | S open / S close, 0.5 s gap | 0.7 | CockpitDoor |
| Brake squeal | on ground, 0.5-12 kt, brake demand >= 0.3 (off < 0.2), held 0.3 s (all GENERIC) | L | 0.5 x demand x slowness | MainGearBay |
| Flap / slat motor | flap, slat surface 0..1 | L while moving, S stop after 0.3 s still (FBW pack value) | 0.5 | WingRoot |
| Spoilers | mean spoiler deflection 0..1 | L while moving | louder with rate | Wing |
| Wipers L/R | `wipers` Off/Slow/Fast | L slow or fast, S park on Off | 0.6 | Windshield L/R |
| Rain | `precip_mm` | L on >= 0.1 mm (FBW), off < 0.05 | sqrt(rate/10 mm) x (0.6 + 0.4 TAS/250) | FlightDeckAir |
| Hail | `hail` 0..1 | L on >= 0.02, off < 0.01 | 0.8 x hail x (0.4 + 0.6 TAS/250) | FlightDeckAir |
| Seat motors (4 axes) | seat axis positions | L while moving, S stop after 0.1 s (FBW fade) | 0.4 | Seat L/R |
| Oxygen mask test (2) | press-to-test held | L while held | 0.5 | Seat L/R |
| CVR test | `cvr_test` and `cvr_ground_control` (FBW `Requires`) | L | 0.4 | Pedestal (`PEDALS_LEFT`, as FBW) |

Pause stops every loop at once; loops restart on unpause from the current state. A steady state sends no commands.

## 4. Source audio plan

Already usable in the user's install, not shipped: FBW Wwise events `epump1..4`, `contactor`, `acbus1..4`, `bat1`, `TR380`, `AEVC380`, `frontcabfan`, `cockpitpacks380`, `a380pack*ext`, `startcontactors`, `apu*`, `wiper_*`, `rain_sound`, `cockpitdooropen/closed`, `seatmotor*`, `cvr_test`. The first wiring step maps soundscape ids onto these names so a user with the FBW package hears them, and onto pack events where the pack has them.

For a shippable set (needed for every row, since nothing above can be shipped):
1. Synthesise in code, as `hail_sound.rs` does: contactor click, valve clunk, latch, TR/transformer hum (400 Hz family), fan and pump whine (rotor tone + broadband), rain, CVR test tone, oxygen flow hiss, spoiler/actuator hydraulic hiss. A `crates/soundscape_synth` (no deps) can render 44.1 kHz PCM loops for each.
2. Record ourselves or take CC0/CC-BY recordings with the licence file stored next to them (freesound.org CC0 first): seat motors, cockpit door, cargo door actuator, gear motion/locks, APU start/run/shutdown, brake squeal, wipers. Each file gets a `LICENSES/sounds.md` row: source URL, author, licence.
3. Never: audio extracted from the FBW banks, the x_A380 pack, the voice pack, or any payware aircraft.

## 5. Wiring (next step, not done here)

1. `src/soundscape_feed.rs` (new): reads the native variables in the table into a `soundscape::Snapshot` each tick (registered once with `vars.get`).
2. `Command` -> playback: Wwise event name when `sound::Sound` has the package (reuse `voices::Voice` with the node position), else the shipped synthesised/recorded PCM through `xp::play_pcm16_on_bus` on the interior bus with `XPLMSetAudioPosition` from `sound_nodes.txt`. Gains multiply by the `SOUND_LEVEL_COCKPIT` / `AIRFRAME` / `WEATHER` group levels.
3. To avoid double playback, sound.xml triggers whose event the soundscape owns are dropped from `TriggerState` (same pattern as `sound_pack::replaces`), and `hail_sound.rs` is retired once the Hail row plays.
4. Shared-file edits needed: root `Cargo.toml` path dependency and one `src/lib.rs` slot; needs the lead's go-ahead.
5. In-sim check: a log line `soundscape: <n> loops, <m> one-shots in 10 s` and a cold-and-dark to APU-start run where each contactor click is heard once.

## 6. Open items

- EMP nominal speed 7600 rpm and pack nominal 1 kg/s are GENERIC; replace with the A380 FBW values once found in `a380_systems` hydraulic/air-conditioning setup.
- Native variables not yet located: wiper mode, oxygen mask test, cockpit door lock state, CVR test; `A32NX_COCKPIT_DOOR_LOCKED` and `A32NX_RCDR_TEST` are not written by `src/`.
- `src/sound/triggers.rs` module doc still says RTPC entries are skipped; `rtpc.rs` now feeds them.
