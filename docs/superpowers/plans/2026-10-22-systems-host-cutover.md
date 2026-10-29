# SystemsHost / ExtrasHost cutover (syshost69)

Every SystemsHost/ExtrasHost module that has a native counterpart now runs in shadow beside the JS (`src/native_systems_host.rs`), with comparison logging (`native shadow: <module> <var> native=.. fbw=..` plus the 10 s summary), a bit in `L:A380X_NATIVE_SYSHOST_MASK` and a JS stand-down patch (`src/native_systems_host_patches.rs`). Authoritative per module via `nativeSystems` (same setting as `native_shadow.rs`; names below; `all` leaves out fws, btv, baro_unit, pilot_seat until their blockers close).

## Wiring for int53 (not done here: lib.rs and root Cargo.toml are int53's)

Root `Cargo.toml` `[dependencies]`:

```toml
rmp = { path = "crates/rmp" }
oans_native = { package = "oans", path = "crates/oans" }
fws = { path = "crates/fws" }
```

Root `Cargo.toml` `[workspace] exclude` add: `"crates/rmp", "crates/oans", "crates/fws", "crates/fws_data"` (each has its own `[workspace]`). `oans` is renamed because `src/lib.rs` already has `mod oans`.

`src/lib.rs`, next to `mod native_shadow;` / `mod native_shadow_patches;`:

```rust
#[cfg(feature = "js")]
mod native_systems_host;
mod native_systems_host_patches;
```

`src/lib.rs` `static_source_patches()`, after `native_shadow_patches`: `patches.extend(native_systems_host_patches::source_patches());`
`src/js_bridge.rs` `native_ports()`, after `native_shadow_patches`: `patches.extend(crate::native_systems_host_patches::source_patches());`
`src/js_bridge.rs` `direct_call`, after the `native_shadow::coherent_call` line:

```rust
if let Some(json) = crate::native_systems_host::coherent_call(name, args_json) {
    return Some(CallReply::Resolved(json));
}
```

`src/lib.rs` tick, right after `native_shadow::tick(&mut self.vars, self.time);`:

```rust
native_systems_host::tick(&mut self.vars, self.time);
h_events.extend(native_systems_host::take_h_events());
```

(`h_events` must be `let mut` unconditionally then.) Tests: `cargo test --features js native_s` (harness run: E:/fbw-build/syshost69/harness, 37 pass incl. the real-bundle anchor test with `FBW_HTML_UI`).

## Module table

| JS module | native | name / bit | compared |
|---|---|---|---|
| LegacyGpws, LegacyTcasComputer, FWS health | native_shadow.rs | gpws 1, tcas 2, hosts_misc 4 (`L:A380X_NATIVE_SYSTEMS_MASK`) | yes |
| VhfRadio x3 | rmp::VhfRadioUnit | vhf 1 | `FBW_VHF{n}_FREQUENCY` |
| Transponder | rmp::TransponderUnit (MFD AUTO/ALT RPTG mirrored by the patch) | xpdr 2 | `TRANSPONDER STATE:1` |
| AudioManagementUnit x2 + SimAudioManager | rmp::Amu + sim_audio_target, keys translated from SimAudioManager | amu 4 | `COM RECEIVE/TRANSMIT:n` |
| BrakeToVacate | oans::btv | btv 8 | all its L vars |
| EfisTawsBridge failure flags (+ wxr patch) | translated | efis_taws 16 | TERR/GPWS/WXR 1-2 FAILED |
| FwsCore (vars/strings) | crates/fws (fwsG67 API) | fws 32 | every var/string it emits |
| LightSync | hosts_misc::light_sync (5 s steady init as extra_backend_fbw) | light_sync 64 | `A32NX_CABIN_AUTOBRIGHTNESS` |
| A380XKeyInterceptor | hosts_misc::key_interceptor + rmp::KeyInterceptorRmp, keys forwarded by the patch | key_interceptor 128 | event-driven: twin tests only |
| PilotSeatManager | hosts_misc::pilot_seat | pilot_seat 256 | `FBW_PILOT_SEAT`, `FBW_IN_FLIGHT_DECK` |
| PushbuttonCheck | hosts_misc::pushbutton_check | pushbutton_check 512 | notification only |
| BaroUnitSelector | hosts_misc::baro_unit | baro_unit 1024 | HPA vars, 10 s after the choice |
| `A32NX_IS_READY` | written natively | ready 2048 | yes |
| LegacyFuel, GPUManagement | fuel_transfer.rs, efb.rs (already removed from the hosts) | - | - |

## Remaining blockers (the hidden views cannot stop until these close)

1. FWS: crates/fws emits no `fws_*` bus topics (EWD/SD procedures, checklists, INOP SYS, limitations, `a380x_ois_fws_debug_data`), no FwsSoundManager aurals, no ECP/SD-page bus inputs.
2. Datalink: AtsuSystem, AtcDatalink, Acr, SimVarHandling. crates/datalink has the router/ATC logic but no host loop; MFD ATCCOM/AOC requests arrive on the JS bus; running it in shadow would duplicate network calls.
3. EfisTawsBridge terrain feed: aircraft status and VD path posts to the terrain worker, EfisDataSync (FCU bus per side).
4. FmsSymbolsPublisher('L'), FmsMessagePublisher: FMS (native FMS plan).
5. BTV inputs from the bus (OANS runway/exit selection, FMS landing runway, manual stopping distance) and a live `NavDb` (`set_btv_navdb`, e.g. FmsNavDb + navdata nearest): needs a JS bus tap into the plugin (none today; `native_displays/inputs.rs` has no reader) — same tap would let bus outputs be compared.
6. Transponder AUTO/ALT RPTG come from the MFD bus (mirrored through L vars by the patch until the MFD is native).
7. ENGINE_AUTO_START/SHUTDOWN confirmation dialogs (UI), notification display (shown by ExtrasHost via the TAKE call).
8. Stored data (`CONFIG_PILOT_SEAT`, `CONFIG_INIT_BARO_UNIT`) and nearest-airport search are not reachable from the live Io (`stored`/`nearest_airport` return None: seat Left, baro waits).
9. Patches gated at view start (baro selection) only stand down if the mask is written before ExtrasHost's `connectedCallback`.
10. No counterpart needed: ~25 SimVarPublishers (inputs only), Clock/HEventPublisher, LegacySoundManager (GPWS/TCAS native), VersionCheck, TelexCheck, MsfsVersionPopupMonitor, GsxSync, AircraftSync (empty), ExtrasHost NavigationDatabase.

crates/row_rop, cids, fap_pages, fuel_mgmt, cpcs_extra, wxr_extras do not map to any SystemsHost/ExtrasHost module (FwsCore only reads `A32NX_ROW_ROP_WORD_*`).
