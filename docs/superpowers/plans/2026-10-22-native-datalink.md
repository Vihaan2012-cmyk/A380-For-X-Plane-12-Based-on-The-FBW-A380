# Native datalink (ATSU / ATC / AOC)

Date: 22 October 2026. Crate: `crates/datalink` (own Cargo.toml, std + serde/serde_json only, no X-Plane dependency). Test: `cargo test --manifest-path crates/datalink/Cargo.toml`.

Source (FBW GPL, translated by hand, not framework): `fbw-aircraft/fbw-common/src/systems/datalink/{common,atc,aoc,router}`, A380X side `fbw-a380x/src/systems/instruments/src/MFD/ATCCOM`, `.../MFD/pages/ATCCOM`, `.../SDv2/AtcMailbox.tsx`. Framework pieces (EventBus, Publisher, Subject, SimVar) are not translated; the crate exposes plain structs and traits instead.

## Phases

1. Message model: AtsuMessage family (freetext, METAR/TAF/ATIS, CPDLC, DCL, OCL), timestamps, serialization formats (Network, Mailbox, FmsDisplay, Printer), word wrap, JSON round trip. (done in this wave)
2. CPDLC catalogue: all downlink (DM) and uplink (UM) elements with templates, FANS A/B mask, expected response, urgency and typed content slots; content validators and the template filler; input validation (FANS A/B altitude and speed). Catalogue is generated from the TS table by `tools/gen_catalogue.py`. (done)
3. ACARS network: `HttpClient` trait; Hoppie/SAI/BATC `connect.html` request builder; ping, poll, telex and cpdlc send; poll reply parser; uplink classification (Levenshtein over templates); SayIntentions getWX and BeyondATC METAR/ATIS clients. (done)
4. Uplink interpretation tables (semantic answers, non automatic closing, request set). (done as data and queries; the data-filling part needs the ATC digital inputs, phase 6)
5. FANS area table and ATC logon station state. (area table done)
6. ATC / AOC / Router state machines. Done: `atc.rs` (ATC.ts + MailboxBus.ts merged, UplinkMessageStateMachine, AppendSemanticAnswer / present-data fill over `AtcInputs`), `monitor.rs` (UplinkMonitor, UplinkMessageMonitoring), `ats623.rs`, `input_checks.rs` (offset, vertical speed, persons on board, endurance, temperature, wind, speed ranges, altitude range), `router.rs` (Router.ts), `aoc.rs` (AOC.ts), `vhf.rs` (VHF.ts + VDL.ts + Common.ts; airport list is `data/vhf_airports.txt`, 1382 entries / 1379 unique, loaded at runtime with `VhfAirportList::load`), `nxapi.rs` (NXApiConnector telex + METAR/TAF/ATIS), `winds.rs` (WindUplinkMessage formatters + SimBriefConnector wind parser), `xp_weather.rs` (X-Plane weather source and route wind sampler), `mfd_atccom.rs` (A380X AtcDatalinkSystem + MessageStorage). All validators of InputValidation / FansA / FansB are now ported (`validation.rs`, `input_checks.rs`).
7. Bus bridge: map the topics below onto the plugin bus; MFD ATCCOM pages (native, in a380_displays) subscribe through it. Not started.

## Bus topics and vars the readers use

Everything flows over the EventBus; no L: var other than the two below.

L vars: `A32NX_ACARS_ACTIVE` (written by the Hoppie connector once the ping answers; read by every send/poll), `A32NX_ATSU_RMP_FREQUENCY` (router VHF).

MFD ATCCOM (`MFD/ATCCOM/AtcDatalinkSystem.ts`, `MessageStorage.ts`, `MfdFmsInit`, `MfdAtccomConnect`) reads from datalink:
- atc to FMS: `atcResetData`, `atcRequestAtsuStatusCode {requestId, code}`, `atcGenericRequestResponse`, `atcSystemStatus`, `atcStationStatus {current, next, notificationTime, mode, logonInProgress}`, `atcActiveAtisAutoUpdates`, `atcAtisReports`, `atcMonitoredMessages`, `atcMaxUplinkDelay`, `atcAutomaticPositionReportActive`, `atcResynchronizeCpdlcMessage|DclMessage|OclMessage`, `atcDeleteMessage`, `atcMessageModify`, `atcPositionReport`, `atcPrintMessage`, `atcPrintAtisReportsPrint`.
- router to FMS: `routerDatalinkStatus {vhf, satellite, hf}`, `routerDatalinkMode {vhf, satellite, hf}`, `routerManagementResponse {requestId, status}`.
- published by the MFD itself (A380X): `atcAtis` (indexed), `atcCallsign`.
- aoc to FMS (weather, company messages): `aocResetData`, `aocWeatherResponse`, `aocWindsResponse`, `aocSystemStatus`, `aocTransmissionResponse`, `aocRequestSentToGround`, `aocResynchronizeWeatherMessage|FreetextMessage`, `aocDeleteMessage`, `aocPrintMessage`.

MFD ATCCOM writes to datalink: `atcLogon {station, requestId}`, `atcLogoff`, `atcRequestAtis`, `atcActivateAtisAutoUpdate`, `atcDeactivateAtisAutoUpdate`, `atcTogglePrintAtisReportsPrint`, `atcSetMaxUplinkDelay`, `atcToggleAutomaticPositionReport`, `atcRequestPositionReport`, `atcRegisterCpdlcMessages|DclMessages|OclMessages`, `atcUpdateMessage`, `atcMessageRead`, `atcRemoveMessage`, `atcCleanupMessages`, `atcResetAtisAutoUpdate`, `atcRouteData {lastWaypoint, activeWaypoint, nextWaypoint, destination}`, `routerConnect {requestId, callsign}`, `routerDisconnect`, `routerRequestStationAvailable`, and the aoc set (`aocSendFreetextMessage`, `aocRequestAtis`, `aocRequestWeather`, `aocRegisterWeatherMessages|FreetextMessages`, `aocMessageRead`, `aocRemoveMessage`, `aocRequestWinds`).

SD v2 AtcMailbox (`SDv2/AtcMailbox.tsx`): `cpdlcMessages`, `dclMessages` (mailbox bus, `AtsuMailboxMessages` in `common/databus/Mailbox.ts`: also `oclMessages`, `systemStatus`, `messageStatus`, `logonMessage`, `uplinkResponse`, `downlinkTransmit`, `deleteMessage`, `closeMessage`, `readMessage`, `visibleMessage`, `recallMessage`, `modifyMessage`, `printMessage`, `updateMessageMonitoring`, `stopMessageMonitoring`, `invertSemanticResponse`, `resetSystem`) and `atcMessageButtonActive|Pressed` for the ATC MSG button.

Router to ATC/AOC (internal, would be function calls natively): `routerSendFreetextMessage|CpdlcMessage|DclMessage|OclMessage`, `routerSendMessageResponse`, `routerRequestAtis|Metar|Taf|Winds`, `routerRequestSent`, `routerReceivedWeather`, `routerReceivedWinds`, `routerReceivedFreetextMessage|CpdlcMessage`.

OIT: in this fork the OIT reads nothing from datalink. Its AOC pages use the local `OIT/System/AocMessageStore.ts` fed by `AocUplink.ts` (loadsheet poll, 10 s). A real company-message feed (`msfsCompanyMessageCount`, `aocReceived*`) would be an addition for phase 7.

All 95 topic names (the inventory said about 83) are in `crates/datalink/src/tables.rs`, grouped by direction, generated by `tools/gen_tables.py`.

## Tests

No `*.spec.ts` exists for the datalink in FBW's tree, so tests are written from the wire examples in the sources: `/data2/{cur}/{prev}/{response}/{text}` CPDLC lines, the poll reply layout `ok {CALLSIGN telex {TEXT}}`, the connect.html query layout in `AcarsClient.ts`, and template/serialization behaviour traced by hand from `CpdlcMessage.ts`.


## Native design (atc.rs)

No async, no timers from the host. `Atc` is a synchronous state machine: the host sets `atc.inputs` (`AtcInputs`, `None` = value not in normal operation), calls `tick(now_ms, link)` and drains `take_events()`. Sending goes through the `AtcLink` trait (`send_message`, `receive_atis`); every publisher/DigitalOutputs call of the TS becomes an `AtcEvent`. Timers (5 s monitoring watchdog, 5 s SENT status reset, 300 s logon timeout, 1 s handover poll + 15 s settle, 60 s ATIS auto update, ring interval) live in a scheduled list inside `Atc`. MailboxBus command handlers are `mailbox_*` methods (delete, uplink_response, downlink_transmit, modify, print, close, update/stop monitoring, invert semantic response, recall, read, visible).

Kept from the TS on purpose: `insertMessages` assigns IDs in an inner loop per outer iteration (counter advances N*N for N messages, final IDs match), `MailboxMessage.MessageSent` is never set so handover waits for the station's blocks to be closed, the mailbox `deleteMessage` does not remove the block (display sends `closeMessage`). Not translated: `RadioUtils.packVhfComFrequencyToArinc` (the RMP frequency event carries Hz), `Simplane` STD read (`AtcInputs.baro_std`).

## Online networks (online.rs)

Everything runs behind `HttpClient` (now with `post(url, content_type, authorization, body)`; default returns an error so existing clients are unaffected). Tests use scripted replies and the fixtures in `tests/fixtures/` (field layout taken from the live feeds on 22 Oct, names and ids replaced).

Endpoints:
- VATSIM online ATC: GET `https://data.vatsim.net/v3/vatsim-data.json` (public, no auth). `controllers[]` and `atis[]`: callsign, frequency (string MHz), facility (0 OBS, 1 FSS, 2 DEL, 3 GND, 4 TWR, 5 APP/DEP, 6 CTR), rating, visual_range, text_atis, atis_code. Controllers carry NO position; the position comes from the callsign prefix through the `PositionLookup` trait (try ICAO, then K/C/P + 3 letters). CTR/FSS without a position are listed after the positioned ones. Frequencies 199.998 (observers) are dropped.
- IVAO online ATC: GET `https://api.ivao.aero/v2/tracker/whazzup` (public). `clients.atcs[]`: callsign, rating, `atcSession.{frequency,position}`, `lastTrack.{latitude,longitude}`, `atis.{lines,revision}`.
- SimBrief OFP: GET `https://www.simbrief.com/api/xml.fetcher.php?username=<name>&json=1` (already used by the EFB); mapped to `FlightPlanFields` (ICAO FPL items).
- PDC / DCL on both networks: Hoppie `connect.html` (existing `acars.rs`): `type=ping` to the station first (`is_station_available`), then `type=telex` with the DCL text (`send_dcl_message`). VATSIM controllers using Hoppie answer this way. IVAO has no network-wide CPDLC that I could confirm; a station that is logged on to Hoppie answers the same ping, so the availability check decides.
- Prefile (POST, bearer token): VATSIM default `https://my.vatsim.net/api/v2/flightplan`, IVAO default `https://api.ivao.aero/v2/flight-plans`. These two paths and the JSON field names are NOT verified: neither network documents a public prefile call that I could read without an account, and no live call was made. Both URLs are overridable in settings (`vatsim_prefile_url`, `ivao_prefile_url`); confirm them against the account API docs before shipping. Body builders: `prefile_body` (VATSIM uses the my.vatsim.net flight-plan field names, IVAO the `flightPlan` field names seen in whazzup pilots).
- AOC company messages: `OooiTracker` (OUT: brake released and moving on ground; OFF: leaves ground; ON: touches down; IN: brake set and stopped), `format_oooi_report`, `send_oooi_telex` (Hoppie telex to the company station); `company_weather_uplinks` builds METAR/TAF/NOTAM `CompanyMessage` lists for departure, destination and alternate through the `WeatherSource` and `NotamSource` traits (no NOTAM endpoint chosen yet).

Credentials: `Secret` prints and Debug-formats masked (`a**********6`), `OnlineSettings::display_summary` shows masked values only, transport errors have the token replaced. The settings file itself still holds the raw value (the plugin settings store has no encryption layer).

## Router, AOC, VHF/VDL (wave 3)

Same pattern as `atc.rs`: no async. `Router::update(now_ms, &mut RouterNet, Option<&VdlInputs>)` runs due jobs, sets the active interface, polls Hoppie and the FBW telex at the TS intervals (45-75 s, seeded xorshift), and pushes `RouterEvent`s (`DatalinkStatus`, `DatalinkMode`, `Received`, `SendResult`, `RequestSent`, `Weather`). `RouterNet` bundles the `HttpClient`, both connectors, their settings, the `WeatherSource` and the aircraft status. `HttpClient` gained `put`, `delete` and `get_authorized` (defaults return an error). `Aoc` keeps the uplink/downlink queues and emits `AocEvent`s; sends go out as `AocEvent::Transmit` and come back through `on_send_result`.

VDL inputs come from the host: `NearbyAirport` list (ICAO, elevation, distance; the host's nearest-airport query, max 50), `Traffic` (TCAS/AI targets), `VoiceStation` (online ATC with distance), `OwnAircraft`.

Kept from the TS: `Router.sendMessage` uses a zero-delay timeout (the message goes out on the next tick, the VDL transmission time is only queued); the voice overlap filter keeps stations at or below 136.975 MHz, so it never overlaps the 137.275/137.975 datalink channels; weather requests run the outbound packet, 300-800 ms ground processing and inbound transfer. Changed from the TS: `Airport.Datarates` is per provider (TS shares one tuple between providers via `Array.fill`), `simulateDatarates` averages only providers that reach the station (TS condition is always true), `updateDatalinkStates` returns AtcAoc when ACARS is active (TS overwrites it with Aoc), the "no VHF connection" check looks at every provider (TS checks index 0 only).

## Weather source decision (22 Oct)

All weather (METAR/TAF uplinks, FMS wind uplinks, AOC weather) comes from X-Plane's own weather. `Router::request_weather` reads `RouterNet.weather` (`online::WeatherSource`); the shipped implementation is `xp_weather::XPlaneWeather` over the `XPlaneWeatherHost` trait. TAF: X-Plane has no TAF call, so the source returns None and the uplink reads `NO TAF AVAILABLE`. Winds: `Router::request_winds` / `xp_weather::xplane_winds` sample `weather_at` along a `WindRoute` (first climb point, requested cruise fixes at each requested level, last descent point, alternate point). Climb/descent sampling levels FL50/100/150/200 plus the cruise level are GENERIC. `nxapi::receive_metar/receive_taf` and `winds::receive_simbrief_winds` stay as code but are not wired. ATIS (`Router::request_atis`) still uses `nxapi::receive_atis` (online D-ATIS); open question whether that counts as weather.

Plugin hook (not in this crate, plugin src not edited): implement `XPlaneWeatherHost` in the plugin with
- `metar_for_airport(icao)`: `XPLMGetMETARForAirport(icao, &mut XPLMFixedString150_t)` (XPLMWeather.h, SDK 4.0+).
- `weather_at(lat, lon, alt_msl_m)`: `XPLMGetWeatherAtLocation(lat, lon, alt_m, &mut XPLMWeatherInfo_t)` with `structSize` set; map the at-altitude wind direction (true deg), wind speed (convert m/s to kt; check the header for the unit) and temperature (deg C). Field names to be checked against the SDK header in use.
- Route points and alternate come from the FMS (crates/fms) flight plan.

## Wind uplink (winds.rs)

`format_wind_downlink_message` (`REQPWI/CQ.../WQ...:.../DQ.../WR...`) and `format_wind_uplink_message` (`PWI/CB.../WD...,FIX,DDDSSS,FLLP|Mtt/DD.../AW...`) follow WindUplinkMessage.ts. `parse_simbrief_winds` reads raw SimBrief JSON (`navlog.fix[].wind_data.level[]`, `alternate.avg_wind_*`, `alternate.cruise_altitude`). Kept: descent backfill only runs when the first navlog fix is TOD/DSC, so with a normal OFP descent winds start from levels at or above 45000 ft. The TS would throw on an index of -1 (closest-level pick on the first level); the port skips that level.

## Plugin host, ATIS, SayIntentions (wave 4, dlh40)

ATIS decision: when the pilot has a VATSIM or IVAO token in the EFB settings, the ATIS is the network station's text (`atis_gen::online_atis`: `{ICAO}_D_ATIS` for departure, `{ICAO}_A_ATIS` for arrival, else `{ICAO}_ATIS`). Otherwise `atis_gen::generate_atis` builds a D-ATIS from X-Plane's METAR: letter, time, runway in use (largest headwind component over the navdata runway ends; calm keeps the first listed), wind, visibility, weather, clouds, temperature/dewpoint, QNH or altimeter, transition altitude and level from navdata when present. The letter steps once an hour from the METAR day/hour (GENERIC). `WeatherSource` gained `atis(icao, kind)` (default None); `Router::request_atis` uses it first, the online D-ATIS services (`nxapi::receive_atis`) only when `online_features_enabled`, else `D-ATIS NOT AVAILABLE`.

Bridge (`host_bridge.rs`): `RouterEvent`, `AocEvent`, `AtcDatalinkEvent` map to the FBW topic names (`routerDatalinkStatus|Mode`, `routerReceivedFreetextMessage|CpdlcMessage`, `routerSendMessageResponse {requestId,status}`, `routerRequestSent`, `routerReceivedWeather {requestId, response:[code, message]}`, `aocResynchronize*`, `aocDeleteMessage`, `aocTransmissionResponse`, `aocResetData`, `atcAtis_<index>`, `atcRequestAtis`, `atcActivate|DeactivateAtisAutoUpdate`, `atcRegisterCpdlc|Dcl|OclMessages`, `atcResetAtisAutoUpdate`) and L vars `A32NX_ACARS_ACTIVE`, `A32NX_ATSU_RMP_FREQUENCY`, `A32NX_COMPANY_MSG_COUNT` (AOC DigitalOutputs). `AocEvent::Transmit` is a router call, not a topic.

SayIntentions (`sayintentions.rs`, SAPI docs at p2.sayintentions.ai/p2/docs, base `https://apipri.sayintentions.ai/sapi/`, `api_key` on every call, optional `X-Developer-Key` header through the new `HttpClient::get_with_header`): CPDLC/PDC go through the existing `AcarsProvider::Sai` Hoppie-compatible connect.html (`acars_settings(logon)`); `getCommsHistory` (`since_id`) feeds `AutoTuner`, which tunes COM1 standby to the last frequency read in a voice instruction; `acceptLastHandoff`, `setFreq` (freq MHz, com 1/2, mode active/standby), `getAirSpaceFrequencies`, `assignGate`; `parse_simapi_output` turns `simAPI_output.jsonl` setvar lines (COM*_RADIO_SET_HZ, TRANSPONDER_CODE_SET) into tune/squawk commands; `flight.json` (`http://localhost:43117/flightJSON`) is parsed and compared with the FMS plan (`plan_differences`). Gap: SAPI documents no call that accepts a flight plan, so "flight plan sent from the FMS" is not possible through a documented endpoint; the adapter reports differences between the FMS and SayIntentions' plan instead. No live calls made; fixtures in `tests/fixtures/sayintentions_*`.

Plugin side (`src/datalink_host.rs`): `PluginHttp` (HttpClient on `net::request_bytes`, blocking: worker threads only), `XpWeather` (XPlaneWeatherHost; `XPLMGetMETARForAirport` through `xp::metar_for_airport`, `XPLMGetWeatherAtLocation` with its own v1 struct mirror: `wind_dir_alt` degrees, `wind_spd_alt` m/s converted to kt, `temperature_alt` deg C; SDK 4.0.0 header D:/A380/SDK), `HostWeather` (router WeatherSource: METARs cached from the main thread, no TAF, ATIS chooser), online poller thread (60 s, only with a network token), bus store (`publish`, `take_topics`, `string_var`, `var`). Credentials come from `app_settings` (`hoppieLogon`, `vatsimToken`, `ivaoToken`, `sayIntentionsKey` in xphfbw.json; `MaskedText` Debug prints stars). EFB Settings > Connections has the four masked rows; the existing FBW rows `CONFIG_HOPPIE_USERID` / `CONFIG_SAI_LOGON_KEY` still feed the JS ACARS.

Hook lines for the owner of `src/lib.rs` / root `Cargo.toml` (not edited here):
- Cargo.toml `[dependencies]`: `datalink = { path = "crates/datalink" }`
- src/lib.rs: `mod datalink_host;`
- plugin start: `datalink_host::spawn_online_poller();` and `datalink_host::set_airport_lookup(Box::new(|icao| /* navdata apt::Airport -> AtisAirport { runways: ends ident + true heading, transition_level_ft: tl_ft, transition_alt_ft: ta_ft } */));`
- flight loop: `datalink_host::main_thread_tick(lat, lon);` and log `datalink_host::take_last_error()`.
- xphfbw_host.rs `read_string`: `if let Some(t) = crate::datalink_host::string_var(name) { return Some(t); }`; L var reads: `crate::datalink_host::var(name)`.
- Router/Aoc/AtcDatalinkSystem run on a datalink worker thread with `PluginHttp` and `HostWeather`; their `take_events()` go to `publish_router` / `publish_aoc` / `publish_atc_datalink`. Not wired yet.


## Worker, shadow/authoritative and JS stand-down (dlN94)

`src/datalink_host.rs`: `Worker` wraps `DatalinkSystem` (`set_settings` from the EFB credentials, Hoppie first, else SayIntentions), `spawn_worker()` runs it on thread `fbw-datalink` at 10 Hz (GENERIC) or at once when an input arrives. Main thread: `submit(Snapshot)` each frame, `push_bus_input` / `push_mfd_command` / `push_input(Input::Winds)`, `take_output()`. The worker uses `PluginHttp` only when the snapshot is authoritative; in shadow it gets `Offline` (every call errors), so the JS datalink keeps the only network connection. A panicking step resets the system and sets `take_last_error`.

`src/datalink_native.rs`: `Native::frame` reads SimVarHandling's sources (ZULU, ADIRS 1 ARINC words, AP/A/THR, FMGC phase, DC 1 bus, transponder, FCU altitude, `L:FBW_RMP_MODE_ACTIVE_3`, `L:A32NX_CPIOM_D1/D3_AVAIL`, AC 1 bus, `L:A32NX_DCDU_ATC_MSG_ACK`). Selection: `datalink` in `nativeSystems` (or `native=all`) is authoritative; anything else is shadow; `off` stops it. Authoritative: worker L vars are written, `cpdlc_ring` plays, bus topics go to the JS (`A380X_NATIVE_DATALINK_TAKE`) and to the `datalink_host` store (MFD-local `atcAtis_N` only to the store), wind requests are answered from X-Plane weather along the route from `set_wind_route_provider` (none set: COM FAILED). Shadow: nothing is written; `L:A32NX_DCDU_ATC_MSG_WAITING`, `L:A32NX_ATSU_RMP_FREQUENCY`, `L:A32NX_COMPANY_MSG_COUNT` are compared with the JS values (2 s debounce, GENERIC) while the JS `L:A32NX_ACARS_ACTIVE` is 0 (once the JS is online the native side, offline by design, cannot match).

`src/datalink_native_patches.rs` (SystemsHost.js, after the systems host patches; bit 1 of `L:A380X_NATIVE_DATALINK_MASK`): the init lines forward the 41 input topics (`INPUT_TOPICS`) through `A380X_NATIVE_DATALINK_INPUT` always (shadow sees the same commands) and skip `acr/atc/atsu.init()` + `simVarHandling.initialize()` when native; `Router.update`, `AcarsConnector.activateAcars` and `attemptActivation` return early (no ping, no ACARS_ACTIVE write); `simVarHandling.onUpdate/startPublish` stand down; after the `this.gameState = gamestate;` block the native topics are taken each frame and published with sync. Limit: the init decision is made at `connectedCallback`; the plugin writes the mask on its first flight loop, which runs before the SystemsHost page loads. Switching authoritative to shadow mid-flight needs a SystemsHost reload.

Hook lines (owner of lib.rs / js_bridge.rs / native_mode.rs):
- src/lib.rs after `mod datalink_host;`: `#[cfg(feature = "js")] mod datalink_native;` and `mod datalink_native_patches;`
- src/lib.rs `source_patches()` after `patches.extend(native_systems_host_patches::source_patches());`: `patches.extend(datalink_native_patches::source_patches());`
- src/lib.rs flight loop after `native_systems_host::tick(&mut self.vars, self.time);`: `datalink_native::tick(&mut self.vars, self.time);`
- src/js_bridge.rs coherent calls after the `native_systems_host::coherent_call` block: `if let Some(json) = crate::datalink_native::coherent_call(name, args_json) { return Some(CallReply::Resolved(json)); }`
- src/js_bridge.rs patch list after `patches.extend(crate::native_systems_host_patches::source_patches());`: `patches.extend(crate::datalink_native_patches::source_patches());`
- src/native_mode.rs `SYSTEMS`: `(Group::Syshost, "datalink", false),` so `systems_spec()` keeps the name (until then only `native=all` selects it).
- Native FMS owner: `datalink_native::set_wind_route_provider(Box::new(|| /* crates/fms plan -> WindRoute */))`; native MFD ATCCOM: `datalink_host::push_mfd_command`, `datalink_host::push_bus_input("atcRouteData", ...)`.

Tests: `cargo test --features js --lib datalink` (host 8, native 7, patches 3; the real-bundle anchor test runs with `FBW_HTML_UI`).
