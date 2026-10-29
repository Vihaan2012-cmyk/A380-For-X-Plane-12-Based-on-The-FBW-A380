# Native FMS and MFD

Date: 21 October 2026

## Goal

Run the A380's flight management system in Rust inside `fbw_a380_systems`. Draw the MFD with a separate C++ X-Plane plugin, `a380_displays`. No Microsoft framework code is involved anywhere in the FMS or the MFD.

This is the first of the native displays. The FMS comes first because every other screen consumes its output. Once the FMS lives in Rust, the ND, PFD, EWD and SD become drawing jobs.

## Decisions already made

- **Drawing:** C++, in a separate X-Plane plugin built with CMake and the X-Plane SDK (`D:/A380/SDK`).
- **Logic:** all logic stays in the Rust plugin. The C++ plugin only draws and handles input. ECAM logic will move to Rust in a later spec.
- **Order:** the FMS and MFD come first; the other screens follow.

## Licence rules for this work

- **FMS logic:** FlyByWire's FMS logic (fbw-a32nx `fmgc`, fbw-a380x `MFD/FMC`) may be translated into Rust, with FlyByWire's permission.
  - Two files carry Working Title copyright and are not translated: `utils/LzUtf8.js` and `wtsdk.ts`.
- **Framework utilities:** what the FMS imports from the framework (`EventBus`, `Subject`, `ConsumerValue`, `MathUtils`, `AeroMath`, `UnitType`, `BitFlags`, `UUID`, `Wait`) is not translated. Rust gets its own types for these, written from standard formulas and Rust idioms.
- **Framework sources stay unread:** the framework's source (`@microsoft/msfs-sdk`, the avionics mirror, xp-kit, xp-avionics) is never opened.
- **MFD behaviour:** page behaviour may come from FlyByWire's pages, meaning:
  - which data a field shows;
  - its format;
  - which entries it accepts;
  - what a button does.
- **MFD structure:** the MFD's structure does not follow FlyByWire's component tree. The C++ code is an immediate-mode drawer of a data model. It has no components, subjects or event bus.
- **Layouts:** layouts come from a private reference index and from captures of the current MFD.

## Architecture

```
X-Plane ── fbw_a380_systems (Rust) ─────────────────────────────┐
           crates/fms        flight plan, LNAV, VNAV, perf,      │
                             fuel, messages, uplink              │
           src/navdata       X-Plane CIFP + nav/fix/awy (exists) │
           src/fms_live.rs   ticks the FMS, writes the vars the  │
                             AP/PRIM/FCU/PFD read, sends the     │
                             EFIS events the ND reads            │
           datarefs  fbw/fms/mfd/model (data, read)              │
                     fbw/fms/mfd/action (data, write)            │
                                                                 │
X-Plane ── a380_displays (C++) ─ reads model, draws SCREEN_DU_MFD,
                                 sends actions ──────────────────┘
```

### crates/fms (Rust library, no X-Plane dependency)

- **Pure logic.** The crate takes inputs (aircraft state, pilot actions, time) and returns outputs (variables, EFIS events, the MFD model). Because of that, the emulator and offline tests drive it exactly like the sim does.
- **Navdata through a trait.** Navigation data comes through `trait NavDatabase`. The plugin implements it with the existing `src/navdata` (X-Plane's CIFP and dat files), and tests use fixtures.
- **Modules mirror FlyByWire's:**
  - `plan` (legs, segments, procedures, discontinuities, holds, secondary and alternate plans, temporary revisions);
  - `uplink` (SimBrief);
  - `lnav` (geometry, transitions, path vectors, sequencing, guidance laws);
  - `vnav` (profiles, predictions, constraints, cruise steps, descent);
  - `perf`, `fuel`, `messages`, `efis` (symbols and vectors for the ND), `phase` (flight phase manager);
  - `model` (the MFD data model).

### Outputs the FMS must produce

The contract is whatever FlyByWire's FMS produces today, and the oracle (below) records it:

- **Variables** the MFD view writes: guidance (cross-track error, track angle error, bank command, …), FMGC data, flight phase, V-speeds, and the others.
- **Generic-data events:**
  - `A32NX_EFIS_VECTORS_{L,R}_{group}`, from FlyByWire's EfisVectors;
  - the ND symbol events, from EfisSymbols;
  - the flight plan sync events.
- Because Rust sends these with the same keys and payloads, the Chromium ND and PFD keep working unchanged until they too go native.

### a380_displays (C++ plugin, `E:/fbw-int/a380-displays`)

- **Build and libraries:**
  - CMake, built for Windows first; Linux and Mac stay buildable (`linux-mac-port-goal`).
  - The X-Plane SDK headers come from `D:/A380/SDK`.
  - NanoVG (zlib licence) with its GL3 backend draws; its bundled stb_truetype (public domain) renders fonts.
  - nlohmann/json (MIT) parses the model.
- **Device:** it creates the `SCREEN_DU_MFD` avionics device, 1646 x 1024: two MFDs side by side with a 110 px gap.
- **Power and dimming:** each side dims with its own potentiometer (98 and 99) and needs power on its buses. The C++ side reads the same datarefs the Rust display code reads today.
- **Input:**
  - the avionics device callbacks (touch, cursor, scroll, keyboard);
  - the KCCU hardware key commands.
  - Typing is sent as raw text in an action. Rust validates the entry and answers through the model (field value, or scratchpad message such as "NOT ALLOWED").
- **State split:** the C++ side keeps only presentation state: open page per side, open dropdown, cursor position, and the text being typed. Everything else comes from the model.
- **Frame budget:** under 0.3 ms of CPU per frame for both MFDs. The model is parsed only when its sequence number changes.
- **Hand-off with the Rust plugin:** the C++ plugin claims `SCREEN_DU_MFD` from `XPluginEnable` with an `XPLMSendMessageToPlugin` message. The Rust plugin skips or destroys its own device for a claimed screen, and also checks for the plugin's signature (`a380.displays`) when it makes its devices.

### Model and actions

- **Model:** `fbw/fms/mfd/model` is UTF-8 JSON, versioned with `fbw/fms/mfd/seq`.
  - It holds one object per side: page id, the page's fields (value, colour, size, entry allowed), lists (F-PLN lines with their predictions) and the scratchpad/message line.
  - Rust rebuilds it at 10 Hz, and at once after an action.
- **Actions:** `fbw/fms/mfd/action` is a write-only data dataref. Each write is one JSON action:
  - `{side, page, field, kind: "enter"|"click"|"select"|"clear", text?, index?}`.
  - Rust queues the actions and applies them on its next flight loop.

## Verification

1. **Oracle.** FlyByWire's FMS runs offline in the plugin's QuickJS harness (`src/js/msfs/fms_offline.rs`) on the xp-ui build.
   - Scripted scenarios (uplink, manual entries, procedures, edits, a simulated flight along the route) drive it.
   - The harness records the flight plans, the predictions, every variable the MFD writes and every generic-data event, as JSON goldens in `E:/fbw-build/fms-oracle/`.
2. **Comparator.** The Rust FMS runs the same scenarios through the same navdata. A comparator checks every recorded value against tolerances: positions 1e-6 deg, distances 0.01 NM, angles 0.01 deg, predictions 1 ft / 0.5 kt / 1 s / 10 kg. Every scenario must pass.
3. **Shadow flight.** Before the switch, the Rust FMS runs alongside FlyByWire's in the sim.
   - It gets the pilot's entries from FlyByWire's flight plan sync events.
   - It logs every difference as `fms shadow:` lines.
   - It writes nothing.
4. **MFD pictures.** A headless test executable draws a recorded model with the same NanoVG code into a PNG. The PNG is compared with the Chromium capture of the same state (`tools/reference/compare/compare.mjs`).
5. **In the sim.** The user flies. I never launch X-Plane without asking.

## Phases

1. Oracle and scenarios.
2. FMS crate: flight plan and uplink.
3. FMS crate: LNAV.
4. FMS crate: VNAV, performance, fuel, messages, EFIS outputs.
5. Live in the plugin: shadow mode, then authoritative behind the C++ MFD's claim.
6. C++ plugin foundation: device, fonts, drawing, input, model and action datarefs, headless renderer.
7. MFD pages.
8. Cut-over: the release ships the native FMS and MFD; the MFD's Chromium page is removed from the install.

Each phase gets its own detailed plan when it starts. Phase 1's plan is in `docs/superpowers/plans/2026-10-21-native-fms-mfd.md`.

## Out of scope

- The native PFD, ND, EWD, SD, OIT, HUD, ISIS, RMP and FCU.
- Moving the FWS/ECAM into Rust.
- Removing the framework from the rest of the shipped JavaScript.

Each of these gets its own spec after phase 8.
