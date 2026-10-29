# CEF removal inventory (inv25)

Date: 22 October 2026. Read-only research. Line counts are TS/TSX/JS source lines, tests excluded. Counts of outputs come from the golden `writes`/`calls` samples (E:/golden-tour/flight-v4/emu-10-cruise/captures, 46 captures, writes capped at 500 per capture) plus a static grep of `SetSimVarValue`/`writeToSimVar`/`publisher.pub`. Static counts undercount (template-string names, Arinc429 registers). Anything marked UNKNOWN was not measured.

## 1. What XPHFBW loads

Views = panel.cfg `[VCockpitNN]` first surviving htmlgauge (`src/xphfbw_bridge_views.rs` `view_list`; excluded: A380X/EFB/, A380X/OITlegacy/, A380X/popup/, WasmInstrument/). The EFB screen is the app's own UI (`app/ui`, `app/src/views.rs` `spawn_efb_view`), not FBW's flyPad (flyPad TS, 32903 lines in fbw-common plus 307 in a380x, is not loaded).

Source roots: A=`fbw-a380x/src/systems`, C=`fbw-common/src/systems`, N=`fbw-a32nx/src/systems` (the A380 imports fmgc, tcas, shared and extras modules from fbw-a32nx via tsconfig paths) under E:/fbw-int/fbw-aircraft.

| View (panel.cfg) | URL | FBW TS lines (own + shared dirs) | Main classes |
|---|---|---|---|
| VCockpit01 MFD | A380X/MFD/mfd.html | MFD 33475 (FMC 5402, pages/FMS 18135, pages/common 3580, ATCCOM 2326+MFD/ATCCOM, SURV 1029, FCUBKUP 648) + fmgc 49141 | `FlightManagementComputer`, `FmcAircraftInterface`, `FmcService`, `fmgc.ts`, `MfdPageDirectory`, `AtcDatalinkSystem` |
| VCockpit03 EWD | A380X/EWD/ewd.html | 3796 | `EWD`, procedure/checklist renderers |
| VCockpit04 SD, SDv2 | SD/sd.html (layer 0), SDv2/sdv2.html (layer 1) | SD 8418, SDv2 4109 | `SystemDisplay` (legacy React pages), `SD`, `StatusArea`, `AtcMailbox` |
| VCockpit05/06 PFD x2 | A380X/PFD/pfd.html | 11342 + 274 common | `PFD`, `FMA`, attitude/speed/alt/heading indicators, `FlightPathVector` |
| VCockpit07/08 ND x2 | A380X/ND/nd.html | 3810 + C ND 8542 + OANC 4971 | `NavigationDisplay`, `OansControlPanel`, `OANC`, `OANSRunwayInfoBox` |
| VCockpit09 FCU | A380X/FCU/FCU.html | 964 | `FCU` |
| VCockpit10 ISIS | A380X/ISISlegacy/isislegacy.html | 1486 | legacy ISIS indicators |
| VCockpit11 SND | A380X/SND/snd.html | no FBW source (installed snd.js 25.7 KB, plugin page) | UNKNOWN |
| VCockpit12-14 Clock, RTPI, BAT | A380X/Clock, RTPI, BAT | 63 + 535 / 46 / 28 + 94 | trivial |
| VCockpit16-18 RMP x3 | A380X/RMP/rmp.html | 2227 + C 395 | `RMP` instrument |
| VCockpit19-20 OIT x2 | A380X/OIT/oit.html | 9730 | `OIT`, `OitFltOpsContainer`, `OitAvncsContainer`, `OisLaptop`, `OisInternalPublisher` |
| VCockpit22 SystemsHost (hidden, 1x1) | A380X/SystemsHost/systems-host.html | A systems-host 22294 (CpiomC 15706, Misc 4864, PseudoPRIM 509, CpiomF 653, CpiomD 178, SystemsHost.ts 362) + N systems-host 10575 + shared | `FwsCore` (6272), `FwsAbnormalSensed` (4877), `FwsAbnormalNonSensed`, `FwsNormalChecklists`, `FwsMemos`, `FwsInopSys`, `FwsFlightPhases`, `FwsSoundManager`, `BrakeToVacate` (509), `LegacyTcasComputer` (1377), `LegacyGpws` (971), `LegacyFuel` (653), `EfisTawsBridge` (604), `VhfRadio`, `AudioManagementUnit`, `SimAudioManager`, `Transponder`, `AtsuSystem`, `AtcDatalink`, `Acr`, ~25 bus publishers |
| VCockpit23 ExtrasHost (hidden) | A380X/ExtrasHost/extras-host.html | A 927 + N 553 | RMP VHF H-events, brightness, aircraft presets, baro units |
| VCockpit25 IFE | A380X/IFE/ife.html | no FBW source (plugin page, installed ife.js 10 KB) | UNKNOWN |
| not in panel.cfg | HUD, FAP (installed html_ui) | HUD TS 14880 + 226 | not opened by XPHFBW; native `src/hud.rs` (619 lines) |

Installed bundle sizes (D:/Steam Games/.../html_ui/Pages/VCockpit/Instruments/A380X, read only): MFD 4.1 MB, SystemsHost 3.5 MB, ExtrasHost 2.0 MB, SD 1.8 MB, PFD 1.3 MB, ND 1.4 MB, SDv2 1.2 MB, EWD 1.1 MB, OIT 1.0 MB, RMP 0.6 MB, FCU 0.6 MB. These are mostly framework (Microsoft/Working Title) bundled in.

Not loaded in XPHFBW: terronnd.wasm, systems.wasm, fbw.wasm, fadec, extra-backend, recorder (WASM; the plugin supplies these natively: `src/extra_backend`, `src/fbw_computers.rs`, `src/prim.rs`, `src/fadec.rs`, Rust systems via `src/remote`).

Plugin-side QuickJS (`src/js/msfs/*`, 5.8k lines, `app/js`) is the fallback engine, not CEF. It also hosts the offline FMS oracle (`fms_oracle.rs`, `fms_offline.rs`).

## 2. Outputs per module

Notation: L = L: var, H = H event, K = key event, bus = EventBus topic, GD = generic data (GENERIC_DATA call).

### MFD view (FMS host)
FMS logic runs here (one `FlightManagementComputer` for both MFD sides, panel.cfg comment). Outputs:
- L vars written (static, 60 names in MFD + 50 in fmgc): guidance `A32NX_FG_CROSS_TRACK_ERROR/TRACK_ANGLE_ERROR/PHI_COMMAND/PHI_LIMIT/TARGET_ALTITUDE/TARGET_VERTICAL_SPEED/ALTITUDE_CONSTRAINT/REQUESTED_VERTICAL_MODE/CURRENT_LATERAL_LAW`; `A32NX_FMGC_FLIGHT_PHASE`, `AIRLINER_FLIGHT_PHASE`; speeds `A32NX_SPEEDS_{VLS,VS,F,S,GD,VAPP,VMAX,VFEN,MANAGED_PFD,MANAGED_ATHR}`; V speeds `AIRLINER_V1/VR/V2_SPEED`; `A32NX_FM_{GROSS_WEIGHT,LS_COURSE,HOLD_SPEED,HOLD_DECEL,LATERAL_FLIGHTPLAN_AVAIL,VERTICAL_PROFILE_AVAIL,TURN_STATE,HEADING_SYNC,ENABLE_APPROACH_PHASE}`; PFD `A32NX_PFD_{UPPER,LOWER}_SPEED_MARGIN`, `PFD_SHOW_SPEED_MARGINS`, `PFD_TARGET_ALTITUDE`, `PFD_VERTICAL_PROFILE_LATCHED`, `PFD_LINEAR_DEVIATION_ACTIVE`, `PFD_MSG_*`; ND `A32NX_EFIS_{L,R}_{TO_WPT_IDENT_0/1,TO_WPT_BEARING,TO_WPT_TRUE_BEARING,TO_WPT_DISTANCE,TO_WPT_ETA,MAP_PARTLY_DISPLAYED,ND_FM_MESSAGE_FLAGS,APPR_MSG_0/1}`; `A32NX_FMGC_{L,R}_{RNP,NAV_ACCURACY_HIGH,LDEV_REQUEST}`, `FMGC_TRUE_REF`; `A32NX_FM{1,2}_HEALTHY_DISCRETE`, `FMS_{L,R}_FAILED`; `A380X_FMS_DEST_EFOB_BELOW_MIN`; GPWS/WXR switches; presel speed/mach.
- K events: `A32NX.FCU_ALT_PUSH/ALT_SET`, `A32NX.FMGC_DIR_TO_TRIGGER`, `A32NX.FMS_PRESET_SPD_ACTIVATE`, `A32NX.ATHR_RESET_DISABLE`, `AP_MANAGED_SPEED_IN_MACH_ON/OFF`, `XPNDR_SET/IDENT_ON`.
- GD: `A32NX_EFIS_VECTORS_{L,R}_{ACTIVE,ACTIVE_EOSID,ALTERNATE,DASHED,MISSED,SECONDARY,TEMPORARY}` (EfisVectors), `A32NX_EFIS_{L,R}_SYMBOLS` (EfisSymbols). In the oracle they travel inside `EB_EVENTS` batches. 952 GENERIC_DATA calls in the MFD capture.
- bus (reaches native displays via the golden `bus` record): `flightPlanManager.create/delete`, `SYNC_flightPlanManager.*`, `fmsOrigin/Destination/Alternate/DepartureRunway/LandingRunway/FlightNumber`, `a32nx_fms_vertical_{actual,target,descent}_profile`, `a32nx_fms_vertical_constraints`, `a32nx_fms_vd_track_change_distance`, `srwyP1..9`, `srwyPl1..4`, `thresholdLocation`, `startLocation`, `latitude/longitude/length/width/direction/elevation/gradient/thresholdCrossingHeight/location` (runway data), `flightPlan.setPerformanceData.zeroFuelWeight*`, `atcResetAtisAutoUpdate`, `routerDatalink{Status,Mode}`, `mfd_xpdr_set_auto`, `mfd_tcas_alert_level`, `resetSystem`, `popup_enqueue_popup`, `ropsDetectedAirport/Runway`.
- Coherent calls: `GENERIC_DATA`, `INTERCEPT_KEY_EVENT` (228), `START_NEAREST_SEARCH_SESSION_WITH_STRUCT`, `SET_NEAREST_*_FILTER`, `SEARCH_NEAREST`, `LOAD_VOR`, `FBW_NXDATASTORE_UPDATE`, `GET_EFB_ROUTE`, `TRIGGER_KEY_EVENT`.
- Persistence: NXDataStore keys `A380X_XP_FPLN_BACKUP`, `A380X_AUTO_STEP_CLIMB`, `A380X_PAUSE_AT_TOD(_DISTANCE)`, `A380X_LATLON_EXT_FMT`, SimBrief/Navigraph keys.
- Inputs: nav data (plugin `src/navdata`, fetched through the Coherent nearest-search stubs), ADIRS/AFS/engine/fuel L vars, SimBrief, FCU/EFIS buttons.

### Systems host (hidden)
- FWS (`FwsCore`): L `A32NX_FWC_FLIGHT_PHASE`, `MASTER_WARNING/CAUTION`, `FWC_{1,2}_DISCRETE_WORD_126`, `FWC_{1,2}_LG_RED_ARROW`, `STATUS_NORMAL`, `TO_CONFIG_NORMAL`, `FWS_TO_CONFIG_TEST`, `NO_SMOKING_MEMO`, `CABIN_READY`, EWD/PFD message strings (`ewdMessageSimVarsLeft/Right`, `pfdMemoSimVars`, `pfdLimitationsSimVars`), `ECAM_SFAIL`, `ECAM_FAILURE_ACTIVE`, `ECAM_SD_PAGE_TO_SHOW`, FWS health and sound vars (`FWS_AUDIO_VOLUME`, per-sound `localVarName`). bus (to EWD/SD/ND/OIT): `fws_active_item`, `fws_active_procedure`, `fws_show_from_line`, `fws_show_{abn_sensed,abn_non_sensed,normal_checklists,sts_indication,failure_pending,adv_indication}`, `fws_normal_checklists(_id)`, `fws_abn_sensed_procedures`, `fws_abn_non_sensed_id`, `fws_deferred_procedures`, `fws_limitations_{all_phases,appr_ldg}`, `fws_information`, `fws_inop_sys_{all_phases,appr_ldg,redundancy_loss}`, `fws_alerts_impacting_ldg_perf`, `fws_cancelled_caution`, `fws_normal_attention_getter_eng`, `a380x_ois_fws_debug_data`. Coherent `PLAY_INSTRUMENT_SOUND`. Inputs: ~hundreds of L vars (ADIRS, elec buses, hydraulics, engines, doors), `L:A32NX_BTN_*` ECP, plugin deep-ecam triggers.
- BTV (`BrakeToVacate`): L `A32NX_OANS_BTV_*`, `A32NX_OANS_RWY_BEARING/LENGTH`; bus `ndBtvMessage`, `oans_*`, `oansSelectedExit`, `oansSelectedLandingRunway`, `ropsDetected*`.
- TCAS (`LegacyTcasComputer`): L `A32NX_TCAS_STATE`, `TCAS_VSPEED_GREEN/RED:1,2`, `TCAS_MODE_WORD`, `TCAS_FAULT_SUMMARY_WORD`, `TCAS_VERTICAL_RESOLUTION_ADVISORY_WORD`; GD `A32NX_TCAS_{L,R}_TRAFFIC`; Coherent `GET_AIR_TRAFFIC`.
- GPWS (`LegacyGpws`), `EfisTawsBridge`: L `A32NX_GPWS_Warning_Active`, `GPWS_GS_Warning_Active`, `GPWS_{1,2}_FAILED`, `TERR_{1,2}_FAILED`, `WXR_{1,2}_FAILED`.
- Radio: `VhfRadio`, `AudioManagementUnit`, `Transponder` (K `XPNDR_SET`), `RmpAmuBusPublisher`.
- ATSU/ATC datalink (`AtsuSystem`, `AtcDatalink`, `Acr`; N atsu 898, C datalink 13211): bus topics (about 83 in datalink), L `A32NX_ACARS_ACTIVE`, `A32NX_ATSU_RMP_FREQUENCY`; METAR/TAF/traffic via Coherent (`GET_METAR_BY_IDENT`, `GET_TAF_BY_IDENT`, `GET_AIR_TRAFFIC`).
- `LegacyFuel`: L `A32NX_FUEL_DESIRED`, fuel pump/refuel vars.
- ND symbols: `FmsSymbolsPublisher('L')` and the symbol bus (`oans_symbols_for_feature_ids`, `nd_show_oans`).
- Bus publishers (about 25: FCDC, FG data, EGPWC, AESU, IR, RA, LGCIU, fuel, FQMS, CPIOM data, power supply, WBC, BTV simvars, switching panel, pilot seat, reset panel).

### Extras host (hidden)
H `RMP_{1,2,3}_VHF_CALL_n_PRESSED/RELEASED`; L `A380X_RMP_n_VHF_VOL_*`, `A32NX_CABIN_AUTOBRIGHTNESS`, `A32NX_MCDU_{L,R}_BRIGHTNESS`, `A32NX_PANEL_DCDU_*`, `A380X_PED_LIGHTING_*`, `A32NX_FCU_EFIS_{L,R}_BARO_IS_INHG`, `XMLVAR_Baro_Selector_HPA_*`, `A32NX_AIRCRAFT_PRESET_LOAD`, `A32NX_IS_READY`; K `ELECTRICAL_CIRCUIT_TOGGLE`. Aircraft presets and sync modules. Consumed by plugin `lights.rs`, `radios.rs`.

### Display pages (measured in goldens)
| Page | Writes observed | Notes |
|---|---|---|
| PFD (pfd-1 none, pfd-2 28 names) | `L:A32NX_BTN_{ABNPROC,CL,DOWN,RCL}`, `A32NX_OVHD_ADIRS_*`, `A380X_WXR_*`, `A32NX_WXR_TAWS_SYS_SELECTED`, `A380X_SWITCH_LAPTOP_POWER_*`, K `A32NX.FCU_*_PUSH`, `FCU_EFIS_*_FD_PUSH` | These are click handlers in the page (bezel/ECP/laptop switches), not display logic. Native PFD must route clicks instead. |
| ND (1/2) | `A32NX_VD_{1,2}_RANGE_LOWER/UPPER`, `A32NX_FCU_EFIS_{L,R}_LS_LIGHT_ON`, `A380X_EFIS_{L,R}_LS_BUTTON_IS_ON`, `A32NX_OANS_AVAILABLE/FAILED`; GD 44-55 calls | VD range vars and LS lights are display-derived state. Read by plugin? UNKNOWN (no hit for `A32NX_VD_` in plugin src; FCU light read in prim.rs/lights UNKNOWN). |
| EWD | `A32NX_BTN_{CHECK_LH,UP,DOWN}` (click) | Pure renderer otherwise. |
| SD / SDv2 | `A32NX_ECAM_SD_CURRENT_PAGE_INDEX`, `ECAM_SD_STS_MORE_AVAILABLE`, `ECAM_SD_STS_NUMBER_OF_PAGES`, H `A32NX_SD_STS_NEXT_PAGE` | Consumed by FWS in systems-host (`FwsSystemDisplayLogic`). |
| ISIS | `A32NX_ISIS_MANUAL_BRIGHTNESS_OFFSET` | Internal brightness knob state. |
| FCU | none observed, none in static scan | Renderer only. |
| RMP 1-3 | 18-27 names each: `A380X_RMP_n_{STATE,RED_LED,GREEN_LED,*_VOL_RX}`, `FBW_RMP_FREQUENCY_ACTIVE/STANDBY_n`, `FBW_RMPn_*_VHFx_FREQUENCY`, `FBW_VHFn_BUS_A`, `FBW_HF1_BUS_A`; GD, K `TRIGGER_KEY_EVENT` | Real logic: radio panel state. Consumed by systems-host `VhfRadio`/AMU/Transponder, plugin `radios.rs`, `lights.rs` (`A380X_RMP_` hits). Cannot drop without porting RMP logic (2622 lines). |
| OIT 1/2 | `A32NX_NSS_ANSU_1_IS_HEALTHY`, `A32NX_FLTOPS_ANSU_1_IS_HEALTHY`, `A32NX_FLTOPS_LAPTOP_n_IS_HEALTHY`, `A32NX_OIS_n_SHOW_CHARTS/SHOW_OFP`, `A32NX_BKR_*_CMD` (breakers), `A32NX_OIS_SYNCHRO_AVIONICS`; Coherent `LOAD_AIRPORT` | Real logic (OIT pages, 9730 lines). Native `oit`/`oitx` paint it from the DOM snapshot. |
| Clock | `A32NX_CHRONO_ET_ELAPSED_TIME`, `A32NX_CHRONO_ELAPSED_TIME` (669 writes) | Chrono state; the plugin reads `A32NX_CHRONO` in `js_bridge.rs`/`xphfbw_host.rs`. Native clock must keep the chrono. |
| BAT, RTPI, IFE, SND | none | Renderers only. |

## 3. Who consumes the outputs

- Native displays (`src/native_displays/read_sets.txt`, 2571 lines, per display counts of var/str/env reads): bat 5, clock 12, ewd 132, fcu 32, ife 14, isislegacy 34, mfd 271, nd-1 147, nd-2 143, oit-1 65, oit-2 66, pfd-1 261, pfd-2 264, rmp-1 46, rmp-2 41, rmp-3 40, rtpi 6, sd 528, sdv2 119, snd 20 vars. Plus bus/GD: all 81 bus topics and 20 GD keys are fed to every view, but a380-displays source reads fws_* only in ewd and sd, `EFIS_VECTORS`/`EFIS_*_SYMBOLS` and `oans_*`/`nd*` only in nd, flightPlan topics only in nd. mfdx/oitx read a DOM snapshot (`a380.mfd.dom`) produced by the CEF page; this keeps the MFD and OIT JS and DOM alive.
- Rust plugin systems: `src/prim.rs` (reads `A32NX_FG_*`, `FM_*`, `FMA_*`, `FMGC_FLIGHT_PHASE`, `SPEEDS_*`, `PFD_*_SPEED_MARGIN`, `TCAS_*`, `EFIS_L_ND_*`), `radios.rs`, `lights.rs`, `scripted_failures.rs`, `maintenance_log.rs`, `sound/triggers.rs` (`A32NX_FWS_*`), `surveillance.rs` (TCAS/GPWS), `js_bridge.rs`, FBW's C++ computers via `fbw_computers.rs`, deep ecam (`src/deep/ecam`: writes into FwsCore, does not replace it).
- Other JS modules: all FWS bus topics to EWD/SD/ND/OIT; EFIS vectors/symbols from MFD/systems-host to ND/PFD; TCAS traffic to ND/MFD; SD page vars to FWS; RMP vars to systems-host.
- EFB UI (`app/ui`, `src/efb.rs`): reads plugin datarefs, not FBW JS outputs (UNKNOWN beyond that; not audited).

## 4. Native already in the repo (do not port twice)

- FWS/ECAM: NOT native. `src/deep/ecam` (10.8k lines incl. `fbw/ata*.rs` triggers, codegen, `deep_ecam_bridge.js`, `patches.rs`) feeds triggers into FBW's `FwsCore` through source patches. The logic of FwsCore stays JS.
- FMS: NOT native. No `crates/fms` exists (planned in docs/superpowers/plans/2026-10-21-native-fms-mfd.md). Phase 1 oracle done: `src/js/msfs/fms_oracle.rs` (240), `fms_offline.rs` (880), `fms_oracle_snapshot.js`, `tools/fms_oracle/`, 4 scenarios with goldens in E:/fbw-build/fms-oracle (uplink-egll-kjfk, city-pair-procedures, edits-tmpy, secondary). Native nav data exists: `src/navdata` (5076), `src/airac.rs`.
- OANS: only data. `src/oans` (1164) serves AMDB (`hf.rs`, `plugin.rs`); OANC logic (4971) and BTV (509) are JS.
- ND layers: native terrain/WXR (`src/mapdata` 4935, `src/wxr` 1278, `terronnd.rs`).
- TCAS/GPWS: `src/surveillance.rs` (511) reads the JS outputs; JS computers still run.
- ATSU/datalink: JS; `src/atc_online.rs` (244), `src/navigraph.rs` (146) are network helpers.
- Computers: PRIM/SEC/FCU C++ via `prim.rs`/`fbw_computers.rs`; FADEC, FQMS, extra backend native (`extra_backend` 2468).
- HUD: native `src/hud.rs` (619).
- OIT files/charts/keyboard: `oit_files.rs`, `charts.rs`, `ois_keyboard.rs`.
- Display painting: a380-displays (24651 lines in displays/*, core 6268): nd 3930, sdsys 3182, mfdx 1779 (+dom), pfd 1764, oit 1307, sd 1300, ewd 1229, mfd 1359 (old), isis 408, ife 288, snd 245, fcu 233, rmp 168, rtpi 130, bat 101, clock 94. Painted from state; per the integration plan, JS keeps running for logic.

## 5. Can CEF stop loading each display page?

Answer: for painting, all native displays make the page's pixels unnecessary. For logic, split by page:
- Safe to stop now (no observed writes, no consumers): BAT, RTPI, IFE, SND, FCU page (renderer only; FCU buttons are X-Plane manipulators). Caveat: input must reach the native screens (click handling).
- Stop after moving a few lines of state into native/plugin: Clock (chrono elapsed var), ISIS (brightness offset), EWD (ECP button writes `BTN_*`), PFD (bezel/ADIRS/WXR/laptop click writes and `K:A32NX.FCU_*` presses), SD/SDv2 (`ECAM_SD_*` page vars and `SD_STS_NEXT_PAGE`; FWS reads them), ND (`VD_n_RANGE_*`, LS button/light, `OANS_AVAILABLE/FAILED`, symbols are produced elsewhere). The page logic is small; PFD/ND/EWD/SD do not produce the FMS/FWS data the plugin needs.
- Must stay until ported: MFD (the FMS host: guidance, speeds, vectors, symbols, V-speeds, flight plan; 83k lines with fmgc), OIT (page logic; mfdx/oitx read its DOM), RMP (radio state), ND only for OANC/OansControlPanel/VD profile logic if those are not yet native (UNKNOWN whether native nd reproduces OANS selection actions).
- Hidden hosts: SystemsHost (FWS, BTV, TCAS, GPWS, VHF/AMU/XPDR, ATSU, fuel; about 33k lines) and ExtrasHost (about 1.5k lines) are the largest non-display logic and block removing CEF.

## 6. Framework vs FBW logic

FBW own logic (translatable): fmgc guidance/lnav 8749, vnav 12042, flightplanning ~18851 (plans 8508, legs 927, uplink 1233), efis 1809, navigation 2313, flightphase 471; MFD FMC 5402; FWS 15706 (CpiomC); BTV 509; TCAS ~1377+tcas 2378; GPWS 971; OANC 4971; page logic in MFD pages (~25k), OIT 9730, RMP 2622, EWD/SD/PFD/ND page logic.
Framework (must not be translated or read): `@microsoft/msfs-sdk`/WT `wtsdk.ts`, `utils/LzUtf8.js`, `EventBus`, `Subject`, `ConsumerValue`, `MathUtils`, `AeroMath`, `UnitType`, `BitFlags`, `UUID`, `Wait`, DisplayComponent/FSComponent, the CEF-bundled framework in the 1-4 MB bundles. Exact split of lines within MFD/SD/PFD: UNKNOWN (not measured).

## 7. Proposed removal order

| # | Group | Size | Depends on | Proof |
|---|---|---|---|---|
| 0 | Drop idle pages: BAT, RTPI, IFE, SND, FCU, and (after moving small state) Clock, ISIS, EWD, PFD, SD, SDv2 | under 600 lines of native/plugin glue | native displays at 98% on goldens; input routing to native screens | golden-tour all-display run with these views not loaded; compare vars written (diff the `writes`) |
| 1 | FMS: `crates/fms` plan, LNAV, VNAV, perf, fuel, messages, efis (vectors/symbols), phase, fms_live | fmgc 49k + FMC 5.4k (about 55k TS) | Phase 1 oracle (done), `src/navdata` NavDatabase trait | 4 oracle goldens in E:/fbw-build/fms-oracle (every var and GD event matches); then 3 simulated flights from golden-tour (flight-v4) |
| 2 | MFD pages as native page trees on DomDisplay/mfdx painter, no DOM from CEF | pages ~25k TS + shared | group 1 (model) | mfd.dom.html / mfd.png goldens at 98% strict; action-to-state replay of the oracle scenarios |
| 3 | FWS: `FwsCore` and Fws* into Rust, deep ecam triggers feed it directly (no source patches) | CpiomC 15.7k | none for logic; ECP buttons, SD page vars | ECAM goldens (flight-v4 ewd/sd captures with exact inputs, 46 captures), fws_* bus topics compared per frame, plus existing `src/deep/ecam/fbw_tests.rs` |
| 4 | OANS/BTV (OANC 4971 + BTV 509 + OansControlPanel), TCAS (3.7k), GPWS (971), EfisTawsBridge (604) | about 11k | group 1 for runway data; AMDB server exists | BTV/OANS L vars and `oans_*` topics replayed from golden states; TCAS vars vs recorded traffic scenarios |
| 5 | RMP + VHF/AMU/XPDR + ExtrasHost | RMP 2.6k + Misc/Communications ~0.8k + extras 1.5k | none | RMP LED/frequency write sets in the goldens (rmp-1..3) |
| 6 | ATSU/ATC datalink/AOC, MFD ATCCOM, SD AtcMailbox | datalink 13.2k + ATCCOM 8.5k | group 2 | UNKNOWN (no goldens for datalink found) |
| 7 | OIT page logic native (oitx without DOM) | OIT 9.7k | groups 1 and 6 for FLT FOLDER/charts | oit-1/oit-2 goldens (dom.html, model.json) |
| 8 | EFB UI | app/ui is already the app's own UI; FBW flyPad not loaded | CEF removal also needs a non-CEF host for `app/ui` (UNKNOWN: out of scope) | screenshots (efbshot) |
| 9 | Remove XPHFBW CEF host, QuickJS fallback, JS bundles from install | n/a | all above | full golden tour with no JS loaded |

Critical path: 1 then 2; 3 independent and large; 0 can start immediately.

## 8. Totals

- Views opened: 25 panel.cfg sections give about 19 JS views (counting MFD, EWD, SD, SDv2, PFD x2, ND x2, FCU, ISIS, SND, Clock, RTPI, BAT, RMP x3, OIT x2, IFE, SystemsHost, ExtrasHost) plus the app's EFB UI.
- FBW TS lines that run in XPHFBW (approximate, framework excluded): MFD 33475 + fmgc 49141 + systems-host 22294 + N systems-host 10575 + datalink 13211 + ND 12352 + OANC 4971 + PFD 11616 + SD 8418 + SDv2 4109 + EWD 3796 + OIT 9730 + RMP 2622 + tcas 2378 + ISIS 1486 + FCU 964 + extras 1480 + small gauges about 800 + shared 21611 (lines may overlap between systems-host dirs; treat as about 215k, some shared).
