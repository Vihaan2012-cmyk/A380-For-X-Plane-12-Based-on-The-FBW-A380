# SmartCopilot shared cockpit (A380 for X-Plane 12)

Goal: two pilots share one A380 through SmartCopilot. Whoever hosts the
SmartCopilot session is the master and runs every system; the other seat
(slave) runs the cockpit, the displays and its own inputs only.

Status: plan + generated config. No plugin code changed yet.

## 1. What SmartCopilot does (public sources)

Sources: Sky4Crew's config reference (sky4crew.com/hangar), SmartCopilot v3
release notes (fsnews.eu), and shipped `smartcopilot.cfg` files on GitHub
(Tu-154B2-CE, Laminar 747 SC fork, SmartFlying).

- `smartcopilot.cfg` sits in the aircraft root. Since v3 only the master's
  file counts.
- Sections and direction:

| Section | Direction | Sent | Value after `=` |
|---|---|---|---|
| `[TRIGGERS]` | both ways | on change (min. cycle 25-30 ms, FPS bound) | `0`; non-zero = press-and-hold position |
| `[CLICKS]` | both ways | pulse 0-1-0 | needs a satellite `<name>_SCP` dataref the add-on sets |
| `[CONTINUED]` | master -> slave | continuously | `0` soft sync, `1` hard set each frame |
| `[SEND_BACK]` | side with control -> side without | continuously | `0` |
| `[COMMANDS]` | both ways | when executed | `0`; a handler that returns 0 hides the command from SCP |
| `[OVERRIDE]` | local | by state | bitmask: 1 master+control, 2 master no control, 4 slave+control, 8 slave no control, 16 not connected |
| `[SLOW]` | master -> slave | every `SLOW_INTERVAL` s | `0` |
| `[WEATHER]` | master -> slave | every `WEATHER_INTERVAL` s, master toggles | `0` |
| `[TRANSPONDER]` | both ways | on change, master toggles | `0` dataref, `1` command |
| `[SETUP]` | - | - | `SLOW_INTERVAL`, `WEATHER_INTERVAL`, `USE_AUTOPILOT_SEQUENCER`, `ENHANCED_SLAVE_SMOOTHNESS`, `DISABLE_SMOOTHNESS_ON_GROUND`, `SMOOTHNESS_*` gains |
| `[INFO]` | - | - | `ID`, `VERSION`, `AUTHOR`, each at most 20 characters |

- Types: int, float, int/float arrays (`name[i]`), byte arrays (strings).
  Arrays in `[WEATHER]`/`[SLOW]` appear in shipped files as
  `name[i]_FIXED_INDEX_`.
- Role datarefs (read-only ints): `scp/api/ismaster` and
  `scp/api/hascontrol_1`; 0 = SmartCopilot not running/not defined,
  1 = slave / no control, 2 = master / has control. Sky4Crew's text gives one
  value table for both; read both and log them on the first session to
  confirm.
- Add-ons with their own systems read `scp/api/ismaster` and stop their
  system logic on the slave (Sky4Crew: "disable slave systems or block
  throttle input for a user without control").

## 2. Audit: what SmartCopilot can and cannot see today

Can see (datarefs or commands already):

- Every FBW variable the plugin registers (simulator and `L:`) is published as
  a writable `fbw/<sanitised name>` dataref (`src/lib.rs` `Vars::publish`,
  accessors in `src/xp.rs` `Xplm::publish`: int/float/double, scalar only).
  Sanitising turns everything except `[A-Za-z0-9_/]` into `_`, drops nothing,
  and numbers collisions `_2`, `_3`; the `L:`/`A:` prefixes are not part of
  the stored name.
- Cockpit manipulators in the installed OBJs (counted 22 Oct): 453
  `command`, 217 `toggle`, 197 `push`, 72 `drag_axis`, 71 `command_knob`,
  27 `axis_knob`, 23+2 `axis_switch_*`, 11+2 `command_switch_*`,
  14 `device` (screens). Unique: 611 commands, 480 datarefs (464 plain,
  16 functional).
- Command handlers: SASL `command()` in the generated
  `plugins/sasl/data/modules/main.lua` returns 1, and `published.rs`
  `on_command` returns 1, so SmartCopilot sees both. Only
  `on_intercepted` (X-Plane's own commands the plugin takes over) returns 0;
  those are invisible to SCP and must be forwarded by the plugin (hook H6).

Cannot see:

| State | Where | Why not |
|---|---|---|
| 16 functional inputs `fbw/cockpit/*_pp`, `*_hold` | SASL `knob_pp`/`switch_hold` | getter always returns 0, the write is an event |
| SASL `LV[...]` table | main.lua | MSFS-local template state, Lua only |
| 8 string vars the displays read (`ADF IDENT:n`, `NAV IDENT:n`, ...) | display inputs | `fbw/*` datarefs are numbers only |
| Screen clicks (`ATTR_manip_device` on MFD x4, ND L/R, EFB x4, OIT L/R x2) | `native_displays`, XPHFBW | not datarefs at all |
| Display bus topics, generic keys, fetched bodies | `native_displays/inputs.rs` `InputSink::set_bus/set_generic/set_fetched` | in-process only |
| Native FMS (flight plans, perf, temporary/secondary/uplink) | `crates/fms` (`Fms`, `FlightPlan`) | Rust memory |
| Deep model cross-area bus (~4,200 values, most internal) | `src/deep` | kept in `Deep`, only couplings published |
| Systems process memory | `src/remote` | shared block, mirrored into `Vars` only |
| Page-held state for stopped pages | `src/native_page_state.rs` | partly written to `L:` vars (those sync), rest in a `Mutex` |
| Failures armed/scheduled, MEL, wear, persistence (`airframe.json`) | `failures.rs`, `mel.rs`, `wear.rs`, `persistence.rs` | files / Rust state |
| EFB/OIT app state (SimBrief, loadsheet, settings) | XPHFBW app, `efb.rs`, `oit_files.rs` | app process |

## 3. Architecture

Master = SmartCopilot host = `scp/api/ismaster == 2`. Not connected (0)
behaves exactly as today.

Master:

- Runs everything: systems (local or `src/remote`), deep layer, FADEC,
  flight controls, FMS, displays, SASL.
- Owns X-Plane's overrides as today.
- Applies slave inputs from the scp channel (H4) as if clicked locally.
- Publishes the FMS snapshot (H5).

Slave (`ismaster == 1`):

- Does not tick systems, deep, FADEC, fuel network, flight controls,
  physics damage, failures, random/scheduled failures, persistence writes,
  GSE/walkaround side effects. `Vars` stays alive as plain storage so
  SmartCopilot's writes land in it and every reader (displays, SASL `look`
  animations, sounds) sees master's values.
- Releases the plugin's own overrides (control surfaces, throttles, gear
  brakes, steering, fuel flow) so SmartCopilot's position sync drives the
  airframe; `[OVERRIDE]` in the cfg then holds joystick/throttles on whichever
  side has no control.
- Runs displays (native or CEF) from `Vars`. Display-local state (MFD page,
  cursor, ND range knob echo) stays local per side.
- Clicks: toggle/push/axis manipulators write the dataref locally and
  `[TRIGGERS]` carries the new value to master. Command manipulators: SCP
  forwards the command to master; the slave must NOT also run it locally
  (stepping knobs would double-step when master's value comes back) -- H3.

Role changes mid-flight (master leaves): SmartCopilot ends the session; the
remaining slave restarts systems from the last synced `Vars` through the same
path `restore_computed_values` uses (H8).

### Which L:vars become synced datarefs

All of them already are datarefs. The generated list picks:

- `[TRIGGERS]` (on change, both ways): every manipulator dataref (cockpit
  switch L:vars and `fbw/cockpit/*` positions, 464) plus every display-read
  variable whose unit is bool/enum (462) plus the 4 scp channels. 930 entries.
- `[CONTINUED]` (master -> slave): display-read variables with continuous
  units (angles, speeds, temperatures, quantities). 816 entries.
- `[SEND_BACK]`: sidesticks (`sim/joystick/yoke_*`, `fbw/cockpit/yoke_lever_*`),
  thrust levers (`throttle_ratio[0..3]`), toe brakes. 11 entries.
- `[COMMANDS]`: every manipulator command. 611 entries.
- `[SLOW]`: time, payload and total fuel. Fuel/payload detail rides in the
  display read set already.
- `[OVERRIDE]`: `override_joystick = 10`, `override_throttles = 10` (the
  pattern shipped by other airliners).

Variables outside the display read set that the slave still needs (sound
triggers, SASL lights/anim inputs, cabin) are the next audit step: run the
plugin as master with a dataref-read logger on the slave-side readers and
add what they read to `sync_list.json` (H9).

### Input forwarding

New plugin datarefs (TRIGGERS, both ways):

- `fbw/scp/in_seq` (int), `fbw/scp/in_event` (byte array, <= 256 bytes):
  slave writes one event and bumps the sequence; master applies it and
  echoes the sequence. Events: `pp <dataref> <v>`, `hold <dataref> <v>`,
  `scr <screen> <x> <y> <phase>`, `key <screen> <code>`, `fms <json>`,
  `cmd <name>` (intercepted X-Plane commands). One event in flight; slave
  queues the rest (GENERIC: queue depth 64).

### FMS / MFD

- The FMS model runs on master only.
- Slave's MFD UI runs locally (own page, cursor, scratchpad). Data comes
  from an FMS snapshot; edits become `crates/fms` `service::Command`
  values sent as `fms <json>` events. `Command` needs `Serialize/Deserialize`
  (plus `Fix`, `AltitudeConstraint`, `PlanRef`).
- Snapshot: `fbw/scp/fms_seq` (int) + `fbw/scp/fms_blob` (byte array):
  compact JSON of active/temporary/secondary plans and perf fields, sent on
  change only. If over SCP's practical byte-array size, chunk it
  (`fms_blob` carries `part/total` header, GENERIC: 4 KB per chunk).
- A rejected command (master `FmsError`) comes back as an `err` event so the
  slave scratchpad shows the message.

### Bandwidth budget (GENERIC estimates)

- `[CONTINUED]` worst case: 816 values x ~8 bytes = ~6.5 KB per cycle; at
  30 ms cycles ~215 KB/s (~1.7 Mbit/s) if SCP sends all of them every cycle.
  Target <= 64 KB/s: move temperatures/pressures/quantities (celsius, psi,
  volts, kg, percent on SD pages) to a 1 s tier and drop variables the slave
  can compute from synced X-Plane state (radio frequencies, air data simvars
  fed by `input_for`).
- `[TRIGGERS]`: on change only; idle cost ~0.
- FMS blob: only on edit; < 20 KB per change.

## 4. Plugin hooks (exact places)

- H1 `src/lib.rs` `Plugin::tick`: read `scp/api/ismaster` and
  `scp/api/hascontrol_1` once per tick into a `Role { None, Master, Slave }`
  (new `src/smartcopilot.rs`). Log transitions.
- H2 `src/lib.rs` systems step: when `Role::Slave`, skip
  `remote::Systems::tick`, the deep layer tick (`src/deep/plugin.rs`),
  `fadec.rs`, `fuel.rs`/`fuel_network.rs`, `flight_controls.rs`,
  `handling.rs`, `physics`, `failures.rs` / `random_failures.rs` /
  `scripted_failures.rs`, `persistence.rs` writes, `wear.rs`; keep `Vars`
  publish, displays, sound, SASL bridge. Release the overrides those modules
  set (`override_control_surfaces`, `override_throttles`, `override_gearbrake`,
  `override_toe_brakes`, `override_wheel_steer`, `override_fuel_flow`,
  `override_prop_mode`).
- H3 `msfs2xp-aircraft` SASL emitter (generated `main.lua` `command()`,
  `knob_pp`, `switch_hold`): when `scp/api/ismaster == 1`, `command()` does
  nothing locally (SCP forwards it); `knob_pp`/`switch_hold` write an
  `in_event` instead of acting.
- H4 new `fbw/scp/*` datarefs via `published.rs`; master consumes
  `in_event` in `Plugin::tick` before the systems step.
- H5 `crates/fms`: serde on `Command` and plan types; snapshot writer on
  master, reader on slave; the MFD's FMS calls go through one function that
  either executes locally or sends `fms <json>`.
- H6 `published.rs` `on_intercepted`: on slave, send `cmd <name>` instead of
  pressing locally.
- H7 `native_displays` / XPHFBW screen input: on slave, MFD/OIT/EFB clicks
  that change shared state go out as `scr`/`key` events; pure page
  navigation stays local.
- H8 role change Slave -> None: restart systems from current `Vars`
  (`restore_computed_values` path) instead of a cold start.
- H9 audit tool: list every `fbw/*` dataref the slave-side readers read in
  one flight and diff against `sync_list.json`.
- H10 strings: publish the 8 string vars as byte-array datarefs so they can
  sync (or have the slave derive them from X-Plane navaid lookups).

## 5. Tools

- `tools/smartcopilot/derive_list.py`: scans the cockpit OBJ manipulators
  and `src/native_displays/read_sets.txt`, writes `sync_list.json`
  (entries + `not_syncable` + counts).
- `tools/smartcopilot/gen_cfg.py sync_list.json --out <cfg>`: writes
  `smartcopilot.cfg` (CRLF), refuses unknown sections, a dataref in two
  sync sections, or `[INFO]` values over 20 characters.
- Output: `E:/fbw-build/smartcopilot/smartcopilot.cfg`. Ship it in the
  aircraft root only after H1-H4 exist; without them both sides run full
  systems and fight over every value.

## 6. Test plan

1. Two X-Plane instances on one LAN, same build, master hosts.
2. Cold and dark: slave flips BAT 1 -> master's ELEC SD shows it, slave SD
   follows.
3. Knob step (FCU ALT): one click on slave = one step on both.
4. Sidestick hand-over: control transfer in SCP, check `SEND_BACK` drives
   master.
5. MFD: slave inserts a waypoint; master plan changes; both NDs draw it.
6. Measure SCP bandwidth in its UI at cruise; trim `[CONTINUED]` to budget.
