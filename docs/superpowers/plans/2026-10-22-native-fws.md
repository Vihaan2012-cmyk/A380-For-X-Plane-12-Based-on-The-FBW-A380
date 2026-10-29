# Native FWS (Group 3 of the CEF removal inventory)

Goal: FlyByWire's A380X Flight Warning System (`FwsCore` and the Fws* classes in `systems-host/CpiomC/FlightWarningSystem`) as native Rust in the plugin, proven against recorded FBW behaviour. Phase A is the oracle, Phase B the translation.

## Phase A: the oracle

FBW's own `SystemsHost.js` (the whole hidden VCockpit22 view: FwsCore, PseudoPRIM, GPWS, TCAS, ...) runs offline in the plugin's QuickJS `Cockpit` with a recording host, the same approach as `fms_oracle.rs`. No deep-ecam source patches are applied: the goldens are pristine FBW.

### File map

| File | Role |
|---|---|
| `tools/fws_oracle/make_pkg.py` | builds `E:/fbw-build/fws-oracle-pkg` (html_ui of the FBW build + `systems-host.html`, panel.cfg with VCockpit22 only, panel.xml) |
| `tools/fws_oracle/scenarios/*.json` | scenario scripts |
| `tools/fws_oracle/same.py` | byte comparison of two golden directories |
| `src/js/msfs/fws_oracle.rs` | host (`FwsHost`), scenario runner, `record_fws_oracle` ignored test (cfg(test), registered in `src/js/msfs/mod.rs`) |
| `E:/fbw-build/fws-oracle/<name>.json` | goldens |
| `E:/fbw-build/fws-oracle/<name>.reads.json` | input vars FBW read that the scenario never set (coverage gaps; not part of the golden) |

Inputs come from the complete simulator state dumps in `E:/fbw-build/emu-snapshots/emu-NN-*.json` (about 5000 vars each: `[isLocalVar, name, value]`). Vars the FWS itself writes (`A32NX_FWC_*`, `FWS`, `MASTER_*`, `ECAM_*`, `STATUS_*`, `TO_CONFIG_*`, `NO_SMOKING`, `CABIN_READY`) are skipped when a state is applied so the FWS starts from its own empty state.

### Scenario steps

`{"state": "emu-12-approach"}` apply a snapshot (every var, L: and A:), `{"set": {"L:NAME": v, "A:NAME": v}}` override, `{"run_ms": n}` run n ms in 50 ms ticks, `{"snapshot": "label"}` record the latest value of every output so far.

### Scenarios

1. `cold_dark_powerup`: cold and dark, batteries, ground power, APU.
2. `engine_start`: APU, engines idle.
3. `takeoff_config_warning`: taxi with flaps 0, TO CONFIG test button, takeoff roll with the warning.
4. `engine_fire`: cruise, then engine 2 fire.
5. `hydraulic_failure`: cruise, then green hydraulic loss.
6. `approach_gear_not_down`: approach, gear lever up and gear unlocked.
7. `landing_memos`: approach, landing configuration, rollout.
8. `elec_emer_decompression`: electrical emergency and cabin decompression.

### Output schema

```
{ "scenario": name, "package": "...", "states": [...],
  "events": [ {"t": ms, "var": "L:NAME", "value": n}            // output var change
            | {"t": ms, "str": "NAME", "value": s}              // string var change
            | {"t": ms, "gd": key, "json": "..."}               // generic data / bus topic (fws_* among them)
            | {"t": ms, "trigger": name, "args": "..."}         // Coherent trigger
            | {"t": ms, "call": name, "args": "..."}            // Coherent call (PLAY_INSTRUMENT_SOUND = aural)
            | {"t": ms, "key": name, "values": [..]} ],         // K:/H: event sent
  "snapshots": [ {"label", "t", "vars": {name: n}, "strings": {...}, "bus": {key: json}} ] }
```

Only changes are recorded (a repeat of the previous value is dropped). Deterministic: fixed date, seeded `Math.random`/crypto, fixed 50 ms step, simulation time derived from the step count. Determinism check: record twice, `tools/fws_oracle/same.py`.

Run: `FWS_ORACLE_PKG=... cargo test --release --features js --lib record_fws_oracle -- --ignored --nocapture`, outputs to `FWS_ORACLE_OUT` (default `E:\fbw-build\fws-oracle`).

## Phase B: the translation

Module `src/fws/` (plugin crate, no dependency on the JS runtime), own Rust equivalents of the framework pieces FwsCore leans on (confirm nodes, monostables, SR latches, rising edge, pulse, subjects as plain struct fields; no EventBus). Comparator `src/fws/compare.rs` replays a golden's input timeline into the Rust FWS and diffs the output events.

Order: logic nodes, flight phases, memos, alert evaluation loop (`FwsCore.update`), abnormal sensed procedures, ECP/ECL, aural/sound manager, inhibitions. Each step lands only when the comparator matches the goldens for the outputs it covers.

## Status (22 Oct)

Phase A done. Eight scenarios recorded to `E:/fbw-build/fws-oracle/` (275 KB to 937 KB each, 3.9 MB total, 800 to 3550 events, 0 JS errors); two full runs are byte-identical (`same.py`: 16/16). A run of all eight takes about 13 s. Recorded outputs: 56 to 81 L: var writes per scenario (master warning/caution, FWC phase, TO/LDG memo, `EWD_LOWER_*`/`PFD_MEMO`/`PFD_LIMITATIONS` strings, discrete words, FWS health), the `fws_*` bus topics (decoded from `xp-ui.bus` and `EB_EVENTS`), aural requests (`PLAY_INSTRUMENT_SOUND` calls and `A32NX_AUDIO_*` vars), K: events.

Known gaps: the `emu-NN` snapshots do not carry every discrete other computers produce, so the TO and LDG memo lines stay empty in the snapshots; memo coverage needs hand-set inputs (`.reads.json` lists the 150 vars FwsCore read that no snapshot set). `GET_AIR_TRAFFIC` answers `[]`, `fetch` answers 404.

Phase B not started: `src/lib.rs` is being edited by other agents, so the `src/fws/` module registration waits.

## Phase B status (fwsB28, 22 Oct)

Standalone crate `crates/fws` (own `[workspace]`, `cargo test --manifest-path crates/fws/Cargo.toml`; ignored `replay_goldens_report` prints per-scenario counts). The `src/fws/` registration in the plugin crate is not needed; the crate has no dependency on the JS runtime.

Translated from FBW: `fbw-common/src/systems/instruments/src/NXLogic.ts` 13-262 (`nodes.rs`), `shared/src/arinc429.ts` (`arinc.rs`, discrete bits are read from the float32 value, as FBW does), `UpdateThrottler.ts` (125 ms, offset fitted from the goldens), `FwsFlightPhases.ts` 1-357 (`phases.rs`: phases 1-12, T.O memo, LDG memo; not the altitude-alert part), `FwsCore.ts` update 2818-2960 (ECP failed, TO CONFIG pulse chain, update gating, `healthInjector`), acquisition inputs for the phase logic and `FwsMemos.ts` 598-670 (T.O and LDG memo lines to `L:A32NX_EWD_LOWER_LEFT_LINE_n`), `FwsCore.ts` 5905-5990 left-line output. Phase inputs lag one update, as in FBW (acquisition runs after `flightPhases.update`).

Comparator (`compare.rs`) replays a scenario's state/set/run steps into `Fws::tick` at 50 ms and diffs `(t, name, value)` against the golden for the covered outputs: `FWC_FLIGHT_PHASE`, `FWC_TOMEMO`, `FWC_LDGMEMO`, `NO_SMOKING_MEMO`, `CABIN_READY`, `FWS_AUDIO_VOLUME`, `EWD_LOWER_LEFT_LINE_*`. Result: approach_gear_not_down 15/15, elec_emer_decompression 15/15, engine_fire 15/15, engine_start 15/15, hydraulic_failure 15/15, takeoff_config_warning 15/15, landing_memos 32/32, to_ldg_memos 52/52, cold_dark_powerup 5 matching, 10 golden-only, 10 ours-only (the FWS power/startup gate `handlePowerChange` and the 60 s self-test are not ported, the left lines appear at 3150 instead of 1100).

New scenario `tools/fws_oracle/scenarios/to_ldg_memos.json` (hand-set ARINC RA, LGCIU, FCDC words, flaps, seat belts, autobrake): 8 min of simulated time covering phases 2, 3, 4, 6, 8, 9, 10, 11, the T.O memo after the 120 s timer, spoilers armed/disarmed lines, the LDG memo in approach, gear/flaps/spoiler lines. Recorded to `E:/fbw-build/fws-oracle/to_ldg_memos.json` (existing goldens untouched).

Not translated yet: the alert loop (`FwsCore.ts` 5500-5990), all `ewdAbnormal` conditions (about 45 alerts fire in these scenarios, from unset snapshot inputs), master warning/caution, CRC/SC aural, ECAM page requests, right-hand memos, the `fws_*` bus topics, `FwsSoundManager`, INOP SYS, limitations, information. Master warning/caution, `ECAM_SFAIL`, `STATUS_NORMAL`, `ECAM_SD_PAGE_TO_SHOW` after the first alert are therefore only emitted as the initial values. T.O CONFIG normal evaluation (5424-5448) is stubbed to false.

## Phase B status (fwsC31, 22 Oct)

Power/startup gate done (`core.rs`): `Fws` is now the SystemsHost layer around a recreatable core. SystemsHost destroys the core when both CPIOM C are unavailable (`L:A32NX_CPIOM_C1_AVAIL`/`C2`, read one tick late as the ConsumerSubject is) and builds a new one (fresh throttler, phases, alert state) when one comes back; FWS1/FWS2 health = CPIOM C available and startup completed. `handlePowerChange` on DC ESS: startup after 0 ms, or `CONFIG_SELF_TEST_TIME` (12, FBW default) x 5 s when `L:A32NX_COLD_AND_DARK_SPAWN` and not quick mode; the timer fires on the next tick. `init()` (on startup) attaches the left-line, ABN PROC impact, master caution/warning and flight-phase outputs. cold_dark_powerup now 19/19 (the core is destroyed at 1050 and rebuilt at 3050 when the batteries come on; startup 3150). `FWS1_IS_HEALTHY`/`FWS2_IS_HEALTHY` added to the covered outputs: all nine scenarios 0 differences.

Alert loop (`alerts.rs`, translated from `FwsCore.ts` 5498-5970): data-driven from `fws_data::ALERTS` (sensed + non-sensed entries in numeric id order, as `Object.entries` orders integer keys): activation time per alert, monitor confirm time (0.6 s default), `notActiveWhenItemActive` suppression, flight-phase inhibition of new alerts, presented/recall/all-current lists, master caution/warning requests, non-cancellable warning count, CRC/SC aural keys, failure system count, `ECAM_FAILURE_ACTIVE`, `ECAM_SFAIL` (in-place priority sort of the presented list). Not yet: clear/recall buttons, procedure item states, deferred procedures, STS/INOP/LIMITATIONS/INFO keys, right-hand memos, `STATUS_NORMAL`, SD page requests, sound manager.

Conditions (`conditions.rs`): `Signals` holds the `this.fws.<name>` booleans that are acquired so far (FMC A/B/C and FMS 1/2 faults, FDAC/TADD temperature control, RMP 1-3, anti-skid, PRIM/SEC 1-3 fault conditions); an alert's `simVarIsActive` is evaluated when its expression is a single field, an `and()` of fields or a `([a, b]) => a && b` lambda over fields that are all acquired. 25 alerts evaluable. Check: `ported_alert_procedures_match_the_recorded_goldens` compares, per ported alert, the times it enters and leaves the golden `fws_abn_sensed_procedures` topic with ours: 0 differences in all nine scenarios (cold_dark 19 transitions, elec_emer 13, engine_fire 7, ...). This also confirms the confirm-time and update-grid timing of the loop.

Master caution/warning, `ECAM_SFAIL`, `ECAM_FAILURE_ACTIVE` are computed but not yet in `COVERED`: they depend on every firing alert. 42 alerts fire across the goldens; 12 are ported (211800010, 211800041, 221800001, 221800004, 221800006, 230800018, 271800036/37/38/58/60, 320800008). Still to port (signals in FwsCore.ts update): 211800046/47/52-55, 213800001, 213800005 (CPIOM B CPCS words via `populateCpiomBWords`), 260800006, 271800009, 271800069, 272800001/2, 281800076, 290800005-10/12/39/40, 320800014, 340800008, 341800015/16/19, 999800001/3 (secondary failures). Once those are in, add the four outputs to `COVERED`.

## Phase B status (fwsD36, 22 Oct)

All 42 alerts that fire in the goldens are ported (67 evaluable in total). `conditions.rs` now evaluates `this.fws.x`, `and()`/`or()` over fields and any `([a, b]) => ...` lambda built from `!`, `&&`, `||` and parentheses. New acquisitions (FwsCore.ts update): engine bleed off, hot air pushbuttons, hydraulics (G/Y abnormal low pressure with the 1 s engine/phase confirm, engine pump faults with the 10 s off confirm, yellow elec pumps), ADR 1-3 faulty, CPIOM B CPCS words (`populateCpiomBWords` reachability, word selection, excess cabin altitude, auto control fault), the FWS on-ground logic (LGCIU/RA votes, `aircraftOnGround`), FDU engine 2 fire, direct law, flap/slat T.O config SR latches, overspeed (Vmo/Mmo/Vle/Mle/VFE from `PerformanceConstants.ts`, ADR on the previous update's knob), TCAS standby, TCAS 1/2 fault, ROW/ROP/BTV lost, FQMS ZFW not set.

Comparator: var writes that other SystemsHost computers make in the golden (TCAS mode/fault, GPWS, ...; every `var` event not FWS-owned) are now fed back as inputs at their recorded time (applied on the first tick after `t`). Without this the TCAS STBY alert came 14 s early.

`COVERED` now also has `MASTER_CAUTION`, `MASTER_WARNING`, `ECAM_SFAIL`, `ECAM_FAILURE_ACTIVE`: 0 differences in all nine scenarios (25 to 64 matching writes each). Alert transitions: 0 differences in all nine except one open case, excluded by name in the test (`OPEN`): in elec_emer_decompression the golden drops 213800001 EXCESS CAB ALT for one update at 25050 (back at 25700 after the confirm time), right after the decompression state is applied. Both the CPCS word (bit 13 set) and `PRESS_MAN_EXCESSIVE_CABIN_ALTITUDE` (1) say true in that state, so the cause is not in the condition; to be looked at with the procedure-list handling (FwsAbnormalSensed) in step 2.

Not done yet: procedure item states, clear/recall, deferred procedures, STS/INOP SYS/LIMITATIONS/INFO, right-hand memos, `STATUS_NORMAL`, SD page requests, the remaining alert conditions.

FBW behaviour believed wrong (ported faithfully, not changed):
1. `FwsFlightPhases.toConfigTest` is never assigned (FwsCore keeps its own `toConfigTest`), so pressing T.O CONFIG in phase 2 does not bring up the T.O memo; only the 120 s all-engines timer does. On the real aircraft the T.O CONFIG pushbutton in phase 2 triggers the T.O memo.
2. `eng4TLAFullPwr` in `FwsFlightPhases` uses `eng3TLA` (FwsFlightPhases.ts, engine 4 block).
3. `airDataFoOn3` is `airKnob === 0`, the same condition as `airDataCaptOn3`; the F/O side should use the F/O ON 3 knob position. `airKnob` is also only updated after the phase logic runs.
4. `healthInjector` forces `L:A32NX_NO_SMOKING_MEMO` true every update, so the memo never follows the real NO SMOKING switch.
5. `flightPhase` starts at 1 and the `FWC_FLIGHT_PHASE` subscription is only attached at init, so when the first update already yields another phase (spawn in the air) the var is never written and stays 0 until the next transition (matches the golden: no write in the cruise-start scenarios). The "uninitialised phase" branch (`flightPhase === null`) is dead code.
6. The phase logic reads its radio height, N1, fire button and engine-running inputs from the previous update (acquisition happens after `flightPhases.update`).
7. `toConfigPulseNode` is written twice per update (buffer, then raw button), so the second write overwrites the first edge detection state.
8. In `itemIsActiveConsideringFaultSuppression`, the suppressing alert's confirm time is measured from the suppressed alert's activation time (`failureActivationTime.get(key)`), not from the suppressing alert's own (`val`).
9. `presentedFailures.sort(...)` in the ECAM_SFAIL block sorts the persistent presented list in place, so the order of the presented failures changes as a side effect of computing the SD page.
10. `fmcAFault`/`fmcBFault`/`fmcCFault` use DC ESS/DC 2/DC 1 as the FMC power condition (FBW's own mapping, not checked against the real bus allocation).
11. `eng4APumpFault`: `!this.eng4APumpAuto` and `this.engine4Running && ... && !this.greenYellowAbnormLoPressure` test the Subject objects, not their values, so the off-confirm input is always false and the second term is always false: Y ENG 4 PMP A PRESS LO can never fire.
12. `flapConfigWarning` ORs in `slatConfigSr.read()` (the slat latch, read before it is written this update) instead of `flapConfigSr`, so FLAPS NOT IN T.O CONFIG in phases 3-5 follows the slats, one update late.
13. ADR CAS/Mach for the overspeed alerts use `airKnob` from the previous update (the knob is read further down in the update).

## Phase B notes (fwsE56, 22 Oct)

No code change; time box spent on the 213800001 blip. Ruled out: the condition inputs (CPCS words B1-B4, reachability, `PRESS_MAN_EXCESSIVE_CABIN_ALTITUDE` are all true-making in both emu-17 and emu-19), key collisions in `allSuppressableItems` (213800001 is only in `ewdAbnormalSensed`), suppression (`notActiveWhenItemActive` empty), CLR/RCL buttons (not in the states), a second FwsCore (SystemsHost builds one, FWS2 health is only a flag). What the golden shows: at 25050 the first `fws_abn_sensed_procedures` publication already lacks 213800001 (deleted first, by insertion order, in the "delete inactive failures" loop at FwsCore.ts 5822), so it was not in `allFailureKeys` that update; it is re-added at 25700 as a new warning, i.e. its activation time was reset at the 25050 update (0.6 s confirm + 125 ms grid). The only writer of `excessCabinAltitude` is FwsCore.ts 4276, so the reset has to come from the emulator side (state application order or `E:SIMULATION TIME` handling in the oracle harness); next step is to log `excessCabinAltitude` and `simTime` per update in the oracle for t 24900-25100. Small port difference found: `populateCpiomBWords` keeps the old value and only sets SSM FW when not reachable (ours uses an empty word); no effect on the bit reads.

## Phase B status (fwsF60, 22 Oct)

`L:A32NX_STATUS_NORMAL`, `L:A32NX_ECAM_SD_PAGE_TO_SHOW` and `L:A32NX_ECAM_SD_CURRENT_PAGE_INDEX` are now in `COVERED`: 0 differences in all nine scenarios (28 to 69 matching writes each); all 14 crate tests pass. `sd.rs` is `FwsSystemDisplayLogic.update` (phase pages, APU/ENG/STS checks, fail page takeover writing CURRENT_PAGE_INDEX -1), without the ALL button cycle and the STS next page / MORE h-events. `STATUS_NORMAL` is computed from the STS id lists of the current procedure alerts (info, INOP SYS all phases / appr-ldg, limitations all phases / appr-ldg, LAND ASAP/ANSA when airborne), taking a non-empty static `IdList` as non-empty.

Not done yet: the STS key lists themselves (`fws_information`, `fws_inop_sys_*`, `fws_limitations_*` topics, `PFD_LIMITATIONS_LINE_*`, `fws_show_sts_indication`) need the `FwsInopSys`/`FwsInformation`/`FwsLimitations` dictionaries with their own conditions (1.0 s confirm), the run-time (`conditional`) id callbacks and `messagePriority` from the EcamLimitations colours; the comparator also has no topic diff yet. Then right-hand memos, procedure item states and clear/recall, deferred procedures (`crates/fws_ecl` deferred.rs), remaining conditions. 213800001 blip still OPEN.

## Phase B status (fwsJ90, 22 Oct)

All of FwsCore is native and the runtime expression evaluator (`conditions.rs`, `alerts.rs`) is gone. `tools/fws_gen/gen.cjs` now also translates `FwsCore.update` from the throttle check to CLEAR AND RECALL (statements, locals, node writes, word registers, sound manager calls) into `gen/update.rs` and the FwsCore mapped subjects into `gen/mapped.rs`; 18 mapped subjects are hand-written (`hand.rs`). Hand-written: `fwc.rs` (Core: fields, own-var overlay, flight phases and altitude alert, consumers, constructor/`init()` subscriptions, startup gate), `procs.rs` (CLR/RCL, the abnormal loop with item states, deferred procedures through `fws_ecl` `core_*`, memos, INOP SYS/INFO/LIMITATIONS, PFD lines, master lights, FwsAbnormalSensed with ProcedureLinesGenerator navigation, FwsAbnormalNonSensed, FwsAutoCallouts, display arbitration), `sound.rs` (FwsSoundManager), `sd.rs` (FwsSystemDisplayLogic incl. ALL cycle and STS NEXT PAGE / MORE h-events), `jsort.rs` (QuickJS `rqsort`, needed for `mapOrder`'s comparator that never returns 0). Host API additions: `Fws::take_extra` (bus topics, Coherent calls), `Fws::h_event`, `Fws::enqueue_sound` (GPWS callouts).

Comparator: bus-topic diff (value at the end of each tick, repeats of the previous tick dropped), all non-FWS var writes of the golden fed back (A: vars too), Xpndr1/GPWS writes seen in the same frame (they run before the FWS), GPWS callouts re-enqueued 150 ms before each recorded `PLAY_INSTRUMENT_SOUND`. Result: all nine scenarios 0 differences on 32 var/string outputs, 22 `fws_*` topics, PLAY_INSTRUMENT_SOUND calls and procedure transitions; 14 crate tests pass (incl. CLR -> recall -> RCL).

213800001 blip solved: `excessCabinAltitude.set(bit13 || manExcessAltitude)` stores `true` in emu-17 and the number `1` in emu-19; Subject.set compares with `===`, so the change notifies and resets the activation time. The generator keeps the raw value of such mixed boolean subjects (`set_raw`) and the activation bookkeeping compares it.

FBW behaviour believed wrong (ported faithfully):
14. Bare `this.fws.X` operands (`!this.engine4Running`, `this.x && ...`, array elements) are the Subscribable object, always truthy; the generator now treats them so (generalises 11; also 290800035/36 checked items, A/THR LIMITED items to show).
15. The LAND ASAP / LAND ANSA limitation is pushed with `pushKeyUnique(fn, keys)` without a state argument, so it is never added.
16. `mapOrder` sorts with a comparator that never returns 0; the right memo order depends on the engine's sort algorithm.
17. ECL take-off checklist flaps item tests `slatFlapSelectionS18F10 || ...` (Subjects), always true; `fws_ecl` uses the handle position instead (only matters when the ECL is shown).
18. FwsSoundManager loops forever on audio function lost while a non-continuous sound plays (`stopCurrentSound` returns early); the port stops after one pass.

Not compared against goldens (no recording exercises them): CHECK/UP/DOWN/ABN PROC/ALL buttons, h-events, attention getter topics, OIS debug data. `openIfDeferredApplicable` sets the `fws_ecl` fields directly (its navigate is private). The plugin's root Cargo.lock needs `fws_ecl` added to the `fws` package dependencies.
