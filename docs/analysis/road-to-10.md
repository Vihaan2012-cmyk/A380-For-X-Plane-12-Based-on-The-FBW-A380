# Road to 10

Every open item from the 3 Nov 2026 reviews, grouped by area, in the order they should be built.
Items marked *sim* need X-Plane test flights to finish.

## 1. Flight model and engines (foundation)

- Custom .acf from a blank Plane Maker aircraft instead of Laminar's A330 (also ends the
  installer's rebuild-from-A330 licensing step). FlyByWire flight_model.cfg weights, CG limits and
  moments of inertia; geometry from the model; NASA SC(2) airfoils with computed polars. *sim*
- Calibrate X-Plane's own drag against the FCOM (cruise fuel flow, approach thrust); delete
  `src/drag_match.rs` and its uncited polar. *sim*
- Our engine model makes the thrust: drop X-Plane's generic turbine and the +/-12 % throttle trim
  (`src/engine_commands.rs`), apply per-engine forces and moments from the plugin. *sim*
- Re-test GitHub #10 (pitch oscillation on short final) once inertia and aero are the A380's. *sim*
- Engine heat blur at the nozzles (`_engn/N/_exhaust_os_xyz` is 0 today). *sim*

## 1b. The engine model itself (before it makes the thrust)

- Calibrate to several public points, not just sea-level static thrust: take-off EGT against the
  EASA TCDS limits (Trent 900 and GP7200: max continuous and 5 min take-off), idle fuel flow,
  cruise fuel flow (A380 about 10-13 t/h total). Pilot test cards (N1, EGT, fuel flow at take-off,
  climb, cruise, idle descent at a stated weight and OAT; idle-to-TOGA time; one engine out) as
  regression tests. Today cruise fuel flow only has to land in 0.3-3.0 kg/s per engine.
- GP7200 as a real engine: its own 2-spool gas path (no IP spool), bypass ratio, idle and limits
  (`physics/engine/engine_data.rs:53` maps the Trent's TGT onto it today).
- Connect the deep start sequencer (`deep/engine_accessories/start_sequencer.rs`: auto-abort on
  hot, hung or no-light starts, 2 retries after a 30 s dry crank, manual start, quick relight,
  windmill relight above 260 kt) to `engine_commands.rs`, which ignores it today.
- FADEC: real EEC channel A/B swap built around the compiled FADEC (today a monitor cap); check
  minimum and approach idle, FLX, derate and alpha floor in flight.
- Wear and accessories: EGT margin shrinking over cycles, oil use with servicing, vibration from
  fan imbalance or damage, fan ice build-up and shedding, water or hail ingestion flameout.

## 2. ECAM / warnings (6.5)

- The 10 FlyByWire warning-system bugs listed "fix pending" in `docs/fbw-differences.md`
  (engine 4 reads engine 3's thrust lever, T.O CONFIG test never assigned, flight phase 0 when
  spawning airborne, ...).
- Trigger the 686 FlyByWire procedures nothing triggers yet, with the `src/deep/ecam/fbw/`
  mechanism: ATA 70 (153), 28 (97), 31/32/33 (93), 24 (86), 21/22/23 (73), 26 (69), 27 (68),
  34 (58). FCOM for every trigger, inhibit and level.

## 3. Integration between computers (7)

- The deep avionics-network area decides what each computer receives: a failed network switch or
  bus stops the data, with status bits, latency and stale data, instead of shared variables.
- The 45 cockpit circuit breakers that gate nothing (`src/circuits.rs` PANEL_CB_NODES; only
  ESS TR, TR 1, TR 2A, LGCIS 1/2, PACK 1/2 CTL work): each cuts its computer's power through
  this network (FMC A/B/C, FWS 1/2, CIDS 1-3, CPCS 1/2, TCS/VCS, SDF 1-3, FQMS 1/2, BSCS 1/2,
  AESU, AICU, NSS, DSMS, ATC, flaps/slats, EIPM, GCU, ...).

## 4. FMS (5)

- FLS beam, OFFSET, SEC flight plan predictions, RTA, GLS, three FMCs with master/slave/standby
  (FlyByWire gaps: `FmcService.ts:99`, `FplnRevisionsMenu.tsx:100`, `MfdFmsFplnVertRev.tsx:1063`,
  `FlightManagementComputer.ts:614`).
- FM position blended from GPS/IRS/radio with ANP/RNP and NAV ACCUR DOWNGRAD.
- The VNAV items in `docs/fbw-differences.md`: constant econ speeds, the cost-index law, no
  descent path intercept, no take-off or go-around profile, current wind only.
- An A380 fuel model from our engine model, replacing the A320 tables x 2.8.
- Missing MFD pages: POSITION/GNSS, TIME, REPORT, DATA/ROUTE, PRINTER (only with a real layout
  reference, see the MFD pages rule).

## 5. Surveillance (6.5)

- Predictive windshear; GPWS modes 1-7 checked one by one; TCAS RA geometry and aurals against
  AI traffic; weather radar gain and tilt; ROW/ROP; one terrain reading per radio altimeter.

## 6. Communications (5, cap ~9.5)

- Working HF 1/2 and SATCOM, SELCAL, ATSU/CPDLC over Hoppie, COM 3 modelled in the plugin.

## 7. Electrical, remaining

- Four AC ground-service buses and EXT 1 ground servicing, 108PH, APU start bus, refuel buses,
  BCRU charge regulation, PEPDC/SEPDC/SPDB structure in the breaker catalogue. ATA 24 has about
  1,000 failure numbers in use: new ATA 24 components need a numbering change first.

## 8. Open GitHub issues needing a log or the sim

- #11 FWS 1+2 fault on a first runway start, ND flight plan dead; #12 black screens on a second
  session; #13 rudder trim knob blacks MFD/ND/OIS/EFB.

## 9. acsys (separate product)

- Whole-aircraft systems library for X-Plane: Rust core, C header, TOML aircraft files + C API,
  rustc-style checker, MPL-2.0, clean room. Plan document still to write.
