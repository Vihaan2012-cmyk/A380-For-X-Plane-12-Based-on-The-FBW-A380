# Native hosts, remaining modules (host26)

Date: 22 October 2026. Scope: FlyByWire A380X `SystemsHost` and `ExtrasHost` hidden views minus what fws25 (FWS), tcas26, gpws26, rmp26 (RMP/VHF/XPDR/AMU), oans26 (OANS/BTV), atsu26 (datalink) and the FMS agents own. Source roots under E:/fbw-int/fbw-aircraft: A = `fbw-a380x/src/systems`, C = `fbw-common/src/systems`. Plugin = E:/fbw-int/plugin. Line counts are `wc -l` of the TS file. Plugin coverage was searched read-only in `src/` and `crates/`.

Correction to the inventory: the A380X `systems-host.html` is built from `A/systems-host/SystemsHost.ts` (esbuild entry, `A/systems-host/build.js`) and imports only A, `@shared/*` (A/shared), `@providers/*` (A/instruments/.../providers), `@failures`, `@datalink/*` and `@flybywiresim/fbw-sdk` (C/shared). The a32nx `systems-host` dir (10575 lines, PseudoFWC etc.) is NOT part of the A380X host. The same holds for the a32nx `extras-host` (553 lines).

## 1. SystemsHost.ts (362) wiring, not owned by another agent

| Piece | File | Lines | Outputs | Consumers | Plugin covers? |
|---|---|---|---|---|---|
| FWS host health: fws1/2 Failed (failure OR reset-panel pulled OR CPIOM C unpowered), Healthy (also needs `startupCompleted`), ECP failed (failure OR no DC ESS OR all four AFDX ECP endpoints unreachable), FwsCore create/destroy on both-failed edge | A/systems-host/SystemsHost.ts 288-328, 253-286 | about 70 | L `A32NX_FWS1_IS_HEALTHY`, `A32NX_FWS2_IS_HEALTHY`, `A32NX_FWS_ECP_FAILED`; `FwsCore.sendFailureWarning` | EWD/SD (FWS healthy), FwsCore, sound triggers | No. Only inside JS (`src/js/msfs/tests.rs:452-547` asserts the JS output). **Ported (fws_host)** |
| Power supply buses `PowerSupplyBusses` | A/systems-host/Misc/powersupply.ts | 130 | bus `acBus1..4`, `acBusEss`, `dcBus1/2/Ess`, `dcBusEssInFlight` = `L:A32NX_ELEC_*_BUS_IS_POWERED != 0` | VhfRadio, Transponder, SystemsHost powered subs | No; pure bool mapping on top of SimVarPublisher (framework). Trivial once a native bus exists. Not ported |
| CPIOM availability `CpiomDataPublisher` | A/instruments/src/MsfsAvionicsCommon/providers/CpiomPublisher.ts | 54 | bus `cpiom_{a..g}_available_{n}` from `L:A32NX_CPIOM_<X><n>_AVAIL` | SystemsHost (`cpiom_c_available_1/2` -> FWS powered) | Plugin writes the L vars (`src/deep/avionics_network`), the publisher is plumbing. Not ported |
| Reset panel `ResetPanelSimvarPublisher` | A/.../ResetPanelPublisher.ts | 41 | bus `a380x_reset_panel_*` from `L:A32NX_RESET_PANEL_*` | SystemsHost (fws1/2), FMC/AESU hosts | Plumbing. Not ported |
| AESU bus `AesuBusPublisher` | A/.../AesuBusPublisher.ts | 72 | bus `a32nx_aesu_*` from `L:A32NX_WXR_TAWS_SYS_SELECTED`, `GPWS_TERR_OFF`, `TERR_n_FAILED`, `WXR_n_FAILED`, `VD_n_RANGE_LOWER/UPPER` | ND, EfisTawsBridge (gpws26) | Plumbing, no logic. Not ported |
| Pseudo FWC publisher | A/.../PseudoFwcPublisher.ts | 65 | bus `engine_master_n`, `engine_state_n`, `adr_cas_word_n`, `throttle_position_n`, `ths_position`, gear/hyd | FwsCore | Plumbing |
| FCDC bus publisher | A/shared/src/publishers/FcdcPublisher.ts | 199 | bus `fcdc_*` discrete words from `L:A32NX_FCDC_*` | FwsCore, PFD | Plumbing (typed re-publication of ARINC words). Not ported |
| FQMS bus publisher | A/shared/src/publishers/FqmsBusPublisher.ts | 95 | bus `fqms_*` from `L:A32NX_FQMS_*`; FuelSystemPublisher (A/.../FuelSystemPublisher.ts, 114) `fuel_tank_quantity_n`, `fuel_trigger_status_n`, `fuel_junction_setting_n` | LegacyFuel, FwsCore | Plumbing; FQMS itself is Rust FBW (`systems` crate) |
| EGPWC / FGData / IR / RA / LGCIU / switching panel / pilot seat / MSFS misc / flight model / BTV simvar publishers | A/.../EgpwcBusPublisher.ts 75, FGDataPublisher.ts 50, C/shared/.../IrBusPublisher.ts 326, RaBusPublisher.ts 51, LgciuBusPublisher.ts 133, SwitchingPanelPublisher.ts 26, MsfsMiscPublisher.ts 50, MsfsFlightModelPublisher.ts 55, BtvPublisher.ts 64 | 830 | typed bus topics | FwsCore, TCAS, GPWS, BTV | Plumbing (SimVarPublisher subclasses, framework class). No logic; native consumers read L vars directly |
| `FmsMessagePublisher` | A/shared/src/publishers/FmsMessagePublisher.ts | 20 | bus FMS messages | FWS | Plumbing |
| `FmsSymbolsPublisher('L')` | A/instruments/src/ND/FmsSymbolsPublisher.ts | 82 | bus `nd_show_oans`, `oans_symbols_for_feature_ids` | ND | FMS/OANS agents |
| `LegacySoundManager` | A/systems-host/Misc/LegacySoundManager.ts | 150 | Coherent `PLAY_INSTRUMENT_SOUND` queue for GPWS/TCAS | gpws26/tcas26 | Plugin `sound/triggers.rs` plays natively; left to those agents |
| `SimVarHandling` | C/datalink/common/src/msfs/SimVarHandling.ts | 222 | ACARS/ATSU L vars | atsu26 | atsu26 |

## 2. Systems-host modules that stay with other agents (not ported here)

| Module | Lines | Owner |
|---|---|---|
| CpiomC FlightWarningSystem (FwsCore 6272, AbnormalSensed 4877, AbnormalNonSensed 883, Memos 684, NormalChecklists 613, SoundManager 481, SystemDisplayLogic 329, FlightPhases 532, InopSys 792, Limitations 98, Information 75, AutoCallouts 70) | 15706 | fws25 |
| CpiomD Acr 40, AtcDatalink 28, AtsuFmsClient 75, atsu 35 | 178 | atsu26 |
| Misc/Communications (VhfRadio 127, AudioManagementUnit 163, SimAudioManager 268, Transponder 108, RmpAmuBusPublisher 117) | 783 | rmp26 |
| Misc/LegacyGpws 971, EfisTawsBridge 604 | 1575 | gpws26 |
| Misc/tcas (LegacyTcasComputer 1377, TcasConstants 849) | 2226 | tcas26 |
| PseudoPRIM/BrakeToVacate | 509 | oans26 |
| CpiomF/LegacyFuel | 653 | Already native: `src/fuel_transfer.rs` (1054) is "a line-by-line port" (header cites LegacyFuel.ts lines), run by `src/fuel.rs` (1894) on `src/fuel_network.rs` (3561); `key_events.rs` deliberately ignores the script's FUELSYSTEM_* events so it is not actuated twice. Covered |

## 3. ExtrasHost (A/extras-host, 927 lines in A380X: index.ts 212 + modules 715)

| Module | File | Lines | Outputs | Consumers | Plugin covers? |
|---|---|---|---|---|---|
| LightSync | modules/light_sync/LightSync.ts | 198 | K `LIGHT_POTENTIOMETER_SET` for ids 96,97,84,87,10,11,88,89,94,98,78,71,90,91,95,99,79,72,92,93,76,85, 60 (cabin), L `A380X_RMP_#RMP_ID#_BRIGHTNESS_KNOB`, `A380X_PED_LIGHTING_MIP_FLOOD_LT_KNOB`, `AMBIENT_LT_KNOB`, `A32NX_CABIN_AUTOBRIGHTNESS`, K `ELECTRICAL_CIRCUIT_TOGGLE` 151 | `key_events.rs` (potentiometer -> `LIGHT POTENTIOMETER:n`), `lights.rs`, cabin lights, displays brightness | Runs as JS; plugin only patches it (`extra_backend_fbw.rs:96-111`: wait for steady auto brightness, integral 85 follows 84) and supplies its inputs (`E:TIME OF DAY`, `GLASSCOCKPIT AUTOMATIC BRIGHTNESS`, `ON ANY RUNWAY`). **Ported (light_sync)** |
| A380XKeyInterceptor + base KeyInterceptor | modules/key_interceptor/KeyInterceptor.ts 353 + C/shared/src/extras/KeyInterceptor.ts 98 | 451 | COM key events -> `L:A380X_RMP_n_VHF_VOL_*`, `VHF_VOL_RX_SWITCH_*`, standby/active frequency vars, H `RMP_n_VHF_CALL_k_PRESSED/RELEASED`; ENGINE_AUTO_START/SHUTDOWN preset dialog -> `L:A32NX_AIRCRAFT_PRESET_LOAD`; MASTER_CAUTION/WARNING_ACKNOWLEDGE -> `L:PUSH_AUTOPILOT_MASTERCAUT/AWARN_L/R` for 500 ms | X-Plane hardware/keyboard COM events (radios.rs handles raw `K:COM*` for the X-Plane radios, separate from the RMP vars), RMP logic (rmp26) | No native. `grep` of `shouldControlRmp`, `PUSH_AUTOPILOT_MASTER` finds nothing in src/. **Ported (key_interceptor)** except the frequency arithmetic (`RadioUtils` 373, rmp26) |
| PilotSeatManager (+ PilotSeatPublisher) | C/shared/src/PilotSeat.ts | 98 | L `FBW_PILOT_SEAT`, `FBW_IN_FLIGHT_DECK` | KeyInterceptor `shouldControlRmpN`, PilotSeatPublisher, EFB | No (no hit for `FBW_PILOT_SEAT` in src/). Camera source differs in X-Plane. **Ported (pilot_seat)** |
| PushbuttonCheck | modules/pushbutton_check/PushbuttonCheck.ts | 59 | notification after TO CONF held 30 s in phase 8 | EFB notification | No. **Ported (pushbutton_check)** |
| BaroUnitSelector | C/shared/src/extras/BaroUnitSelector.ts | 46 | L `XMLVAR_Baro_Selector_HPA_1/2` from CONFIG_INIT_BARO_UNIT or nearest airport ICAO region | PFD/baro | No (`prim.rs` handles `BARO_IS_INHG` only). **Ported (baro_unit)** the region decision; the facility search stays Coherent |
| AircraftSync | modules/aircraft_sync/AircraftSync.ts | 38 | none (empty) | none | n/a, no logic |
| VersionCheck / TelexCheck / MsfsVersionPopupMonitor | modules/version_check 29 + C 24 + C 32 | 85 | popups, `AircraftGithubVersionChecker.setEditionLocalVar` | none in X-Plane | n/a, MSFS-only |
| AircraftPresetsList | modules/common/AircraftPresetsList.ts | 25 | preset names | KeyInterceptor | `extra_backend/aircraft_presets.rs` (760) runs the presets natively |
| GPUManagement | C/shared/src/GPUManagement.ts | 142 | K `REQUEST_POWER_SUPPLY` | EFB services | Covered: `key_events.rs` row REQUEST_POWER_SUPPLY, `efb.rs` GPU toggle |
| GsxSyncA380X + GsxSimVarPublisher | C/shared/src/GsxSync.ts 406 + C/.../GsxSimVarPublisher.ts 55 | 461 | GSX service sync | none (X-Plane has no GSX) | n/a |
| ExtrasSimVarPublisher, MsfsElectricsPublisher, GroundSupportPublisher, NotificationManager | C/shared | 40+46+43+171 | bus / popups | notifications | Plumbing |
| ExtrasHost index.ts | index.ts | 212 | `L:A32NX_IS_READY`=1 at ingame | EFB, presets | Not set by a native host (`A32NX_IS_READY` appears only in preset test fixtures, `src/extra_backend/aircraft_presets.rs`); one-line write, wiring only |

## 4. Choice and result

Most-consumed remaining logic, in order: (1) FWS host health (EWD/SD/sounds read `FWS1/2_IS_HEALTHY`, `FWS_ECP_FAILED`), (2) LightSync (every display knob and cabin brightness, currently a JS file with two source patches), (3) KeyInterceptor RMP routing and MASTER ack, (4) PilotSeatManager (input to 3), (5) small ones. Everything else in the two hosts is either owned by another agent, already native (LegacyFuel), or SimVarPublisher plumbing that disappears when consumers read L vars directly.

Port: standalone crate `E:/fbw-int/plugin/crates/hosts_misc` (std only), modules `fws_host`, `light_sync`, `key_interceptor`, `pilot_seat`, `pushbutton_check`, `baro_unit`. Each is a plain state machine: an input struct per tick, a list of output commands (`SetLvar`, `TriggerKey`, `HEvent`, `Notify`, `ShowPresetDialog`). Tests: 40 unit tests derived from the TS branches, plus an offline oracle (`E:/fbw-build/hosts-oracle/light_sync_oracle.mjs`, node 24 `--experimental-transform-types`) that runs the original `LightSync.ts` with stubbed SimVar/KeyEventManager over 2178 input combinations and writes `light_sync.golden.json`; `crates/hosts_misc/tests/light_sync_oracle.rs` compares every command and value (passes). SystemsHost, KeyInterceptor, PilotSeat, PushbuttonCheck and BaroUnitSelector have unit tests only (their TS pulls in the Microsoft SDK or fbw-sdk classes, so no oracle).

Quirks kept from FBW: `setRmpBrightness` writes the literal var `L:A380X_RMP_#RMP_ID#_BRIGHTNESS_KNOB` (the rmp argument is unused) with value brightness*100 on a 15-85 scale; `COM1/2_TRANSMIT_SELECT` pass an object without `value0`, so both select VHF 1. `LightSyncConfig.integral_follows_glareshield` reproduces the plugin's source patch for potentiometer 85 (default off = FBW). The plugin's other patch (wait for a steady automatic brightness) is not in the crate.

Left: RadioUtils frequency stepping used by the COM_RADIO_* intercepts (rmp26), the facility search for BaroUnitSelector, hooking the crate into the plugin (not done: other agents are editing it), the plumbing publishers above once a native bus exists.
