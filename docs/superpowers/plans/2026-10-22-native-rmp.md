# Native RMP (crates/rmp)

Goal: the A380X radio management panel logic and the VHF / transponder / audio-management systems-host logic as a std-only Rust crate (`crates/rmp`), so the RMP instrument JS no longer has to run in a CEF view. The crate has no X-Plane dependency. The plugin hooks (not done here) copy its var writes into the same names `src/radios.rs`, `src/lights.rs` and the native `rmp` display already read.

## Sources translated (FBW)

- `fbw-a380x/src/systems/instruments/src/RMP/`: `instrument.tsx` (update order), `Systems/RmpStateController.ts`, `KeypadController.ts`, `AudioControlManager.ts`, `RmpMessageManager.ts`, `TransmitMessageMonitor.ts`, `Flasher.ts`, `Data/*Publisher.ts` (var names), `Components/RmpPageStack.tsx`, `Pages/VhfCom/VhfComPage.tsx` + `VhfComController.ts`, `Pages/Transponder/TransponderPage.tsx` + `TransponderController.ts`.
- `fbw-common/src/systems/instruments/src/RMP/`: `RmpUtils.ts`, `Systems/VhfComManager.ts`, `Systems/RmpStateControllerTypes.ts`.
- `fbw-common/src/systems/shared/src/RadioUtils.ts`, `RadioTypes.ts`, `Arinc429.ts` (word packing only).
- `fbw-a380x/src/systems/systems-host/Misc/Communications/`: `VhfRadio.ts`, `Transponder.ts`, `AudioManagementUnit.ts`, `RmpAmuBusPublisher.ts`, `SimAudioManager.ts` (decisions only; the key events become `SimAudioCommand`s).
- `fbw-a380x/src/systems/extras-host/modules/key_interceptor/KeyInterceptor.ts`: the RMP-targeting COM key events (`shouldControlRmpN`, fract/whole increment, active set, swap, receive switches).

## Crate shape

- `radio`: BCD32 helpers, channel tables, validity, next-valid-channel.
- `vars`: name -> f64 store (the crate's whole I/O surface) plus a `KeyEvent` out list.
- `rmp`: one RMP (index 1..3): state controller, 3 VHF managers, keypad, pages (VHF, squawk), messages, audio control manager, transmit monitor. `Rmp::hevent(name)`, `Rmp::update(dt_ms, &mut Vars)`, `Rmp::view()`.
- `host`: `VhfRadioUnit` x3, `TransponderUnit`, `Amu` x2, `SimAudio` (commands), `KeyInterceptor` RMP part.

## Vars the native `rmp` display reads that this crate must produce

Written by the RMP model (per `Rmp::update`, index n = the RMP shown by the display, all three RMPs share the frequency vars through the inter-RMP bus):

- `L:FBW_RMP_FREQUENCY_ACTIVE_1..3`, `L:FBW_RMP_FREQUENCY_STANDBY_1..3` (BCD32)
- `L:FBW_RMP_MODE_ACTIVE_1..3`, `L:FBW_RMP_MODE_STANDBY_1..3` (0 frequency, 1 data, 2 emergency)
- `L:FBW_RMP3_BUS_A` read by RMP 1/2 (written by RMP 3 select-bus logic via `L:FBW_RMP3_BUS_A`)
- `L:A380X_RMP_n_STATE`, `_RED_LED`, `_GREEN_LED` (lights.rs input)
- `L:A380X_RMP_n_*_VOL_RX*`, `_TX*` audio vars (radios.rs / AMU consumers)

Written by the host model:

- `L:FBW_VHF1..3_FREQUENCY` (ARINC word, ssm in the upper 32 bits, 0 = failure warning drives the amber active frequency)
- `TRANSPONDER STATE:1` (enum) and, for the squawk line, `TRANSPONDER CODE:1` is MSFS/X-Plane owned (the crate only reads it).

Inputs only (display and crate read, not produced): `L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED`, `L:A32NX_ELEC_DC_1_BUS_IS_POWERED`, `L:A380X_RMP_n_BRIGHTNESS_KNOB`, `TRANSPONDER CODE:1`, `CIRCUIT AVIONICS ON`, `COM ACTIVE/STANDBY FREQUENCY:n` (initial load only).

Not drawn by the current native display (it only paints the active/standby rows and the squawk line): the entry digits, the amber flash, the message line, the loudspeaker symbol, the transmit reverse-video, the SQWK page, the TRANSMITTING IDENT text. `Rmp::view()` returns all of those so the display can adopt them.

## Oracle (design; not built in the first pass)

`crates/rmp/oracle/` holds a driver that bundles FBW's own RMP TypeScript (state controller, VHF manager, keypad, audio control manager, message manager, transmit monitor, both page controllers, RadioUtils, Arinc429 packing) with esbuild against a small independent SDK stand-in (`sdk-shim.ts`: bus, subjects, consumer subjects, timers, simvar publisher), runs it in QuickJS (rquickjs) with a virtual clock and records every var write and key event while scripted key presses are applied. Scenarios (JSON) live in `oracle/scenarios/`, goldens are recorded under `E:/fbw-build/rmp-oracle/golden/`. `tests/oracle.rs` replays each scenario through the Rust model and compares the write traces. Regenerating the goldens needs `RMP_ORACLE_RECORD=1` and node + esbuild from `E:/fbw-int/fbw-aircraft/node_modules`.

Limit: the SDK stand-in is not the real MSFS SDK, so the oracle proves the translation of FBW's RMP logic, not the SDK semantics; the comparison against the real installed RMP bundle needs a plugin hook in `src/js/msfs` (see the report).

## Steps

1. Plan (this file).
2. Crate: radio, vars, rmp model, host model, unit tests.
3. Oracle: sdk-shim, driver, scenarios, goldens, comparator.
4. Report what is left (the oracle step 3 is the open item) (plugin hooks, display adoption of `view()`).
