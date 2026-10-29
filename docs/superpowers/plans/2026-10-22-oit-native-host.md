# Native OIT host (src/oit_native_host.rs)

Goal: the two OITs drawn by oitx from a document the plugin builds itself (crates/oit_native + crates/oit_pages), with FlyByWire's OIT behaviour, instead of the CEF OIT's DOM snapshot.

## What the module does

- Display unit (OitDisplayUnit.tsx): Off / Standby / Bootup / Selftest / On from the side's buses (CAPT AC 2, DC 2; F/O AC ESS, DC ESS IN FLIGHT, DC 1), LIGHT POTENTIOMETER 78/79, `CONFIG_SELF_TEST_TIME` (quick mode 0), 10 s standby; "Not available." when the domain's ANSU or the laptop is lost.
- Sessions: FLT OPS and NSS AVNCS login screens, loading bars (self-test time x2 / x1.5, 4 Hz), domain switch `L:A380X_SWITCH_OIT_SIDE_LEFT/RIGHT`, laptop loss resets the FLT OPS session, ANSU loss returns AVNCS to its menu.
- Navigation (OitUiService.tsx): per-domain stacks with back/forward, MENU and FUNCTIONS page selectors (HOME, PREVIOUS, NEXT, PRINT, STORE, UPDATE, HELP, HIDE SWITCHING BAR, CLOSE APPLICATION, EXIT SESSION), AVNCS HOME/BACK/FORWARD, folder navigator expand/collapse.
- Fields and typing: STS (flt number, from, to), T.O PERF (airport, T.O shift), LDG PERF (airport, LW), remote C/B search; selected/editing states, commit on Enter or blur, Delete clears; AOC compose inputs and the charts ICAO input.
- Page models from live data: T.O PERF / LDG PERF (runways from the navigation database, weather, FQMS weights, SimBrief TLR, /perf/cal, engine rating; results computed by oit_pages), IN-FLT PERF (live ADR/FQMS data, steppers, FM calibration learner), LOADSHEET (+ SEND TO AOC, 10 s PRELIM/FINAL uplink), FLT FOLDER (SimBrief OFP via oit_native::ofp), CHARTS (/study/charts, /oit/charts, OFP route maps, zoom/rotate/night), OPS LIBRARY (/oit/docs, /oit/doc, /study/mel), LOAD BOX, EXPORT BOX and PRINT/STORE (/oit/export), AOC store per side, STS; AVNCS COMPANY COM inbox (AOC store plus datalink AOC uplinks `aocResynchronize*Message`) and flight log (oit_native::data::Oooi), MAINTENANCE via `maintenance_log::avncs_model`, remote C/B commands (`L:A32NX_BKR_<id>_CMD`), A380X SYSTEMS app and ldg capability and FWS debug.
- Output: `a380.oit.dom.1` (CAPT) / `.2` (F/O) on the native displays' bus tap, only when the document changed; rebuilt on input or every 0.25 s (GENERIC).
- Input: pointer presses hit-tested against oitx's boxes (`data-act` marks), console keys while a field has focus.
- Shadow: the CEF snapshot is compared element by element (`oit_native::compare`), logged as `native shadow: oit N a/b elements agree` on the first and every 600th mismatch, then dropped.

Tests: 19 unit tests in the module (state machine, navigation, loading, login/menu flow, domain switch and laptop loss, menus, field typing and synchro, T.O PERF runways/dropdowns/compute, AOC compose/read, AVNCS navigation and inbox, remote C/B, FLT FOLDER fetch, charts ICAO entry, IN-FLT PERF steppers, loadsheet to AOC, hit testing, key map, compare, navdb facility). They run in the stub harness `E:/fbw-build/oit-host` (`CARGO_TARGET_DIR=E:/fbw-build/target cargo test --release`), which includes the file by path with stand-ins for the plugin modules it calls; once hooked they run with the plugin's own tests.

## Hook lines (not applied)

Cargo.toml, `[workspace] exclude`: append

```toml
"crates/oit_native", "crates/oit_pages"
```

Cargo.toml, `[dependencies]` after `mfd_native = { path = "crates/mfd_native" }`:

```toml
oit_native = { path = "crates/oit_native" }
oit_pages = { path = "crates/oit_pages" }
```

src/lib.rs, after `mod mfd_native_host;`:

```rust
#[cfg(feature = "native-displays")]
mod oit_native_host;
```

src/lib.rs, `tick`, inside the `#[cfg(feature = "native-displays")] { ... }` block right after the `efb_native::host::tick();` if-block:

```rust
                oit_native_host::tick(&mut self.vars, self.time);
```

src/display/mod.rs, `native_pointer`, after the `mfd_native_host::pointer` if-block:

```rust
        if matches!(id, "SCREEN_OIT_LEFT" | "SCREEN_OIT_RIGHT") && matches!(kind, Pointer::Down) && crate::oit_native_host::pointer(id, cx, cy) {
            return true;
        }
```

src/display/mod.rs, `dispatch_key`, after the MFD `#[cfg(feature = "native-displays")]` block:

```rust
        #[cfg(feature = "native-displays")]
        if matches!(self.screens[screen].def.id, "SCREEN_OIT_LEFT" | "SCREEN_OIT_RIGHT") && flags & 16 == 0 && crate::native_displays::input::is_native(self.screens[screen].def.id) && crate::oit_native_host::key(self.screens[screen].def.id, ch, vkey) {
            return true;
        }
```

src/xphfbw_host.rs, `route_view_trigger`, replace the condition under the first `GENERIC_DATA` arm:

```rust
                if !(s(1).starts_with(MFD_DOM_BATCH) && crate::mfd_native_host::observe_cef(&s(1))) && !(s(1).starts_with(OIT_DOM_BATCH) && crate::oit_native_host::observe_cef(&s(1))) {
```

Two more lines outside those four files are needed for the feature to be complete:

src/native_displays/screen.rs, `step`, after the `mfdx` branch:

```rust
            } else if s.spec.target.display == "oitx" {
                crate::oit_native_host::set_boxes(s.spec.target.variant, &s.layers[0].display.boxes());
```

src/native_fms_shadow.rs, `tick`, after `running.host.tick(dt_ms, Inputs::from_vars(&mut get));` (flight number, airports, runways for STS, perf pages, loadsheet, AVNCS header):

```rust
        #[cfg(feature = "native-displays")]
        if crate::oit_native_host::fms_due(time) {
            if let Some(s) = running.host.snapshot() {
                crate::oit_native_host::set_fms(crate::oit_native_host::fms_info(s.plans.first().and_then(Option::as_ref)));
            }
        }
```

## Left

- The glue (`mod plugin`) is checked against stand-ins only; the first plugin build after hooking is the real compile check of its calls into js_bridge, net, oit_files, perf_cal, study::web, oans::plugin, efb_native::host, livery_identity, datalink_host, maintenance_log and native_fms_shadow.
- Not wired: OIT failures (`A380Failure.CaptainOit/FirstOfficerOit` power the screen off in FBW), FWS debug transmit control (FBW publishes a bus event; the toggle is local), remote C/B breaker list (taken from `content.breakers` of the maintenance JSON, which `maintenance_log::avncs_model` does not provide yet), logbook rows, system status rows by ATA (the model has no ATA number to navigate with), OPS LIBRARY page up/down scrolling, chart image fit to natural size (FIT resets zoom to 1), chart/OFP pictures are given as `src` on the `img` but oitx must load them.
- Runways from the navigation database have no width or displaced thresholds (width 0, LDA = length).
- Per-page state persists across navigation (FlyByWire rebuilds most pages on each visit); UPDATE re-runs the page's entry actions.
- Verify against v15 captures once hooked: compare logs, then fix the first differing element per page.
